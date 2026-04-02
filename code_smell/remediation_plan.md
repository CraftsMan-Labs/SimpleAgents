# Code Smell Remediation Plan

Date: 2026-04-02  
Source report: `code-review/code_smell_report.md`

## Goal

Eliminate the documented DRY, KISS/pragmatism, complexity, ambiguous return-contract, and repo-size violations without regressing behavior or breaking language bindings.

## Principles

- Keep Rust as source of truth for behavior and contracts.
- Prefer thin bindings/adapters over duplicated per-language implementations.
- Replace ambiguous object returns with explicit typed models.
- Refactor in small reversible slices with parity tests at each step.
- Preserve backward compatibility using wrappers/deprecation paths.

---

## Program Structure

## Phase 0: Baseline and Safety Rails (1-2 days)

### Deliverables

- Baseline metrics captured:
  - exact duplicate file count
  - top 20 largest tracked files
  - number of `Any`/`any`/`map[string]any`/`serde_json::Value` hotspots in public APIs
  - high-complexity functions identified in Rust/Python/TS/Go
- Cleanup acceptance checklist copied into working tracker.

### Technical plan

1. Record static baselines from existing report and targeted scans.
2. Define regression gates per change batch:
   - Rust: `make test-rust`, `make clippy`, `make fmt`
   - Python: `make test-python`
   - Node: `make build-node`, `make test-node`
   - Go: `make release-go`, `make test-go-bindings`
   - parity: `make test-binding-contracts`, `make test-binding-layers`
3. Lock migration policy:
   - no public API removals without wrappers
   - all behavior changes require fixtures/tests.

---

## Phase 1: Quick Wins (Low/Medium effort, high impact)

### 1.1 Canonicalize duplicate skill assets

#### Violations addressed

- Duplicate tracked files under `.agents/`, `.opencode/`, `skills/`.

#### Technical plan

1. Select canonical source: `skills/simpleagents-builder/`.
2. Add sync script (deterministic copy or symlink strategy by environment).
3. Replace tracked duplicate files with generated/synced mirrors.
4. Add CI check to fail if mirrors drift.

#### Outcome

- Zero exact duplicate tracked skill/reference files.

### 1.2 Remove duplicate Python tests and parametrize

#### Violations addressed

- Duplicate tests in `crates/simple-agents-py/tests/test_client.py` and `crates/simple-agents-py/tests/test_healing.py`.

#### Technical plan

1. Create shared fixture/factory for provider/env error cases.
2. Convert duplicate tests to `pytest.mark.parametrize` cases.
3. Keep semantic coverage unchanged.

#### Outcome

- Smaller test surface with same behavior coverage.

### 1.3 Shared provider transport helpers

#### Violations addressed

- HTTP/retry/error/metrics duplication across OpenAI/Anthropic/OpenRouter.

#### Technical plan

1. Introduce internal module in providers crate:
   - `execute_json_request(...)`
   - `execute_stream_request(...)`
   - shared retry-after extraction + timeout/network mapping.
2. Incrementally migrate OpenAI -> Anthropic -> OpenRouter.
3. Keep provider-specific payload building/parsing local.

#### Outcome

- Single reusable transport path, lower drift risk.

### 1.4 Consolidate WASM stream aggregation

#### Violations addressed

- Repeated stream aggregation/result normalization in JS runtime.

#### Technical plan

1. Extract one helper (`aggregateStreamEvents`) in WASM JS runtime module.
2. Replace duplicate code paths in fallback + Rust-backed flows.
3. Add focused stream behavior regression tests.

#### Outcome

- Smaller JS runtime surface and consistent stream behavior.

---

## Phase 2: Typed Contract Foundation (1-2 weeks)

### 2.1 Typed workflow output/event contracts in Rust

#### Violations addressed

- Ambiguous core outputs (`BTreeMap<String, Value>`, `Option<Value>`).

#### Technical plan

1. Define typed core models:
   - `WorkflowRunOutput`
   - `WorkflowNodeOutput` enum by node kind
   - typed `WorkflowMetadata` and event payload structs
2. Keep compatibility adapters that still expose legacy map/JSON shape as needed.
3. Add conversion tests covering old and new forms.

#### Outcome

- Explicit contracts at source-of-truth boundary.

### 2.2 Propagate strict typing to bindings

#### Violations addressed

- `Any`/`any`/`map[string]any` in public binding contracts.

#### Technical plan

1. Python: update `simple_agents_py.pyi` to TypedDict/dataclass-like typed surfaces for workflow output/events/tool metadata.
2. Node: update `crates/simple-agents-napi/index.d.ts` with concrete interfaces and typed promises.
3. Go: add typed wrappers/structs as primary API while preserving map-based compatibility methods.
4. Add cross-language parity fixtures for typed field presence/shape.

#### Outcome

- Stronger static guarantees and less defensive consumer code.

---

## Phase 3: Workflow Core Decomposition (2-3 weeks)

### 3.1 Split `yaml_runner.rs` into focused modules

#### Violations addressed

- Monolithic 5.6k-line mixed-responsibility file.

#### Technical plan

1. Create module structure:
   - `yaml_runner/types.rs`
   - `yaml_runner/execute.rs`
   - `yaml_runner/telemetry.rs`
   - `yaml_runner/globals.rs`
   - `yaml_runner/output.rs`
   - `yaml_runner/ir_bridge.rs`
2. Move code in behavior-preserving chunks, keeping facade exports stable.
3. After each move, run Rust tests + clippy.

#### Outcome

