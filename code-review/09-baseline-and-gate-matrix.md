# Baseline and Gate Matrix

Date: 2026-04-02  
Scope: `T0.1` and `T0.2` from `TODO.md`

## T0.1 Program Baselines

### Duplicate-file baseline

- Scan method: `git ls-files -s` + SHA-256 content hashing for regular tracked files (`100644` only).
- Result:
  - `regular_files=383`
  - `symlink_files=9`
  - `duplicate_groups_regular=0`
  - `duplicate_files_total_regular=0`

Note: If symlink targets are dereferenced, mirrored skill paths appear as content duplicates by design.

### Largest tracked files (top 20)

1. `190145` `crates/simple-agents-workflow/src/yaml_runner.rs`
2. `114737` `crates/simple-agents-workflow/src/runtime.rs`
3. `102878` `crates/simple-agents-py/src/lib.rs`
4. `100998` `Cargo.lock`
5. `87942` `docs/package-lock.json`
6. `65363` `bindings/wasm/simple-agents-wasm/rust/src/lib.rs`
7. `50357` `crates/simple-agents-workflow/src/yaml_runner/client_executor.rs`
8. `45378` `crates/simple-agents-providers/src/openai/mod.rs`
9. `45250` `examples/uv.lock`
10. `42799` `crates/simple-agents-ffi/src/lib.rs`
11. `42221` `bindings/go/simpleagents.go`
12. `41766` `crates/simple-agents-cli/src/main.rs`
13. `39354` `crates/simple-agents-napi/src/lib.rs`
14. `38696` `crates/simple-agents-workflow/src/runtime/engine.rs`
15. `38168` `crates/simple-agents-workflow/src/validation.rs`
16. `36025` `crates/simple-agents-workflow/src/worker.rs`
17. `35610` `crates/simple-agents-healing/src/parser.rs`
18. `34953` `bindings/wasm/simple-agents-wasm/index.js`
19. `33335` `crates/simple-agents-providers/src/anthropic/mod.rs`
20. `28825` `crates/simple-agents-healing/src/coercion.rs`

### Weak typing hotspot counts (public surfaces)

- `crates/simple-agents-py/simple_agents_py.pyi`: `Any` occurrences = `41`
- `crates/simple-agents-napi/index.d.ts`: `any` occurrences = `1`
- `bindings/go/simpleagents.go`: `map[string]any` occurrences = `25`

### High-complexity hotspot list (tracked)

- `crates/simple-agents-workflow/src/yaml_runner.rs:1102` (`extract_last_parsable_object`)
- `crates/simple-agents-workflow/src/yaml_runner/client_executor.rs:11` (`complete_structured`)
- `crates/simple-agents-workflow/src/runtime/engine.rs:105` (`execute`)
- `crates/simple-agents-workflow/src/validation.rs:250` (`validate_node_kind_fields`)
- `crates/simple-agents-workflow/src/yaml_runner/execute.rs:9` (`run_workflow_yaml_with_custom_worker_and_events_and_options_impl`)

These remain prioritized decomposition candidates for follow-up hardening and file-size reduction.

### Acceptance checklist (active)

- Duplicate-file baseline captured and reproducible command documented.
- Largest-file baseline captured as a top-20 ranked list.
- Weak typing hotspot counts captured for Python/Node/Go public surfaces.
- High-complexity hotspot list captured with file+line anchors.
- Baseline artifact linked from active task tracker.

## T0.2 Regression Gate Matrix

Run date: 2026-04-02

- `make test-rust`: `pass`
- `make clippy`: `pass` (warnings only)
- `make fmt`: `pass`
- `make test-python`: `partial` (80 pass / 16 fail; failures are live-network tests requiring local proxy at `http://localhost:4000`)
- `make build-node`: `pass`
- `make test-node`: `pass`
- `make release-go`: `pass`
- `make test-go-bindings`: `pass`
- `make test-binding-contracts`: `pass`
- `make test-binding-layers`: `pass` (after serial re-run; first parallel run hit transient `napi` PATH issue)

Gate interpretation:

- Core non-live regression gates pass.
- Python live-integration failures are environment-dependent, not code-level regressions.
- Matrix is now attached for task batches and can be repeated as a standard release gate.
