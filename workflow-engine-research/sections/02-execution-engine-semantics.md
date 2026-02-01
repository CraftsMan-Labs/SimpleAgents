# Execution Engine Semantics

## Recommended Repo
- https://github.com/temporalio/temporal

## Why This Repo
- Durable workflow engine with clear execution semantics and state handling.
- Strong references for fail-fast behavior, retries, and compensation.

## Pros
- Deterministic workflow model with replay semantics.
- Clear separation of workflow logic and activities.
- Strong retry, timeout, and failure semantics.

## Cons
- Event-sourcing overhead and operational complexity.
- Focused on long-running workflows, not always real-time.

## What We Want To Build From This
- Deterministic execution semantics with event history and replay.
- Explicit failure handling and compensation patterns.
- Clear boundaries between orchestration logic and node execution.

## Why
- Temporal is the best reference for durability and replayability in production systems.

## Sources
- https://temporal.io/blog/workflow-engine-principles
- https://docs.temporal.io/workflows

## Notes
- Use Temporal semantics to guide DAG execution, retry boundaries, and durable state.
