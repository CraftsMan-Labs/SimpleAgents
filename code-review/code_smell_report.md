# Code Smell Review Report

Date: 2026-04-02  
Repository: `SimpleAgents`

## Scope and Method

This report summarizes a multi-agent repository review focused on:

- DRY violations and duplication hotspots
- Pragmatism/KISS violations and over-engineering
- Large functions/files and high cyclomatic complexity
- Ambiguous return contracts (generic objects/dicts/`any`)
- Code smells and optimization opportunities
- Ways to reduce repository size while preserving feature richness

Review coverage included Rust core crates, Python bindings/tests/examples, Node/TypeScript bindings/workers, Go bindings, workflows, docs, parity fixtures, and scripts.

---

## Executive Summary

Primary systemic issues:

1. **Monolithic modules** with mixed responsibilities are increasing complexity and regression risk.
2. **Cross-surface duplication** is widespread (providers, bindings, tests, skills/assets).
3. **Loose return contracts** (`serde_json::Value`, `dict[str, Any]`, `any`, `map[string]any`) reduce safety and force defensive downstream code.
4. **Binding layers duplicate core behavior** instead of acting as thin adapters to Rust source-of-truth logic.
5. **Repository bloat from duplicated assets/examples** can be reduced with canonical sources + generation/sync patterns.

---

## Highest-Impact Findings (Prioritized)

### 1) Monolithic workflow runner (Rust)

- Severity: **High**
- Files:
  - `crates/simple-agents-workflow/src/yaml_runner.rs:1`
  - `crates/simple-agents-workflow/src/yaml_runner.rs:5603`
- Smell:
  - 5.6k-line file combining DTOs, tracing, telemetry, validation bridges, runtime orchestration, and output shaping.
  - Violates SRP/KISS and raises change blast radius.
- Recommendation:
  - Split into focused modules (`types`, `execute`, `telemetry`, `ir_bridge`, `globals`, `output`, `tests`) with thin facade re-exports.

### 2) Large orchestration function with high branching (Rust)

- Severity: **High**
- File:
  - `crates/simple-agents-workflow/src/yaml_runner/execute.rs:3`
- Smell:
  - Single path handles validation, runtime selection, event streaming, globals mutation, metrics/timing, and multiple node types.
- Recommendation:
  - Extract per-node executors (`execute_llm_node`, `execute_switch_node`, `execute_custom_worker_node`) and shared cancellation/event helpers.

### 3) Runtime node dispatch complexity (Rust)

- Severity: **High**
- File:
  - `crates/simple-agents-workflow/src/runtime/engine.rs:148`
- Smell:
  - Large `match` over many node kinds with repeated patterns; cyclomatic complexity is high.
- Recommendation:
  - Introduce strategy handlers per node kind (`NodeExecutor` trait), keep dispatcher thin.

### 4) Duplicated provider HTTP/retry/error logic (Rust)

- Severity: **High**
- Files:
  - `crates/simple-agents-providers/src/openai/mod.rs:385`
  - `crates/simple-agents-providers/src/anthropic/mod.rs:290`
  - `crates/simple-agents-providers/src/openrouter/mod.rs:218`
- Smell:
  - Repeated transport/retry/error mapping/metrics logic across providers (DRY violation).
- Recommendation:
  - Add shared provider transport helpers (`execute_json_request`, `execute_stream_request`) with provider-specific adapters/parsers.

### 5) Ambiguous workflow output contracts in core (Rust)

- Severity: **High**
- Files:
  - `crates/simple-agents-workflow/src/yaml_runner.rs:109`
  - `crates/simple-agents-workflow/src/yaml_runner.rs:114`
  - `crates/simple-agents-workflow/src/yaml_runner.rs:132`
- Smell:
  - Generic outputs (`BTreeMap<String, Value>`, `Option<Value>`) obscure shape guarantees and increase runtime schema checks.
- Recommendation:
  - Define typed output models (`WorkflowRunOutput`, node-output enums, typed metadata) and map to language bindings.

### 6) Binding-layer duplication of workflow entrypoint wiring

- Severity: **High**
- Files:
  - `crates/simple-agents-py/src/lib.rs:2865`
  - `crates/simple-agents-ffi/src/lib.rs:1036`
  - `crates/simple-agents-napi/src/lib.rs:1047`
  - `bindings/go/simpleagents.go:520`
- Smell:
  - Similar parsing/options/event wiring duplicated across bindings; drift risk.
- Recommendation:
  - Move invocation normalization/options assembly into shared Rust core API; bindings become thin adapters.

### 7) Validation boilerplate repetition (Rust workflow)

- Severity: **Medium**
- File:
  - `crates/simple-agents-workflow/src/validation.rs:250`
- Smell:
  - Repeated “required field”/diagnostic push patterns and per-node boilerplate.
- Recommendation:
  - Centralized reusable validators + declarative node validation definitions.

### 8) Monolithic Python binding module and large example runtimes

- Severity: **Medium/High**
- Files:
  - `crates/simple-agents-py/src/lib.rs:1`
  - `examples/workflow_email/run_with_chat_history.py:440`
  - `examples/workflow_email/run_yaml.py:293`
- Smell:
  - Large files mixing conversion, runtime bridge, API wrappers, CLI/session logic.
- Recommendation:
  - Split into internal modules; add reusable utility layer for config/path/options/session operations.

### 9) Weak typing in public API surfaces (Py/TS/Go)

- Severity: **High**
- Files:
  - `crates/simple-agents-py/simple_agents_py.pyi:330`
  - `crates/simple-agents-py/simple_agents_py.pyi:138`
  - `crates/simple-agents-napi/index.d.ts:94`
  - `bindings/go/simpleagents.go:170`
