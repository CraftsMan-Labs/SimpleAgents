# Workflow Performance and Profiling

This guide shows how to benchmark and profile `simple-agents-workflow` and how CI protects concurrency throughput against regressions.
By the end, you will be able to run local benchmark guards, inspect hot paths, and interpret the performance contract.

## Prerequisites

- Rust toolchain with benchmark support
- Access to workspace benchmark target
- Familiarity with workflow execution model

## Quick Path

1. Run the benchmark suite once to establish baseline.
2. Re-run with stricter guard thresholds if needed.
3. Inspect Criterion output under `target/criterion/`.
4. Compare sequential vs concurrent medians against guard policy.

## Benchmark Surfaces

- Benchmark target: `crates/simple-agents-workflow/benches/runtime_benchmarks.rs`
- CI workflow: `.github/workflows/workflow-benches.yml`
- Regression guard: median concurrent gain must exceed configured minimum

Guard environment overrides:

- `WORKFLOW_BENCH_GUARD_RUNS` (default `7`, minimum `3`)
- `WORKFLOW_BENCH_MIN_GAIN_PERCENT` (default `15`, maximum `99`)

## Run Commands

Run benchmark suite:

```bash
cargo bench -p simple-agents-workflow --bench runtime_benchmarks -- --sample-size 10
```

Run with stricter local guard threshold:

```bash
WORKFLOW_BENCH_GUARD_RUNS=9 WORKFLOW_BENCH_MIN_GAIN_PERCENT=20 \
  cargo bench -p simple-agents-workflow --bench runtime_benchmarks -- --sample-size 10
```

## Profiling Hot Paths

Primary hotspots to inspect:

- `runtime::execute_from_node` (orchestration loop)
- `runtime::execute_tool_with_policy_for_scope` (tool-heavy paths)
- `scheduler::DagScheduler::run_bounded` (map/parallel fan-out)

Recommended profiling loop:

1. Warm build/cache with one benchmark pass.
2. Re-run benchmarks and open Criterion HTML output.
3. Identify highest-cost path and test one focused optimization.
4. Re-run guard to verify no concurrency regression.

## Performance Contract

- Concurrent map/parallel workflows must remain measurably faster than equivalent sequential flows.
- CI fails when median gain drops below configured threshold.
- Benchmark entrypoints must stay deterministic on CI runners.

## Troubleshooting

### Benchmark variance is too noisy

Increase sample size and guard runs, then compare medians rather than single-run outliers.

### Concurrent gain unexpectedly drops

Inspect scheduler and tool execution hotspots first; these usually dominate fan-out path regressions.

### Local pass but CI fail

Use CI-like settings locally and avoid running with heavy background load when collecting baseline numbers.

## Next Steps

- Debug runtime anomalies with [Workflow Debugging UX](/WORKFLOW_DEBUGGING).
- Apply runtime limits from [Workflow Security](/WORKFLOW_SECURITY).
- Revisit workflow modeling guidance in [YAML Workflow System Guide](/YAML_WORKFLOW_SYSTEM).
