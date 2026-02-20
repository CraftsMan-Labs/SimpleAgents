# Python Workflow Example

Run via package API (Rust orchestrator + Python custom handler bridge):

```bash
uv run --directory examples python workflow_email/run_with_python_package.py \
  --workflow examples/workflow_email/email-intake-classification.yaml \
  --email "Termination request, second warning already issued"
```

Custom handler implementation:

- `examples/workflow_email/handlers.py` (`get_rag_data`)
