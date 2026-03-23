use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use criterion::{criterion_group, criterion_main, Criterion};
use serde_json::{json, Value};
use simple_agents_workflow::{
    LlmExecutionError, LlmExecutionInput, LlmExecutionOutput, LlmExecutor, Node, NodeKind,
    ReduceOperation, ToolExecutionError, ToolExecutionInput, ToolExecutor, WorkerHandler,
    WorkerOperation, WorkerPool, WorkerPoolOptions, WorkerProtocolError, WorkerRequest,
    WorkerResult, WorkflowDefinition, WorkflowRuntime, WorkflowRuntimeOptions,
};
use tokio::time::sleep;

const DEFAULT_GUARD_RUNS: usize = 7;
const DEFAULT_CONCURRENCY_GAIN_PERCENT: u128 = 15;

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

struct DelayEchoTool {
    delay: Duration,
}

#[async_trait]
impl ToolExecutor for DelayEchoTool {
    async fn execute_tool(&self, input: ToolExecutionInput) -> Result<Value, ToolExecutionError> {
        sleep(self.delay).await;
        Ok(input.input)
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

fn sequential_comparison_workflow() -> WorkflowDefinition {
    WorkflowDefinition {
        version: "v0".to_string(),
        name: "bench-sequential".to_string(),
        nodes: vec![
            Node {
                id: "start".to_string(),
                kind: NodeKind::Start {
                    next: "tool_1".to_string(),
                },
            },
            Node {
                id: "tool_1".to_string(),
                kind: NodeKind::Tool {
                    tool: "delay".to_string(),
                    input: json!(1),
                    next: Some("tool_2".to_string()),
                },
            },
            Node {
                id: "tool_2".to_string(),
                kind: NodeKind::Tool {
                    tool: "delay".to_string(),
                    input: json!(2),
                    next: Some("tool_3".to_string()),
                },
            },
            Node {
                id: "tool_3".to_string(),
                kind: NodeKind::Tool {
                    tool: "delay".to_string(),
                    input: json!(3),
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

fn concurrent_comparison_workflow() -> WorkflowDefinition {
    WorkflowDefinition {
        version: "v0".to_string(),
        name: "bench-concurrent".to_string(),
        nodes: vec![
            Node {
                id: "start".to_string(),
                kind: NodeKind::Start {
                    next: "map".to_string(),
                },
            },
            Node {
                id: "map".to_string(),
                kind: NodeKind::Map {
                    tool: "delay".to_string(),
                    items_path: "input.values".to_string(),
                    next: "reduce".to_string(),
                    max_in_flight: Some(3),
                },
            },
            Node {
                id: "reduce".to_string(),
                kind: NodeKind::Reduce {
                    source: "map".to_string(),
                    operation: ReduceOperation::Count,
                    next: "end".to_string(),
                },
            },
            Node {
                id: "end".to_string(),
                kind: NodeKind::End,
            },
        ],
    }
}

fn dense_scope_workflow(node_count: usize) -> WorkflowDefinition {
    let mut nodes = Vec::with_capacity(node_count + 2);
    let first = if node_count == 0 {
        "end".to_string()
    } else {
        "tool_0".to_string()
    };
    nodes.push(Node {
        id: "start".to_string(),
        kind: NodeKind::Start { next: first },
    });

    for index in 0..node_count {
        let next = if index + 1 >= node_count {
            Some("end".to_string())
        } else {
            Some(format!("tool_{}", index + 1))
        };
        nodes.push(Node {
            id: format!("tool_{index}"),
            kind: NodeKind::Tool {
                tool: "echo".to_string(),
                input: json!({"i": index}),
                next,
            },
        });
    }

    nodes.push(Node {
        id: "end".to_string(),
        kind: NodeKind::End,
    });

    WorkflowDefinition {
        version: "v0".to_string(),
        name: "bench-dense-scope".to_string(),
        nodes,
    }
}

fn assert_concurrency_regression_guard(rt: &tokio::runtime::Runtime) {
    let llm = BenchLlm;
    let tool = DelayEchoTool {
        delay: Duration::from_millis(12),
    };
    let options = WorkflowRuntimeOptions {
        enable_trace_recording: false,
        ..WorkflowRuntimeOptions::default()
    };

    let guard_runs = std::env::var("WORKFLOW_BENCH_GUARD_RUNS")
        .ok()
        .and_then(|raw| raw.parse::<usize>().ok())
        .unwrap_or(DEFAULT_GUARD_RUNS)
        .max(3);
    let required_gain_percent = std::env::var("WORKFLOW_BENCH_MIN_GAIN_PERCENT")
        .ok()
        .and_then(|raw| raw.parse::<u128>().ok())
        .unwrap_or(DEFAULT_CONCURRENCY_GAIN_PERCENT)
        .min(99);

    let mut sequential_samples = Vec::with_capacity(guard_runs);
    let mut concurrent_samples = Vec::with_capacity(guard_runs);
    for _ in 0..guard_runs {
        sequential_samples.push(rt.block_on(async {
            let runtime = WorkflowRuntime::new(
                sequential_comparison_workflow(),
                &llm,
                Some(&tool),
                options.clone(),
            );
            let started = std::time::Instant::now();
            runtime
                .execute(json!({}), None)
                .await
                .expect("sequential comparison run should succeed");
            started.elapsed()
        }));

        concurrent_samples.push(rt.block_on(async {
            let runtime = WorkflowRuntime::new(
                concurrent_comparison_workflow(),
                &llm,
                Some(&tool),
                options.clone(),
            );
            let started = std::time::Instant::now();
            runtime
                .execute(json!({"values": [1, 2, 3]}), None)
                .await
                .expect("concurrent comparison run should succeed");
            started.elapsed()
        }));
    }

    let sequential_elapsed = median_elapsed(sequential_samples);
    let concurrent_elapsed = median_elapsed(concurrent_samples);

    assert!(
        concurrent_elapsed.as_millis() * 100
            <= sequential_elapsed.as_millis() * (100 - required_gain_percent),
        "concurrent workflow regression: concurrent={:?}, sequential={:?}, min_gain_percent={}, guard_runs={}",
        concurrent_elapsed,
        sequential_elapsed,
        required_gain_percent,
        guard_runs,
    );
}

fn median_elapsed(mut samples: Vec<Duration>) -> Duration {
    samples.sort_unstable();
    samples[samples.len() / 2]
}

fn runtime_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("workflow_runtime");
    group.measurement_time(Duration::from_secs(10));

    let rt = tokio::runtime::Runtime::new().expect("tokio runtime should build");
    let llm = BenchLlm;
    let tool = BenchTool;
    let delay_tool = DelayEchoTool {
        delay: Duration::from_millis(10),
    };
    let workflow = benchmark_workflow();
    let sequential_workflow = sequential_comparison_workflow();
    let concurrent_workflow = concurrent_comparison_workflow();
    let dense_scope = dense_scope_workflow(32);
    let runtime_options = WorkflowRuntimeOptions {
        enable_trace_recording: false,
        ..WorkflowRuntimeOptions::default()
    };

    assert_concurrency_regression_guard(&rt);

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

    group.bench_function("sequential_execute", |b| {
        b.to_async(&rt).iter(|| async {
            let runtime = WorkflowRuntime::new(
                sequential_workflow.clone(),
                &llm,
                Some(&delay_tool),
                runtime_options.clone(),
            );
            let _ = runtime
                .execute(json!({}), None)
                .await
                .expect("sequential benchmark run should succeed");
        })
    });

    group.bench_function("concurrent_execute", |b| {
        b.to_async(&rt).iter(|| async {
            let runtime = WorkflowRuntime::new(
                concurrent_workflow.clone(),
                &llm,
                Some(&delay_tool),
                runtime_options.clone(),
            );
            let _ = runtime
                .execute(json!({"values": [1, 2, 3]}), None)
                .await
                .expect("concurrent benchmark run should succeed");
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

    group.bench_function("dense_scope_execute", |b| {
        b.to_async(&rt).iter(|| async {
            let runtime = WorkflowRuntime::new(
                dense_scope.clone(),
                &llm,
                Some(&tool),
                runtime_options.clone(),
            );
            let _ = runtime
                .execute(json!({"request_id": "dense"}), None)
                .await
                .expect("dense scope benchmark run should succeed");
        })
    });

    group.finish();
}

criterion_group!(benches, runtime_benchmarks);
criterion_main!(benches);
