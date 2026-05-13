# Workflow API Migration

The Node binding supports two workflow API styles:

- Preferred lower-level API: `client.runWorkflow(...)` and `client.streamWorkflow(...)`
- Messages-first typed API: `client.run(...)` and `client.stream(...)`

The messages-first API parses a request object with `workflowPath`, `messages`, execution flags, workflow options, and optional custom worker dispatch.

Use `runWorkflow` / `streamWorkflow` when you already have normalized workflow input. Use `run` / `stream` when you want a messages-first request object.
