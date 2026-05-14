# Node.js / TypeScript Binding (simple-agents-node)

Node bindings are provided by `simple-agents-node` (napi-rs). The API is Promise-based with optional streaming callbacks.

## Install

```bash
npm install simple-agents-node
```

## Creating a client

The binding uses an **OpenAI-compatible HTTP** stack (`OpenAiCompatProvider`). You pass an API key and optional base URL, or use `Client.fromEnv()`.

```javascript
const { Client } = require("simple-agents-node");

// Explicit key (typical for apps)
const client = new Client(process.env.CUSTOM_API_KEY, process.env.CUSTOM_API_BASE ?? undefined);

// Or load key/base from the environment (see below)
const clientFromEnv = Client.fromEnv();
```

Repository convention (see `.env.example` at the repo root): set `CUSTOM_API_KEY`, optional `CUSTOM_API_BASE`, and use the same key for workflows and completions. The Node live tests also accept `OPENAI_API_KEY` / `OPENAI_API_BASE` and copy `CUSTOM_API_KEY` into `OPENAI_API_KEY` when needed.

Example runners under `examples/napi-test-simpleAgents/` sometimes use `WORKFLOW_API_KEY` / `WORKFLOW_API_BASE`; those are **example-local names**—map them to `CUSTOM_*` or pass them straight into `new Client(apiKey, baseUrl)`.

## Quick Start

```javascript
const { Client } = require("simple-agents-node");

const client = new Client(process.env.CUSTOM_API_KEY, process.env.CUSTOM_API_BASE ?? undefined);
const response = await client.complete(
  process.env.CUSTOM_API_MODEL ?? "gpt-4o-mini",
  [{ role: "user", content: "Hello from Node." }],
  { maxTokens: 128, temperature: 0.7 },
);

console.log(response.content);
console.log(response.usage);
```

## Streaming

Use `streamComplete` (not `stream`):

```javascript
await client.streamComplete(
  "gpt-4o-mini",
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
  "gpt-4o-mini",
  'Respond with JSON: {"message":"hello"}',
  { mode: "healed_json" },
);
console.log(healed.healed?.value);

const coerced = await client.complete(
  "gpt-4o-mini",
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

- **`Client.fromEnv()`** delegates to `OpenAiCompatProvider::from_env()` in Rust (same family as `OPENAI_API_KEY` / `OPENAI_API_BASE` when those are used by the provider).
- **`new Client(apiKey, baseUrl?)`** ignores provider name strings; always supply a real key as the first argument.

For local development in this monorepo, `npm run test:live` loads `<repo-root>/.env` via `scripts/node-test-with-env.mjs` and expects at least `CUSTOM_API_MODEL` plus `CUSTOM_API_KEY` or `OPENAI_API_KEY` (see `test/live.test.js`).

## API Surface (Types)

```ts
new Client(apiKey: string, baseUrl?: string | null, options?: ClientOptions)
Client.fromEnv(options?: ClientOptions | null): Client
client.complete(model: string, promptOrMessages: string | MessageInput[], options?: CompleteOptions): Promise<CompletionResult>
client.streamComplete(model: string, promptOrMessages: string | MessageInput[], onChunk: (chunk: StreamChunk) => void, options?: CompleteOptions): Promise<CompletionResult>
client.run(request: WorkflowYamlRunRequest): Record<string, unknown> | Promise<Record<string, unknown>>
client.stream(request: WorkflowYamlRunRequest, onEvent: (eventJson: string) => void): Promise<Record<string, unknown>>
client.runWorkflow(workflowPath, workflowInput, workflowOptions?, workflowExecution?, resume?, humanResponse?, customWorkerDispatch?)
client.streamWorkflow(workflowPath, workflowInput, onEvent, workflowOptions?, workflowExecution?, resume?, humanResponse?, customWorkerDispatch?)
client.runEvalSuite(request: EvalSuiteRequest): Promise<EvalReport>
```

`WorkflowYamlRunRequest` (messages-first) includes `workflowPath`, `messages`, optional execution flags (`healing`, `workflowStreaming`, `nodeLlmStreaming`, `splitStreamDeltas`, `debugStreamParse`), `extraWorkflowInput`, `workflowOptions`, optional `resume` / `humanResponse` (or `human_response`) for HITL, and optional `customWorkerDispatch`. When `splitStreamDeltas` is true, the Rust runner emits split thinking/output stream events.

`CompleteOptions` supports `maxTokens`, `temperature`, `topP`, `mode`, and `schema`.

## Workflow YAML Runner (Rust-backed)

Prefer the **messages-first** APIs `client.run` / `client.stream`:

```ts
import { Client } from "simple-agents-node";

