# Workflow Performance and Profiling

This guide documents how to profile `simple-agents-workflow` and how the benchmark
regression harness protects concurrency throughput.

## Benchmarks

- Benchmark target: `crates/simple-agents-workflow/benches/runtime_benchmarks.rs`
- CI workflow: `.github/workflows/workflow-benches.yml`
- Regression guard: compares median sequential vs concurrent execution and fails if
  concurrent performance regresses below a required gain percentage.

Environment overrides used by the guard:

- `WORKFLOW_BENCH_GUARD_RUNS` (default `7`, minimum `3`)
- `WORKFLOW_BENCH_MIN_GAIN_PERCENT` (default `15`, maximum `99`)

## Local Commands

Run the benchmark suite:

```bash
cargo bench -p simple-agents-workflow --bench runtime_benchmarks -- --sample-size 10
```

Run a stricter regression threshold locally:

```bash
WORKFLOW_BENCH_GUARD_RUNS=9 WORKFLOW_BENCH_MIN_GAIN_PERCENT=20 \
  cargo bench -p simple-agents-workflow --bench runtime_benchmarks -- --sample-size 10
```

## Profiling Workflow Runtime Hot Paths

1. Run benchmark once to warm caches and compile artifacts.
2. Re-run with Criterion HTML reports enabled (already configured in dev-dependencies).
3. Inspect generated reports under `target/criterion/`.

Primary hot paths:

- `runtime::execute_from_node` orchestration loop
- `runtime::execute_tool_with_policy_for_scope` for tool-heavy workflows
- `scheduler::DagScheduler::run_bounded` for map/parallel fan-out

## Performance Contract (Phase 9)

- Concurrent map/parallel workflows must remain measurably faster than equivalent
  sequential flows on the same payload.
- Any concurrency regression that violates the configured gain threshold fails CI.
- Runtime benchmark entrypoints must stay deterministic and runnable on CI runners.
