# Workflow Engine Research Thoughts

## Executive take

The workflow-engine plan is feasible on top of the current SimpleAgents architecture, but it should be introduced as a new orchestration layer that composes existing core/router/provider systems rather than changing them. The safest path is incremental: prove a deterministic minimal runtime first, then expand node types, parallelism, and language workers.

## Technical feasibility and integration with current runtime

- **Feasible architecture fit:** Keep `simple-agents-core` as the outer API entry point and add a workflow runtime that calls into existing core/provider/router execution paths.
- **Reusable strengths today:** Existing routing, retry, health, fallback, and circuit-breaker logic already solves provider-level resilience and should be reused instead of reimplemented.
- **Main gap:** The repository currently lacks first-class DAG execution semantics (scheduler, scoped state model, replayable traces, workflow IR/compiler).
- **Cross-language caution:** The workflow proposal depends on strong behavior parity across Rust/Python/Go workers, but current parity work is still active; rollout should be Rust-first with fixture gates.

## Current-state mapping

- **IR/compiler:** Not present yet; can be added as independent crates with no breaking API changes.
- **Execution engine/scheduler:** Not present yet; should remain isolated and invoke core through stable interfaces.
- **Node taxonomy:** Mostly new capability; only implicit orchestration exists today through app-level code.
- **Scoped state/capabilities:** New capability.
- **Trace/replay:** Partial observability exists, but no canonical workflow trace model yet.
- **Worker protocol:** New subsystem; can borrow existing health/retry patterns conceptually.

## Gap and risk assessment

| Area | Risk | Impact | Mitigation |
|---|---|---|---|
| Determinism | CEL + parallel scheduling diverge across languages | Replay/debug instability | Canonical IR/serialization + conformance tests first |
| Scope complexity | Large initial node set increases blast radius | Slow delivery, more defects | Start with minimal node subset |
| Performance | Trace/state overhead and worker hops add latency | Throughput/latency regressions | Bounded scheduler + benchmark gates |
| Cross-language parity | Rust/Python/Go semantics drift | Inconsistent workflow outcomes | Golden fixtures in CI before expansion |
| Resilience ownership | Overlap between workflow retries and router retries | Double-retry/pathological behavior | Define strict layer ownership early |

## Recommended phased implementation path (zero-breaking-change)

### Phase 0 - Thin vertical slice

- Add minimal IR schema + loader + validator.
- Implement core node subset: `start`, `llm`, `tool`, `condition`, `end`.
- Route model/tool calls through current core APIs to maximize reuse.

### Phase 1 - Deterministic runtime contract

- Add hierarchical scoped state and capability checks.
- Define canonical execution trace format.
- Add replay + golden-trace tests before broadening features.

### Phase 2 - Parallelism and workers

- Add bounded parallel scheduler and selected advanced nodes.
- Introduce gRPC worker protocol with health checks, while keeping single-process executor as default.
- Formalize policy split: router owns provider-level resilience; workflow runtime owns node-level policies.

### Phase 3 - DX and language expansion

- Improve authoring SDK/DSL ergonomics and debugging UX.
- Expand to Python/Go/FFI only after trace-fixture parity passes.
- Keep existing non-workflow APIs fully supported.

## DX-focused assessment

### Authoring

- Strong runtime foundations exist, but workflow authoring needs first-class validation/linting and actionable errors.
- Support YAML/JSON and code DSL, but enforce one canonical IR so behavior and errors stay consistent.

### Debugging

- Replayable traces are the highest-value DX feature for multi-node/multi-language workflows.
- Provide node timeline, input/output snapshots, retry reasons, and failure provenance.

### Observability

- Existing metrics hooks are useful but insufficient for workflow diagnosis.
- Add workflow-level spans/events and per-node lifecycle metadata.

### Testing loop

- Golden traces are the right default strategy.
- Add deterministic local mode to reduce flakiness in parallel/worker tests.

### Onboarding and ergonomics

- Add a concise “first workflow in 10 minutes” path and production checklist.
- Prioritize explainable failures and quick feedback over feature breadth in early versions.

## Prioritized recommendations

### P0

- Ship minimal runtime + deterministic trace/replay first.
- Lock canonical IR and expression semantics early.
- Add cross-language compatibility fixture gates.

### P1

- Add workflow-specific observability (node timeline/state/retry causes).
- Clarify resilience ownership boundaries across layers.

### P2

- Expand advanced node taxonomy and distributed worker features after DX baseline is solid.

## Open questions

- Where exactly to split retry/timeout/circuit-breaker ownership between workflow and router.
- Required determinism guarantees across languages (CEL + serialization details).
- Which node types are GA vs experimental in v1.
- Backward-compatibility contract for existing direct `SimpleAgentsClient` users.
- Whether worker protocol is required in v1 or a pluggable backend.

## Evidence base used

- Research docs under `workflow-engine-research/` (architecture, execution model, IR, decisions, integration guide, examples).
- Existing architecture/runtime docs: `docs/ARCHITECTURE.md`, `docs/RUST_CORE_SYSTEMS.md`, `README.md`, `features.md`.
- Core crates and bindings: `crates/simple-agents-core`, `crates/simple-agents-router`, `crates/simple-agents-providers`, `crates/simple-agent-type`, plus bindings and parity signals in `TODO.md`.