const client = new Client(process.env.CUSTOM_API_KEY!, process.env.CUSTOM_API_BASE);
const result = await client.run({
  workflowPath: "workflow.yaml",
  messages: [
    {
      role: "user",
      content: "Classify this email about an invoice from Google.",
    },
  ],
  healing: false,
  workflowStreaming: false,
  nodeLlmStreaming: true,
});

console.log(result.terminal_output);
console.log(result.step_timings);
console.log(result.total_elapsed_ms);
```

Use `extraWorkflowInput` for additional keys merged into runner input (for example legacy `email_text` when the YAML still references it).

**Streaming:** `client.stream(request, onEvent)` emits live workflow events via `onEvent(eventJson)` (JSON strings) and resolves to the final structured output.

### Human in the loop (HITL)

Workflows that include a `human_input` node pause with `status: "awaiting_human_input"` and a `human_request` payload. Resume by calling `client.run` again with the **same** `workflowPath`, the paused object as `resume`, and the user answer as `humanResponse` (camelCase) or `human_response` (snake_case). `messages` are still required by the typed request shape; reuse the same messages as the first call unless your YAML reads other input fields.

```ts
import { Client } from "simple-agents-node";

const client = new Client(process.env.CUSTOM_API_KEY!, process.env.CUSTOM_API_BASE);
const paused = await client.run({
  workflowPath: "workflows/invoice-hitl/approve-reject.yaml",
  messages: [{ role: "user", content: "…" }],
});

if (paused.status === "awaiting_human_input") {
  const resumed = await client.run({
    workflowPath: "workflows/invoice-hitl/approve-reject.yaml",
    messages: [{ role: "user", content: "…" }],
    resume: paused,
    humanResponse: "approve", // choice value | string for text | object for form
  });
}
```

Equivalent lower-level shape: extra tail arguments on `runWorkflow` / `streamWorkflow` after `workflowExecution`: `(resume, humanResponse, customWorkerDispatch)`.

YAML semantics (choice / text / form) match [YAML Workflow System](/YAML_WORKFLOW_SYSTEM#human_input). Python-oriented runnable HITL samples live under `examples/python-test-simpleAgents/runners/`; the resume contract is the same in Node.

### Workflow evals

Eval datasets are output-shaped golden records (JSONL): each row has `id`, `input`, and `expected_output`. `runEvalSuite` runs the workflow per row (`input` is passed through as the workflow input object—include `messages` when the YAML uses `messages_path: input.messages`) and passes each case to your **JavaScript** evaluator (boolean or structured result).

```ts
import { Client, type EvalReport } from "simple-agents-node";

const client = new Client(process.env.CUSTOM_API_KEY!, process.env.CUSTOM_API_BASE);
const report: EvalReport = await client.runEvalSuite({
  workflowPath: "workflows/friendly/friendly.yaml",
  datasetPath: "evals/friendly/friendly-eval.dataset.jsonl",
  evaluator: ({ expectedOutput, actualOutput }) =>
    expectedOutput.terminal_node === actualOutput.terminal_node,
});

console.log(report.status);
console.log(report.cases[0]?.evaluations?.[0]?.reason);
```

If the workflow uses `custom_worker` nodes, pass `customWorkerDispatch` on the same options object as for `client.run`.

This stack delegates to Rust `simple-agents-workflow` as the source of truth.

Workflow events parity with Python:

- Set `workflowOptions.includeEvents = true` on `run(...)` to include final `events` in the output.
- `stream(...)` uses the same event callback JSON shape as `streamWorkflow(...)`.

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

Pass **`customWorkerDispatch`** inside the object passed to `client.run` / `client.stream`, or as the **last tail argument** to `runWorkflow` / `streamWorkflow` (after optional `resume` and `humanResponse`).

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

await client.streamWorkflow(
  workflowPath,
  { messages },
  defaultOnEvent,
  { telemetry: { nerdstats: true } },
  undefined,
  dispatch,
);
```

When you pass **`customWorkerDispatch`**, **`Client.runWorkflow`**, **`Client.streamWorkflow`**, and **`Client.resume`** run on a worker thread and return a Promise.

The handler must return a **JSON-serializable value synchronously** (the same practical contract as Python `handlers.py`). Promises are not awaited by the binding in this version.

YAML may set `handler_file` for documentation; the dispatch callback receives `handlerFile` but file loading is your responsibility (for example `import()` your module and branch on `req.handler`).

## Testing Notes

The Node binding tests require the native addon (`index.node`) to be built.

- `npm run test:unit` and `npm run test:contract` run `build:debug` automatically via pretest hooks.
- `npm run test:live` exercises real providers when `<repo-root>/.env` supplies credentials.
- CI/layered scripts also run `build:debug` explicitly after `npm ci` to guarantee deterministic behavior in clean environments.
