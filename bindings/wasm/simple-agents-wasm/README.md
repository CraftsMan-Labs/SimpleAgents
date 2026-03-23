# simple-agents-wasm

Browser-compatible SimpleAgents client for OpenAI-compatible providers.

## Status

This package now loads a Rust-compiled WASM core (`rust/src/lib.rs`) for
runtime execution when wasm artifacts are available. A JS fallback remains for
non-wasm environments and local Node tests.

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
  baseUrl: "https://api.openai.com/v1"
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
- Use `hasRustBackend()` to check whether Rust wasm backend was loaded.
