# Question and Answer

Q1. What is the primary user-facing API to define a DAG?
A1. Use a code DSL plus JSON and YAML. The system should make it easy to work across languages (primary JS/TS, Go, Rust, Python), with strong cross-language parity and guidance/examples for each.

Q2. What node types are required?
A2. Start with a simple decision-tree core (switch/if). The roadmap must include map, reduce, loop, subgraph, parallel fan-out, and merge. Also request research into additional node types worth supporting beyond this list.

Q3. How should conditional logic be expressed?
A3. Use a mix of boolean expressions and custom scripting with pluggable evaluators. CEL is preferred as a portable core, but not sufficient alone. Conditions should support more than binary (e.g., range/multi-branch) and include a way to validate/test expressions.

Q4. What should the state model be?
A4. Combination of global state and scoped state. Global state is needed to track results across steps; scoped state is needed for permission boundaries so specific nodes cannot access everything. The system should support configurable scoping.

Q5. What data model should flow between nodes?
A5. Schema-validated objects should flow between nodes. Streaming is supported for client visibility, but node-to-node transitions should occur only after schema validation.

Q6. Should streaming outputs be first-class edges?
A6. Yes. All nodes should be able to stream outputs. Streaming should be a first-class concept alongside schema-validated handoff.

Q7. What are requirements for retries, backoff, circuit breakers at node vs graph level?
A7. Defaults should be at graph level for ease of use, with node-level overrides as an advanced feature.

Q8. What is the concurrency model?
A8. Support all: parallel branches, shared max in-flight, worker pools, and rate limits. These are core requirements.

Q9. Should graph execution be deterministic/replayable?
A9. Yes. It should be deterministic and replayable where possible, but since LLMs are involved strict determinism is not always achievable. The system should support replay from any node, with checkpoints and resumability from the point of failure.

Q10. How should failures propagate?
A10. Fail fast, never best-effort. Use explicit fallback mechanisms through compensation steps. If it fails, it fails (enterprise-grade correctness).

Q11. How should long-running steps be handled?
A11. Timeouts, checkpoints, and resumability are required. On failure, resume from the exact failed node, not from the top.

Q12. What observability is required?
A12. Spans per node, time spent per node, metrics, lineage, and debug snapshots. Also track inputs and outputs for evaluation and debugging. Request additional metrics ideas.

Q13. How are external functions packaged and executed?
A13. Combination of container images and native binaries. WASM is of interest but needs evaluation (pros/cons).

Q14. Serverless cold-start/runtime constraints?
A14. Max memory and max init time are important. Concurrency is critical because of potentially massive request bursts; the system should handle high throughput without spawning a binary per request. Specific numeric limits are not yet defined. Constraints to start with:
- Cold start: minimize init time via precompiled bundles and warm pools.
- Memory: keep resident footprint predictable per worker so autoscaling is safe.
- Concurrency: high per-worker concurrency with backpressure; avoid per-request process spawn; use a small pool of long-lived workers.
- Throughput: design for burst handling with queue and rate limiting; prioritize steady latency under load.
- Binary size: keep deployment artifacts lean to reduce pull/init time.

Q15. Which languages must be supported and dependency management?
A15. Python, TypeScript/JavaScript, Go, Rust (more later). For JS/TS, prefer Bun and pnpm. For Python, prefer uv (Astral). Go is to be explored; request guidance and proposal.

Q16. How are secrets/config injected at runtime?
A16. Use an inbuilt secret manager plus integrations to third-party systems (AWS Secrets Manager, Google Key Vault, etc.). Start with first-party secret manager.

Q17. Versioning/compatibility for graphs and node schemas?
A17. Graph-level versioning only. A new graph version is created on release/publish. Subgraphs are supported; architecture should account for graph-to-graph references.

Q18. Security boundaries/sandboxing?
A18. Likely monolithic deployment with internal sandboxing and graph-based access policies. Want additional perspectives and risks from a security lens.

Q19. Multi-tenant isolation and resource quotas?
A19. Isolation should be configurable. Default is running within a monolith with resource quotas that are generous and configurable.

Q20. Authoring/testing workflow?
A20. Local runner, unit tests for nodes, and golden traces for evaluation (map inputs to expected outputs).

Additional architecture notes (from user)
- The system should avoid spawning a binary per request; instead, scale to a small number of binaries/workers that can handle high request volume (e.g., 1M requests) efficiently.
- Real-time, agentic execution focus (Temporal-like reliability, but more real-time and not primarily queue-based).

Follow-up answers

F1. Additional node types research shortlist?
F1. Yes, provide a proposed shortlist.

F2. Expression validation approach?
F2. Both a DSL test harness (fixtures + expected branch) and runtime validation.

F3. State scoping model?
F3. Use token/capability-based scoping, plus hierarchical scoping so each node can allow/deny models/resources. Provide an example showing impact in this project.

F4. Schema validation during streaming?
F4. Validate only the final payload against the schema before edge transition (no partial-schema validation).

F5. Replayability with LLMs?
F5. No seeded prompts required. Use cached responses when available and configurable reuse. Full trace of decisions/responses makes replay from any node sufficient.

F6. Subgraph versioning?
F6. Stick to graph versioning only. If Graph A calls Graph B, resolve to the latest compatible Graph B version, with DSL checks and tests before update; roll back to last working version if needed.

F7. Secret manager scope?
F7. Keep a simple key/value store (string/JSON). It stores secrets only; configuration is separate. Per-graph secrets are needed; per-node overrides are not part of the secret manager.

F8. External function execution model?
F8. Prefer long-lived workers per language/runtime, but want analysis of tradeoffs vs a single polyglot runtime process.

Decisions and additions

- Keep all proposed node types (join/aggregate, filter/guard, batch/window, debounce/throttle, retry/compensate, human-in-the-loop, cache read/write, event trigger, map/reduce/loop, subgraph, router/selector, transform).
- Node outputs must be addressable for downstream nodes (referencing prior node outputs in later steps).
- Use long-lived per-language workers.
- Nodes can be implemented in Rust/Python/Go/TS and should work uniformly; support conversion into a config file.
- Create a separate folder to store language-specific logic.
