# Domain Constraints: Python Data/ETL

- Prefer idempotent transforms and checkpointing.
- Separate schema validation from transform logic.
- Track row-level/partition-level failure metrics.
- Avoid loading entire datasets when streaming/chunking fits.
