# Active TODO

Date: 2026-03-10
Purpose: Scratchpad for current execution tasks.

## Status values

- `pending`
- `in_progress`
- `completed`
- `blocked`

## Scratchpad

| ID | Task | Why this is needed | Expected outcome | Status |
|---|---|---|---|---|
| NS8 | Rename telemetry keys to `reasoning_tokens` | We are standardizing on reasoning terminology and removing `thinking_tokens` naming drift | All workflow/provider/binding outputs use `reasoning_tokens` and `total_reasoning_tokens`; no `thinking_tokens` keys remain | completed |
| NS9 | Propagate provider reasoning usage into workflow metrics | Reasoning token counts are currently dropped in usage flow, causing null totals despite reasoning stream deltas | Provider response/stream-final usage populates `reasoning_tokens` and workflow totals aggregate it | completed |
| NS10 | Move model attribution into `step_details` and remove `llm_node_models` | Nerdstats contract should attribute models per step instead of maintaining a separate top-level map | Every `llm_call` entry in `step_details` includes `model_name`; top-level `llm_node_models` is removed | completed |
| NS11 | Align Go and Python consumers with new nerdstats contract | Bindings/examples must decode renamed token fields and new per-step model field | Go structs and Python fallback paths emit/consume `reasoning_tokens`, `total_reasoning_tokens`, and `step_details[].model_name` | completed |
| NS12 | Refresh tests and docs for the schema break | Contract changes need regression coverage and documentation parity across surfaces | Rust/provider/binding tests and docs assert new keys and remove stale `thinking_tokens`/`llm_node_models` references | completed |
| NS13 | Verify end-to-end via `make run-go-chat-history` | Fix must be validated in the exact user-reported workflow path | Repro run shows expected nerdstats schema and reasoning totals key (`total_reasoning_tokens`), non-null when provider usage includes reasoning data | completed |
