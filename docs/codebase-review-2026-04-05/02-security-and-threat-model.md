# Security and threat model

This section describes **inherent risks** of a YAML-driven agent runtime and what the codebase already mitigates. It is not a penetration test.

## Trust boundaries

| Boundary | Trusted side | Untrusted side | Notes |
|----------|--------------|----------------|-------|
| Workflow YAML | Often treated as **config** supplied by the integrator | If YAML is **user-uploaded**, it must be treated as code | Expressions, routing, and `custom_worker` dispatch are powerful |
| `custom_worker.handler` | Resolved only inside **host-provided** executors (Python/Node/etc.) | YAML chooses **which registered handler name** to invoke | Host must enforce allowlists and safe handlers |
| LLM providers | Network egress, API keys | Remote model behavior | Standard prompt-injection and data-exfiltration concerns |
| File-based workflow load | Paths chosen by host | Symlinks, oversized files | Code uses `canonicalize`, metadata checks, extension allowlist, size/depth limits (see `WORKFLOW_SECURITY.md`) |
| FFI / WASM | Host process | Callers of `unsafe` C API or browser context | Memory safety and secret exposure depend on caller discipline |

## Controls already documented in-repo

`docs/WORKFLOW_SECURITY.md` lists:

- Expression complexity limits (`ExpressionError::ComplexityLimitExceeded`).
- Runtime resource guards (`RuntimeSecurityLimits`).
- Worker request validation (`WorkerPoolError::InvalidRequest`).
- YAML file load guardrails (canonical path, regular file, `.yaml/.yml`, size/depth).
- Guidance on not embedding secrets in YAML; redacted provider serialization.

## Additional observations from code review

1. **OpenAI / Anthropic `try_healing`** uses `self.healing.as_ref().unwrap()` inside a private helper. Call sites in `transform_response` are guarded with `if self.healing.is_some()`. This is **logically consistent today** but **brittle under refactor**; prefer `if let Some(healing) = &self.healing` inside `try_healing` to eliminate the panic path entirely.

2. **WASM client patterns** (`bindings/wasm/...`) accept `api_key` in the JS-facing API. In a browser, any secret passed into WASM/JS is **exposed to XSS** and should not be treated as a secure vault. Document “server-side only” or proxy patterns for production.

3. **FFI (`simple-agents-ffi`)** — Expected `unsafe` for C ABI. Consumers must respect documented allocation ownership (`sa_string_free`, etc.). Risk is **incorrect host usage**, not necessarily a defect in the crate.

4. **Example `handlers.py`** — Demo logic treats certain substrings in user content as policy violations. That is **not an authentication or authorization control**; security reviewers may flag it unless clearly labeled as demo-only.

5. **Trace / session artifacts** — `.gitignore` excludes `examples/workflow_email/traces/`. Ensure trace files are never committed if they can contain PII or prompts.

## Automated scanning (recommended, not run here)

The audit environment did not have `cargo-audit` available. Recommended checks for a later CI pass:

- **Dependencies:** `cargo audit` (RustSec advisory DB).
- **Secrets:** `gitleaks` or GitHub secret scanning on PRs.
- **Static analysis:** `clippy` with `-D warnings` in CI; optional Semgrep rules for `unwrap()` in non-test paths.
- **License compliance:** `cargo deny` if distributing binaries.

## Residual risks (by design)

- **YAML + expressions + LLM** cannot be fully “sandboxed” without a separate isolation strategy (separate process, WASM worker with no FS, etc.).
- **Custom workers** run arbitrary host code; YAML only selects the handler name—the host must enforce policy.
