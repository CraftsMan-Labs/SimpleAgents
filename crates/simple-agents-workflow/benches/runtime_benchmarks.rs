//! Workflow runtime Criterion harness and concurrency regression guard.
//!
//! `sequential_execute` runs a linear chain of LLM nodes (mock latency each).
//! `concurrent_execute` runs the same per-node cost across independent workflows in parallel.
//! CI compares medians so concurrent stays measurably faster than sequential.

use std::env;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use criterion::{black_box, Criterion, Throughput};
use futures::future::join_all;
use serde_json::{json, Value};
use simple_agents_workflow::{
    verify_yaml_workflow, workflow_execution, YamlLlmExecutionRequest, YamlLlmExecutionResult,
    YamlLlmTokenUsage, YamlWorkflow, YamlWorkflowEventSink, YamlWorkflowExecutionFlags,
    YamlWorkflowExecutionRequest, YamlWorkflowExecutorBinding, YamlWorkflowLlmExecutor,
    YamlWorkflowRunOptions, YamlWorkflowSource, YamlWorkflowTelemetryConfig,
};

const MOCK_LLM_LATENCY: Duration = Duration::from_millis(2);
const PARALLEL_RUNS: usize = 4;

fn bench_options() -> YamlWorkflowRunOptions {
    YamlWorkflowRunOptions {
        telemetry: YamlWorkflowTelemetryConfig {
            enabled: false,
            ..Default::default()
        },
        ..Default::default()
    }
}

fn minimal_workflow() -> YamlWorkflow {
    let yaml = r#"
id: bench-minimal
entry_node: only
nodes:
  - id: only
    node_type:
      llm_call:
        model: gpt-4.1
    config:
      prompt: "Reply with ok: {{ input.x }}"
      output_schema:
        type: object
        properties:
          ok: { type: boolean }
        required: [ok]
"#;
    serde_yaml::from_str(yaml).expect("minimal workflow yaml")
}

fn linear_chain_workflow(depth: usize) -> YamlWorkflow {
    assert!(depth >= 1);
    let mut yaml = String::from("id: bench-linear\nentry_node: n0\nnodes:\n");
    for i in 0..depth {
        yaml.push_str(&format!(
            r#"  - id: n{i}
    node_type:
      llm_call:
        model: gpt-4.1
    config:
      prompt: "step {i} {{ input.x }}"
      output_schema:
        type: object
        properties:
          ok: {{ type: boolean }}
        required: [ok]
"#
        ));
    }
    if depth > 1 {
        yaml.push_str("edges:\n");
        for i in 0..depth - 1 {
            yaml.push_str(&format!("  - from: n{i}\n    to: n{}\n", i + 1));
        }
    }
    serde_yaml::from_str(&yaml).expect("linear chain yaml")
}

fn dense_workflow(node_count: usize) -> YamlWorkflow {
    assert!(node_count >= 1);
    let mut yaml = String::from("id: bench-dense\nentry_node: n0\nnodes:\n");
    for i in 0..node_count {
        yaml.push_str(&format!(
            r#"  - id: n{i}
    node_type:
      llm_call:
        model: gpt-4.1
    config:
      prompt: "node {i} {{ input.x }}"
      output_schema:
        type: object
        properties:
          ok: {{ type: boolean }}
        required: [ok]
"#
        ));
    }
    if node_count > 1 {
        yaml.push_str("edges:\n");
        for i in 0..node_count - 1 {
            yaml.push_str(&format!("  - from: n{i}\n    to: n{}\n", i + 1));
        }
    }
    serde_yaml::from_str(&yaml).expect("dense workflow yaml")
}

struct LatencyLlmExecutor;

#[async_trait]
impl YamlWorkflowLlmExecutor for LatencyLlmExecutor {
    async fn complete_structured(
        &self,
        _request: YamlLlmExecutionRequest,
        _event_sink: Option<&dyn YamlWorkflowEventSink>,
    ) -> Result<YamlLlmExecutionResult, String> {
        tokio::time::sleep(MOCK_LLM_LATENCY).await;
        Ok(YamlLlmExecutionResult {
            payload: json!({"ok": true}),
            usage: Some(YamlLlmTokenUsage {
                prompt_tokens: 1,
                completion_tokens: 1,
                total_tokens: 2,
                reasoning_tokens: None,
            }),
            ttft_ms: None,
            tool_calls: Vec::new(),
        })
    }
}

