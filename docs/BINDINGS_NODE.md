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

The bindings read provider configuration from environment variables:

- OpenAI: `OPENAI_API_KEY`, optional `OPENAI_API_BASE`
- Anthropic: `ANTHROPIC_API_KEY`
- OpenRouter: `OPENROUTER_API_KEY`, optional `OPENROUTER_API_BASE`

The test/examples convention also supports:
`CUSTOM_API_BASE`, `CUSTOM_API_KEY`, `CUSTOM_API_MODEL`, `PROVIDER`.

## API Surface (Types)

```ts
new Client(provider: string)
client.complete(model: string, promptOrMessages: string | MessageInput[], options?: CompleteOptions)
client.stream(model: string, promptOrMessages: string | MessageInput[], onChunk, options?: CompleteOptions)
client.runEmailWorkflowYaml(workflowPath: string, emailText: string)
client.runWorkflowYamlWithEvents(workflowPath: string, workflowInput, workflowOptions?)
client.runWorkflowYamlStream(workflowPath: string, workflowInput, onEvent, workflowOptions?)
```

`CompleteOptions` supports `maxTokens`, `temperature`, `topP`, `mode`, and `schema`.

## Workflow YAML Runner (Rust-backed)

```ts
import { Client } from "simple-agents-node"

const client = new Client("openai")
const result = client.runEmailWorkflowYaml(
  "examples/workflow_email/email-intake-classification.yaml",
  "Please process supply chain replacement, order 9921 arrived damaged.",
)

console.log(result.terminal_output)
console.log(result.step_timings)
console.log(result.total_elapsed_ms)
```

This method delegates to Rust `simple-agents-workflow` as the source of truth.

Workflow events parity with Python is also available:

- `runWorkflowYamlWithEvents(...)` returns output with `events` attached.
- `runWorkflowYamlStream(...)` emits live workflow events via `onEvent(eventJson)` as JSON strings and returns the final structured output object.

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
