use std::collections::HashMap;
use std::time::Duration;

use serde_json::Value;
use simple_agents_workflow::{
    WorkerErrorCode, WorkerHealth, WorkerHealthStatus, WorkerOperation, WorkerPoolError,
    WorkerProtocolError, WorkerRequest, WorkerResponse, WorkerResult,
};
use tonic::transport::Channel;

use crate::proto::{
    worker_service_client::WorkerServiceClient, ExecuteRequest, HealthRequest, HealthStatus,
};

/// gRPC client wrapper for one language worker endpoint.
pub struct GrpcWorkerClient {
    worker_id: String,
    endpoint: String,
    channel: Channel,
}

impl GrpcWorkerClient {
    /// Connects to one worker endpoint.
    pub async fn connect(
        worker_id: impl Into<String>,
        endpoint: impl Into<String>,
    ) -> Result<Self, WorkerPoolError> {
        let worker_id = worker_id.into();
        let endpoint = endpoint.into();
        let channel = Channel::from_shared(endpoint.clone())
            .map_err(|error| map_transport_error(&worker_id, error.to_string()))?
            .connect()
            .await
            .map_err(|error| map_transport_error(&worker_id, error.to_string()))?;

        Ok(Self {
            worker_id,
            endpoint,
            channel,
        })
    }

    /// Returns the stable worker id configured for this client.
    pub fn worker_id(&self) -> &str {
        &self.worker_id
    }

    /// Returns the worker endpoint URL.
    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    /// Executes one worker request through gRPC.
    pub async fn execute(
        &self,
        request: WorkerRequest,
        timeout_budget: Option<Duration>,
    ) -> Result<WorkerResponse, WorkerPoolError> {
        let mut grpc_request = tonic::Request::new(to_execute_request(request)?);
        if let Some(timeout) = timeout_budget {
            grpc_request.set_timeout(timeout);
        }

        let response = WorkerServiceClient::new(self.channel.clone())
            .execute(grpc_request)
            .await
            .map_err(map_status_error)?
            .into_inner();

        from_execute_response(response)
    }

    /// Probes worker health via gRPC.
    pub async fn health(&self) -> WorkerHealth {
        let result = WorkerServiceClient::new(self.channel.clone())
            .health(tonic::Request::new(HealthRequest {}))
            .await;

        match result {
            Ok(response) => {
                let payload = response.into_inner();
                WorkerHealth {
                    worker_id: payload.worker_id,
                    status: match HealthStatus::try_from(payload.status)
                        .unwrap_or(HealthStatus::Unknown)
                    {
                        HealthStatus::Serving => WorkerHealthStatus::Healthy,
                        HealthStatus::NotServing => WorkerHealthStatus::Unavailable,
                        HealthStatus::Unknown => WorkerHealthStatus::Degraded,
                    },
                    consecutive_failures: payload.consecutive_failures,
                    last_probe_unix_ms: payload.last_probe_unix_ms,
                }
            }
            Err(_) => WorkerHealth {
                worker_id: self.worker_id.clone(),
                status: WorkerHealthStatus::Unavailable,
                consecutive_failures: 1,
                last_probe_unix_ms: None,
            },
        }
    }
}

fn to_execute_request(request: WorkerRequest) -> Result<ExecuteRequest, WorkerPoolError> {
    let (operation, target, payload_json) = match request.operation {
        WorkerOperation::Llm {
            model,
            prompt,
            scoped_input,
        } => (
            "llm".to_string(),
            model,
            serde_json::to_string(&serde_json::json!({
                "prompt": prompt,
                "scoped_input": scoped_input,
            }))
            .map_err(|error| serialization_error(error.to_string()))?,
        ),
        WorkerOperation::Tool {
            tool,
            input,
            scoped_input,
        } => (
            "tool".to_string(),
            tool,
            serde_json::to_string(&serde_json::json!({
                "input": input,
                "scoped_input": scoped_input,
            }))
            .map_err(|error| serialization_error(error.to_string()))?,
        ),
    };

    Ok(ExecuteRequest {
        request_id: request.request_id,
        workflow_name: request.workflow_name,
        node_id: request.node_id,
        operation,
        target,
        payload_json,
        timeout_ms: request.timeout_ms,
        metadata: HashMap::new(),
    })
}

fn from_execute_response(
    response: crate::proto::ExecuteResponse,
) -> Result<WorkerResponse, WorkerPoolError> {
    if response.ok {
        let output = serde_json::from_str::<Value>(&response.output_json)
            .map_err(|error| serialization_error(error.to_string()))?;
        return Ok(WorkerResponse {
            request_id: response.request_id,
            worker_id: response.worker_id,
            result: WorkerResult::Success { output },
            elapsed_ms: response.elapsed_ms,
        });
    }

    let error = response.error.unwrap_or_default();
    let protocol_error = WorkerProtocolError {
        code: map_error_code(&error.code),
        message: error.message,
        retryable: error.retryable,
    };
    Err(WorkerPoolError::Worker(protocol_error))
}

fn map_error_code(code: &str) -> WorkerErrorCode {
    match code {
        "queue_full" => WorkerErrorCode::QueueFull,
        "unavailable" => WorkerErrorCode::Unavailable,
        "timeout" => WorkerErrorCode::Timeout,
        "execution_failed" => WorkerErrorCode::ExecutionFailed,
        "circuit_open" => WorkerErrorCode::CircuitOpen,
        "cancelled" => WorkerErrorCode::Cancelled,
        "invalid_request" => WorkerErrorCode::InvalidRequest,
        _ => WorkerErrorCode::ExecutionFailed,
    }
}

fn map_status_error(error: tonic::Status) -> WorkerPoolError {
    let retryable = matches!(
        error.code(),
        tonic::Code::Unavailable
            | tonic::Code::ResourceExhausted
            | tonic::Code::DeadlineExceeded
            | tonic::Code::Internal
    );
    let code = match error.code() {
        tonic::Code::ResourceExhausted => WorkerErrorCode::QueueFull,
        tonic::Code::DeadlineExceeded => WorkerErrorCode::Timeout,
        tonic::Code::InvalidArgument => WorkerErrorCode::InvalidRequest,
        tonic::Code::Cancelled => WorkerErrorCode::Cancelled,
        tonic::Code::Unavailable => WorkerErrorCode::Unavailable,
        _ => WorkerErrorCode::ExecutionFailed,
    };
    WorkerPoolError::Worker(WorkerProtocolError {
        code,
        message: error.message().to_string(),
        retryable,
    })
}

fn map_transport_error(worker_id: &str, message: String) -> WorkerPoolError {
    WorkerPoolError::Worker(WorkerProtocolError {
        code: WorkerErrorCode::Unavailable,
        message: format!("failed to connect to worker {worker_id}: {message}"),
        retryable: true,
    })
}

fn serialization_error(message: String) -> WorkerPoolError {
    WorkerPoolError::Worker(WorkerProtocolError {
        code: WorkerErrorCode::ExecutionFailed,
        message,
        retryable: false,
    })
}
