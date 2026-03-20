# Active TODO

Date: 2026-03-20
Purpose: Non-breaking remediation program for all findings under `code-review/`.

## Status values

- `pending`
- `in_progress`
- `completed`
- `blocked`

## Non-breaking constraints (applies to every task)

1. Preserve existing public behavior across Rust/Python/Node/Go/FFI unless explicitly versioned + deprecated.
2. Prefer additive changes (new builders/helpers/typed APIs) with compatibility wrappers first.
3. Keep Rust as source of truth; bindings must reuse Rust behavior, not fork logic.
4. Every fix must include regression tests for both success and failure paths.
5. No unsafe expansion in core crates; FFI/Py unsafe invariants must be documented and tested.

## Master tasks

| ID | Task | Why this is needed | Expected outcome | Status |
|---|---|---|---|---|
| QR0 | Baseline and guardrails for remediation | Code-review metrics are partly stale; we need reproducible baselines before changing behavior | Baseline report checked in (counts, perf smoke, API surface map) and a remediation checklist that tracks true vs stale findings | completed |
| QR1 | Fix critical Python binding runtime overhead and GIL blocking | Current Python path creates Tokio runtimes per stream poll and holds GIL during blocking calls | Single reused runtime for streaming iterators, `py.allow_threads` around blocking calls, and no behavior regressions in Python tests | completed |
| QR2 | Eliminate workflow runtime hot-path cloning pressure | `RuntimeScope::scoped_input()` currently deep-clones accumulated outputs each step | Scoped-input path redesigned for lower allocation pressure with identical functional output and benchmark coverage | completed |
| QR3 | Replace cache eviction strategy with true O(1)-style LRU behavior | Current memory cache sorts entries on eviction path and scales poorly under load | In-memory cache uses efficient eviction data structure, preserves TTL semantics, and passes cache behavior parity tests | completed |
| QR4 | Remove production mock fallback behavior in YAML runner safely | Mock custom-worker output currently exists in production execution path | Runtime fails fast with actionable error when custom worker is missing; mock data moved to test-only fixtures | completed |
| QR5 | Consolidate workflow run API without breaking callers | Workflow crate exposes combinatorial `run_*` functions causing API bloat | Introduce `WorkflowRunner` builder; keep existing `run_*` wrappers as backward-compatible adapters with deprecation notices/docs | completed |
| QR6 | Decompose `yaml_runner.rs` and `runtime.rs` into focused modules | God-module/function complexity slows development and increases regression risk | Module split by responsibility (types/api/executor/telemetry/streaming/tools/validation/etc.) with unchanged external behavior | in_progress |
| QR7 | Resolve duplicated/unsafe structural risks | Duplicate `ScopeAccessError`, weak unsafe invariants, stringly event types increase risk | Canonical error type ownership, explicit `// SAFETY:` contracts + tests, typed workflow event kinds, and reduced duplication | completed |
| QR8 | Harden security boundaries from code-review findings | Need guardrails for path handling, YAML resource bounds, sensitive serialization, and protocol defaults | Path normalization policy, YAML size/depth limits, `ProviderConfig` secret-safe serialization, and safer HTTP client defaults | completed |
| QR9 | Binding parity and DX quality improvements | Node/Go/Python surfaces have gaps vs Rust API quality and parity | Typed Node return types, parity tests expanded, shared binding utilities, and preserved runtime behavior across bindings | completed |
| QR10 | Expand test strategy execution for high-risk subsystems | Workflow/router/cache/CLI and timing paths remain under-tested relative to risk | Priority test plan executed in phases (P0/P1 first), including integration and temporal tests for runtime/retry/cache/circuit-breaker | completed |
| QR11 | Documentation and migration notes for non-breaking rollout | We need contributors and users to adopt new APIs safely | Updated docs: deprecations, builder migration, security/perf notes, and release checklist with compatibility guarantees | completed |
| QR12 | Complete previous tracing carry-over tasks | Existing TODO has unresolved tracing integration/quality gate items | OTLP HTTP ingestion integration test complete and full quality gates pass (or blockers documented with remediation owner) | completed |

## Technical notes

- Keep compatibility wrappers for at least one release cycle when introducing builder-based replacements.
- Prioritize behavior-preserving refactors before semantic changes.
- For every performance fix, add a benchmark and a correctness parity test.
- For every security fix, add a negative test proving rejection behavior.
- Track all subagent ownership in `SUBAGENT_TODO.md` mapped to `QR*` tasks.

## Archive: previous tracing program

- Prior tracking IDs `LF1`..`LF11` remain historically completed/blocked work from 2026-03-18.
- Their active continuation is now represented by `QR12` above.
