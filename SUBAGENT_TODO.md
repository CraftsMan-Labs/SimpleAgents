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
| NS1, NS2, NS3, NS7 | SA-Nerdstats-Core | `crates/simple-agents-workflow/src/yaml_runner.rs` | Core schema and model tracking changes must be implemented at Rust source-of-truth layer | Nerdstats hard break implemented with `step_details` + `llm_node_models`, no `nerdstats.llm_node_metrics`, and unchanged token-availability diagnostics | completed | Implemented in `yaml_runner.rs`; targeted `workflow_nerdstats` tests passed |
| NS4 | SA-Nerdstats-Tests | `crates/simple-agents-workflow/src/yaml_runner.rs` test module | Schema break requires deterministic tests to prevent regressions | Updated assertions for new nerdstats keys and removed old-key assertions | completed | Updated tests in `yaml_runner.rs`; targeted cargo tests passed |
| NS5 | SA-Nerdstats-Python-Fallback | `examples/workflow_email/run_with_chat_history.py` | Fallback path must match new schema for consistency | Fallback nerdstats emits new keys and shape | completed | Updated fallback schema and validated with `py_compile` |
| NS6 | SA-Nerdstats-Docs | `docs/BINDINGS_PYTHON.md`, `PERFORMANCE.md` | Contract changes must be documented for binding users and perf readers | Docs reflect renamed and removed nerdstats fields plus model map addition | completed | Corrected docs to distinguish return payload (`step_timings`) vs nerdstats (`step_details`) |
