# Code smells and architecture

## Module size and responsibility

| Area | Approx. size | Smell | Suggested direction (no behavior change) |
|------|--------------|-------|------------------------------------------|
| `crates/simple-agents-workflow/src/yaml_runner.rs` | ~5.6k lines | God module mixing types, loading, execution bridge, and large `#[cfg(test)]` suite | Split into `yaml_runner/types.rs`, `load.rs`, `tests.rs` (or keep tests near logic but in submodules) |
| `crates/simple-agents-workflow/src/runtime.rs` | ~3.5k lines | Core engine + extensive tests in one file | Same pattern: extract test modules / subsystems |
| `crates/simple-agents-py/src/lib.rs` | ~3.2k lines | Monolithic PyO3 surface | Consider submodules (`workflow`, `client`, `streaming`) behind `mod` |
| `bindings/wasm/simple-agents-wasm/rust/src/lib.rs` | ~1.8k lines | Large wasm_bindgen layer | Split by concern (client vs workflow) |

Large files are not automatically “wrong,” but they correlate with **harder reviews**, **merge conflicts**, and **inconsistent patterns** across regions of the same file.

## Panics and `unwrap` / `expect`

- **Tests and benches** use `expect` liberally — acceptable per project guidelines for test-only paths.
- **Production-adjacent code:** `try_healing`’s `unwrap` on `healing` (see security doc) is the main **non-test** smell spotted in this pass; guarded by caller today.
- **Lock poisoning:** `expect` on mutex guards appears in tests (`recording sink lock`); ensure production paths use `map_err` or `unwrap_or_else` if similar patterns exist outside `#[cfg(test)]`.

## Circular / layered dependencies

The workflow crate intentionally sits above core, providers, and types. No deep cycle was traced in this audit; the main issue is **horizontal complexity inside** `simple-agents-workflow` rather than wrong crate boundaries.

## Binding parity churn

Multiple languages duplicate similar method names (`runWorkflowYaml`, `runEmailWorkflowYaml`, `*Stream`, `*WithEvents`). Each addition multiplies:

- Type declarations (`.d.ts`, `.pyi`, Go wrappers).
- Contract tests (`crates/simple-agents-napi/test/contract.test.js`, `crates/simple-agents-py/tests/test_contract_fixtures.py`).

This is a **maintainability** smell more than a runtime bug.

## Prior baseline alignment

`code-review/08-baseline-truth-matrix.md` already labels **`yaml_runner.rs` god-module complexity** and **combinatorial `run_*` workflow API** as `true` / `partially_true`. This audit agrees and adds emphasis on **example and skill duplication** as separate inflation sources.
