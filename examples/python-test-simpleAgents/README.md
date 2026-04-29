# python-test-simpleAgents

Sample scripts that run bundled YAML workflows under `simple-agents-py` (local crate via the `examples` workspace).

## Layout

```text
workflows/
  email-classification/   test.yaml + handlers.py (custom worker lookup)
  friendly/               friendly.yaml
  rag/                    rag-eval-workflow.yaml + rag_eval_handlers.py

evals/
  friendly/               friendly-eval.{yaml,dataset.jsonl}
  invoice/               invoice-image-* eval suites (+ handlers.py mirror for run_eval_suite)
  rag/                    rag-eval.{yaml,dataset.jsonl} (+ mirrored rag_eval_handlers.py)

runners/                  CLI entrypoints (test-py-simple-agents*.py)

apps/
  fastapi_workflow_stream.py

assets/
  test-invoice.jpeg       Place a small invoice JPEG here for image examples (gitignored).

example_paths.py          Resolves workflow / eval / asset paths from any script location.
handlers.py               (removed from root — live under workflows/email-classification/)
```

## Prerequisites

- [uv](https://docs.astral.sh/uv/) installed
- API credentials for an OpenAI-compatible provider

## Install

From the **workspace root** `examples/` (recommended — picks up `simple-agents-py` from `../crates/simple-agents-py`):

```bash
cd examples
uv sync
```

Or only this member:

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

Set variables in your process (shell, IDE, or CI) — **usually from the SimpleAgents repository root**. The scripts only read `os.environ` and do **not** load `.env` files.

| Variable           | Required | Description                                              |
|--------------------|----------|----------------------------------------------------------|
| `WORKFLOW_PROVIDER`| yes      | Provider name passed to `Client` (e.g. `openai`)        |
| `WORKFLOW_API_BASE`| yes      | Base URL for the API (OpenAI-compatible endpoint)       |
| `WORKFLOW_API_KEY` | yes      | API key                                                   |

Example from repo root:

```bash
cd /path/to/SimpleAgents
export WORKFLOW_PROVIDER=openai
export WORKFLOW_API_BASE=https://api.openai.com/v1
export WORKFLOW_API_KEY=sk-...
```

If you keep secrets in a root `.env` file, load it with your shell (for example `set -a && source .env && set +a` in bash) before `uv run`.

## CLI scripts

Run commands **from this directory** with `uv run`:

**Blocking run** (single JSON result):

```bash
cd examples/python-test-simpleAgents
uv run python runners/test-py-simple-agents.py
```

**Streaming** (events to stdout, then final JSON):

```bash
uv run python runners/test-py-simple-agents-streaming.py
```

**Text eval bundles** (`evals/` JSONL + evaluator callbacks):

```bash
# Friendly (plain string message) + RAG (mocked provider; offline-friendly)
uv run python runners/test-py-simple-agents-text-evals.py

# Single friendly suite only
uv run python runners/test-py-simple-agents-eval.py
```

**FastAPI server** (HTTP chat + SSE):

```bash
uv run uvicorn apps.fastapi_workflow_stream:app --reload --host 127.0.0.1 --port 8000
```

Then:

- Health: `curl -s http://127.0.0.1:8000/health`
- Chat (JSON): `curl -s -X POST http://127.0.0.1:8000/chat -H 'Content-Type: application/json' -d '{"message":"Hello"}'`
- Chat (SSE): `curl -sN -X POST http://127.0.0.1:8000/chat/stream -H 'Content-Type: application/json' -d '{"message":"Hello"}'`

## Custom workers

`handlers.py` lives next to `workflows/email-classification/test.yaml`. The runner loads it for `custom_worker` nodes.

**Eval suites:** eval runners now pass `workflow_path`, `dataset_path`, and an evaluator function in code. Workflow custom workers resolve relative to the workflow YAML, and eval-specific assertions live in the runner callback.

## Related

- Node/Bun twin: `examples/napi-test-simpleAgents/` (same workflow shape; pass `customWorker` explicitly in TypeScript).
