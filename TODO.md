# Active TODO

Date: 2026-03-24
Purpose: Deliver `simple-agents-wasm` with near-API parity to `simple-agents-node`, then integrate YamSLAM with WASM-first runtime and Node fallback during rollout.

## Status values

- `pending`
- `in_progress`
- `completed`
- `blocked`

## Rollout decisions (locked)

1. Keep `/api/complete` Node route as temporary fallback for one release while WASM path stabilizes.
2. Browser workflow execution supports YAML string/object input only (no file path-based APIs in WASM).

## Core constraints (applies to every task)

1. Rust remains source-of-truth; WASM and Node bindings must reuse shared core behavior.
2. Preserve public behavior unless change is intentional, documented, and parity-tested.
3. Maintain explicit typed contracts and actionable errors across bindings.
4. Add regression tests for success and failure paths for every behavior change.
5. Do not leak secrets in logs, telemetry, or serialized payloads.

## Master tasks

| ID | Task | Why this is needed | Expected outcome | Status |
|---|---|---|---|---|
| WS0 | Define parity contract and migration guardrails | WASM rollout needs clear compatibility targets before implementation | Versioned Node/WASM contract doc + migration rules + explicit non-parity exceptions approved | completed |
| WS1 | Implement `simple-agents-wasm` package scaffold | Browser runtime cannot use N-API package directly | Publishable WASM package structure with loader, typed exports, and init flow | completed |
| WS2 | Port completion + streaming APIs to WASM | Core LLM operations must match Node behavior | `Client.complete`, `Client.stream`, and `Client.streamEvents` function in browser with typed outputs | completed |
| WS3 | Add browser-safe workflow execution APIs | Node path-based workflow APIs do not map to browser | `runWorkflowYamlString`/object-based workflow methods with explicit errors for path-only calls | completed |
| WS4 | Build Node/WASM parity test suite | Near-replica claim must be enforced by tests | Shared fixtures verify response/event/type parity and document acceptable differences | pending |
| WS5 | Integrate YamSLAM runtime adapter (WASM-first, Node fallback) | YamSLAM needs safe staged migration without breaking active users | Runtime selector defaults to WASM with opt-in or auto fallback to `/api/complete` route | pending |
| WS6 | Security and DX hardening for browser BYOK | Browser runtime increases CORS and credential UX concerns | Redaction-safe logs, clear CORS/auth errors, and documented BYOK handling guarantees | pending |
| WS7 | Deployability and release automation | Vercel deployment currently blocked by platform-specific native binary limits | WASM package release + YamSLAM deploy checklist pass + smoke tests green on Vercel preview | pending |
| WS8 | Documentation and cutover plan | Users need adoption guidance and rollback path | Updated docs for WASM usage, fallback policy, known constraints, and final cutover steps | in_progress |

## Technical notes

- Keep `simple-agents-node` as server runtime fallback until WS4+WS7 pass and preview deployments are stable.
- Keep browser API key handling explicit: forwarded only to target provider in WASM mode; if fallback is enabled, forwarded per-request to server runtime.
- Do not expose file-system based workflow helpers in browser-facing APIs.
- Prefer one adapter layer in YamSLAM (`runtime: "wasm" | "node"`) to avoid split logic throughout UI code.
- Add parity-focused fixtures once and reuse them in Node + WASM test layers.
