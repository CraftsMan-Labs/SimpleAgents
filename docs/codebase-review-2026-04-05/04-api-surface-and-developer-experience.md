# API surface and developer experience

## What confuses new library users

### 1. `runEmailWorkflowYaml` vs `runWorkflowYaml`

**What it is:** A thin adapter that supplies workflow input as:

```json
{ "email_text": "<string>" }
```

**Implementation reference:** `run_email_workflow_yaml_with_client` in `crates/simple-agents-workflow/src/yaml_runner/api.rs` delegates to `run_workflow_yaml_with_client` with that JSON object. File-based variants call `WorkflowRunner::from_file(...).with_email_text(...)`, which builds the same object in `crates/simple-agents-workflow/src/yaml_runner/runner.rs`.

**Why it feels “stupid”:** For a **generic** YAML agent platform, email-specific function names suggest a second engine. There is not—it is **naming and wrapper duplication**.

**Developer-friendly direction:** One documented pattern: `run_workflow_yaml*(path, { "email_text": "..." })` plus a short doc example; keep email helpers as deprecated aliases if backward compatibility is required.

### 2. `YamlWorkflowRunOutput.email_text`

The output struct includes `email_text` as a first-class field even when the workflow is **not** email-related. That is consistent if the runner always normalizes an `email_text` key from input, but the **name** primes wrong mental models.

**Direction:** Consider neutral naming (`primary_text`, `scalar_input`, or document `email_text` as “legacy alias for the primary string slot”) in a future semver-aware change.

### 3. Too many ways to run the same example

Under `examples/workflow_email/`, newcomers see parallel scripts:

- `run_yaml.py`, `run_with_python_package.py`, `run_with_python_streaming.py`, `run_with_unified_system.py`, `run_with_chat_history.py`, `python/run_all_yaml_workflows.py`, Node and Go counterparts, etc.

Each may teach something different, but together they obscure **the one path** you want productized: “install package → point at YAML → pass JSON input.”

**Direction:** One **canonical** script per language, with flags (`--stream`, `--events`, `--trace-dir`) instead of separate files where possible.

## Positive DX elements already present

- **`WorkflowRunner` builder** — Document as the preferred Rust API (`yaml_runner/runner.rs`).
- **Existing docs** — `docs/YAML_WORKFLOW_SYSTEM.md`, `docs/WORKFLOW_QUICKSTART.md`, `docs/WORKFLOW_SECURITY.md` give serious structure; the gap is **curated entry** and **reduced duplicate examples**.

## WASM and Node ergonomics

Generated or hand-maintained JS layers (`bindings/wasm/simple-agents-wasm/index.js`, `index.d.ts`) expose many methods. For developer intuition:

- Group under a single `workflow` namespace in docs.
- Clearly separate **file path** APIs (Node-like) from **inline YAML string** APIs (browser-friendly).

## Type looseness

Some binding surfaces use `any` or broad `Record<string, unknown>` for options (e.g. NAPI declarations). That speeds shipping but hurts **discoverability**. Tighter optional types for `telemetry` / `trace` options would improve IDE help without removing features.
