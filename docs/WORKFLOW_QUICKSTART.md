# SimpleAgents Workflow Quickstart

Every agentic SaaS is a config. Zero to running workflow in 5 minutes. Copy-paste every block.

## Install

**Python**

```bash
pip install simple-agents-py python-dotenv
```

**Node / Bun (TypeScript)**

```bash
npm install simple-agents-node dotenv
# or
bun add simple-agents-node dotenv
```

## Environment

Create a `.env` file:

```bash
WORKFLOW_PROVIDER=openai
WORKFLOW_API_BASE=https://api.openai.com/v1
WORKFLOW_API_KEY=sk-your-key-here
```

Works with any OpenAI-compatible provider (Azure, Requesty, OpenRouter, etc.) -- just change the base URL.

## Create a YAML Workflow

Save as `workflow.yaml`:

```yaml
id: hello-workflow
version: 1.0.0
entry_node: reply

nodes:
  - id: reply
    node_type:
      llm_call:
        model: gpt-4.1-mini
        messages_path: input.messages
        append_prompt_as_user: true
        stream: true
        heal: true
    config:
      output_schema:
        type: object
        properties:
          answer:
            type: string
        required: [answer]
        additionalProperties: false
      prompt: |
        Answer the user's question concisely.
        Return JSON only: {"answer": "..."}
```

## Run It

### Python -- Normal

```python
import json, os
from pathlib import Path
from dotenv import load_dotenv
from simple_agents_py import Client
from simple_agents_py.workflow_payload import workflow_execution_request_to_mapping
from simple_agents_py.workflow_request import (
    WorkflowExecutionRequest, WorkflowMessage, WorkflowRole,
)

load_dotenv()

client = Client(
    os.environ["WORKFLOW_PROVIDER"],
    api_base=os.environ["WORKFLOW_API_BASE"],
    api_key=os.environ["WORKFLOW_API_KEY"],
)

req = WorkflowExecutionRequest(
    workflow_path=str(Path("workflow.yaml").resolve()),
    messages=[WorkflowMessage(role=WorkflowRole.USER, content="What is 2+2?")],
)

result = client.run_workflow(workflow_execution_request_to_mapping(req))
print(json.dumps(result, indent=2))
```

### Python -- Streaming

```python
import json, os
from pathlib import Path
from dotenv import load_dotenv
from simple_agents_py import Client
from simple_agents_py.workflow_payload import workflow_execution_request_to_mapping
from simple_agents_py.workflow_request import (
    WorkflowExecutionFlags, WorkflowExecutionRequest, WorkflowMessage, WorkflowRole,
)

load_dotenv()

client = Client(
    os.environ["WORKFLOW_PROVIDER"],
    api_base=os.environ["WORKFLOW_API_BASE"],
    api_key=os.environ["WORKFLOW_API_KEY"],
)

req = WorkflowExecutionRequest(
    workflow_path=str(Path("workflow.yaml").resolve()),
    messages=[WorkflowMessage(role=WorkflowRole.USER, content="What is 2+2?")],
    execution=WorkflowExecutionFlags(
        node_llm_streaming=True,
        split_stream_deltas=False,
    ),
)

result = client.stream_workflow(
    workflow_execution_request_to_mapping(req),
    on_event=lambda event: print(event),
)
print(json.dumps(result, indent=2))
```

### Python -- With Image Input

```python
import json, os, base64
from pathlib import Path
from dotenv import load_dotenv
from simple_agents_py import Client
from simple_agents_py.workflow_payload import workflow_execution_request_to_mapping
from simple_agents_py.workflow_request import (
    WorkflowExecutionRequest, WorkflowMessage, WorkflowRole,
)

load_dotenv()

client = Client(
    os.environ["WORKFLOW_PROVIDER"],
    api_base=os.environ["WORKFLOW_API_BASE"],
    api_key=os.environ["WORKFLOW_API_KEY"],
)

b64 = base64.b64encode(Path("invoice.jpeg").read_bytes()).decode("ascii")

req = WorkflowExecutionRequest(
    workflow_path=str(Path("workflow.yaml").resolve()),
    messages=[
        WorkflowMessage(
            role=WorkflowRole.USER,
            content=[
                {"type": "text", "text": "Describe this image."},
                {"type": "image_url", "image_url": {"url": f"data:image/jpeg;base64,{b64}"}},
            ],
        ),
    ],
)

result = client.run_workflow(workflow_execution_request_to_mapping(req))
print(json.dumps(result, indent=2))
```

### TypeScript / Bun -- Normal

