# Node.js / TypeScript Binding (simple-agents-node)

Node bindings are provided by `simple-agents-node` (napi-rs). The API is Promise-based with optional streaming callbacks.

## Install

```bash
npm install simple-agents-node
```

## Quick Start

```javascript
const { Client } = require("simple-agents-node");

const client = new Client("openai");
const response = await client.complete(
  "gpt-4",
  [{ role: "user", content: "Hello from Node." }],
  { maxTokens: 128, temperature: 0.7 },
);

console.log(response.content);
console.log(response.usage);
```

## Streaming

```javascript
await client.stream(
  "gpt-4",
  "Say hello in two words.",
  (chunk) => {
    if (chunk.content) process.stdout.write(chunk.content);
    if (chunk.finishReason) console.log("\nfinish:", chunk.finishReason);
  },
  { maxTokens: 32 },
);
```

Streaming currently aggregates content on completion; healing/schema modes are not supported for streams.

## Healed JSON and Schema Coercion

```javascript
const healed = await client.complete(
  "gpt-4",
  "Respond with JSON: {\"message\":\"hello\"}",
  { mode: "healed_json" },
);
console.log(healed.healed?.value);

const coerced = await client.complete(
  "gpt-4",
  "Return JSON with name and age",
  {
    mode: "schema",
    schema: {
      type: "object",
      properties: { name: { type: "string" }, age: { type: "number" } },
      required: ["name", "age"],
    },
  },
);
console.log(coerced.coerced?.value);
```

## Environment Variables

The bindings support both explicit credentials and environment-based fallback:

- Explicit: `Client.withProviderConfig({ provider, apiKey, apiBase? })`
- Fallback: `new Client(provider)` (reads environment variables)

Environment variables used by fallback:

- OpenAI: `OPENAI_API_KEY`, optional `OPENAI_API_BASE`
- Anthropic: `ANTHROPIC_API_KEY`
- OpenRouter: `OPENROUTER_API_KEY`, optional `OPENROUTER_API_BASE`

The test/examples convention also supports:
`CUSTOM_API_BASE`, `CUSTOM_API_KEY`, `CUSTOM_API_MODEL`, `PROVIDER`.

## API Surface (Types)

```ts
new Client(provider: string) // env fallback
Client.withProviderConfig(config: { provider: string; apiKey?: string; apiBase?: string })
client.complete(model: string, promptOrMessages: string | MessageInput[], options?: CompleteOptions)
client.stream(model: string, promptOrMessages: string | MessageInput[], onChunk, options?: CompleteOptions)
client.runWorkflowYaml(workflowPath: string, workflowInput)
client.runWorkflowYamlWithEvents(workflowPath: string, workflowInput, workflowOptions?)
client.runWorkflowYamlStream(workflowPath: string, workflowInput, onEvent, workflowOptions?)
client.executeWorkflowYaml(request: WorkflowYamlRunRequest)
client.executeWorkflowYamlStream(request: WorkflowYamlRunRequest, onEvent)
```

`WorkflowYamlRunRequest` includes optional `splitStreamDeltas` (default false when omitted): when true, the Rust runner emits split thinking/output stream events.

`CompleteOptions` supports `maxTokens`, `temperature`, `topP`, `mode`, and `schema`.

## Workflow YAML Runner (Rust-backed)

Prefer the **messages-first** APIs (see [Workflow API Migration](WORKFLOW_API_MIGRATION.md)):

```ts
import { Client } from "simple-agents-node"

const client = new Client("openai")
const result = client.executeWorkflowYaml({
  workflowPath: "examples/workflow_email/email-unified-chat-intake-classification.yaml",
  messages: [
    {
      role: "user",
      content: "Please process supply chain replacement, order 9921 arrived damaged.",
    },
  ],
  healing: false,
  workflowStreaming: false,
  nodeLlmStreaming: true,
})

console.log(result.terminal_output)
console.log(result.step_timings)
console.log(result.total_elapsed_ms)
```

Use `extraWorkflowInput` for additional keys merged into runner input (for example legacy `email_text` when the YAML still references it).

**Streaming:** `executeWorkflowYamlStream(request, onEvent)` emits live workflow events via `onEvent(err, eventJson)` (JSON strings) and resolves to the final structured output.

### Legacy path helpers

`runWorkflowYaml`, `runWorkflowYamlWithEvents`, and `runWorkflowYamlStream` take `(workflowPath, workflowInput)` and remain for compatibility. Prefer `executeWorkflowYaml` / `executeWorkflowYamlStream` for new code.

This stack delegates to Rust `simple-agents-workflow` as the source of truth.

Workflow events parity with Python:

- `runWorkflowYamlWithEvents(...)` returns output with `events` attached.
- Legacy stream: `runWorkflowYamlStream(...)` — same callback shape as `executeWorkflowYamlStream`.

Workflow telemetry options follow Rust runner semantics:

- `workflowOptions.telemetry.sample_rate` must be between `0.0` and `1.0`.
- Sampling is deterministic per trace id.
- Final output metadata includes `metadata.telemetry.sampled`.

Tracing exporter env configuration is shared across runtimes:

- `SIMPLE_AGENTS_TRACING_ENABLED`
- `OTEL_EXPORTER_OTLP_ENDPOINT`
- `OTEL_EXPORTER_OTLP_PROTOCOL` (`grpc` or `http/protobuf`)
- `OTEL_EXPORTER_OTLP_HEADERS`
- `OTEL_SERVICE_NAME`

### Custom workers (`custom_worker`)

If a workflow contains any `custom_worker` node, you must register a **JavaScript dispatch callback** on the same call that runs the workflow. Otherwise execution is rejected at startup with the node id and handler name.

Pass **`customWorker`** inside the unified `opts` object for `Client.run`, `Client.stream`, and `Client.resume`, or **`customWorkerDispatch`** as the last argument to `runWorkflow` / `streamWorkflow`:

```ts
function dispatch(req: {
  handler: string;
  handlerFile?: string;
  payload: unknown;
  context: unknown;
}): unknown {
  if (req.handler === "get_seller_name") {
    const company = String((req.payload as { company_name?: string }).company_name ?? "");
    return { stakeholder_name: lookupStakeholder(company) };
  }
  throw new Error(`unknown custom worker handler: ${req.handler}`);
}

await client.stream(workflowPath, messages, defaultOnEvent, {
  workflowOptions: { /* … */ },
  customWorker: dispatch,
});
```

When you pass **`customWorker`** or **`customWorkerDispatch`**, **`Client.run`**, **`Client.resume`**, and **`Client.runWorkflow`** return a **Promise** (the Rust side runs on a worker thread so the Node event loop can execute your JavaScript dispatch). Without a custom worker, those methods stay synchronous.

The handler must return a **JSON-serializable value synchronously** (the same practical contract as Python `handlers.py`). Promises are not awaited by the binding in this version.

YAML may set `handler_file` for documentation; the dispatch callback receives `handlerFile` but file loading is your responsibility (for example `import()` your module and branch on `req.handler`).

## Testing Notes

The Node binding tests require the native addon (`index.node`) to be built.

- `npm run test:unit` and `npm run test:contract` run `build:debug` automatically via pretest hooks.
- CI/layered scripts also run `build:debug` explicitly after `npm ci` to guarantee deterministic behavior in clean environments.
