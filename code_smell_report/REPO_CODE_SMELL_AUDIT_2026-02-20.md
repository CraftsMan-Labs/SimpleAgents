# Repository Code Smell and Stale Artifact Audit

Date: 2026-02-20

## Scope and method

- Scanned all tracked repository files (`git ls-files`): 383 files total.
- Reviewed all code files in main languages (`.rs`, `.py`, `.go`, `.ts`, `.js`): 173 files.
- Reviewed markdown docs (`.md`): 128 files.
- Included key untracked workspace artifacts from `git ls-files --others --exclude-standard`.
- Used smell heuristics + targeted source inspection for line-level evidence.

## High-confidence findings

### Bloaters

1. Long method + switch statement hotspot
   - `crates/simple-agents-workflow/src/runtime.rs:1071`
   - `execute_node(...)` spans through `crates/simple-agents-workflow/src/runtime.rs:2220` and contains a giant `match` over many `NodeKind` variants.
   - Smell tags: `Long Method`, `Switch Statements`, `Divergent Change` risk.

2. Long method with repeated validation patterns
   - `crates/simple-agents-workflow/src/validation.rs:92`
   - `validate(...)` is very large and contains many repeated empty-field checks.
   - Smell tags: `Long Method`, `Duplicate Code`, `Shotgun Surgery` risk.

3. Large class/module style hotspots (file-level concentration)
   - `crates/simple-agents-workflow/src/runtime.rs` (4280 LOC)
   - `crates/simple-agents-workflow/src/yaml_runner.rs` (3469 LOC)
   - `crates/simple-agents-py/src/lib.rs` (3128 LOC)
   - `crates/simple-agents-workflow/src/validation.rs` (1171 LOC)
   - Smell tags: `Large Class` (module analog in Rust), `Change Preventers` risk.

### Primitive obsession / long parameter lists / data clumps

4. Long parameter list in Python binding API surface
   - `crates/simple-agents-py/src/lib.rs:2718`
   - `Client.complete(...)` accepts many primitive options and flags; explicitly suppressed by `#[allow(clippy::too_many_arguments)]` at `crates/simple-agents-py/src/lib.rs:2717`.
   - Smell tags: `Long Parameter List`, `Primitive Obsession`.

5. Repeated data clump (same context object literal in multiple places)
   - `crates/simple-agents-workflow/src/yaml_runner.rs:1641`
   - `crates/simple-agents-workflow/src/yaml_runner.rs:1744`
   - `crates/simple-agents-workflow/src/yaml_runner.rs:1767`
   - `crates/simple-agents-workflow/src/yaml_runner.rs:2585`
   - `crates/simple-agents-workflow/src/yaml_runner.rs:2612`
   - Repeated `{"input": workflow_input, "nodes": outputs, "globals": Value::Object(globals.clone())}` shape.
   - Smell tags: `Data Clumps`, `Duplicate Code`.

6. Wrapper chain with repeated argument forwarding
   - `crates/simple-agents-workflow/src/yaml_runner.rs:1036`
   - `crates/simple-agents-workflow/src/yaml_runner.rs:1052`
   - `crates/simple-agents-workflow/src/yaml_runner.rs:1070`
   - `crates/simple-agents-workflow/src/yaml_runner.rs:1119`
   - Many near-identical `run_*workflow*` functions forward same arguments/options.
   - Smell tags: `Long Parameter List`, `Middle Man`, `Speculative Generality` risk.

### Duplicate code

7. Near-duplicate large examples
   - `crates/simple-agents-providers/examples/custom_api.rs`
   - `examples/full_api_example.rs`
   - Approx line similarity: 0.642; shared functions include `example_fuzzy_matching`, `example_streaming_graph`, `example_streaming_healing`, `example_streaming_structured`, `example_type_coercion`, `main`.
   - Smell tags: `Duplicate Code`.

### Dead code / speculative generality

8. Unused production items explicitly suppressed as dead code
   - `crates/simple-agents-providers/src/utils.rs:8`
   - `crates/simple-agents-providers/src/utils.rs:12`
   - `crates/simple-agents-providers/src/utils.rs:31`
   - `DEFAULT_TIMEOUT`, `DEFAULT_MAX_RETRIES`, and `parse_retry_after(...)` are marked `#[allow(dead_code)]`; references appear only in local tests.
   - Smell tags: `Dead Code`, `Speculative Generality`.

