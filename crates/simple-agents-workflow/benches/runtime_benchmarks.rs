use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use criterion::{criterion_group, criterion_main, Criterion};
use serde_json::{json, Value};
use simple_agents_workflow::{
    LlmExecutionError, LlmExecutionInput, LlmExecutionOutput, LlmExecutor, Node, NodeKind,
    ToolExecutionError, ToolExecutionInput, ToolExecutor, WorkerHandler, WorkerOperation,
    WorkerPool, WorkerPoolOptions, WorkerProtocolError, WorkerRequest, WorkerResult,
    WorkflowDefinition, WorkflowRuntime, WorkflowRuntimeOptions,
};

struct BenchLlm;

#[async_trait]
impl LlmExecutor for BenchLlm {
    async fn execute(
        &self,
        _input: LlmExecutionInput,
    ) -> Result<LlmExecutionOutput, LlmExecutionError> {
        Ok(LlmExecutionOutput {
            content: "bench-output".to_string(),
        })
    }
}

struct BenchTool;

#[async_trait]
impl ToolExecutor for BenchTool {
    async fn execute_tool(&self, _input: ToolExecutionInput) -> Result<Value, ToolExecutionError> {
        Ok(json!({"ok": true}))
    }
}

struct EchoWorker;

#[async_trait]
impl WorkerHandler for EchoWorker {
    async fn handle(&self, request: WorkerRequest) -> Result<Value, WorkerProtocolError> {
        match request.operation {
            WorkerOperation::Tool { .. } => Ok(json!({"status": "tool_ok"})),
            WorkerOperation::Llm { .. } => Ok(json!({"status": "llm_ok"})),
        }
    }
}

fn benchmark_workflow() -> WorkflowDefinition {
    WorkflowDefinition {
        version: "v0".to_string(),
        name: "bench-linear".to_string(),
        nodes: vec![
            Node {
                id: "start".to_string(),
                kind: NodeKind::Start {
                    next: "llm".to_string(),
                },
            },
            Node {
                id: "llm".to_string(),
                kind: NodeKind::Llm {
                    model: "gpt-4o-mini".to_string(),
                    prompt: "hello".to_string(),
                    next: Some("tool".to_string()),
                },
            },
            Node {
                id: "tool".to_string(),
                kind: NodeKind::Tool {
                    tool: "echo".to_string(),
                    input: json!({"x": 1}),
                    next: Some("end".to_string()),
                },
            },
            Node {
                id: "end".to_string(),
                kind: NodeKind::End,
            },
        ],
    }
}

fn runtime_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("workflow_runtime");
    group.measurement_time(Duration::from_secs(10));

    let rt = tokio::runtime::Runtime::new().expect("tokio runtime should build");
    let llm = BenchLlm;
    let tool = BenchTool;
    let workflow = benchmark_workflow();
    let runtime_options = WorkflowRuntimeOptions {
        enable_trace_recording: false,
        ..WorkflowRuntimeOptions::default()
    };

    group.bench_function("linear_execute", |b| {
        b.to_async(&rt).iter(|| async {
            let runtime =
                WorkflowRuntime::new(workflow.clone(), &llm, Some(&tool), runtime_options.clone());
            let _ = runtime
                .execute(json!({"request_id": "bench"}), None)
                .await
                .expect("runtime bench run should succeed");
        })
    });

    group.bench_function("worker_pool_submit", |b| {
        b.to_async(&rt).iter(|| async {
            let pool = WorkerPool::new_inprocess(
                vec![Arc::new(EchoWorker), Arc::new(EchoWorker)],
                WorkerPoolOptions {
                    queue_capacity: 64,
                    ..WorkerPoolOptions::default()
                },
                None,
            )
            .expect("worker pool should initialize");

            let response = pool
                .submit(WorkerRequest {
                    request_id: "bench-1".to_string(),
                    workflow_name: "bench".to_string(),
                    node_id: "tool".to_string(),
                    timeout_ms: None,
                    operation: WorkerOperation::Tool {
                        tool: "echo".to_string(),
                        input: json!({"k": "v"}),
                        scoped_input: json!({"input": {}}),
                    },
                })
                .await
                .expect("worker pool submit should succeed");

            assert!(matches!(response.result, WorkerResult::Success { .. }));
            pool.shutdown().await;
        })
    });

    group.finish();
}

criterion_group!(benches, runtime_benchmarks);
criterion_main!(benches);
