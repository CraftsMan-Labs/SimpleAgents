# Sample specification for ``run_spec_hitl_pipeline.py``

## Goal

Build a small HTTP API that accepts CSV uploads, validates column headers, and returns JSON summaries.

## Requirements

- Single-region deployment is acceptable for v1.
- Authentication is required but the mechanism is not decided yet.

## Open questions (intentionally vague)

- Error handling strategy for malformed CSV rows is TBD.
- Retention policy for uploaded files is not specified.