- Reduced blast radius and improved maintainability.

### 3.2 Refactor execution orchestration into per-node executors

#### Violations addressed

- Large branch-heavy executor function.

#### Technical plan

1. Introduce per-node functions:
   - `execute_llm_node`
   - `execute_switch_node`
   - `execute_custom_worker_node`
2. Add shared helpers for cancellation/event emission/timing capture.
3. Keep output semantics stable using snapshot/parity tests.

#### Outcome

- Lower cyclomatic complexity and clearer control flow.

### 3.3 Runtime dispatch strategy pattern

#### Violations addressed

- Giant `match` in runtime engine.

#### Technical plan

1. Create `NodeExecutor` trait with typed context/result.
2. Implement one executor per `NodeKind`.
3. Keep a minimal dispatcher registry.

#### Outcome

- Extensible runtime with smaller per-node complexity.

### 3.4 Declarative validation helpers

#### Violations addressed

- Repeated validation boilerplate.

#### Technical plan

1. Create reusable validators (`require_non_empty`, `require_positive`, etc.).
2. Build per-node validation schemas/descriptors.
3. Unify diagnostic emission format.

#### Outcome

- DRY validation with consistent diagnostics.

---

## Phase 4: Binding Adapter Simplification (2 weeks)

### 4.1 Shared workflow invocation core utilities

#### Violations addressed

- Duplicated options/event wiring across Py/NAPI/FFI/Go.

#### Technical plan

1. Add shared Rust helpers for:
   - options normalization
   - input validation
   - event sink bridging
   - output conversion
2. Migrate each binding to call shared helpers.
3. Keep public API signatures stable using wrappers where necessary.

#### Outcome

- Bindings become thin adapters with less drift risk.

### 4.2 Module split for large binding files

#### Violations addressed

- Monolithic `lib.rs`/`simpleagents.go` files.

#### Technical plan

1. Split internal code by responsibility:
   - conversions
   - runtime bridge
   - workflow bridge
   - errors
2. Keep exports/backward-compat interfaces unchanged.

#### Outcome

- Smaller files, easier review and ownership.

---

## Phase 5: Workflow, Example, and Docs Concision (1-2 weeks)

### 5.1 Workflow YAML template/fragments

#### Violations addressed

- Near-duplicate workflow families and mirrored variants.

#### Technical plan

1. Extract common route/classify/RAG blocks into reusable fragments.
2. Generate final YAML variants from fragment + overrides.
3. Validate generated outputs against existing behavior fixtures.

#### Outcome

- Major reduction in duplicated YAML logic.

### 5.2 Example runtime utility extraction

#### Violations addressed

- Oversized Python/Node example runners with duplicated session/config logic.

#### Technical plan

1. Add shared example utility modules for:
   - env/config loading
   - path resolution
   - session/trace handling
   - event rendering
2. Keep examples readable and purpose-specific.

#### Outcome

- Smaller, clearer examples with lower maintenance cost.

### 5.3 Docs snippet single-sourcing

#### Violations addressed

- Repeated Rust snippets across docs.

#### Technical plan

1. Create shared snippet files for common examples.
2. Include/reference snippets from multiple docs.
3. Add docs check for snippet drift.

#### Outcome

- Better docs consistency and less duplication.

---

## Phase 6: Architecture Hardening (Optional, recommended)

### 6.1 Worker contract typing

#### Violations addressed

- Generic JSON string payload contracts in worker proto.

#### Technical plan

1. Add typed protobuf payloads (`oneof`) with JSON fallback for compatibility.
2. Migrate worker adapters gradually.
3. Add compatibility tests for old/new payload formats.

### 6.2 WASM source-of-truth alignment

#### Violations addressed

- Potential divergence from Rust workflow core behavior.

#### Technical plan

1. Identify behavior duplicated in WASM runtime.
2. Reuse Rust core paths where feasible or explicitly scope reduced behavior set.
3. Add parity tests to enforce alignment.

---

## Workstream Mapping

- WS-A Rust Workflow Core: phases 2.1, 3.1-3.4
- WS-B Provider DRY: phase 1.3
- WS-C Binding Typing: phase 2.2
- WS-D Binding Adapter Refactor: phase 4
- WS-E WASM Runtime: phases 1.4, 6.2
- WS-F Repo Concision: phases 1.1, 1.2, 5.1-5.3
- WS-G Parity and Regression: continuous across all phases

---

## Definition of Done

- No exact duplicate tracked skill/reference files remain.
- Provider transport/retry behavior is centralized.
- Workflow output/events are strongly typed in Rust and reflected in all binding types.
- Mega-files are decomposed with behavior parity preserved.
- Contract tests validate semantics (not only symbol presence).
- Backward compatibility maintained through wrappers/deprecations.

---

## Risk Management

- **Risk:** Breaking existing binding consumers.  
  **Mitigation:** Compatibility wrappers + staged deprecations + contract fixtures.
- **Risk:** Refactor regressions in workflow runtime.  
  **Mitigation:** Snapshot tests, semantic parity fixtures, phase-gated rollouts.
- **Risk:** Template/generation complexity for YAML/docs.  
  **Mitigation:** deterministic generators + generated artifact checks.

---

## Immediate Next Actions

1. Start Phase 1.1/1.2 in parallel (asset dedupe + test dedupe).
2. Start Phase 1.3 provider transport extraction as first Rust core DRY refactor.
3. Prepare typed workflow contract RFC (Rust model + Py/TS/Go mapping) before Phase 2 implementation.
