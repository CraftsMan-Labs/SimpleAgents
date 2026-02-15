# Workflow Email Examples

This folder contains an LLM-driven email intake classification demo in both Python and YAML forms.

## Files

- `python_email_workflow_demo.py`: direct Python implementation (LLM + mock RAG routing)
- `email-intake-classification.yaml`: YAML workflow definition
- `run_yaml.py`: lightweight YAML runner for this example

## Prerequisites

Create `examples/.env` (or export env vars) with:

- `WORKFLOW_PROVIDER` (optional, default: `openai`)
- `WORKFLOW_API_BASE`
- `WORKFLOW_API_KEY`
- `WORKFLOW_MODEL`

Backward-compatible fallback env names are also supported:

- `CUSTOM_API_BASE`
- `CUSTOM_API_KEY`
- `CUSTOM_API_MODEL`

## Run Python version

```bash
uv run --directory examples python workflow_email/python_email_workflow_demo.py
```

## Run YAML version

```bash
uv run --directory examples python workflow_email/run_yaml.py email-intake-classification.yaml
```

Pass a custom email inline:

```bash
uv run --directory examples python workflow_email/run_yaml.py email-intake-classification.yaml --email "Termination request, second warning already issued"
```
