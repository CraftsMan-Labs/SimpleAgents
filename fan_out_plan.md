# Fan-Out / Guardrail Future Plan

## Goal

Add first-class YAML support for fan-out DAG patterns (parallel + merge) while preserving backward compatibility, deterministic behavior, and cross-language parity.

## Current State

- YAML currently supports `llm_call`, `switch`, and `custom_worker` nodes.
- Canonical IR/runtime already supports advanced DAG nodes (`parallel`, `merge`, `map`, `reduce`, `subgraph`, etc.) with bounded concurrency.
- Runtime security controls already include fan-out limits (`max_parallel_branches`) and scope/payload guards.

## Target Outcome

Enable this pattern directly in YAML:

1. Run guardrail policy checks deterministically.
2. Fan out to parallel LLM/tool retrieval branches.
3. Merge branch outputs with explicit merge policy.
4. Run final synthesis over consolidated context.

## Primary Risks

1. **Execution path drift**  
   Adding new YAML node types may route more workflows through IR runtime and change subtle behavior compared to fallback YAML execution.

2. **State shape drift**  
   Differences in output/global-memory handling across YAML execution paths can break downstream switch conditions.

3. **Concurrency nondeterminism**  
   Branch completion order can vary; implicit ordering assumptions in consumers may produce flaky outcomes.

4. **Unsupported branch behavior**  
   Parallel branch execution currently supports only `llm`/`tool` branch node kinds in runtime internals.

5. **Throughput and cost spikes**  
   Fan-out can increase token, latency, and provider rate-limit pressure if limits/policies are not enforced consistently.

6. **Telemetry contract changes**  
   Existing event consumers may assume mostly linear node progression; interleaved branch events may break dashboards/parsers.

7. **Binding parity risk**  
   Rust/Node/Python/Go may diverge unless YAML node-surface and result contracts are rolled out together.

## Potential Breaking Points

- YAML verification not updated for new node schemas (`parallel`, `merge`) causing late runtime failures.
- Invalid merge wiring (`sources`, `policy`, `quorum`) that passes authoring but fails at runtime.
- Changes in terminal output shape from merged branch outputs impacting existing downstream prompt templates.
- Cross-language wrappers relying on old YAML taxonomy or old error messages.
- Replay/debug tooling assuming linear traces, not branch-level fan-out/merge paths.

## Safe Rollout Plan

### Phase 1: Contract + Validation (No behavior change)

- Extend YAML schema/types to include `parallel` and `merge` nodes.
- Extend YAML verifier with strict checks:
  - branch/source IDs must exist
  - `parallel.max_in_flight >= 1`
  - `merge.policy` valid
  - `merge.quorum` only valid for `quorum` policy and within source bounds
- Keep behind feature flag.

### Phase 2: IR Mapping + Runtime Wiring

- Map YAML `parallel`/`merge` to canonical IR `NodeKind::Parallel` / `NodeKind::Merge`.
- Preserve deterministic output ordering (stable branch/source order).
- Enforce runtime limits (`max_parallel_branches`, scheduler bounds, timeouts/retries).

### Phase 3: Guardrail-First Pattern

- Add recommended pattern documentation:
  - classify/intent
  - deterministic guardrail worker
  - allow/deny switch
  - parallel retrieval branches
  - merge
  - final synthesis
- Keep policy-sensitive checks in deterministic workers, not probabilistic LLM-only paths.

### Phase 4: Observability + Debugging Stability

- Add explicit branch lifecycle events (branch start/complete/fail).
- Keep existing top-level event fields stable for backward compatibility.
- Validate replay and timeline tooling against fan-out workflows.

### Phase 5: Cross-Language Parity + Examples

- Add/refresh examples demonstrating guardrail + parallel retrieval + merge.
- Ensure Node/Python/Go wrappers expose identical behavior and errors.
- Add parity fixtures and contract tests for YAML fan-out workflows.

## Test Strategy

1. **Validation tests**: malformed branch/source/quorum cases hard-fail early.
2. **Runtime tests**: successful fan-out + merge with deterministic merged output.
3. **Security tests**: fan-out limits and expression scope limits enforced.
4. **Retry/timeout tests**: branch-level failure and recovery behavior.
5. **Replay tests**: branch traces remain structurally valid and deterministic.
6. **Binding tests**: same workflow fixture yields equivalent outputs across languages.

## Backward Compatibility Rules

- Existing YAML workflows must run unchanged.
- New node types are additive only.
- Existing event/output fields remain stable; new branch metadata is additive.
- Error codes/messages for old paths should not regress.

## Rollback Strategy

- Feature flag controls rollout.
- If regression is detected, disable fan-out YAML nodes while retaining IR/runtime internals.
- Keep fallback execution path operational for non-fan-out workflows.

## Recommended First Deliverable

Implement a minimal, safe v1:

- YAML `parallel` + `merge` support
- strict validation
- deterministic merge semantics
- bounded concurrency/security enforcement
- one canonical example + one end-to-end parity fixture
