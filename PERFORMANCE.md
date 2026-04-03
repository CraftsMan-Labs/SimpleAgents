# Performance Baseline (Documentation-Only)

This document records the current, repository-visible performance picture for workflow execution across Rust, Python, Node.js/TypeScript, and Go.

This file combines two sources:

- repository-declared metrics surfaces (bench/binding docs and committed trace artifacts)
- executed command measurements captured on 2026-04-03 in this environment

## Cross-Language Workflow Performance Coverage

| Language | Workflow runtime source | Runtime metrics currently exposed | Peak memory currently exposed |
| --- | --- | --- | --- |
| Rust | `simple-agents-workflow` runtime + Criterion bench harness | Criterion timing (`linear_execute`, `sequential_execute`, `concurrent_execute`, `worker_pool_submit`); concurrency gain guard (`WORKFLOW_BENCH_MIN_GAIN_PERCENT`, default `15`) | Not exposed in runtime output or benchmark docs |
| Python | Rust-backed workflow runner via `simple-agents-py` | Return payload: `total_elapsed_ms`, `step_timings`, `llm_node_metrics`, token totals, tokens/sec; nerdstats: `step_details` (including `model_name`), optional `ttft_ms` | Not exposed by binding output |
| Node.js / TypeScript | Rust-backed workflow runner via `simple-agents-node` | `total_elapsed_ms`, `step_timings` (same Rust source-of-truth runner) | Not exposed by binding output |
| Go | Rust-backed workflow runner via Go FFI | `TotalElapsedMS`, `StepTimings` (same Rust source-of-truth runner) | Not exposed by binding output |

## Current Time-to-Run Snapshot From Committed Trace Artifacts

Source scanned: `examples/workflow_email/traces/*.jsonl` (first JSON object per line with `total_elapsed_ms`).

- Sample count: `104` workflow runs
- Min total runtime: `318 ms`
- Median total runtime: `10244 ms`
- Mean total runtime: `9600.46 ms`
- Max total runtime: `27787 ms`

Representative extremes:

- Fastest observed: `examples/workflow_email/traces/chat-session-20260222T161853Z-ad453a4f-061b-4b5e-a6a7-7a1ea094594c.jsonl` (`318 ms`, single-node draft workflow)
- Slowest observed: `examples/workflow_email/traces/chat-session-20260221T101422Z.jsonl` (`27787 ms`, multi-step clarify/capabilities flow)

## Peak Memory Consumption Status

Peak memory consumption is not currently emitted as a first-class metric by the workflow runtime output payloads in any binding (Rust/Python/Node/Go).

Current state:

- Runtime latency/timing metrics: available
- Token/throughput metrics: available (strongest in Python payload surface)
- Peak RSS / heap / allocation high-water mark: not available in committed outputs

## Where Time Metrics Are Defined

- Rust benchmark and guard: `crates/simple-agents-workflow/benches/runtime_benchmarks.rs`
- Workflow performance process: `docs/WORKFLOW_PERFORMANCE.md`
- Python workflow metrics surface: `docs/BINDINGS_PYTHON.md`
- Node workflow metrics surface: `docs/BINDINGS_NODE.md`
- Go workflow metrics surface: `docs/BINDINGS_GO.md`

## Language-Specific Details

### Rust

- Primary perf harness: Criterion benchmark at `crates/simple-agents-workflow/benches/runtime_benchmarks.rs`
- Bench functions exercised:
  - `linear_execute`
  - `sequential_execute`
  - `concurrent_execute`
  - `worker_pool_submit`
- Concurrency regression guard:
  - `WORKFLOW_BENCH_GUARD_RUNS` (default `7`, min `3`)
  - `WORKFLOW_BENCH_MIN_GAIN_PERCENT` (default `15`, max `99`)
- Runtime-time metric shape in docs:
  - per-benchmark timing via Criterion reports (`target/criterion/...`)
- Memory metric status:
  - no benchmark field in repository docs for peak RSS/heap

