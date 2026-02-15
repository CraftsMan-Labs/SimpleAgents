use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use simple_agents_workflow::{
    WorkerHealth, WorkerPoolClient, WorkerPoolError, WorkerRequest, WorkerResponse,
};

use crate::client::GrpcWorkerClient;

/// Runtime options for the gRPC worker pool.
#[derive(Debug, Clone)]
pub struct GrpcWorkerPoolOptions {
    /// Number of retries after the first failed attempt.
    pub max_retries: usize,
    /// Default timeout used for requests that do not include one.
    pub default_request_timeout: Option<Duration>,
}

impl Default for GrpcWorkerPoolOptions {
    fn default() -> Self {
        Self {
            max_retries: 1,
            default_request_timeout: Some(Duration::from_secs(10)),
        }
    }
}

/// Round-robin gRPC worker pool with retry support.
pub struct GrpcWorkerPool {
    clients: Vec<Arc<GrpcWorkerClient>>,
    next: AtomicUsize,
    options: GrpcWorkerPoolOptions,
}

impl GrpcWorkerPool {
    /// Connects to all provided worker endpoints.
    pub async fn connect(
        endpoints: Vec<(String, String)>,
        options: GrpcWorkerPoolOptions,
    ) -> Result<Self, WorkerPoolError> {
        if endpoints.is_empty() {
            return Err(WorkerPoolError::NoHealthyWorker);
        }

        let mut clients = Vec::with_capacity(endpoints.len());
        for (worker_id, endpoint) in endpoints {
            let client = GrpcWorkerClient::connect(worker_id, endpoint).await?;
            clients.push(Arc::new(client));
        }

        Ok(Self {
            clients,
            next: AtomicUsize::new(0),
            options,
        })
    }

    fn timeout_for_request(&self, request: &WorkerRequest) -> Option<Duration> {
        request
            .timeout_ms
            .map(Duration::from_millis)
            .or(self.options.default_request_timeout)
    }

    fn should_retry(error: &WorkerPoolError) -> bool {
        match error {
            WorkerPoolError::Worker(protocol) => protocol.retryable,
            WorkerPoolError::Timeout => true,
            _ => false,
        }
    }
}

#[async_trait]
impl WorkerPoolClient for GrpcWorkerPool {
    async fn submit(&self, request: WorkerRequest) -> Result<WorkerResponse, WorkerPoolError> {
        let timeout_budget = self.timeout_for_request(&request);
        let len = self.clients.len();
        let start = self.next.fetch_add(1, Ordering::Relaxed) % len;
        let max_attempts = self.options.max_retries.saturating_add(1).min(len.max(1));

        let mut last_error = None;

        for attempt in 0..max_attempts {
            let idx = (start + attempt) % len;
            let client = &self.clients[idx];

            match client.execute(request.clone(), timeout_budget).await {
                Ok(response) => return Ok(response),
                Err(error) => {
                    if !Self::should_retry(&error) || attempt + 1 == max_attempts {
                        return Err(error);
                    }
                    last_error = Some(error);
                }
            }
        }

        Err(last_error.unwrap_or(WorkerPoolError::NoHealthyWorker))
    }

    async fn health_snapshot(&self) -> Vec<WorkerHealth> {
        let mut snapshot = Vec::with_capacity(self.clients.len());
        for client in &self.clients {
            snapshot.push(client.health().await);
        }
        snapshot
    }
}