- Smell:
  - Broad `Any`/`any`/`map[string]any` contracts for workflow output/events/tool calls/usage.
- Recommendation:
  - Define strict shared models (TypedDict/dataclass/interface/struct) and generate/maintain parity across bindings.

### 10) WASM JS runtime complexity and duplicate streaming/result paths

- Severity: **High**
- Files:
  - `bindings/wasm/simple-agents-wasm/index.js:527`
  - `bindings/wasm/simple-agents-wasm/index.js:786`
  - `bindings/wasm/simple-agents-wasm/index.js:1108`
- Smell:
  - Large branch-heavy workflow runtime + repeated stream aggregation/normalization logic.
- Recommendation:
  - Modularize (`client`, `stream`, `workflow-runner`, `schema`) and centralize stream aggregation helper.

---

## Additional Findings

### DRY Violations and Duplicate Assets

- Exact duplicate tracked files detected in skill trees:
  - `.agents/skills/simpleagentsbuilder/examples/python-intern-fun-interview-system.yaml`
  - `.opencode/skills/SimpleAgentsBuilder/examples/python-intern-fun-interview-system.yaml`
  - `skills/simpleagents-builder/examples/python-intern-fun-interview-system.yaml`
  - `.agents/skills/simpleagentsbuilder/references/patterns.md`
  - `.opencode/skills/SimpleAgentsBuilder/references/patterns.md`
  - `skills/simpleagents-builder/references/patterns.md`
  - `.agents/skills/simpleagentsbuilder/references/checklist.md`
  - `.opencode/skills/SimpleAgentsBuilder/references/checklist.md`
  - `skills/simpleagents-builder/references/checklist.md`
- Near-duplicate workflow families:
  - `examples/workflow_email/email-intake-classification.yaml`
  - `examples/workflow_email/email-unified-chat-intake-classification.yaml`
  - plus mirrored variants under `skills/simpleagents-builder/examples/`.

### Test Duplication and Contract Drift Risk

- Repeated contract-assertion logic across language tests:
  - `crates/simple-agents-py/tests/test_contract_fixtures.py:63`
  - `bindings/go/contract_fixture_test.go:38`
  - `crates/simple-agents-napi/test/contract.test.js:6`
- Python duplicate test content:
  - `crates/simple-agents-py/tests/test_client.py:4`
  - `crates/simple-agents-py/tests/test_healing.py:6`

### Code Smells / Pragmatism Issues

- Hidden-side-effect/silent-failure tendencies in workflow metrics/globals handling:
  - `crates/simple-agents-workflow/src/yaml_runner.rs:2179`
  - `crates/simple-agents-workflow/src/yaml_runner.rs:2440`
  - `crates/simple-agents-workflow/src/yaml_runner/globals.rs:27`
- Import-time side effects and temporary generation in Python worker:
  - `workers/python/worker.py:21`
- Inefficient typed-options-to-map conversion in Go:
  - `bindings/go/simpleagents.go:353`

---

## Repo Size and Concision Analysis

### Largest Tracked Files (Top Impact Targets)

- `crates/simple-agents-workflow/src/yaml_runner.rs` (~183 KB)
- `crates/simple-agents-workflow/src/runtime.rs` (~115 KB)
- `crates/simple-agents-py/src/lib.rs` (~104 KB)
- `bindings/wasm/simple-agents-wasm/rust/src/lib.rs` (~65 KB)
- `crates/simple-agents-workflow/src/yaml_runner/client_executor.rs` (~50 KB)
- `bindings/go/simpleagents.go` (~34 KB)

### Notable Notes

- Tracked repository source footprint is concentrated in a small set of mega-files.
- No accidental tracking of `target/`, `node_modules/`, `.go-cache/`, `.ruff_cache/`, `.uv-cache` was observed.
- Some lockfiles and generated typing artifacts are intentionally tracked (expected).

---

## Prioritized Remediation Roadmap

### Quick Wins (Low/Medium Effort, High Return)

1. Canonicalize skill assets in one location; sync mirrors via script/CI.
2. Remove duplicate Python tests and parameterize shared cases.
3. Extract shared provider transport/retry helpers for OpenAI/Anthropic/OpenRouter.
4. Add shared stream aggregation helper in WASM/JS runtime.

### Medium Initiatives

1. Introduce typed workflow output/event contracts in Rust and propagate to Py/TS/Go surfaces.
2. Consolidate workflow invocation API shape per binding (`run_workflow(..., options)`), keep compatibility wrappers thin.
3. Refactor workflow YAML variants into template/fragments + generated final YAMLs.

### Large Initiatives

1. Continue decomposition of `yaml_runner.rs` and runtime dispatch into modular executors.
2. Move duplicated binding logic into shared Rust core utilities (schema parsing, message validation/building, workflow options normalization).
3. Evaluate rebasing WASM workflow behavior on Rust core to reduce source-of-truth divergence.

---

## Suggested Acceptance Criteria for Cleanup Work

- No exact duplicated skill/reference files in tracked tree.
- Workflow core output contracts are strongly typed in Rust and reflected in all binding typings.
- Provider transport/retry behavior lives in one shared internal implementation.
- Mega-files reduced by modular extraction with no behavior regressions.
- Contract tests assert semantic behavior from shared fixtures, not repeated extraction logic per language.

---

## Appendix: Architecture-Level Smells

1. Binding packages currently behave as mini-platforms rather than adapters.
2. Schema parsing/conversion duplicated in FFI/NAPI/Python.
3. Request/message validation and building repeated across surfaces.
4. Workflow APIs are repeated per language with many variant wrappers.
5. Worker payload contracts rely on generic JSON strings rather than typed protobuf contracts.
6. Parity checks focus heavily on symbol presence vs behavior equivalence.
