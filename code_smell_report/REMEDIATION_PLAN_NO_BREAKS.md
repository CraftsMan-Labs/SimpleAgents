# Remediation Plan (No Existing Behavior Breakage)

Date: 2026-02-20
Related audit: `code_smell_report/REPO_CODE_SMELL_AUDIT_2026-02-20.md`

## Goal

Fix identified code smells and stale docs/artifacts while preserving all current public behavior and binding parity.

## Non-breaking guardrails

1. Preserve public APIs first
   - Do not remove or rename public functions/types in one step.
   - Add new internal abstractions behind existing entrypoints.
   - If API cleanup is needed, keep backward-compatible wrappers and deprecate gradually.

2. Change in small slices
   - One smell family per PR (example: only `runtime.rs` extraction in one PR).
   - No mixed refactor + feature work.
   - Keep each PR reversible.

3. Verification gates on every PR
   - `make test-rust`
   - `make clippy`
   - `make fmt`
   - If bindings touched:
     - `make build-node && make test-node`
     - `make test-python`
     - `make release-go && make test-go-bindings`
     - `make test-binding-contracts && make test-binding-layers`

4. Safety-first rollout
   - Add regression tests before/alongside refactors for fragile areas.
   - Compare traces/snapshots for workflow runtime behavior before and after.
   - Keep commits focused so rollback is trivial.

## Execution sequence

### Phase 0: Baseline and freeze behavior (P0)

1. Capture baseline test status and representative workflow outputs.
2. Add characterization tests for hotspots:
   - `crates/simple-agents-workflow/src/runtime.rs`
   - `crates/simple-agents-workflow/src/validation.rs`
   - `crates/simple-agents-workflow/src/yaml_runner.rs`
3. Add parity checks for bindings if signatures/wrappers are touched.

Exit criteria:
- Baseline tests pass and snapshots/traces recorded for comparison.

### Phase 1: Low-risk cleanup first (P1)

1. Docs/link cleanup (no runtime behavior impact)
   - Fix stale links in `README.md`.
   - Fix incorrect paths in `workflow-engine-research/INTEGRATION_GUIDE.md`.
   - Resolve or remove empty placeholder markdown (`workflow-engine-research/preview.md`).

2. Stale artifact hygiene
   - Remove untracked trace dumps and accidental nested output path.
   - Add/adjust ignore rules for trace output locations.

Exit criteria:
- Documentation links resolve.
- Workspace is clean from stale runtime artifacts.

### Phase 2: Duplicate and dead code reduction (P1)

1. Consolidate duplicate examples
   - Extract shared helper logic from:
     - `crates/simple-agents-providers/examples/custom_api.rs`
     - `examples/full_api_example.rs`
   - Keep one canonical flow; keep compatibility wrappers where needed.

2. Dead code resolution in providers utils
   - Evaluate usage of `DEFAULT_TIMEOUT`, `DEFAULT_MAX_RETRIES`, `parse_retry_after`.
   - Either wire into production paths or remove and update tests.

Exit criteria:
- No unnecessary `#[allow(dead_code)]` for production symbols.
- Example behavior/output remains equivalent.

### Phase 3: Structural refactors in workflow runtime (P2, highest risk)

1. Split giant node execution method
   - Refactor `execute_node(...)` in `runtime.rs` into per-node handlers.
   - Keep existing dispatcher signature unchanged initially.

2. Split giant validator
   - Refactor `validate(...)` in `validation.rs` into node-specific validators + shared rule helpers.

3. Remove data clumps in YAML runner
   - Introduce a typed context builder for repeated `{input, nodes, globals}` construction.

4. Reduce middle-man wrappers
   - Introduce a run-options struct and route wrappers through one canonical implementation.
   - Preserve existing public wrapper functions.

Exit criteria:
- Behavior parity tests pass for runtime/validation/yaml flows.
- Public API unchanged (or backward-compatible shims in place).

### Phase 4: API ergonomics hardening (P2)

1. Python `complete(...)` parameter simplification
   - Add typed options object in binding layer.
   - Keep current signature supported as compatibility path.

2. Replace stringly event discriminators where safe
   - Introduce enums internally for event/node kind while preserving serialized output compatibility.

Exit criteria:
- Existing Python/Node/Go usage continues to work unchanged.
- Binding contract tests pass.

## PR breakdown recommendation

1. PR-1: docs + stale artifacts cleanup only.
2. PR-2: duplicate examples + provider dead code cleanup.
3. PR-3: `validation.rs` modularization (no API changes).
4. PR-4: `runtime.rs` execute-node extraction (internal only).
5. PR-5: `yaml_runner.rs` context/data-clump extraction + wrapper consolidation.
6. PR-6: Python options object + compatibility wrappers.

## Test strategy per phase

- Add regression tests for each touched hotspot before heavy refactor.
- Run full required gates after each PR.
- For workflow runtime changes, compare:
  - terminal node id
  - node outputs
  - retry events
  - trace/replay results
- For bindings, run contract/parity suites after any signature or mapping touch.

## Rollback strategy

- Keep each refactor in isolated commits.
- If regression appears, revert only the offending PR/commit batch.
- Avoid broad multi-file behavior changes in single PRs.

## Definition of done

1. Smell hotspots reduced with measurable complexity drop.
2. All existing tests and binding parity checks pass.
3. Public behavior preserved (or compatibility wrappers provided).
4. Stale docs/artifacts removed and link integrity restored.
