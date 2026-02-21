# Node/npm Workflow Example

Build the Node binding, then run the npm-focused example script:

```bash
npm --prefix crates/simple-agents-napi run build:debug
node examples/workflow_email/node/npm_email_workflow_example.js \
  examples/workflow_email/email-intake-classification.yaml \
  "Termination request, second warning already issued"
```

If you also want the custom-worker handler bridge demo, run:

```bash
node examples/workflow_email/run_with_node_package.js
```

Interactive chat-history workflow runner (equivalent to Python `run_with_chat_history.py`):

```bash
node examples/workflow_email/node/run_with_chat_history.js \
  --workflow examples/workflow_email/email-chat-draft-or-clarify.yaml

# Optional flags for Python parity:
# --include-events
# --stream
# --show-thinking
# --show-step-json
```
