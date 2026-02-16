# Node/TS Workflow Example

Build the Node binding, then run:

```bash
npm --prefix crates/simple-agents-napi run build:debug
node examples/workflow_email/run_with_node_package.js \
  examples/workflow_email/email-intake-classification.yaml \
  "Termination request, second warning already issued"
```

This runner uses Rust YAML execution and then applies the Node custom handler function for `custom_worker` nodes.
