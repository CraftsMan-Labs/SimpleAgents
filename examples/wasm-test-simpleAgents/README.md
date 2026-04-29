# wasm-test-simpleAgents

Minimal Bun + TypeScript examples for `simple-agents-wasm`, aligned with:

- `examples/napi-test-simpleAgents/runners/test-simple-agents.ts` - blocking workflow run
- `examples/napi-test-simpleAgents/runners/test-simple-agents-streaming.ts` - streamed events + final result
- `examples/napi-test-simpleAgents/runners/test-simple-agents-invoice-image.ts` - multimodal text + image input

## Quick start

```bash
cd examples/wasm-test-simpleAgents
npm install
export WORKFLOW_PROVIDER=openai
export WORKFLOW_API_KEY=...
# optional: export WORKFLOW_API_BASE=https://...
bun run run
# or
bun run stream
```

## Build prerequisite

`simple-agents-wasm` must have generated wasm artifacts before running these examples:

```bash
cd bindings/wasm/simple-agents-wasm
npm install
npm run build
```

Then run the scripts from `examples/wasm-test-simpleAgents`.

## Scripts

```bash
bun run run            # non-streaming
bun run stream         # streaming
bun run invoice-image  # multimodal invoice image
```

## Environment

- `WORKFLOW_PROVIDER` - `openai` (default) or `openrouter`
- `WORKFLOW_API_KEY` - required
- `WORKFLOW_API_BASE` - optional
- `WORKFLOW_TIMEOUT_SECONDS` - optional positive number
- `WORKFLOW_RETRY_ATTEMPTS` - optional integer >= 1
- `WORKFLOW_RETRY_STRATEGY` - optional `none`, `fixed`, or `exponential`
- `WORKFLOW_YAML_PATH` - optional override for workflow YAML path
- `INVOICE_IMAGE_PATH` - optional image path for invoice example (falls back to a tiny embedded PNG)

By default, scripts use `../napi-test-simpleAgents/workflows/email-classification/test.yaml` to keep workflow behavior in sync.
