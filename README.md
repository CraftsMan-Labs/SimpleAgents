# SimpleAgents

SimpleAgents is a YAML workflow engine for AI products.
Define nodes, routing, schemas, and workers in one config. Run it in Python or TypeScript. Ship today.

**Every agentic SaaS is a config.**

[![GitHub Stars](https://img.shields.io/github/stars/CraftsMan-Labs/SimpleAgents?style=flat-square)](https://github.com/CraftsMan-Labs/SimpleAgents/stargazers)
[![GitHub Forks](https://img.shields.io/github/forks/CraftsMan-Labs/SimpleAgents?style=flat-square)](https://github.com/CraftsMan-Labs/SimpleAgents/network/members)
[![GitHub Issues](https://img.shields.io/github/issues/CraftsMan-Labs/SimpleAgents?style=flat-square)](https://github.com/CraftsMan-Labs/SimpleAgents/issues)
[![License](https://img.shields.io/badge/license-Apache--2.0%20%2F%20MIT-blue?style=flat-square)](LICENSE)

[![PyPI Version](https://img.shields.io/pypi/v/simple-agents-py?style=flat-square&logo=python)](https://pypi.org/project/simple-agents-py/)
[![PyPI Downloads](https://static.pepy.tech/badge/simple-agents-py/month)](https://pepy.tech/project/simple-agents-py)
[![npm Version](https://img.shields.io/npm/v/simple-agents-node?style=flat-square&logo=npm)](https://www.npmjs.com/package/simple-agents-node)
[![npm Downloads](https://img.shields.io/npm/dm/simple-agents-node?style=flat-square)](https://www.npmjs.com/package/simple-agents-node)

## Links

- Docs: https://docs.simpleagents.craftsmanlabs.net/
- Playground: https://yamslam.craftsmanlabs.net/playground

## Start Here

- **New here?** Start with `docs/WORKFLOW_QUICKSTART.md` for your first working workflow.
- **Using Python?** Go to `docs/BINDINGS_PYTHON.md`.
- **Using TypeScript / Node?** Go to `docs/BINDINGS_NODE.md`.
- **Using Rust directly?** Go to `docs/QUICKSTART.md`.
- **Want runnable examples?** Go to `docs/EXAMPLES.md`.

## Install

```bash
pip install simple-agents-py     # Python
npm install simple-agents-node   # TypeScript / Node
```

## How It Works

1. **Define** your workflow as YAML -- nodes, edges, structured outputs, routing
2. **Run** it with 10 lines of Python or TypeScript
3. **Ship** -- streaming, images, Langfuse/Jaeger observability all work out of the box

Every email classifier, document processor, intake system, interview bot, and support agent is the same pattern: **LLM nodes with structured outputs and deterministic routing**. SimpleAgents makes that pattern a config file.

## Who It's For

- Teams building classifiers, support flows, document workflows, intake systems, or other structured AI flows
- Builders who want routing, schemas, streaming, and custom workers without writing framework glue

## Use It When

- You want structured LLM workflows with deterministic routing
- You want the same workflow config to run in Python or TypeScript
- You want custom worker steps and tracing without building the plumbing yourself

If you only need one prompt call and no workflow, use the lower-level bindings directly.

## Quick Example

**workflow.yaml**

```yaml
id: classifier
version: 1.0.0
entry_node: classify

nodes:
  - id: classify
    node_type:
      llm_call:
        model: gpt-4.1-mini
        messages_path: input.messages
        append_prompt_as_user: true
        heal: true
    config:
      output_schema:
        type: object
        properties:
          category:
            type: string
            enum: [billing, support, sales]
        required: [category]
        additionalProperties: false
      prompt: |
        Classify the user message. Return JSON only.

edges: []
```

**run.py**

```python
import json, os
from pathlib import Path
from dotenv import load_dotenv
from simple_agents_py import Client
from simple_agents_py.workflow_request import (
    WorkflowExecutionRequest, WorkflowMessage, WorkflowRole,
)

load_dotenv()
client = Client(os.environ["WORKFLOW_PROVIDER"], api_base=os.environ["WORKFLOW_API_BASE"], api_key=os.environ["WORKFLOW_API_KEY"])

req = WorkflowExecutionRequest(
    workflow_path=str(Path("workflow.yaml").resolve()),
    messages=[WorkflowMessage(role=WorkflowRole.USER, content="I need a refund for order #1234")],
)
result = client.run_workflow(req)
print(json.dumps(result, indent=2))
```

SimpleAgents Builder Skills
```
npx skills add CraftsMan-Labs/SimpleAgents --skill simpleagents-builder
```

That's it. Your agentic SaaS is a config.

## What You Get

- **YAML workflow engine** -- classify, route, extract, generate as a graph config
- **Python + TypeScript** -- `pip install` / `npm install`, run with 10 lines
- **Streaming** -- real-time LLM output streaming
- **Images** -- multimodal input (text + images) in the same workflow
- **Human-in-the-loop (HITL)** -- pause on `human_input` nodes (`choice`, `text`, `form`) and resume safely
- **Workflow evals** -- output-shaped JSONL datasets with code-side evaluators (Python + TypeScript)
- **JSON healing** -- auto-fix truncated/malformed LLM JSON output
- **Observability** -- Langfuse and Jaeger via OpenTelemetry, one env block
- **Custom workers** -- plug your own code (DB lookups, APIs) into the workflow graph
- **Rust core** -- blazing fast engine with Python, TypeScript, and WASM bindings

## Documentation

- **Start here**: `docs/WORKFLOW_QUICKSTART.md` -- install, create YAML, run in Python/TypeScript
- Examples: `docs/EXAMPLES.md`
- YAML system guide: `docs/YAML_WORKFLOW_SYSTEM.md`
- Python binding: `docs/BINDINGS_PYTHON.md`
- Node/TypeScript binding: `docs/BINDINGS_NODE.md`
- WASM binding: `docs/BINDINGS_WASM.md`
- Observability (Langfuse/Jaeger): `docs/TRACING_ARCHITECTURE.md`
- Rust quick start: `docs/QUICKSTART.md`
- Rust usage: `docs/USAGE.md`
- Troubleshooting: `docs/TROUBLESHOOTING.md`
- Development/Contributing: `docs/DEVELOPMENT.md`
- Docs map: `docs/DOCS_MAP.md`

## Contributing

- Start with `CONTRIBUTING.md` and `docs/DEVELOPMENT.md`.
- Follow task-tracking expectations in `TODO.md` (and `SUBAGENT_TODO.md` for larger parallel workstreams).
- Run relevant test/lint/format/parity commands before opening a PR.

## License

- Repository license file: `LICENSE` (Apache License 2.0 text).
- Package metadata in workspace includes `MIT OR Apache-2.0` for crates/bindings where declared.

For redistribution/compliance-sensitive usage, verify root license files and per-package metadata.

## Star History

[![Star History Chart](https://api.star-history.com/svg?repos=CraftsMan-Labs/SimpleAgents&type=Date)](https://star-history.com/#CraftsMan-Labs/SimpleAgents&Date)