## Medium-confidence findings

1. Coupling concentration in runtime execution path
   - `crates/simple-agents-workflow/src/runtime.rs`
   - Execution logic repeatedly maps scope errors and directly manipulates scoped IO concerns inside the same large function family.
   - Smell tags: `Inappropriate Intimacy`, `Feature Envy` risk.

2. If/else dispatch ladder in YAML runner path
   - `crates/simple-agents-workflow/src/yaml_runner.rs:1740`
   - `crates/simple-agents-workflow/src/yaml_runner.rs:1823`
   - Branch-heavy dispatch by node shape could be split into per-node handlers.
   - Smell tags: `Switch Statements`, `Divergent Change` risk.

3. Stringly-typed event descriptors
   - `crates/simple-agents-workflow/src/yaml_runner.rs:1612`
   - `crates/simple-agents-workflow/src/yaml_runner.rs:1615`
   - Event kind fields are string values (`event_type`, `node_kind`) where enums could encode valid states.
   - Smell tags: `Primitive Obsession`.

4. Repeated test scaffolding in router crate
   - `crates/simple-agents-router/src/cost.rs:176`
   - `crates/simple-agents-router/src/fallback.rs:186`
   - `crates/simple-agents-router/src/latency.rs:225`
   - Similar mock/provider setup patterns indicate extractable shared test helpers.
   - Smell tags: `Duplicate Code`.

## Stale markdown/doc findings

### Confirmed stale/broken references

1. README points to missing docs/files
   - `README.md:535` -> `research/` (missing)
   - `README.md:538` -> `OPTIMISATION.md` (missing)
   - `README.md:544` -> `research/litellm-analysis.md` (missing)
   - `README.md:545` -> `research/baml-analysis.md` (missing)
   - `README.md:546` -> `research/implementation-plan.md` (missing)
   - `README.md:547` -> `research/mvp-scope-update.md` (missing)
   - `README.md:674` -> `LICENSE-MIT` (missing)
   - `README.md:675` -> `LICENSE-APACHE` (missing)
   - `README.md:681` -> `THIRD_PARTY_LICENSES.md` (missing)

2. Incorrect self-relative path in research integration guide
   - `workflow-engine-research/INTEGRATION_GUIDE.md:1023` links to `workflow-engine-research/README.md` from inside the same folder, which resolves to a duplicated path.

### Potentially stale guidance content

3. Placeholder implementation marker in documentation sample
   - `workflow-engine-research/INTEGRATION_GUIDE.md:584` includes `todo!("Implement other node types")` in a guide snippet.

4. Reference to non-existent target doc
   - `workflow-engine-research/INTEGRATION_GUIDE.md:1028` references `docs/WORKFLOW_GUIDE.md` (not present).

5. Empty tracked markdown file
   - `workflow-engine-research/preview.md` is zero bytes.

## Stale non-code artifacts in workspace

1. Session trace dumps in example tree (untracked)
   - `examples/workflow_email/traces/chat-session-*.jsonl`
   - `examples/examples/workflow_email/traces/chat-session-*.jsonl`
   - Indicates runtime traces accumulating in repo tree.

2. Duplicate nested examples path (untracked)
   - `examples/examples/workflow_email/traces/...`
   - Likely accidental duplicated output path.

3. Scratch file (untracked)
   - `something.txt`

## Prioritized cleanup recommendations

1. Split `execute_node(...)` in `runtime.rs` into per-node executors (strategy/trait-based dispatch).
2. Break `validate(...)` into node-specific validators and shared helper rules.
3. Introduce typed options objects for YAML runner wrappers and Python `complete(...)` API.
4. Extract repeated `context` construction into one helper function/value object.
5. Consolidate duplicated large examples into one canonical example + shared utilities.
6. Remove or integrate dead-code-suppressed utilities in providers.
7. Repair stale README/doc links and either populate or remove empty markdown placeholders.
8. Add ignore/cleanup policy for generated trace JSONL files.

## Confidence notes

- High-confidence items are directly validated from source and link resolution.
- Medium-confidence items are architectural smells with clear indicators but require maintainer intent checks.
- Some markdown links under `docs/` use VitePress-style absolute paths and were treated as valid when corresponding `docs/*.md` targets exist.
