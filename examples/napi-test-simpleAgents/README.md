# napi-test-simpleAgents

Minimal Bun + TypeScript examples for `simple-agents-node`, aligned with:

- `examples/python-test-simpleAgents/test-py-simple-agents.py` — blocking workflow run
- `examples/python-test-simpleAgents/test-py-simple-agents-streaming.py` — streamed events + final result
- `examples/python-test-simpleAgents/handlers.py` — Python loads this next to the YAML; Node uses `handlers.ts` with explicit `customWorker` (below)

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

From this directory:

```bash
bun install
```

The package depends on the local crate via `file:../../crates/simple-agents-napi` (see `package.json`). If you change the NAPI crate, rebuild it from `crates/simple-agents-napi` (`npm run build` / `napi build`) before re-running.

## Environment

Set at least:

- `WORKFLOW_API_KEY` — required  
- `WORKFLOW_API_BASE` — optional (OpenAI-compatible base URL)

Bun loads `.env` from this folder when present (same idea as Python `python-dotenv` in the sibling example).

## Scripts

**Non-streaming** (like `test-py-simple-agents.py`):

```bash
bun run run
# or
bun run test-simple-agents.ts
```

**Streaming** (like `test-py-simple-agents-streaming.py`):

```bash
bun run stream
# or
bun run test-simple-agents-streaming.ts
```

Each script prompts for input and runs `test.yaml` in this directory.

## Custom workers

Python discovers `handlers.py` next to the workflow automatically. In Node you must pass `customWorker: customWorkerDispatch` from `./handlers.ts` on `Client.run` / `Client.stream` (see the `test-*.ts` files).
