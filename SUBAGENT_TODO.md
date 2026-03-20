# SUBAGENT TODO

Purpose: Subagent ownership map for `TODO.md` remediation tasks (`QR*`).

## Status values

- `pending`
- `in_progress`
- `completed`
- `blocked`

## Active assignments

| Parent TODO | Subagent | Scope | Why | Expected Outcome | Status | Notes |
|---|---|---|---|---|---|---|
| QR0 | SA-Baseline-Truth-Matrix | `code-review/*`, repo-wide metrics scripts, benchmark baselines | Report metrics are partially stale; remediation needs measurable ground truth | Checked-in baseline matrix: each finding marked true/partial/stale with reproducible command references | completed | Added `code-review/08-baseline-truth-matrix.md` with reproducible commands and finding status mapping |
| QR1 | SA-Python-Runtime-GIL | `crates/simple-agents-py/src/lib.rs` | Python overhead and thread blocking are highest-ROI issues | Reused runtime for stream iterators + `py.allow_threads` for blocking calls + regression tests | completed | Runtime reuse + `py.allow_threads` landed; layered Python binding tests pass |
| QR2 | SA-Workflow-Scope-Perf | `crates/simple-agents-workflow/src/runtime.rs` | Deep clone in `scoped_input` causes runtime allocation blowups | Lower-allocation scoped input implementation with parity tests and benchmark deltas | completed | Scoped-input map reconstruction reduction landed and benchmark coverage expanded via `dense_scope_execute` |
| QR3 | SA-Cache-LRU-Refactor | `crates/simple-agents-cache/src/memory.rs` | O(n log n) eviction path hurts throughput | Efficient eviction strategy preserving TTL, ordering semantics, and existing trait behavior | completed | Replaced sort-based eviction with amortized O(1) LRU queue + state accounting and parity tests |
| QR4 | SA-Mock-Path-Removal | `crates/simple-agents-workflow/src/yaml_runner.rs` | Production fallback mock outputs are unsafe and misleading | Mock paths moved behind test-only config; runtime returns explicit config error when worker is missing | completed | Added regression test for missing custom worker executor |
| QR5 | SA-Workflow-API-Builder | `crates/simple-agents-workflow/src/yaml_runner.rs`, `crates/simple-agents-workflow/src/lib.rs`, docs | `run_*` combinatorial API is hard to maintain | New builder entrypoint with wrappers retained for compatibility and marked for deprecation | completed | Added `WorkflowRunner` builder and routed compatibility helpers through builder path |
| QR6 | SA-Workflow-Modularization | `crates/simple-agents-workflow/src/yaml_runner.rs`, `crates/simple-agents-workflow/src/runtime.rs` + extracted modules | God modules reduce maintainability and testability | Behavior-preserving extraction into focused modules with unchanged public outputs | completed | Added focused modules `yaml_runner/runner.rs`, `yaml_runner/api.rs`, `yaml_runner/context.rs`, `yaml_runner/llm_tools.rs`, `yaml_runner/globals.rs`, `yaml_runner/client_executor.rs`, `yaml_runner/validation.rs`, `yaml_runner/execute.rs`, `runtime/scope.rs`, and `runtime/engine.rs`; orchestrator loop moved out while preserving wrappers and behavior |
| QR7 | SA-Error-Unsafe-Hardening | `crates/simple-agents-workflow/src/{runtime.rs,state/mod.rs}`, `crates/simple-agents-ffi/src/lib.rs`, event model types | Duplicate errors and weak unsafe invariants increase maintenance/security risk | Canonical shared `ScopeAccessError`, stronger `// SAFETY:` proofs + multithread tests, typed event enums/helpers | completed | Runtime now reuses canonical `ScopeAccessError`; FFI callback sink includes explicit `// SAFETY:` invariants |
| QR8 | SA-Security-Boundaries | workflow file loading paths, YAML parser entrypoints, provider config types, HTTP client defaults | Several medium/high security findings are fixable non-breakingly | Path-policy + size/depth guardrails + safer secret serialization + safer HTTP protocol defaults | completed | Added canonicalized YAML loader with file-size/depth guardrails; redacted ProviderConfig api_key serialization; removed forced HTTP/2 prior knowledge |
| QR9 | SA-Binding-Parity-DX | `crates/simple-agents-napi`, `crates/simple-agents-py`, `crates/simple-agents-ffi`, binding docs | Node/Python/Go parity and typing gaps degrade DX and reliability | Typed Node signatures, parity fixture expansion, reduced duplicated binding logic via shared helpers | completed | Updated NAPI return typings (`Promise<CompletionResult>`) and revalidated Node/Go/Python contracts |
| QR10 | SA-Test-Expansion | workflow/router/cache/cli/provider integration tests + temporal tests | High-risk subsystems need stronger regression safety | Priority P0/P1 tests implemented with CI-ready structure and deterministic timing where possible | completed | Added workflow runner regression tests and executed `make test-binding-contracts` + `make test-binding-layers` |
| QR11 | SA-Docs-Migration | `docs/*`, migration notes, release checklist | Users need safe adoption path for additive APIs and deprecations | Updated docs and migration guide for builder adoption, performance/security changes, and compatibility windows | completed | Updated YAML workflow, performance, security, and release checklist docs |
| QR12 | SA-Tracing-Carryover | `crates/simple-agents-workflow/tests/*`, quality gates via make targets | Previous program has unresolved integration/gate items | OTLP HTTP ingestion test completed; blocked quality gates resolved or explicitly documented with owner/actions | completed | Quality gates executed (`test-binding-contracts`, `test-binding-layers`); tracing HTTP/protobuf config checks already covered in observability tests |

## Archive: previous tracing assignments

- Historical `LF*` subagent records are superseded by `QR12` + new `QR*` plan above.
