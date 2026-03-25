# Browser/WASM Binding (Preview)

This document defines the current browser-compatible binding surface for
`simple-agents-wasm` and how it maps to `simple-agents-node`.

## Current status

- Package path: `bindings/wasm/simple-agents-wasm`
- Maturity: preview (WS1/WS2 rollout stage)
- Runtime target: modern browsers and fetch-capable JS runtimes
- Runtime engine: Rust `wasm-bindgen` core (`bindings/wasm/simple-agents-wasm/rust`) with JS fallback for non-wasm environments

## Contract goals

- Keep method naming and result shapes aligned with `simple-agents-node`.
- Preserve message and completion type semantics where possible.
- Keep browser-incompatible APIs explicit (fail fast with actionable errors).

## Supported APIs

- `new Client(provider, config)` where `provider` is `openai` or `openrouter`
- `complete(model, promptOrMessages, options?)`
- `stream(model, promptOrMessages, onChunk, options?)`
- `streamEvents(model, promptOrMessages, onEvent, options?)`
- `runWorkflowYamlString(yamlText, workflowInput, workflowOptions?)`

## Deliberate differences vs Node binding

- Browser config is explicit object-based; env-driven provider config is not used.
- `healed_json` and `schema` modes are not yet implemented.
- Path-based workflow methods are not supported in browser runtime.
- Usage metadata in streams may be unavailable (`usageAvailable: false`) depending on provider stream payloads.

## Browser-safe workflow direction

WASM/browser flow support is string/object based:

- Implemented: `runWorkflowYamlString(yamlText, workflowInput, workflowOptions?)`
  - Supports step DSL (`steps`) and graph YAML (`entry_node` + `nodes` + `edges`) for `llm_call`, `switch`, and `custom_worker` node types.
  - `custom_worker` requires a matching `workflowOptions.functions[handler]` implementation in browser/WASM execution.
- Not supported in browser: `runWorkflowYaml(workflowPath, ...)`

## Security and deployment notes

- BYOK credentials are provided at runtime and must not be persisted by default.
- Browser mode still depends on provider CORS behavior.
- Use server fallback only when CORS/provider constraints require it.

## Local development workflow

- Install package deps: `cd bindings/wasm/simple-agents-wasm && npm install`
- Build wasm + JS package: `make build-wasm`
- Run wasm binding tests: `make test-wasm`
- Run repository contract/parity checks: `make test-binding-contracts` and `make test-binding-layers`

## Rust + JavaScript quality gates

- Rust formatting: `cargo fmt --manifest-path bindings/wasm/simple-agents-wasm/rust/Cargo.toml -- --check`
- Rust linting: `cargo clippy --manifest-path bindings/wasm/simple-agents-wasm/rust/Cargo.toml --all-targets`
- JS tests (Node runtime fallback + parity): `cd bindings/wasm/simple-agents-wasm && npm test`

## Type-checking and editor support

- Public TS surface is maintained in `bindings/wasm/simple-agents-wasm/index.d.ts`.
- Keep `index.d.ts` aligned with runtime behavior for both Rust-backed and JS fallback execution.
- Use TypeScript-aware editors (`tsserver`) to validate consumer-facing types and callback signatures when updating APIs.
