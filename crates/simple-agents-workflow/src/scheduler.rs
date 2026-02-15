use std::future::Future;

use futures::stream::{FuturesUnordered, StreamExt};

/// Bounded async scheduler for DAG-adjacent fan-out workloads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DagScheduler {
    max_in_flight: usize,
}

impl DagScheduler {
    /// Creates a scheduler with a bounded number of concurrent tasks.
    pub fn new(max_in_flight: usize) -> Self {
        Self {
            max_in_flight: max_in_flight.max(1),
        }
    }

    /// Returns the configured maximum number of in-flight tasks.
    pub fn max_in_flight(self) -> usize {
        self.max_in_flight
    }

    /// Executes tasks with bounded concurrency and deterministic result ordering.
    pub async fn run_bounded<I, F, Fut, T, E>(
        &self,
        inputs: I,
        mut task_builder: F,
    ) -> Result<Vec<T>, E>
    where
        I: IntoIterator,
        F: FnMut(I::Item) -> Fut,
        Fut: Future<Output = Result<T, E>>,
    {
        let mut indexed_inputs = inputs.into_iter().enumerate();
        let mut in_flight = FuturesUnordered::new();
        let mut results: Vec<Option<T>> = Vec::new();

        loop {
            while in_flight.len() < self.max_in_flight {
                let Some((index, item)) = indexed_inputs.next() else {
                    break;
                };
                if results.len() <= index {
                    results.resize_with(index + 1, || None);
                }
                let task = task_builder(item);
                in_flight.push(async move { (index, task.await) });
            }

            let Some((index, output)) = in_flight.next().await else {
                break;
            };

            match output {
                Ok(value) => {
                    results[index] = Some(value);
                }
                Err(error) => return Err(error),
            }
        }

        Ok(results
            .into_iter()
            .map(|entry| entry.expect("scheduler result slot must be filled"))
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    use tokio::sync::Mutex;
    use tokio::time::sleep;

    use super::DagScheduler;

    #[tokio::test]
    async fn respects_max_in_flight_limit() {
        let scheduler = DagScheduler::new(2);
        let in_flight = Arc::new(Mutex::new(0usize));
        let peak = Arc::new(Mutex::new(0usize));

        let outputs = scheduler
            .run_bounded(0..8usize, {
                let in_flight = Arc::clone(&in_flight);
                let peak = Arc::clone(&peak);
                move |item| {
                    let in_flight = Arc::clone(&in_flight);
                    let peak = Arc::clone(&peak);
                    async move {
                        {
                            let mut active = in_flight.lock().await;
                            *active += 1;
                            let mut peak_guard = peak.lock().await;
                            *peak_guard = (*peak_guard).max(*active);
                        }

                        sleep(Duration::from_millis(10)).await;

                        {
                            let mut active = in_flight.lock().await;
                            *active = active.saturating_sub(1);
                        }

                        Ok::<usize, ()>(item * 2)
                    }
                }
            })
            .await
            .expect("bounded scheduling should succeed");

        assert_eq!(outputs, vec![0, 2, 4, 6, 8, 10, 12, 14]);
        assert!(*peak.lock().await <= 2);
    }

    #[tokio::test]
    async fn runs_concurrently_when_limit_above_one() {
        let scheduler = DagScheduler::new(4);
        let started = Instant::now();

        let _ = scheduler
            .run_bounded(0..4usize, |_| async {
                sleep(Duration::from_millis(20)).await;
                Ok::<(), ()>(())
            })
            .await
            .expect("scheduler should run all tasks");

        assert!(started.elapsed() < Duration::from_millis(60));
    }
}
