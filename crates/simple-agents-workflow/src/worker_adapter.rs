use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use async_trait::async_trait;

use crate::runtime::{ToolExecutionError, ToolExecutionInput, ToolExecutor};
use crate::worker::{WorkerOperation, WorkerPoolClient, WorkerPoolError, WorkerRequest};

/// Runtime tool-executor adapter backed by a worker pool client.
pub struct WorkerPoolToolExecutor {
    workflow_name: String,
    timeout_ms: Option<u64>,
    pool: Arc<dyn WorkerPoolClient>,
    request_seq: AtomicU64,
}

impl WorkerPoolToolExecutor {
    /// Creates a new worker-pool-backed tool executor.
    pub fn new(
        workflow_name: impl Into<String>,
        timeout_ms: Option<u64>,
        pool: Arc<dyn WorkerPoolClient>,
    ) -> Self {
        Self {
            workflow_name: workflow_name.into(),
            timeout_ms,
            pool,
            request_seq: AtomicU64::new(0),
        }
    }

    fn next_request_id(&self, node_id: &str) -> String {
        let seq = self.request_seq.fetch_add(1, Ordering::Relaxed);
        format!("{}-{}", node_id, seq)
    }
}

#[async_trait]
impl ToolExecutor for WorkerPoolToolExecutor {
    async fn execute_tool(
        &self,
        input: ToolExecutionInput,
    ) -> Result<serde_json::Value, ToolExecutionError> {
        let request = WorkerRequest {
            request_id: self.next_request_id(&input.node_id),
            workflow_name: self.workflow_name.clone(),
            node_id: input.node_id,
            timeout_ms: self.timeout_ms,
            operation: WorkerOperation::Tool {
                tool: input.tool,
                input: input.input,
                scoped_input: input.scoped_input,
            },
        };

        let response = self
            .pool
            .submit(request)
            .await
            .map_err(map_worker_pool_error)?;

        match response.result {
            crate::worker::WorkerResult::Success { output } => Ok(output),
            crate::worker::WorkerResult::Error { error } => Err(ToolExecutionError::Failed(
                format!("{:?}: {}", error.code, error.message),
            )),
        }
    }
}

fn map_worker_pool_error(error: WorkerPoolError) -> ToolExecutionError {
    match error {
        WorkerPoolError::Worker(worker_error) => ToolExecutionError::Failed(worker_error.message),
        WorkerPoolError::Timeout => {
            ToolExecutionError::Failed("worker request timed out".to_string())
        }
        WorkerPoolError::QueueFull => {
            ToolExecutionError::Failed("worker queue is full".to_string())
        }
        WorkerPoolError::NoHealthyWorker => {
            ToolExecutionError::Failed("no healthy worker available".to_string())
        }
        WorkerPoolError::ShuttingDown => {
            ToolExecutionError::Failed("worker pool is shutting down".to_string())
        }
        WorkerPoolError::CircuitOpen => {
            ToolExecutionError::Failed("worker circuit is open".to_string())
        }
        WorkerPoolError::InvalidRequest { reason } => ToolExecutionError::Failed(format!(
            "worker request rejected by security contract: {reason}"
        )),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use serde_json::json;

    use super::*;
    use crate::worker::{
        WorkerErrorCode, WorkerHealth, WorkerHealthStatus, WorkerPoolError, WorkerProtocolError,
        WorkerResponse, WorkerResult,
    };

    struct MockPool;

    #[async_trait]
    impl WorkerPoolClient for MockPool {
        async fn submit(&self, request: WorkerRequest) -> Result<WorkerResponse, WorkerPoolError> {
            if let WorkerOperation::Tool { tool, input, .. } = request.operation {
                if tool == "fail" {
                    return Err(WorkerPoolError::Worker(WorkerProtocolError {
                        code: WorkerErrorCode::ExecutionFailed,
                        message: "forced failure".to_string(),
                        retryable: false,
                    }));
                }
                return Ok(WorkerResponse {
                    request_id: request.request_id,
                    worker_id: "mock-0".to_string(),
                    result: WorkerResult::Success {
                        output: json!({"input": input}),
                    },
                    elapsed_ms: 1,
                });
            }
            unreachable!("test only uses tool requests")
        }

        async fn health_snapshot(&self) -> Vec<WorkerHealth> {
            vec![WorkerHealth {
                worker_id: "mock-0".to_string(),
                status: WorkerHealthStatus::Healthy,
                consecutive_failures: 0,
                last_probe_unix_ms: Some(1),
            }]
        }
    }

    #[tokio::test]
    async fn executes_tool_through_worker_pool_client() {
        let executor = WorkerPoolToolExecutor::new("wf", Some(500), Arc::new(MockPool));
        let output = executor
            .execute_tool(ToolExecutionInput {
                node_id: "node-1".to_string(),
                tool: "echo".to_string(),
                input: json!({"x": 1}),
                scoped_input: json!({"input": {"foo": "bar"}}),
            })
            .await
            .expect("worker pool adapter should return output");
        assert_eq!(output, json!({"input": {"x": 1}}));
    }

    #[tokio::test]
    async fn maps_worker_errors_to_tool_errors() {
        let executor = WorkerPoolToolExecutor::new("wf", Some(500), Arc::new(MockPool));
        let error = executor
            .execute_tool(ToolExecutionInput {
                node_id: "node-1".to_string(),
                tool: "fail".to_string(),
                input: json!({}),
                scoped_input: json!({"input": {}}),
            })
            .await
            .expect_err("worker error should map to tool error");

        assert!(matches!(error, ToolExecutionError::Failed(_)));
    }
}
