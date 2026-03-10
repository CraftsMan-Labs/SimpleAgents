# SUBAGENT TODO

Purpose: Scratchpad for active subagent assignments.

## Status values

- `pending`
- `in_progress`
- `completed`
- `blocked`

## Scratchpad assignments

| Parent TODO | Subagent | Scope | Why | Expected Outcome | Status | Notes |
|---|---|---|---|---|---|---|
| NS8 | SA-Types-Contract | `crates/simple-agent-type/src/response.rs` and dependent types/tests | Canonical naming must be standardized at the shared usage type layer | Unified usage contracts expose `reasoning_tokens` (not `thinking_tokens`) | completed | Renamed usage contract; retained deserialize alias for legacy `thinking_tokens` |
| NS9 | SA-Providers-Reasoning-Usage | `crates/simple-agents-providers/src/openai/*` plus provider tests | Provider parsing currently drops reasoning usage, causing null totals downstream | Response and streaming usage mapping sets `reasoning_tokens` when provider emits reasoning token detail fields | completed | Added OpenAI/OpenRouter mapping and provider tests for usage detail parsing |
| NS9, NS10 | SA-Workflow-Nerdstats-Core | `crates/simple-agents-workflow/src/yaml_runner.rs` | Source-of-truth aggregation and nerdstats schema updates belong in Rust workflow core | Workflow totals include `total_reasoning_tokens`; `step_details` includes `model_name`; top-level `llm_node_models` removed | completed | Updated workflow aggregation, nerdstats payload, and yaml_runner tests |
| NS11 | SA-Bindings-Go-Python | `bindings/go/simpleagents.go`, `examples/workflow_email/run_with_chat_history.py` | Consumer surfaces must align with renamed keys and per-step model attribution | Go/Python output and fallback paths use `reasoning_tokens` keys and `step_details[].model_name` | completed | Updated Go structs and Python fallback schema/logic |
| NS12 | SA-Tests-Docs | Workflow/provider/binding tests and `docs/BINDINGS_PYTHON.md`, `PERFORMANCE.md` | Schema break must be protected by tests and reflected in docs | Assertions and docs reference `reasoning_tokens`/`total_reasoning_tokens`, and no `llm_node_models` | completed | Rust/provider tests and docs now match new contract |
| NS13 | SA-E2E-Repro-Validation | Go chat-history example + trace inspection | Final verification should mirror user-reported reproduction path | `make run-go-chat-history` run confirms final nerdstats schema and reasoning-token totals behavior | completed | Verified output includes `step_details[].model_name` and `total_reasoning_tokens` key |