### Python (`simple-agents-py`)

- Workflow entrypoints:
  - `run_email_workflow_yaml(...)`
  - `run_workflow_yaml(...)`
  - `run_email_workflow_yaml_stream(...)`
- Time metrics exposed in return payload:
  - `total_elapsed_ms`
  - `step_timings[]` with per-node `elapsed_ms`
- Throughput/token metrics exposed:
  - return payload: `llm_node_metrics`
  - `total_input_tokens`, `total_output_tokens`, `total_tokens`
  - `tokens_per_second`
  - optional `total_reasoning_tokens`, optional `ttft_ms`
  - nerdstats: `step_details` (`model_name` provides per-node model attribution)
  - nerdstats: `token_metrics_available`, `token_metrics_source`, `llm_nodes_without_usage`
- Memory metric status:
  - no `peak_memory`, RSS, or heap high-water field in output contract

### Node.js / TypeScript (`simple-agents-node`)

- Workflow entrypoints:
  - `runEmailWorkflowYaml(...)`
  - `runWorkflowYamlWithEvents(...)`
  - `runWorkflowYamlStream(...)`
- Time metrics exposed in return payload:
  - `total_elapsed_ms`
  - `step_timings[]`
- Event/stream path:
  - streaming emits event JSON strings via callback and returns final structured output
- Memory metric status:
  - no binding-level peak memory metric in documented API surface

### Go (`bindings/go` via FFI)

- Workflow entrypoints:
  - `RunEmailWorkflowYAML(...)`
  - `RunWorkflowYAMLWithEvents(...)`
  - `RunWorkflowYAMLStream(...)`
- Time metrics exposed in return payload:
  - `TotalElapsedMS`
  - `StepTimings`
- Runtime architecture:
  - delegates to Rust workflow runner through `simple-agents-ffi`
- Memory metric status:
  - no exported peak memory field in binding output

## Metric Name Mapping By Language

| Metric concept | Rust | Python | Node.js/TS | Go |
| --- | --- | --- | --- | --- |
| Total runtime | Criterion benchmark timing | `total_elapsed_ms` | `total_elapsed_ms` | `TotalElapsedMS` |
| Per-step runtime | Benchmark-specific (function timing) | `step_timings[].elapsed_ms` (return payload), `step_details[].elapsed_ms` (nerdstats) | `step_timings[].elapsed_ms` | `StepTimings[].ElapsedMS` |
| Tokens/sec | Not in Criterion bench output | `tokens_per_second` | Via Rust workflow output when included | Via Rust workflow output when included |
| Peak memory | Not exposed | Not exposed | Not exposed | Not exposed |

## Optional Next Measurement Pass (Not Run Here)

When you want to add true per-language peak memory numbers, run each language entrypoint under an OS profiler (for example `/usr/bin/time -v`) and record:

- `Elapsed (wall clock) time`
- `Maximum resident set size (kbytes)`
- Workflow-level `total_elapsed_ms` from runtime output

This will produce comparable wall-clock + peak-memory baselines for Rust, Python, Node.js, and Go.

## Executed Metrics Snapshot (2026-04-03)

The following commands were executed in this repository to capture real elapsed time plus sampled process-tree peak RSS (`/proc` polling).

### Rust (workflow runtime benchmarks)

Command:

```bash
cargo bench -p simple-agents-workflow --bench runtime_benchmarks -- --sample-size 10
```

Observed benchmark timings:

- `workflow_runtime/linear_execute`: `5.2946 µs` to `5.3529 µs`
- `workflow_runtime/sequential_execute`: `33.186 ms` to `33.240 ms`
- `workflow_runtime/concurrent_execute`: `11.084 ms` to `11.097 ms`
- `workflow_runtime/worker_pool_submit`: `12.324 µs` to `13.202 µs`
- `workflow_runtime/dense_scope_execute`: `181.55 µs` to `182.26 µs`

Observed command-level resource metrics:

- Wall-clock elapsed: `99312.21 ms`
- Peak RSS (process tree): `3824836 kB`
- Exit code: `0`

