# Domain Constraints: Go Data Pipelines/Workers

- Use explicit backpressure in worker pools.
- Ensure idempotent processing where retries occur.
- Track per-batch/per-record failure metrics.
- Design for restart safety and checkpointed progress.