```typescript
import { Client } from "simple-agents-node";
import { config as loadEnv } from "dotenv";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));
loadEnv({ path: join(__dirname, ".env") });

const client = new Client(
  process.env.WORKFLOW_API_KEY!,
  process.env.WORKFLOW_API_BASE,
);

const result = await client.runWorkflow(
  join(__dirname, "workflow.yaml"),
  { messages: [{ role: "user", content: "What is 2+2?" }] },
);

console.log(JSON.stringify(result, null, 2));
```

### TypeScript / Bun -- Streaming

```typescript
import { Client } from "simple-agents-node";
import { parseWorkflowEvent } from "simple-agents-node/workflow_event";
import { config as loadEnv } from "dotenv";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));
loadEnv({ path: join(__dirname, ".env") });

const client = new Client(
  process.env.WORKFLOW_API_KEY!,
  process.env.WORKFLOW_API_BASE,
);

function onEvent(eventJson: string): void {
  if (err) { console.error(err); return; }
  const event = parseWorkflowEvent(eventJson) as any;
  if (event.event_type === "node_stream_delta" && event.delta) {
    process.stdout.write(event.delta);
  }
}

const result = await client.streamWorkflow(
  join(__dirname, "workflow.yaml"),
  { messages: [{ role: "user", content: "What is 2+2?" }] },
  onEvent,
  undefined,
  { nodeLlmStreaming: true, splitStreamDeltas: false },
);

console.log("\n" + JSON.stringify(result, null, 2));
```

### TypeScript / Bun -- With Image Input

```typescript
import { readFileSync } from "node:fs";
import { Client } from "simple-agents-node";
import type { MessageInput } from "simple-agents-node";
import { config as loadEnv } from "dotenv";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));
loadEnv({ path: join(__dirname, ".env") });

const client = new Client(
  process.env.WORKFLOW_API_KEY!,
  process.env.WORKFLOW_API_BASE,
);

const b64 = readFileSync(join(__dirname, "invoice.jpeg")).toString("base64");

const messages: MessageInput[] = [
  {
    role: "user",
    content: [
      { type: "text", text: "Describe this image." },
      { type: "image", mediaType: "image/jpeg", data: b64 },
    ],
  },
];

const result = await client.runWorkflow(
  join(__dirname, "workflow.yaml"),
  { messages },
);

console.log(JSON.stringify(result, null, 2));
```

---

## YAML Configuration Reference

### Node-Level LLM Options

Every `llm_call` node accepts these fields:

| Field | Type | Default | What it does |
|---|---|---|---|
| `model` | string | required | LLM model to use (e.g. `gpt-4.1-mini`, `azure/gpt-4.1-mini`) |
| `temperature` | float | provider default | Sampling temperature |
| `max_tokens` | int | provider default | Max response tokens |
| `stream` | bool | `false` | Enable streaming for this node |
| `stream_json_as_text` | bool | `false` | When `true`, streams structured JSON output as raw text deltas instead of parsed JSON snapshots |
| `heal` | bool | `false` | Auto-fix truncated/malformed JSON output |
| `send_schema` | bool | `false` | Send `output_schema` to the model as response format |
| `messages_path` | string | - | JSONPath for input messages (usually `input.messages`) |
| `append_prompt_as_user` | bool | `false` | Append the `config.prompt` as a user message |

### Execution Flags (Runtime)

Pass these when calling `run_workflow` / `stream_workflow` -- they override or combine with per-node YAML settings:

