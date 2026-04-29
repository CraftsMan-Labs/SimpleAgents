# napi-test-simpleAgents

Minimal Bun + TypeScript examples for `simple-agents-node`, mirroring `examples/python-test-simpleAgents/`.

Aligned with:

- `examples/python-test-simpleAgents/runners/test-py-simple-agents.py` — blocking workflow run
- `examples/python-test-simpleAgents/runners/test-py-simple-agents-streaming.py` — streamed events + final result
- `examples/python-test-simpleAgents/runners/test-py-simple-agents-streaming-langfuse.py` — same, with Langfuse OTLP (optional)
- Workflows ship under `workflows/`; Python loads `handlers.py` next to the YAML; Node uses `handlers.ts` with explicit `customWorker`.

## Layout

```text
workflows/
  email-classification/   test.yaml + handlers.ts
  rag/                    rag-eval-workflow.yaml (mocked retrieval for rag eval)

evals/
  friendly/               friendly-eval.dataset.jsonl
  rag/                    rag-eval.dataset.jsonl
  invoice/                generated/*.jsonl (multimodal)

runners/                  CLI scripts (test-simple-agents*.ts, rag-eval*.js)

assets/README.md           Optional local assets (invoice JPEG usually shared from python-test-simpleAgents)

example_paths.ts           join helpers for workflows, eval suites, sibling Python asset paths.
invoice_eval_multimodal.ts generates multimodal JSONL for invoice evals (parity with Python).
```

## Quick start

```bash
cd examples/napi-test-simpleAgents
bun install
export WORKFLOW_API_KEY=...   # required
# optional: export WORKFLOW_API_BASE=https://...
bun run run       # non-streaming CLI
# or
bun run stream    # streaming CLI
```

## Setup

The package depends on the local crate via `file:../../crates/simple-agents-napi` (see `package.json`). If you change the NAPI crate, rebuild it from `crates/simple-agents-napi` (`npm run build` / `napi build`) before re-running.

## Environment

Set at least:

- `WORKFLOW_API_KEY` — required  
- `WORKFLOW_API_BASE` — optional (OpenAI-compatible base URL)

Scripts that call `loadNapiExampleEnv()` (see `example_paths.ts`) load, in order: the monorepo root `.env`, `examples/.env`, then this package’s `.env` (package keys override when duplicated).

## Scripts

Bundled shortcuts (see `package.json`):

| npm script | Direct path |
|---|---|
| `bun run run` | `runners/test-simple-agents.ts` |
| `bun run stream` | `runners/test-simple-agents-streaming.ts` |
| `bun run stream:langfuse` | `runners/test-simple-agents-streaming-langfuse.ts` |
| `bun run invoice-image` | `runners/test-simple-agents-invoice-image.ts` |
| `bun run invoice-image:jaeger` | `runners/test-simple-agents-invoice-image-jaeger.ts` |
| `bun run invoice-image:evals` | `runners/test-simple-agents-invoice-image-evals.ts` |

**Invoice JPEG:** multimodal demos read  
`examples/python-test-simpleAgents/assets/test-invoice.jpeg` (same path helper as Python’s `asset("test-invoice.jpeg")`).

## Custom workers

Pass `customWorkerDispatch` from `../workflows/email-classification/handlers.js` into `Client.runWorkflow` / `Client.streamWorkflow` — see runners.
