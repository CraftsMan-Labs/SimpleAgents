# Workflow Debugging UX

This guide covers the practical debugging surfaces for workflow timeline inspection, retry analysis, and replay validation.
By the end, you will be able to resume failed runs, inspect replay quality, and pinpoint retry causes quickly.

For full authoring/runtime behavior, see [YAML Workflow System Guide](/YAML_WORKFLOW_SYSTEM).

## Prerequisites

- Familiarity with workflow execution in [YAML Workflow System Guide](/YAML_WORKFLOW_SYSTEM)
- Access to workflow runtime results and trace outputs

## Quick Path

1. Resume from failure checkpoint when possible.
2. Build a node timeline from runtime output.
3. Group retry reasons to find dominant failure classes.
4. Run replay inspection to validate trace structure.
5. Render Mermaid graph to validate wiring assumptions.

## Debugging Surfaces

| Surface | Use it for | API/helper |
|---|---|---|
| Resume from failure | Continue from checkpoint instead of full rerun | `WorkflowRuntime::execute_resume_from_failure` |
| Replay with cache policy | Control replay recomputation cost/strictness | `replay_trace_with_options`, `ReplayOptions.cache_policy` |
| Node timeline | UI-friendly step/event sequence | `node_timeline(&result)` |
| Retry summary | Group retries by node + operation | `retry_reason_summary(&result.retry_events)` |
| Replay validation | Structural integrity and violation checks | `inspect_replay_trace(trace)` |

## Inspect + Replay Controls

Replay cache policy options:

- `always`: prefer cached replay metadata.
- `refresh`: always recompute replay validation from events.
- `mixed`: use cache if complete, recompute when partial/missing.

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

## Build a Node Timeline

```rust
use simple_agents_workflow::node_timeline;

let timeline = node_timeline(&result);
for entry in timeline {
    println!("{} {} {}", entry.step, entry.node_id, entry.event);
}
```

Use timeline output to verify event order and identify where execution diverges from expected branch behavior.

## Group Retry Reasons

```rust
use simple_agents_workflow::retry_reason_summary;

let retries = retry_reason_summary(&result.retry_events);
for group in retries {
    println!("{} {} retries={}", group.node_id, group.operation, group.retries);
}
```

This is the fastest way to identify whether failures are concentrated in one node, one provider operation, or one policy path.

## Validate Replay Trace

```rust
use simple_agents_workflow::inspect_replay_trace;

if let Some(trace) = result.trace.as_ref() {
    let inspection = inspect_replay_trace(trace);
    println!("valid={} events={}", inspection.valid, inspection.total_events);
}
```

If replay is invalid, fix graph definitions or runtime event emission assumptions before using replay output for production decisions.

## Workflow Verifier and Streaming Diagnostics

`verify_yaml_workflow(...)` validation covers:

- missing entry node
- unknown edge `from`/`to` references
- unknown `switch` branch/default targets
- empty `llm_call.model`

Streaming diagnostics include non-streamable combinations such as `llm_call.stream=true` with `heal=true`.

Event telemetry also includes `node_llm_input_resolved` metadata for prompt/template provenance and binding resolution details.

## Mermaid Visualization

Use graph rendering helpers for review and debugging:

- Canonical IR: `workflow_to_mermaid(&WorkflowDefinition)`
- YAML: `yaml_workflow_to_mermaid(&YamlWorkflow)`, `yaml_workflow_file_to_mermaid(path)`

YAML rendering prefers YAML->IR conversion when compatible and falls back to direct YAML graph rendering otherwise.

## Troubleshooting

### Replay says valid but behavior still differs

Compare timeline events and retry groups; structural replay validity does not guarantee semantic parity with changed prompt/tool behavior.

### Missing timings in output

Language bindings return the same timing fields on the workflow result (`total_elapsed_ms`, `step_timings`, etc.) for canonical APIs: Python `Client.run` / `stream`, Node `executeWorkflowYaml` / `executeWorkflowYamlStream`, and Rust `WorkflowRunner` (or the compatibility `run_workflow_yaml_*` helpers). If timings are absent, confirm you are reading the returned workflow output object, not only stderr logs.

### Resume from checkpoint fails

Ensure checkpoint is captured from the same runtime version and that referenced node ids still exist.

### Mermaid graph looks correct but run fails

Graph wiring may be valid while schema/model/tool configuration is invalid; run verifier and inspect node-level diagnostics.

## Next Steps

- Review workflow authoring rules in [YAML Workflow System Guide](/YAML_WORKFLOW_SYSTEM).
- Tune runtime behavior using [Workflow Performance](/WORKFLOW_PERFORMANCE).
- Apply guardrails from [Workflow Security](/WORKFLOW_SECURITY).
