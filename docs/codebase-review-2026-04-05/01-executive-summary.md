# Executive summary

## Vision alignment

The stated vision—**declare an agent as YAML**—is supported by a substantial Rust workflow stack (`simple-agents-workflow`), multiple language bindings, and extensive examples. The main friction is not missing features but **surface area**: many near-duplicate entry points, a very large YAML runner module, and example/documentation sprawl that obscures the “happy path” for library consumers.

## Top findings (prioritized)

1. **God modules** — `yaml_runner.rs` (~5.6k lines) and `runtime.rs` (~3.5k lines) concentrate parsing, execution, file I/O, and extensive tests in few files. This raises defect risk and review cost.
2. **Combinatorial `run_*` APIs** — Email-specific helpers duplicate generic workflow runners; a builder (`WorkflowRunner`) exists but legacy wrappers remain across Rust, Python, Node (NAPI), Go, FFI, and WASM.
3. **Misleading domain naming in core types** — `YamlWorkflowRunOutput` includes `email_text` even for non-email workflows, which works but confuses API readers.
4. **Triplicate agent “skills” content** — `skills/simpleagents-builder/`, `.agents/skills/simpleagentsbuilder/`, and `.opencode/skills/SimpleAgentsBuilder/` overlap; drift risk.
5. **Example explosion** — Under `examples/workflow_email/`, many scripts overlap (`run_yaml.py`, `run_with_python_package.py`, `run_with_unified_system.py`, etc.), increasing maintenance and cognitive load.
6. **Demo code that looks like a security footgun** — Example handlers use substring checks such as `"ignore"` / `"bypass"` in user text to branch behavior; fine for a toy interview demo, alarming in a security review without context.

## Existing strengths (already in tree)

- Documented workflow hardening: `docs/WORKFLOW_SECURITY.md` maps limits to `expressions.rs`, `runtime.rs`, `worker.rs`, and YAML file loading in `yaml_runner.rs`.
- `ProviderConfig` API key redaction in serialization (noted as resolved in `code-review/08-baseline-truth-matrix.md`).

## Suggested next steps (when you choose to implement)

- Prefer **one primary API story per language**: `WorkflowRunner` / `run_workflow_yaml_*` with explicit JSON input; deprecate email wrappers or reimplement them as one-liner examples.
- **Split** `yaml_runner.rs` into cohesive submodules (types, file load, IR bridge, tests) without behavior change.
- **Deduplicate** skill trees or generate one from the other.
- Run **cargo-audit**, **semgrep** (or equivalent), and **secret scanning** in CI; this audit did not execute those tools in this environment.
