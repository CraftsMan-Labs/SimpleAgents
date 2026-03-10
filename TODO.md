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
| NS1 | Hard-break nerdstats schema in workflow payload builder | Nerdstats should reflect the new contract without duplicate structures | `workflow_nerdstats` emits `step_details` and `llm_node_models`, and no longer emits `llm_node_metrics` | completed |
| NS2 | Track resolved model per `llm_call` node | Nerdstats must include model identity for each LLM execution node | Node-id to model map is captured during execution and attached to output used by nerdstats | completed |
| NS3 | Preserve runtime output compatibility for now | Requested break is scoped to nerdstats, not full workflow output payload contract | `result.step_timings` and `result.llm_node_metrics` remain unchanged outside nerdstats | completed |
| NS4 | Update nerdstats unit tests in Rust | Hard-break schema must be validated with regression coverage | Tests assert `step_details` and `llm_node_models`, and remove assertions for `nerdstats.llm_node_metrics` | completed |
| NS5 | Update Python fallback nerdstats shape | Fallback output should match new nerdstats contract when event metadata is unavailable | `examples/workflow_email/run_with_chat_history.py` fallback emits `step_details` and `llm_node_models`, without `llm_node_metrics` | completed |
| NS6 | Update docs for nerdstats field changes | Consumers need accurate field names and semantics for integrations | `docs/BINDINGS_PYTHON.md` and `PERFORMANCE.md` document the new nerdstats schema | completed |
| NS7 | Keep token-availability diagnostics semantics | `llm_nodes_without_usage` explains null token totals and stream-usage gaps | Existing behavior for `llm_nodes_without_usage`, `token_metrics_available`, and `token_metrics_source` remains unchanged | completed |
