# simple-agents-wasm

Browser-compatible SimpleAgents client for OpenAI-compatible providers.

## Status

This package is Rust-WASM only. Runtime execution is provided by the
Rust-compiled core in `rust/src/lib.rs` and requires generated wasm artifacts.
If artifacts are missing or initialization fails, the package throws an
explicit backend-load error.

## Install

```bash
npm i simple-agents-wasm
```

## Build wasm artifacts

Prerequisites:

- Rust target: `wasm32-unknown-unknown`
- `wasm-bindgen` CLI (matching `wasm-bindgen` crate version)

```bash
npm run build
```

This compiles Rust to `wasm32-unknown-unknown` and generates browser bindings
under `pkg/` using `wasm-bindgen`.

## Usage

```js
import { Client } from "simple-agents-wasm";

const client = new Client("openai", {
  apiKey: "<BYOK>",
  baseUrl: "https://api.openai.com/v1",
  timeoutSeconds: 60,
  retryAttempts: 3,
  retryStrategy: "exponential"
});

const result = await client.complete("gpt-4o-mini", "Say hi in one line.");
console.log(result.content);
```

## Important notes

- Browser mode still depends on provider CORS support.
- `healed_json` and `schema` completion modes are not supported yet.
- `runWorkflowYaml(workflowPath, ...)` is not supported in browser runtime.
- `runWorkflowYamlString(...)` supports string-based workflow execution for:
  - step workflows (`steps` DSL)
  - graph workflows (`entry_node` + `nodes` + `edges`) with `llm_call`, `switch`, and `custom_worker`.
- Graph `custom_worker` nodes call `workflowOptions.functions[handler]` and throw a runtime error when the handler is missing.
- Use `hasRustBackend()` to check whether the Rust wasm backend is available.
