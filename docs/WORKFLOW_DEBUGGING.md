# Workflow Debugging UX

This guide covers workflow debugging surfaces added for timeline, retries, and replay inspection.

## Inspect + Replay Controls

`simple-agents-workflow` now includes foundational controls for failure recovery and replay tuning:

- `WorkflowRuntime::execute_resume_from_failure` resumes from a `WorkflowCheckpoint`.
- `ReplayOptions.cache_policy` configures replay cache behavior:
  - `always` - prefer cached replay metadata when available.
  - `refresh` - always recompute replay validation from trace events.
  - `mixed` - use cache if complete, recompute when cache is partial/missing.

Example:

```rust
use simple_agents_workflow::{
    replay_trace_with_options, ReplayCachePolicy, ReplayOptions,
};

let report = replay_trace_with_options(
    trace,
    &ReplayOptions {
        cache_policy: ReplayCachePolicy::Mixed,
    },
)?;
println!("replayed {} events", report.total_events);
```

## Node Timeline

Use `node_timeline` to convert runtime events into a UI-friendly sequence:

```rust
use simple_agents_workflow::node_timeline;

let timeline = node_timeline(&result);
for entry in timeline {
    println!("{} {} {}", entry.step, entry.node_id, entry.event);
}
```

## Retry Reasons

Runtime results now expose `retry_events` with operation, failed attempt, and reason.
Use `retry_reason_summary` for grouped diagnostics:

```rust
use simple_agents_workflow::retry_reason_summary;

let retries = retry_reason_summary(&result.retry_events);
for group in retries {
    println!("{} {} retries={}", group.node_id, group.operation, group.retries);
}
```

## Replay Trace Inspection

Use `inspect_replay_trace` to validate trace structure and collect violations:

```rust
use simple_agents_workflow::inspect_replay_trace;

if let Some(trace) = result.trace.as_ref() {
    let inspection = inspect_replay_trace(trace);
    println!("valid={} events={}", inspection.valid, inspection.total_events);
}
```

## End-to-End Example

See `crates/simple-agents-workflow/examples/debug_inspection.rs` for a complete run that prints:

- node timeline entries
- retry reason groups
- replay validation output

## YAML Run Timing Output

For YAML workflow execution, the output includes per-step timing and total runtime:

- `step_timings[]` with `node_id`, `node_kind`, `elapsed_ms`
- `total_elapsed_ms`

Rust API entrypoints:

- `run_workflow_yaml_file_with_client`
- `run_workflow_yaml_with_client`
- compatibility wrappers: `run_email_workflow_yaml_file_with_client`, `run_email_workflow_yaml_with_client`

These are also exposed in Python/Node/Go bindings and return the same timing fields.

## Workflow Verifier

Before execution, workflow YAML validation checks run through `verify_yaml_workflow(...)` and reject invalid graphs.

Validation covers:

- missing entry node
- unknown edge `from`/`to` references
- unknown `switch` branch/default targets
- empty `llm_call.model`

Streaming-related validation includes streamability diagnostics:

- `llm_call.stream=true` with `heal=true` is flagged as non-streamable for that node
- runtime emits explanatory event text when streaming is disabled

Workflow event telemetry includes per-node resolved LLM input details:

- `node_llm_input_resolved` includes `metadata.prompt` and `metadata.prompt_template`
- `metadata.bindings[]` lists template provenance (`expression`, `source_path`, `resolved`, `missing`)

## Workflow Visualization

Use `workflow_to_mermaid(&WorkflowDefinition)` to render canonical IR workflows as Mermaid diagrams for debugging/review.

For YAML workflows, use:

- `yaml_workflow_to_mermaid(&YamlWorkflow)`
- `yaml_workflow_file_to_mermaid(path)`

YAML Mermaid rendering now prefers YAML -> canonical IR conversion (`yaml_workflow_to_ir`) when the YAML feature set is IR-compatible, and falls back to direct YAML graph rendering otherwise.
