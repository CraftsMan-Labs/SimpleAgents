# Remediation Baseline Truth Matrix

Date: 2026-03-20

Purpose: establish a reproducible baseline of high-priority findings from `code-review/` and mark each finding as `true`, `partially_true`, or `stale` against the current repository state.

## Repro Commands

- Rust compile sanity: `cargo check -p simple-agents-workflow -p simple-agents-py -p simple-agents-providers`
- Workflow regression tests: `cargo test -p simple-agents-workflow`
- Cache regression tests: `cargo test -p simple-agents-cache`
- Type serialization tests: `cargo test -p simple-agent-type`
- Bench smoke (local): `cargo bench -p simple-agents-workflow --bench runtime_benchmarks -- --sample-size 10`

## Finding Matrix

| Finding | Source | Baseline status | Evidence | Remediation track |
|---|---|---|---|---|
| `yaml_runner.rs` god-module complexity | `00-queen-consolidated-report.md` | `true` | File remains large and multi-responsibility | `QR6` |
| Runtime scoped input clone pressure | `00-queen-consolidated-report.md` | `partially_true` | Scoped-input map reconstruction reduced; clone pressure still present in hot path | `QR2` |
| Python streaming runtime-per-poll + GIL blocking | `00-queen-consolidated-report.md` | `partially_true` | Shared runtime + `py.allow_threads` applied; additional Python regression coverage required | `QR1` |
| Combinatorial `run_*` workflow API | `00-queen-consolidated-report.md` | `partially_true` | `WorkflowRunner` builder added, wrappers still present for compatibility | `QR5` |
| Duplicate `ScopeAccessError` type ownership | `00-queen-consolidated-report.md` | `resolved` | Runtime now reuses canonical state error type | `QR7` |
| FFI `unsafe impl Send+Sync` without safety proof | `00-queen-consolidated-report.md` | `resolved` | Explicit `// SAFETY:` invariants documented on callback sink | `QR7` |
| Production mock custom worker fallback | `00-queen-consolidated-report.md` | `resolved` | Missing custom worker returns explicit runtime error | `QR4` |
| O(n log n) cache eviction path | `00-queen-consolidated-report.md` | `resolved` | Sort-based eviction replaced by amortized O(1) LRU bookkeeping | `QR3` |
| Sensitive provider API key serialization | `02-security-analysis.md` | `resolved` | `ProviderConfig` serialization redacts `api_key` | `QR8` |
| Forced HTTP/2 prior knowledge default | `02-security-analysis.md` | `resolved` | Client now uses negotiated protocol defaults | `QR8` |

## Notes

- This baseline is intentionally compatibility-first and tracks non-breaking remediation progress.
- `partially_true` entries are actively being completed under their mapped `QR*` tracks.
