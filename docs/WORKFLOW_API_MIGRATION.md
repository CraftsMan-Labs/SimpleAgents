# Workflow API Migration

The Node binding supports two workflow API styles:

- Preferred lower-level API: `client.runWorkflow(...)` and `client.streamWorkflow(...)`
- Messages-first compatibility API: `client.executeWorkflowYaml(...)` and `client.executeWorkflowYamlStream(...)`

The compatibility API delegates to `client.run(...)` / `client.stream(...)`, which parse a request object with `workflowPath`, `messages`, execution flags, workflow options, and optional custom worker dispatch.

Use `runWorkflow` / `streamWorkflow` when you already have normalized workflow input. Use `executeWorkflowYaml` / `executeWorkflowYamlStream` when porting examples that build a messages-first request object.
