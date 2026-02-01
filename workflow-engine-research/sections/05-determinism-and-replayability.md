# Determinism and Replayability

## Recommended Repo
- https://github.com/temporalio/sdk-core

## Why This Repo
- Focused core for deterministic workflow execution and replay.
- Good reference for replay boundaries, event history, and determinism checks.

## Pros
- Event history replay model is well-defined and production-proven.
- Determinism checks help catch non-replayable workflow changes.

## Cons
- Strict determinism is hard with LLM calls and external APIs.
- Event history storage can grow large for high-throughput workflows.

## What We Want To Build From This
- Event history capture at node boundaries (inputs, outputs, decisions).
- Replay from any node using recorded history and cached responses.
- Determinism checks where possible, with LLM-aware exceptions.

## Why
- Replayability is required for debugging, auditing, and recovery.

## Sources
- https://docs.temporal.io/workflows
- https://keithtenzer.com/temporal/temporal_time_travelling_replay/

## Notes
- Mirror event-history based replay with LLM-aware caching and trace capture.
