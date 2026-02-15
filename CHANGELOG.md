# Changelog

## Unreleased

### Workflow Engine (Phase 9 hardening)

- Added workflow runtime benchmark regression harness controls and CI workflow gate
  for concurrency throughput (`.github/workflows/workflow-benches.yml`,
  `crates/simple-agents-workflow/benches/runtime_benchmarks.rs`).
- Added expression/runtime/worker security limits and explicit error contracts for
  complexity and resource guardrails (`expressions.rs`, `runtime.rs`, `worker.rs`).
- Expanded research example library to 10+ workflows and documented release
  checklist and hardening guidance (`workflow-engine-research/examples/`, `docs/`).
