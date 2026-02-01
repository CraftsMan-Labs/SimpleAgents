# Workflow Engine Features

## Authoring and Definition
- Code DSL plus JSON and YAML definitions.
- Multi-language parity and guidance for JavaScript/TypeScript, Go, Rust, and Python.
- Canonical node IR/config to unify language-specific implementations.
- Ability to convert language-specific nodes into the canonical config.
- Separate folder for language-specific logic.

## Node Types (Initial + Research)
- Core decision tree: switch/if.
- Map, reduce, loop.
- Subgraph call.
- Parallel fan-out.
- Merge/join/aggregate with merge policies (first/all/quorum).
- Filter/guard (drop/short-circuit based on predicate).
- Batch/window (collect N items or time window).
- Debounce/throttle.
- Retry/compensate (explicit failure-handling node).
- Human-in-the-loop (approval/edit/override).
- Cache read/write and memoization.
- Event trigger (schedule/cron/webhook entry).
- Router/selector (choose provider/tool/model based on policy).
- Transform (schema mapping, enrichment, normalization).

## Conditional Logic and Expressions
- Boolean expressions, range/multi-branch expressions, and custom scripting.
- Pluggable evaluators with CEL as a preferred portable core.
- Expression test harness in the DSL (fixtures + expected branch).
- Runtime validation of expressions.

## State, Data Flow, and References
- Combination of global state and scoped state.
- Hierarchical scoping with capability tokens; nodes can allow/deny models/resources.
- Downstream nodes can reference outputs from earlier nodes.
- Schema-validated data flow between nodes.
- Streaming outputs to clients; node-to-node transitions occur after final schema validation.

## Streaming
- Streaming outputs supported for all nodes.
- First-class streaming edges in the engine.

## Reliability and Failure Handling
- Graph-level defaults for retries, backoff, and circuit breakers.
- Node-level overrides as an advanced feature.
- Fail fast; no best-effort execution.
- Compensation steps for fallback behavior.
- Timeouts, checkpoints, resumability, and resume-from-failure-node.

## Determinism and Replayability
- Deterministic and replayable where possible.
- Full trace of decisions and responses for replay from any node.
- LLM variability acknowledged; use cached responses when available and configurable.

## Concurrency and Throughput
- Parallel branches, shared max in-flight, worker pools, and rate limits.
- High per-worker concurrency with backpressure.
- Avoid per-request process spawn; use long-lived workers.
- Designed for burst handling with steady latency under load.

## Observability
- Spans per node and time spent per node.
- Metrics, lineage, and debug snapshots.
- Track inputs and outputs for evaluation and debugging.

## External Function Execution
- Long-lived per-language workers (Rust/Python/Go/TS) as default.
- Uniform RPC contract so all languages behave the same.
- Container images and native binaries supported.
- WASM to be evaluated.

## Security and Secrets
- Graph-based access policies and internal sandboxing.
- Simple built-in secret manager (key/value, string or JSON).
- Third-party integrations (AWS Secrets Manager, Google Key Vault, etc.).
- Secrets are separate from configuration management.

## Versioning
- Graph-level versioning only.
- Graph-to-graph calls resolve to the latest compatible version.
- DSL checks and tests before upgrading; roll back to last working version if needed.

## Deployment and Runtime Constraints
- Minimal cold start via precompiled bundles and warm pools.
- Predictable memory footprint per worker.
- Lean artifacts to reduce pull/init time.
- Real-time execution focus (Temporal-like reliability, not primarily queue-based).
- Isolation configurable; default is monolithic deployment with resource quotas.

## Authoring and Testing Workflow
- Local runner.
- Unit tests for nodes.
- Golden traces for evaluation (input to expected output mappings).
