# Workflow Debugging UX

This guide covers workflow debugging surfaces added for timeline, retries, and replay inspection.

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
