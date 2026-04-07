# python-test-simpleAgents

Sample scripts that run the bundled `test.yaml` workflow with `simple-agents-py` (local crate via the `examples` workspace).

## Prerequisites

- [uv](https://docs.astral.sh/uv/) installed
- API credentials for an OpenAI-compatible provider

## Install

From the **workspace root** `examples/` (recommended — picks up `simple-agents-py` from `../crates/simple-agents-py`):

```bash
cd examples
uv sync
```

Or only this member (if you already use the parent workspace):

```bash
cd examples/python-test-simpleAgents
uv sync
```

After the first sync, rebuild the native wheel when you change the Rust/Python bindings:

```bash
cd examples
uv sync --reinstall-package simple-agents-py
```

## Environment

Create a `.env` file in **this directory** (it is gitignored). The scripts use `python-dotenv` to load it.

| Variable | Required | Description |
|----------|----------|-------------|
| `WORKFLOW_PROVIDER` | yes | Provider name passed to `Client` (e.g. `openai`) |
| `WORKFLOW_API_BASE` | yes | Base URL for the API (OpenAI-compatible endpoint) |
| `WORKFLOW_API_KEY` | yes | API key |

Example:

```bash
WORKFLOW_PROVIDER=openai
WORKFLOW_API_BASE=https://api.openai.com/v1
WORKFLOW_API_KEY=sk-...
```

## CLI scripts

Run commands **from this directory** with `uv run` so the workspace environment is used.

**Blocking run** (single JSON result):

```bash
cd examples/python-test-simpleAgents
uv run python test-py-simple-agents.py
```

**Streaming** (events to stdout, then final JSON):

```bash
uv run python test-py-simple-agents-streaming.py
```

**FastAPI server** (HTTP chat + SSE):

```bash
uv run uvicorn fastapi_workflow_stream:app --reload --host 127.0.0.1 --port 8000
```

Then:

- Health: `curl -s http://127.0.0.1:8000/health`
- Chat (JSON): `curl -s -X POST http://127.0.0.1:8000/chat -H 'Content-Type: application/json' -d '{"message":"Hello"}'`
- Chat (SSE): `curl -sN -X POST http://127.0.0.1:8000/chat/stream -H 'Content-Type: application/json' -d '{"message":"Hello"}'`

## Custom workers

`handlers.py` lives next to `test.yaml`. The Rust runner loads it automatically for `custom_worker` nodes (see `test.yaml`).

## Related

- Node/Bun twin: `examples/napi-test-simpleAgents/` (same workflow shape; pass `customWorker` explicitly in TypeScript).
