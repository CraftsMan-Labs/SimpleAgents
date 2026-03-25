# SUBAGENT TODO

Purpose: Subagent ownership map for WASM rollout tasks in `TODO.md` (`WS*`).

## Status values

- `pending`
- `in_progress`
- `completed`
- `blocked`

## Active assignments

| Parent TODO | Subagent | Scope | Why | Expected Outcome | Status | Notes |
|---|---|---|---|---|---|---|
| WS0 | SA-Contract-Parity | Shared TS/Rust API contracts, compatibility matrix, migration notes | Prevent API drift before implementation starts | Signed-off parity spec with explicit browser exceptions | completed | Added preview contract in `docs/BINDINGS_WASM.md` with locked browser differences and migration constraints |
| WS1 | SA-WASM-Packaging | WASM binding package scaffold, loader/init, npm publish config | Browser runtime needs installable package equivalent to Node binding | `simple-agents-wasm` package scaffold with build/release pipeline draft | completed | Added `bindings/wasm/simple-agents-wasm` package with `package.json`, typed exports, and runtime implementation scaffold |
| WS2 | SA-WASM-Completion-Streaming | Completion + stream + streamEvents implementation in WASM binding | Core runtime value depends on LLM calls and streaming parity | Browser-capable completion/stream APIs with typed outputs and error mapping | completed | Rust-backed wasm client now serves completion + stream events via `wasm-bindgen`; healed/schema remains explicitly unsupported with typed errors |
| WS3 | SA-WASM-Workflow-API | YAML workflow runner browser-safe API surface | Node path-based APIs are incompatible with browser sandbox | Add string/object workflow methods and explicit unsupported-path errors | completed | Implemented `runWorkflowYamlString` + `runWorkflowYaml` unsupported-path error behavior in wasm package |
| WS4 | SA-Parity-Tests | Shared Node/WASM fixture tests and failure-mode coverage | "Near replica" claim must be measurable | Contract parity suite passing for result shape, usage, streaming events, and errors | pending | Reuse same fixtures across both bindings |
| WS5 | SA-YamSLAM-Runtime-Adapter | YamSLAM runtime abstraction and progressive rollout wiring | Need zero-downtime switch from Node route to WASM-first | Adapter-based runtime selection with WASM default + Node fallback | pending | Keep `/api/complete` available until parity + deploy checks pass |
| WS6 | SA-Browser-Security-DX | BYOK handling, redaction, error UX, CORS diagnostics | Browser mode increases user-facing failure modes | Safe logging/redaction and clear runtime error guidance in UI | pending | Verify no credential leakage in logs or telemetry payloads |
| WS7 | SA-Deploy-Release | Vercel smoke tests, release checklist, cutover readiness | Deployment confidence is required before fallback removal | Preview deploy passes with WASM default; release checklist completed | pending | Reference `make-it-deploy-vercel-happen.md` for operational checks |
| WS8 | SA-Docs-Cutover | Docs updates for package usage, migration, fallback policy | Users need clear implementation and upgrade guidance | Updated docs in `docs/` plus YamSLAM README/runtime notes | in_progress | Added docs map/index links and preview binding reference; YamSLAM runtime cutover docs still pending |

## Coordination notes

- Keep assignments non-overlapping; share contract artifacts from WS0 into WS1-WS5.
- Any deviation from parity contract must update WS0 notes before merge.
- WS5 should not remove Node fallback until WS4 and WS7 are completed.
