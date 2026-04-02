use std::collections::HashMap;
use std::time::Duration;

use serde_json::Value;
use simple_agents_workflow::{
    WorkerErrorCode, WorkerHealth, WorkerHealthStatus, WorkerOperation, WorkerPoolError,
    WorkerProtocolError, WorkerRequest, WorkerResponse, WorkerResult,
};
use tonic::transport::Channel;

use crate::proto::{
    execute_request::TypedPayload, worker_service_client::WorkerServiceClient, ExecuteRequest,
    HealthRequest, HealthStatus, LlmPayload, ToolPayload,
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
    let (operation, target, payload_json, typed_payload) = match request.operation {
        WorkerOperation::Llm {
            model,
            prompt,
            scoped_input,
        } => {
            let scoped_input_json = serialize_json_value(&scoped_input)?;
            (
                "llm".to_string(),
                model,
                serde_json::to_string(&serde_json::json!({
                    "prompt": prompt,
                    "scoped_input": scoped_input,
                }))
                .map_err(|error| serialization_error(error.to_string()))?,
                Some(TypedPayload::LlmPayload(LlmPayload {
                    prompt,
                    scoped_input_json,
                })),
            )
        }
        WorkerOperation::Tool {
            tool,
            input,
            scoped_input,
        } => {
            let input_json = serialize_json_value(&input)?;
            let scoped_input_json = serialize_json_value(&scoped_input)?;
            (
                "tool".to_string(),
                tool,
                serde_json::to_string(&serde_json::json!({
                    "input": input,
                    "scoped_input": scoped_input,
                }))
                .map_err(|error| serialization_error(error.to_string()))?,
                Some(TypedPayload::ToolPayload(ToolPayload {
                    input_json,
                    scoped_input_json,
                })),
            )
        }
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
        typed_payload,
    })
}

fn serialize_json_value(value: &Value) -> Result<String, WorkerPoolError> {
    serde_json::to_string(value).map_err(|error| serialization_error(error.to_string()))
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

#[cfg(test)]
mod tests {
    use serde_json::json;
    use simple_agents_workflow::{WorkerOperation, WorkerRequest};

    use super::to_execute_request;
    use crate::proto::execute_request::TypedPayload;

    #[test]
    fn to_execute_request_sets_tool_typed_payload_and_legacy_fallback() {
        let request = WorkerRequest {
            request_id: "req-1".to_string(),
            workflow_name: "wf".to_string(),
            node_id: "node-1".to_string(),
            timeout_ms: Some(1000),
            operation: WorkerOperation::Tool {
                tool: "echo".to_string(),
                input: json!({"x": 1}),
                scoped_input: json!({"input": {"y": 2}}),
            },
        };

        let grpc_request = to_execute_request(request).expect("conversion should succeed");

        assert!(!grpc_request.payload_json.is_empty());
        match grpc_request.typed_payload {
            Some(TypedPayload::ToolPayload(payload)) => {
                assert_eq!(payload.input_json, "{\"x\":1}");
                assert_eq!(payload.scoped_input_json, "{\"input\":{\"y\":2}}");
            }
            other => panic!("unexpected typed payload: {other:?}"),
        }
    }

    #[test]
    fn to_execute_request_sets_llm_typed_payload_and_legacy_fallback() {
        let request = WorkerRequest {
            request_id: "req-2".to_string(),
            workflow_name: "wf".to_string(),
            node_id: "node-2".to_string(),
            timeout_ms: None,
            operation: WorkerOperation::Llm {
                model: "gpt-4.1-mini".to_string(),
                prompt: "hello".to_string(),
                scoped_input: json!({"input": {"topic": "rust"}}),
            },
        };

        let grpc_request = to_execute_request(request).expect("conversion should succeed");

        assert!(!grpc_request.payload_json.is_empty());
        match grpc_request.typed_payload {
            Some(TypedPayload::LlmPayload(payload)) => {
                assert_eq!(payload.prompt, "hello");
                assert_eq!(
                    payload.scoped_input_json,
                    "{\"input\":{\"topic\":\"rust\"}}"
                );
            }
            other => panic!("unexpected typed payload: {other:?}"),
        }
    }
}