Note: the command above includes compile/link memory on this run and is not representative of precompiled production runtime.

Precompiled runtime-only measurement (same benchmark binary, no compile step in measured window):

```bash
cargo bench -p simple-agents-workflow --bench runtime_benchmarks -- --sample-size 10
```

- Wall-clock elapsed: `79579.83 ms`
- Peak RSS (process tree): `38500 kB` (~`37.60 MiB`)
- Exit code: `0`

### Python (run all workflow YAML files)

Command:

```bash
uv run --directory examples python workflow_email/python/run_all_yaml_workflows.py --email "Please process damaged order 9921 and suggest next actions"
```

Observed command-level resource metrics:

- Wall-clock elapsed: `39696.85 ms`
- Peak RSS (process tree): `2553836 kB`
- Exit code: `0`

Run summary (8 workflows):

- `ok`: `3`
  - `email-chat-draft-or-clarify.yaml` (`total_elapsed_ms=2356`)
  - `email-chat-draft-with-tool-calling.yaml` (`total_elapsed_ms=2638`)
  - `hr-warning-email-subgraph.yaml` (`total_elapsed_ms=1976`)
- `error`: `5` (custom worker wiring and structured output parse issues)

### Node.js / TypeScript (build + run all workflow YAML files)

Build command:

```bash
npm --prefix crates/simple-agents-napi run build:debug
```

- Build elapsed: `7429.76 ms`
- Build peak RSS (process tree): `1182700 kB`
- Exit code: `0`

Run command:

```bash
node examples/workflow_email/node/run_all_yaml_workflows.js "Please process damaged order 9921 and suggest next actions"
```

- Run elapsed: `19714.10 ms`
- Run peak RSS (process tree): `58044 kB`
- Exit code: `0`

Run summary (8 workflows):

- `ok`: `2`
  - `email-chat-draft-or-clarify.yaml` (`total_elapsed_ms=2581`)
  - `hr-warning-email-subgraph.yaml` (`total_elapsed_ms=1533`)
- `error`: `6` (missing custom worker/tool registry and structured output parse issues)

### Go (FFI build + run all workflow YAML files)

FFI prerequisite build:

```bash
cargo build -p simple-agents-ffi --release
```

- Elapsed: `9784.01 ms`
- Peak RSS (process tree): `1667832 kB`
- Exit code: `0`

Go all-workflows binary build:

```bash
CGO_CFLAGS="-I$PWD/../../crates/simple-agents-ffi/include" \
CGO_LDFLAGS="-L$PWD/../../target/release" \
LD_LIBRARY_PATH="$PWD/../../target/release:${LD_LIBRARY_PATH:-}" \
go build -o /tmp/workflow_email_all_bin ./examples/workflow_email_all
```

- Elapsed: `402.05 ms`
- Peak RSS (process tree): `122820 kB`
- Exit code: `0`

Run command (from repository root):

```bash
WORKFLOW_API_KEY=dummy_key_dummy_key_12345 \
LD_LIBRARY_PATH="$PWD/target/release:${LD_LIBRARY_PATH:-}" \
/tmp/workflow_email_all_bin "Please process damaged order 9921 and suggest next actions"
```

- Run elapsed: `2561.31 ms`
- Run peak RSS (process tree): `22140 kB`
- Exit code: `0`

Run summary (8 workflows):

- `ok`: `0`
- `error`: `8` (all workflows failed with provider invalid API key)

### Error Pattern Notes

- This environment does not have a real provider API key configured (`WORKFLOW_API_KEY`/`CUSTOM_API_KEY` unset).
- Python and Node command runs still exercise non-LLM paths, but LLM-backed steps and custom worker wiring requirements fail in several workflows.
- Go example runner enforces API key validation before execution, so a valid-length dummy key was used; all LLM-backed workflows then failed with provider invalid API key.
- Metrics above are still valid as observed end-to-end command and runtime measurements for this environment.
