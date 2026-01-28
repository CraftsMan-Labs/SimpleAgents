//! Retry helper for routing operations.
//!
//! Provides exponential backoff with jitter for retryable errors.

use simple_agent_type::prelude::{ProviderError, SimpleAgentsError};
use std::future::Future;
use std::time::Duration;

/// Retry policy configuration.
#[derive(Debug, Clone, Copy)]
pub struct RetryPolicy {
    /// Maximum number of retry attempts.
    pub max_attempts: u32,
    /// Initial backoff duration.
    pub initial_backoff: Duration,
    /// Maximum backoff duration.
    pub max_backoff: Duration,
    /// Exponential backoff multiplier.
    pub backoff_multiplier: f32,
    /// Add jitter to backoff.
    pub jitter: bool,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            initial_backoff: Duration::from_millis(100),
            max_backoff: Duration::from_secs(10),
            backoff_multiplier: 2.0,
            jitter: true,
        }
    }
}

impl RetryPolicy {
    fn backoff(&self, attempt: u32) -> Duration {
        let base =
            self.initial_backoff.as_millis() as f32 * self.backoff_multiplier.powi(attempt as i32);
        let capped = base.min(self.max_backoff.as_millis() as f32);

        let duration_ms = if self.jitter {
            let jitter_factor = 0.5 + (random_f32() * 0.5);
            capped * jitter_factor
        } else {
            capped
        };

        Duration::from_millis(duration_ms as u64).min(self.max_backoff)
    }
}

/// Execute an async operation with retry logic.
///
/// Retries only on retryable provider or network errors.
pub async fn execute_with_retry<F, Fut, T>(
    policy: RetryPolicy,
    operation: F,
) -> Result<T, SimpleAgentsError>
where
    F: Fn() -> Fut,
    Fut: Future<Output = Result<T, SimpleAgentsError>>,
{
    let mut last_error: Option<SimpleAgentsError> = None;

    for attempt in 0..policy.max_attempts {
        match operation().await {
            Ok(result) => return Ok(result),
            Err(error) => {
                if !is_retryable(&error) {
                    return Err(error);
                }

                if attempt >= policy.max_attempts - 1 {
                    last_error = Some(error);
                    break;
                }

                tokio::time::sleep(policy.backoff(attempt)).await;
                last_error = Some(error);
            }
        }
    }

    Err(last_error.unwrap())
}

fn is_retryable(error: &SimpleAgentsError) -> bool {
    matches!(
        error,
        SimpleAgentsError::Provider(
            ProviderError::RateLimit { .. }
                | ProviderError::Timeout(_)
                | ProviderError::ServerError(_)
        ) | SimpleAgentsError::Network(_)
    )
}

fn random_f32() -> f32 {
    use rand::Rng;
    rand::thread_rng().gen()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn succeeds_without_retry() {
        let policy = RetryPolicy {
            max_attempts: 3,
            initial_backoff: Duration::from_millis(1),
            max_backoff: Duration::from_millis(5),
            backoff_multiplier: 2.0,
            jitter: false,
        };

        let result =
            execute_with_retry(policy, || async { Ok::<_, SimpleAgentsError>("ok") }).await;
        assert_eq!(result.unwrap(), "ok");
    }

    #[tokio::test]
    async fn retries_on_retryable_error() {
        let policy = RetryPolicy {
            max_attempts: 2,
            initial_backoff: Duration::from_millis(1),
            max_backoff: Duration::from_millis(5),
            backoff_multiplier: 2.0,
            jitter: false,
        };

        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        let attempts = Arc::new(AtomicUsize::new(0));
        let attempts_clone = attempts.clone();

        let result = execute_with_retry(policy, move || {
            let attempts = attempts_clone.clone();
            async move {
                let current = attempts.fetch_add(1, Ordering::Relaxed);
                if current == 0 {
                    Err(SimpleAgentsError::Provider(ProviderError::Timeout(
                        Duration::from_secs(1),
                    )))
                } else {
                    Ok("ok")
                }
            }
        })
        .await;

        assert_eq!(result.unwrap(), "ok");
        assert_eq!(attempts.load(Ordering::Relaxed), 2);
    }

    #[tokio::test]
    async fn fails_on_non_retryable_error() {
        let policy = RetryPolicy {
            max_attempts: 3,
            initial_backoff: Duration::from_millis(1),
            max_backoff: Duration::from_millis(5),
            backoff_multiplier: 2.0,
            jitter: false,
        };

        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        let attempts = Arc::new(AtomicUsize::new(0));
        let attempts_clone = attempts.clone();

        let result = execute_with_retry(policy, move || {
            let attempts = attempts_clone.clone();
            async move {
                attempts.fetch_add(1, Ordering::Relaxed);
                Err::<&str, _>(SimpleAgentsError::Provider(ProviderError::InvalidApiKey))
            }
        })
        .await;

        assert!(matches!(
            result,
            Err(SimpleAgentsError::Provider(ProviderError::InvalidApiKey))
        ));
        assert_eq!(attempts.load(Ordering::Relaxed), 1);
    }
}
