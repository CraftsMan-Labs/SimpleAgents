# Code Review Audit

Consolidated findings from the 7-crate code review (simple-agent-type, simple-agents-core, simple-agents-healing, simple-agents-providers, simple-agents-workflow, simple-agents-py, simple-agents-napi).

## Summary

- **Total findings:** 52
- **Fixed:** 39
- **Deferred (documented rationale):** 7
- **Won't fix (intentional):** 6

---

## Phase 1 — Critical correctness (FIXED)

| Finding | Crate | Resolution |
|---------|-------|------------|
| `end` nodes throw UnsupportedNodeType at runtime | workflow | Implemented end node branch in `execute_single_node_step` |
| resume/humanResponse not forwarded from JS wrapper | napi | Forwarded via `run(request)` / `stream(request)` |
| Redundant workflow methods bloat API (8 methods) | napi | Removed 5 redundant wrappers; kept `run`/`stream`/`runWorkflow`/`streamWorkflow`/`runEvalSuite` |
| JS `runEvalSuite` drift from Rust | napi | Removed dead Rust `EvalSuiteTask`; documented JS-only eval |
| ClientBuilder routing/cache stubs lie | py | Removed 5 stub methods and vestigial error variants |
| `allow_additional_fields` flag unused | healing | Implemented in `coerce_object` |
| `StreamingParser::feed` is a no-op | healing | Implemented incremental array element extraction |
| Truncated JSON + null injection for required fields | healing | Added `strict_required` config with error-on-truncation |
| `try_healing` indexes empty choices | providers | Added empty-array guards |
| `parse_responses_response` returns `""` on missing output | providers | Returns error instead |
| Responses streaming is a silent no-op | providers | Hard-fails with error |
| WorkflowRunOutput returned as untyped dict/Record | py, napi | Added typed pyclass and TS interfaces with enum status |

## Phase 2 — Internal correctness (FIXED)

| Finding | Crate | Resolution |
|---------|-------|------------|
| Jitter field exposed but never applied | core | Implemented random 0.5×–1.5× factor |
| stream + HealedJson/CoercedSchema silently ignored | core | Returns `Config` error early |
| Duplicate `ProviderError` types | providers | Renamed internal to `TransportError` |
| Wrong error kind for healing disabled | providers | Changed `Validation` to `Config` |
| Healing + streaming schema dropped silently | providers | Documented as intentional design |
| `WorkflowError::Workflow(String)` loses structure | workflow | Embeds `YamlWorkflowRunError` directly |
| No `node_type` exclusivity validation | workflow | Added count check in `validation.rs` |
| `messages_to_value` silent Null | workflow | Added warning log on failure |
| Duplicate `build_execution_context` | workflow | Consolidated into one shared helper |
| `SimpleAgentsError::Config` too stringly for healing | core | Added `HealingDisabled` variant |
| Retry body clones undocumented cost | core | Added doc comment |
| `coercion_flags` always empty | py | Populated from engine results |
| `run_eval_suite` doesn't handle Pydantic models | py | Added `model_dump` detection |
| Runtime mutex serialization undocumented | py | Added doc comment on `Client` struct |
| Crate docs claim "no I/O" inaccurately | type | Corrected `lib.rs` documentation |

## Phase 3 — Hardening (FIXED)

| Finding | Crate | Resolution |
|---------|-------|------------|
| Parser has no input size/depth limits | healing | Added `max_input_bytes` (10 MB) and `max_depth` (64) |
| Comment state machine corrupts parser state | healing | Fixed save/restore in `enter`/`exit_comment` |
| `try_parse` loses failure info via `.ok()` | healing | Returns `Result<Option<...>>` |
| Timeout value not stored/reported in errors | providers | Stored on struct, included in error messages |
| Custom worker GIL hold undocumented | py | Added doc comments |
| `is_cancelled` treats poison as cancelled | py | Distinguished with warning log |
| Event sink silently drops serialization failures | py | Sets `callback_error` flag |
| TSFN `NonBlocking` drop risk undocumented | napi | Added doc comments |
| NAPI event serialization failures silent | napi | Added `tracing::warn!` |
| `api_key` serializable and visible in `Debug` | core | `skip_serializing` + redacted `Debug` |

## Phase 4 — Documentation & tests

| Item | Status |
|------|--------|
| This audit document (`docs/CODE_REVIEW_AUDIT.md`) | Created |
| Docs updated for `OpenAiCompatProvider` rename | Updated |
| Docs updated for removed `AnthropicProvider` | Updated |
| Provider README refreshed | Updated |
| Workflow quickstart — typed `WorkflowRunOutput` note added | Updated |
| `cargo test --workspace` passes | Verified |

---

## Deferred / Won't Fix (with rationale)

| Finding | Rationale |
|---------|-----------|
| `event_type` from `String` to enum internally | Wire format is JSON string; Python already has `Literal` type. No user value from internal enum. |
| `parse_messages_*` typed errors | Callers handle `String`. Low ROI for the churn. |
| Multimodal size validation | Gateway concern, not SDK layer. |
| `Provider::execute_stream` default redesign | Breaking trait change; documented instead. |
| Healing outcomes newtypes | Gold-plating; `Value` at boundary is documented. |
| Split NAPI `lib.rs` into modules | Cosmetic refactor with zero user impact. |
| Subworkflow path trust validation | Docs added; validation would break legitimate use cases. |
