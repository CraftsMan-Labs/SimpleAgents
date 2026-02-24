# Python Workflow Example

Run via package API (Rust orchestrator + Python custom handler bridge):

```bash
uv run --directory examples python workflow_email/run_with_python_package.py \
  --workflow examples/workflow_email/email-intake-classification.yaml \
  --email "Termination request, second warning already issued"
```

Interactive chat-history workflow runner (equivalent to Python `run_with_chat_history.py`):

```bash
make run-python-chat-history \
  WORKFLOW_YAML=examples/workflow_email/email-chat-draft-or-clarify.yaml \
  PY_CHAT_FLAGS="--include-events --stream --show-thinking --show-step-json"

# Nerdstats is enabled by default for streamed turns; disable with:
make run-python-chat-history \
  WORKFLOW_YAML=examples/workflow_email/email-chat-draft-or-clarify.yaml \
  PY_CHAT_FLAGS="--stream --no-nerdstats"
```

Custom handler implementation:

- `examples/workflow_email/handlers.py` (`get_rag_data`)

Run all workflow YAML files in one pass (shared input):

```bash
uv run --directory examples python workflow_email/python/run_all_yaml_workflows.py \
  --email "Please process damaged order 9921 and suggest next actions"
```