| Flag | Type | Default | What it does |
|---|---|---|---|
| `node_llm_streaming` | bool | `true` | Master switch: when `false`, no node streams regardless of YAML `stream` |
| `split_stream_deltas` | bool | `false` | Emit separate `node_stream_thinking_delta` and `node_stream_output_delta` events |
| `healing` | bool | `false` | Enable healing globally (OR'd with per-node `heal`) |
| `workflow_streaming` | bool | `false` | Forward token deltas to the event sink |

**Python:**

```python
from simple_agents_py.workflow_request import WorkflowExecutionFlags

execution=WorkflowExecutionFlags(
    node_llm_streaming=True,
    split_stream_deltas=False,
    healing=True,
)
```

**TypeScript:**

```typescript
const executionFlags = {
  nodeLlmStreaming: true,
  splitStreamDeltas: false,
  healing: true,
};
```

### How `heal` and `stream` combine with execution flags

```
heal = (yaml node heal) OR (execution.healing)
stream = (yaml node stream) AND (execution.node_llm_streaming)
```

A node only streams if **both** the YAML `stream: true` and the runtime `node_llm_streaming: true`.
Healing kicks in if **either** the node or the global flag is enabled.

### Custom Workers

For running your own code inside the workflow graph:

**YAML:**

```yaml
- id: lookup_data
  node_type:
    custom_worker:
      handler: my_handler_function
      handler_file: handlers.py   # optional, defaults to handlers.py next to the YAML
  config:
    payload:
      company_name: "{{ nodes.previous_node.output.company_name }}"
```

**handlers.py** (Python -- placed next to the YAML):

```python
def my_handler_function(context, payload):
    company = payload.get("company_name", "")
    return {"result": f"Looked up {company}"}
```

**handlers.ts** (TypeScript -- pass as `customWorkerDispatch`):

```typescript
export function customWorkerDispatch(req: {
  handler: string;
  payload: unknown;
  context: unknown;
}): string {
  if (req.handler === "my_handler_function") {
    const payload = req.payload as Record<string, unknown>;
    return JSON.stringify({ result: `Looked up ${payload.company_name}` });
  }
  throw new Error(`unknown handler: ${req.handler}`);
}

// Pass to runWorkflow / streamWorkflow as the last argument
const result = await client.runWorkflow(path, input, undefined, undefined, customWorkerDispatch);
```

---

## Observability Integrations

### Langfuse (Python)

Add to your `.env`:

```bash
LANGFUSE_PUBLIC_KEY=pk-lf-...
LANGFUSE_SECRET_KEY=sk-lf-...
LANGFUSE_BASE_URL=http://localhost:3000
```

In your script, before creating the client:

```python
import base64, os

public = os.environ["LANGFUSE_PUBLIC_KEY"]
secret = os.environ["LANGFUSE_SECRET_KEY"]
base = os.environ["LANGFUSE_BASE_URL"]

token = base64.b64encode(f"{public}:{secret}".encode()).decode("ascii")
endpoint = base.rstrip("/") + "/api/public/otel"

os.environ["SIMPLE_AGENTS_TRACING_ENABLED"] = "true"
os.environ["OTEL_EXPORTER_OTLP_PROTOCOL"] = "http/protobuf"
os.environ["OTEL_EXPORTER_OTLP_ENDPOINT"] = endpoint
os.environ["OTEL_EXPORTER_OTLP_HEADERS"] = (
    f"Authorization=Basic {token},x-langfuse-ingestion-version=4"
)
```

Then add telemetry to the request:

```python
from simple_agents_py.workflow_request import WorkflowRunOptions, WorkflowTelemetryConfig

req = WorkflowExecutionRequest(
    workflow_path=str(workflow_file),
    messages=[...],
    workflow_options=WorkflowRunOptions(
        telemetry=WorkflowTelemetryConfig(enabled=True, nerdstats=True),
    ),
)
```

### Langfuse (TypeScript)

```typescript
import { syncOtelEnvFromProcess } from "simple-agents-node";

const publicKey = process.env.LANGFUSE_PUBLIC_KEY!;
const secretKey = process.env.LANGFUSE_SECRET_KEY!;
const baseUrl = process.env.LANGFUSE_BASE_URL!;

const token = Buffer.from(`${publicKey}:${secretKey}`).toString("base64");
const endpoint = `${baseUrl.replace(/\/$/, "")}/api/public/otel`;

process.env.SIMPLE_AGENTS_TRACING_ENABLED = "true";
process.env.OTEL_EXPORTER_OTLP_PROTOCOL = "http/protobuf";
process.env.OTEL_EXPORTER_OTLP_ENDPOINT = endpoint;
process.env.OTEL_EXPORTER_OTLP_HEADERS =
  `Authorization=Basic ${token},x-langfuse-ingestion-version=4`;

syncOtelEnvFromProcess(
  process.env.SIMPLE_AGENTS_TRACING_ENABLED,
  process.env.OTEL_EXPORTER_OTLP_PROTOCOL,
  process.env.OTEL_EXPORTER_OTLP_ENDPOINT,
  process.env.OTEL_EXPORTER_OTLP_HEADERS,
  process.env.OTEL_SERVICE_NAME || undefined,
);

// Then pass workflowOptions with telemetry:
const workflowOptions = { telemetry: { enabled: true, nerdstats: true } };
const result = await client.streamWorkflow(path, input, onEvent, workflowOptions, executionFlags);
```

### Jaeger (Python)

```bash
# .env or export
SIMPLE_AGENTS_TRACING_ENABLED=true
OTEL_EXPORTER_OTLP_ENDPOINT=http://localhost:4317
OTEL_EXPORTER_OTLP_PROTOCOL=grpc
OTEL_SERVICE_NAME=my-workflow-service
```

```python
os.environ["SIMPLE_AGENTS_TRACING_ENABLED"] = "true"
os.environ["OTEL_EXPORTER_OTLP_ENDPOINT"] = "http://localhost:4317"
os.environ["OTEL_EXPORTER_OTLP_PROTOCOL"] = "grpc"
os.environ["OTEL_SERVICE_NAME"] = "my-workflow-service"

req = WorkflowExecutionRequest(
    workflow_path=str(workflow_file),
    messages=[...],
    workflow_options=WorkflowRunOptions(
        telemetry=WorkflowTelemetryConfig(enabled=True, nerdstats=True),
    ),
)
```

View traces at `http://localhost:16686` (Jaeger UI).

### Jaeger (TypeScript)

```typescript
import { syncOtelEnvFromProcess } from "simple-agents-node";

process.env.SIMPLE_AGENTS_TRACING_ENABLED = "true";
process.env.OTEL_EXPORTER_OTLP_ENDPOINT = "http://localhost:4317";
process.env.OTEL_EXPORTER_OTLP_PROTOCOL = "grpc";
process.env.OTEL_SERVICE_NAME = "my-workflow-service";

syncOtelEnvFromProcess(
  process.env.SIMPLE_AGENTS_TRACING_ENABLED,
  process.env.OTEL_EXPORTER_OTLP_PROTOCOL,
  process.env.OTEL_EXPORTER_OTLP_ENDPOINT,
  process.env.OTEL_EXPORTER_OTLP_HEADERS ?? "",
  process.env.OTEL_SERVICE_NAME,
);

const workflowOptions = { telemetry: { enabled: true, nerdstats: true } };
```

---

## YAML Building Blocks

Three node types. That's it.

| Type | Purpose | Example |
|---|---|---|
| `llm_call` | Call an LLM, get structured output | Classify text, generate reply, extract data |
| `switch` | Route based on previous node output | If finance -> go here, if HR -> go there |
| `custom_worker` | Run your code | Database lookup, API call, business logic |

### Pattern: Classify -> Route -> Act

```yaml
id: classifier-example
version: 1.0.0
entry_node: classify

nodes:
  - id: classify
    node_type:
      llm_call:
        model: gpt-4.1-mini
        messages_path: input.messages
        append_prompt_as_user: true
        heal: true
    config:
      output_schema:
        type: object
        properties:
          category:
            type: string
            enum: [billing, support, sales]
        required: [category]
        additionalProperties: false
      prompt: |
        Classify the user message into one category.
        Return JSON only: {"category": "billing" | "support" | "sales"}

  - id: route
    node_type:
      switch:
        branches:
          - condition: '$.nodes.classify.output.category == "billing"'
            target: handle_billing
          - condition: '$.nodes.classify.output.category == "support"'
            target: handle_support
        default: handle_sales

  - id: handle_billing
    node_type:
      llm_call:
        model: gpt-4.1-mini
        messages_path: input.messages
        append_prompt_as_user: true
    config:
      output_schema:
        type: object
        properties:
          response:
            type: string
        required: [response]
        additionalProperties: false
      prompt: |
        This is a billing inquiry. Provide a helpful billing response.
        Return JSON only: {"response": "..."}

  - id: handle_support
    node_type:
      llm_call:
        model: gpt-4.1-mini
        messages_path: input.messages
        append_prompt_as_user: true
    config:
      output_schema:
        type: object
        properties:
          response:
            type: string
        required: [response]
        additionalProperties: false
      prompt: |
        This is a support request. Provide helpful technical support.
        Return JSON only: {"response": "..."}

  - id: handle_sales
    node_type:
      llm_call:
        model: gpt-4.1-mini
        messages_path: input.messages
        append_prompt_as_user: true
    config:
      output_schema:
        type: object
        properties:
          response:
            type: string
        required: [response]
        additionalProperties: false
      prompt: |
        This is a sales inquiry. Provide a helpful sales response.
        Return JSON only: {"response": "..."}

edges:
  - from: classify
    to: route
```

### Templating

Reference previous node outputs in prompts and payloads:

```yaml
prompt: |
  The user asked about: {{ nodes.classify.output.category }}
  Reason: {{ nodes.classify.output.reason }}
```

```yaml
config:
  payload:
    company: "{{ nodes.extract_company.output.name }}"
```