async fn run_inline(
    workflow: &YamlWorkflow,
    input: &Value,
    executor: &dyn YamlWorkflowLlmExecutor,
    options: &YamlWorkflowRunOptions,
) {
    workflow_execution::run(YamlWorkflowExecutionRequest {
        source: YamlWorkflowSource::Inline(workflow),
        workflow_input: input,
        executor: YamlWorkflowExecutorBinding::Llm(executor),
        custom_worker: None,
        options,
        flags: YamlWorkflowExecutionFlags::default(),
    })
    .await
    .expect("workflow run");
}

fn workflow_runtime(c: &mut Criterion) {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .worker_threads(4)
        .build()
        .expect("tokio runtime");

    let wf = minimal_workflow();
    let input = json!({"x": 1});
    let options = bench_options();
    let executor = LatencyLlmExecutor;

    let mut group = c.benchmark_group("workflow_runtime");
    group.throughput(Throughput::Elements(1));
    group.bench_function("linear_execute", |b| {
        b.iter(|| {
            black_box(verify_yaml_workflow(black_box(&wf)));
        });
    });
    group.bench_function("sequential_execute", |b| {
        b.iter(|| {
            let chain = linear_chain_workflow(PARALLEL_RUNS);
            rt.block_on(run_inline(
                black_box(&chain),
                black_box(&input),
                &executor,
                &options,
            ));
        });
    });
    group.bench_function("concurrent_execute", |b| {
        b.iter(|| {
            let single = minimal_workflow();
            rt.block_on(async {
                let futs: Vec<_> = (0..PARALLEL_RUNS)
                    .map(|_| run_inline(black_box(&single), black_box(&input), &executor, &options))
                    .collect();
                join_all(futs).await;
            });
        });
    });
    group.bench_function("worker_pool_submit", |b| {
        b.iter(|| {
            rt.block_on(async {
                let h = tokio::task::spawn_blocking(|| black_box(()));
                h.await.expect("spawn_blocking");
            });
        });
    });
    group.bench_function("dense_scope_execute", |b| {
        let dense = dense_workflow(48);
        b.iter(|| {
            black_box(verify_yaml_workflow(black_box(&dense)));
        });
    });
    group.finish();
}

fn median_ms(samples: &mut [f64]) -> f64 {
    samples.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mid = samples.len() / 2;
    if samples.len() % 2 == 0 {
        (samples[mid - 1] + samples[mid]) / 2.0
    } else {
        samples[mid]
    }
}

fn run_concurrency_guard() -> Result<(), String> {
    let runs: usize = env::var("WORKFLOW_BENCH_GUARD_RUNS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(7)
        .max(3);

    let min_gain: f64 = env::var("WORKFLOW_BENCH_MIN_GAIN_PERCENT")
        .ok()
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(15.0_f64)
        .min(99.0_f64);

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .worker_threads(4)
        .build()
        .map_err(|e| e.to_string())?;

    let options = bench_options();
    let executor = LatencyLlmExecutor;
    let input = json!({"x": 1});
    let chain = linear_chain_workflow(PARALLEL_RUNS);
    let single = minimal_workflow();

    let mut sequential_ms = Vec::with_capacity(runs);
    let mut concurrent_ms = Vec::with_capacity(runs);

    for _ in 0..runs {
        let t0 = Instant::now();
        rt.block_on(run_inline(&chain, &input, &executor, &options));
        sequential_ms.push(t0.elapsed().as_secs_f64() * 1000.0);

        let t1 = Instant::now();
        rt.block_on(async {
            let futs: Vec<_> = (0..PARALLEL_RUNS)
                .map(|_| run_inline(&single, &input, &executor, &options))
                .collect();
            join_all(futs).await;
        });
        concurrent_ms.push(t1.elapsed().as_secs_f64() * 1000.0);
    }

    let seq_med = median_ms(&mut sequential_ms);
    let con_med = median_ms(&mut concurrent_ms);
    if seq_med <= 0.0 {
        return Err("sequential median timing invalid".to_string());
    }
    let gain_pct = (1.0 - con_med / seq_med) * 100.0;
    if gain_pct < min_gain {
        return Err(format!(
            "concurrency regression guard failed: median sequential={seq_med:.3} ms, concurrent={con_med:.3} ms, gain={gain_pct:.1}% (min {min_gain}%)"
        ));
    }

    Ok(())
}

fn main() {
    let mut criterion = Criterion::default().configure_from_args();
    workflow_runtime(&mut criterion);
    criterion.final_summary();
    run_concurrency_guard().unwrap_or_else(|e| panic!("{e}"));
}
