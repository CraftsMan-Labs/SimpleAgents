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
