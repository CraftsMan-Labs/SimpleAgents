---
name: simpleagents-builder
description: This skill should be used when the user asks to create, improve, or validate YAML agent workflows, especially requests like "build an agent YAML", "design workflow YAML", "add routing in YAML", or "make interview/email workflow nodes and edges". Also use when the user describes a problem and wants a SimpleAgents solution built for them. The core vision is that every agentic SaaS is a config -- turn their problem into a YAML workflow.
---

# SimpleAgentsBuilder

**Every agentic SaaS is a config.** Turn any AI product idea into a YAML workflow + runner code. When a user describes their problem, gather requirements through targeted follow-up questions, then generate the YAML workflow, handler file, and runner script that makes their agentic SaaS real.

## When to Use This Skill

- User asks to create, design, or improve a YAML agent workflow
- User describes a problem that can be solved with an LLM workflow (classification, extraction, routing, generation, etc.)
- User wants to add routing, guardrails, custom workers, or structured output to a workflow
- User wants a complete runnable solution (YAML + handler + runner code)

## Step 1: Gather Requirements (Ask These Questions)

When a user describes their problem, ask these follow-up questions before generating anything:

1. **Language**: "Which language will you use to run this? (Python / TypeScript / both)"
2. **Streaming**: "Do you need streaming output? (yes / no)"
3. **Observability**: "Do you need Langfuse or Jaeger tracing? (langfuse / jaeger / none)"
4. **Custom logic**: "Does any step need to call your own code (database lookup, API call, business rules)? If so, describe what it does."
5. **Model**: "Which model do you want? (e.g. `gpt-4.1-mini`, `azure/gpt-4.1-mini`, `claude-sonnet-4-20250514`, or any OpenAI-compatible model)"

Then generate:
- The YAML workflow file
- A `handlers.py` / `handlers.ts` if custom workers are needed (assign `handler_file` path in YAML)
- The runner script in the chosen language

## Step 2: Generate the YAML Workflow

### Core Rules

1. Model the workflow as a **graph**, not a linear prompt.
2. Define `config.output_schema` for **every** `llm_call` node.
3. Keep `switch` routing **deterministic** -- simple `==` / `!=` conditions.
4. Each node prompt is **single-responsibility**.
5. Instruct every LLM node: `Return JSON only`.
6. Set `additionalProperties: false` on routing-critical schemas.

### Required YAML Skeleton

```yaml
id: workflow-id
version: 1.0.0
entry_node: first_node

nodes:
  - id: first_node
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
          field:
            type: string
        required: [field]
        additionalProperties: false
      prompt: |
        Your instruction here.
        Return JSON only.

edges:
  - from: first_node
    to: second_node
```

### Node Types

| Type | When to use | Example |
|---|---|---|
| `llm_call` | Classify, extract, generate, summarize | Detect intent, draft response, extract entities |
| `switch` | Route based on a previous node's output | If category == "billing" go to billing handler |
| `custom_worker` | Run deterministic code (DB, API, business logic) | Look up customer, check inventory, call webhook |

### LLM Node Options

| Field | Type | Default | Purpose |
|---|---|---|---|
| `model` | string | **required** | LLM model identifier |
| `temperature` | float | provider default | Sampling temperature |
| `max_tokens` | int | provider default | Max response tokens |
| `stream` | bool | `false` | Enable streaming for this node |
| `stream_json_as_text` | bool | `false` | Stream structured JSON as raw text deltas |
| `heal` | bool | `false` | Auto-fix truncated/malformed JSON |
| `send_schema` | bool | `false` | Send output_schema to the model as response format |
| `messages_path` | string | - | Path to input messages (usually `input.messages`) |
| `append_prompt_as_user` | bool | `false` | Append config.prompt as a user message |

### Switch Node Pattern

```yaml
- id: route_category
  node_type:
    switch:
      branches:
        - condition: '$.nodes.classify.output.category == "billing"'
          target: handle_billing
        - condition: '$.nodes.classify.output.category == "support"'
          target: handle_support
      default: handle_general
```

### Custom Worker Pattern

YAML:
```yaml
- id: lookup_customer
  node_type:
    custom_worker:
      handler: lookup_customer
      handler_file: handlers.py
  config:
    payload:
      customer_id: "{{ nodes.extract_info.output.customer_id }}"
```

Python handler (`handlers.py`):
```python
def lookup_customer(context, payload):
    customer_id = payload.get("customer_id", "")
    return {"name": "John Doe", "plan": "enterprise"}
```

TypeScript handler (pass as `customWorkerDispatch`):
```typescript
export function customWorkerDispatch(req: { handler: string; payload: unknown; context: unknown }): string {
  if (req.handler === "lookup_customer") {
    const p = req.payload as Record<string, unknown>;
    return JSON.stringify({ name: "John Doe", plan: "enterprise" });
  }
  throw new Error(`unknown handler: ${req.handler}`);
}
```

### Templating -- Reference Previous Outputs

In prompts:
```yaml
prompt: |
  The category is: {{ nodes.classify.output.category }}
  Reason: {{ nodes.classify.output.reason }}
```

In custom worker payloads:
```yaml
config:
  payload:
    name: "{{ nodes.extract_name.output.name }}"
```

## Step 3: Generate the Runner Script

### Python -- Normal Run

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
    messages=[WorkflowMessage(role=WorkflowRole.USER, content="your input here")],
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
    messages=[WorkflowMessage(role=WorkflowRole.USER, content="your input here")],
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

### Python -- With Image

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

b64 = base64.b64encode(Path("image.jpeg").read_bytes()).decode("ascii")

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

### Python -- With Langfuse

```python
import json, os, base64
from pathlib import Path
from dotenv import load_dotenv
from simple_agents_py import Client
from simple_agents_py.workflow_payload import workflow_execution_request_to_mapping
from simple_agents_py.workflow_request import (
    WorkflowExecutionFlags, WorkflowExecutionRequest, WorkflowMessage, WorkflowRole,
    WorkflowRunOptions, WorkflowTelemetryConfig,
)

load_dotenv()

# Langfuse OTLP setup
public = os.environ["LANGFUSE_PUBLIC_KEY"]
secret = os.environ["LANGFUSE_SECRET_KEY"]
base = os.environ["LANGFUSE_BASE_URL"]
token = base64.b64encode(f"{public}:{secret}".encode()).decode("ascii")
os.environ["SIMPLE_AGENTS_TRACING_ENABLED"] = "true"
os.environ["OTEL_EXPORTER_OTLP_PROTOCOL"] = "http/protobuf"
os.environ["OTEL_EXPORTER_OTLP_ENDPOINT"] = base.rstrip("/") + "/api/public/otel"
os.environ["OTEL_EXPORTER_OTLP_HEADERS"] = f"Authorization=Basic {token},x-langfuse-ingestion-version=4"

client = Client(
    os.environ["WORKFLOW_PROVIDER"],
    api_base=os.environ["WORKFLOW_API_BASE"],
    api_key=os.environ["WORKFLOW_API_KEY"],
)

req = WorkflowExecutionRequest(
    workflow_path=str(Path("workflow.yaml").resolve()),
    messages=[WorkflowMessage(role=WorkflowRole.USER, content="your input here")],
    execution=WorkflowExecutionFlags(node_llm_streaming=True, split_stream_deltas=False),
    workflow_options=WorkflowRunOptions(
        telemetry=WorkflowTelemetryConfig(enabled=True, nerdstats=True),
    ),
)

result = client.stream_workflow(
    workflow_execution_request_to_mapping(req),
    on_event=lambda event: print(event),
)
print(json.dumps(result, indent=2))
```

### Python -- With Jaeger

```python
import json, os
from pathlib import Path
from dotenv import load_dotenv
from simple_agents_py import Client
from simple_agents_py.workflow_payload import workflow_execution_request_to_mapping
from simple_agents_py.workflow_request import (
    WorkflowExecutionRequest, WorkflowMessage, WorkflowRole,
    WorkflowRunOptions, WorkflowTelemetryConfig,
)

load_dotenv()

os.environ["SIMPLE_AGENTS_TRACING_ENABLED"] = "true"
os.environ["OTEL_EXPORTER_OTLP_ENDPOINT"] = "http://localhost:4317"
os.environ["OTEL_EXPORTER_OTLP_PROTOCOL"] = "grpc"
os.environ["OTEL_SERVICE_NAME"] = "my-workflow"

client = Client(
    os.environ["WORKFLOW_PROVIDER"],
    api_base=os.environ["WORKFLOW_API_BASE"],
    api_key=os.environ["WORKFLOW_API_KEY"],
)

req = WorkflowExecutionRequest(
    workflow_path=str(Path("workflow.yaml").resolve()),
    messages=[WorkflowMessage(role=WorkflowRole.USER, content="your input here")],
    workflow_options=WorkflowRunOptions(
        telemetry=WorkflowTelemetryConfig(enabled=True, nerdstats=True),
    ),
)

result = client.run_workflow(workflow_execution_request_to_mapping(req))
print(json.dumps(result, indent=2))
```

### TypeScript -- Normal Run

```typescript
import { Client } from "simple-agents-node";
import { config as loadEnv } from "dotenv";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";
// import { customWorkerDispatch } from "./handlers.js";  // uncomment if using custom workers

const __dirname = dirname(fileURLToPath(import.meta.url));
loadEnv({ path: join(__dirname, ".env") });

const client = new Client(process.env.WORKFLOW_API_KEY!, process.env.WORKFLOW_API_BASE);

const result = await client.runWorkflow(
  join(__dirname, "workflow.yaml"),
  { messages: [{ role: "user", content: "your input here" }] },
);
console.log(JSON.stringify(result, null, 2));
```

### TypeScript -- Streaming

```typescript
import { Client } from "simple-agents-node";
import { parseWorkflowEvent } from "simple-agents-node/workflow_event";
import { config as loadEnv } from "dotenv";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));
loadEnv({ path: join(__dirname, ".env") });

const client = new Client(process.env.WORKFLOW_API_KEY!, process.env.WORKFLOW_API_BASE);

function onEvent(err: unknown, eventJson: string): void {
  if (err) { console.error(err); return; }
  const event = parseWorkflowEvent(eventJson) as any;
  if (event.event_type === "node_stream_delta" && event.delta) {
    process.stdout.write(event.delta);
  }
}

const result = await client.streamWorkflow(
  join(__dirname, "workflow.yaml"),
  { messages: [{ role: "user", content: "your input here" }] },
  onEvent,
  undefined,
  { nodeLlmStreaming: true, splitStreamDeltas: false },
);
console.log("\n" + JSON.stringify(result, null, 2));
```

### TypeScript -- With Image

```typescript
import { readFileSync } from "node:fs";
import { Client } from "simple-agents-node";
import type { MessageInput } from "simple-agents-node";
import { config as loadEnv } from "dotenv";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));
loadEnv({ path: join(__dirname, ".env") });

const client = new Client(process.env.WORKFLOW_API_KEY!, process.env.WORKFLOW_API_BASE);
const b64 = readFileSync(join(__dirname, "image.jpeg")).toString("base64");

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

### TypeScript -- With Langfuse

```typescript
import { Client, syncOtelEnvFromProcess } from "simple-agents-node";
import { parseWorkflowEvent } from "simple-agents-node/workflow_event";
import { config as loadEnv } from "dotenv";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));
loadEnv({ path: join(__dirname, ".env") });

const token = Buffer.from(
  `${process.env.LANGFUSE_PUBLIC_KEY}:${process.env.LANGFUSE_SECRET_KEY}`
).toString("base64");
const endpoint = `${process.env.LANGFUSE_BASE_URL!.replace(/\/$/, "")}/api/public/otel`;

process.env.SIMPLE_AGENTS_TRACING_ENABLED = "true";
process.env.OTEL_EXPORTER_OTLP_PROTOCOL = "http/protobuf";
process.env.OTEL_EXPORTER_OTLP_ENDPOINT = endpoint;
process.env.OTEL_EXPORTER_OTLP_HEADERS = `Authorization=Basic ${token},x-langfuse-ingestion-version=4`;

syncOtelEnvFromProcess(
  process.env.SIMPLE_AGENTS_TRACING_ENABLED,
  process.env.OTEL_EXPORTER_OTLP_PROTOCOL,
  process.env.OTEL_EXPORTER_OTLP_ENDPOINT,
  process.env.OTEL_EXPORTER_OTLP_HEADERS,
  process.env.OTEL_SERVICE_NAME || undefined,
);

const client = new Client(process.env.WORKFLOW_API_KEY!, process.env.WORKFLOW_API_BASE);

function onEvent(err: unknown, eventJson: string): void {
  if (err) { console.error(err); return; }
  const event = parseWorkflowEvent(eventJson) as any;
  if (event.event_type === "node_stream_delta" && event.delta) {
    process.stdout.write(event.delta);
  }
}

const result = await client.streamWorkflow(
  join(__dirname, "workflow.yaml"),
  { messages: [{ role: "user", content: "your input here" }] },
  onEvent,
  { telemetry: { enabled: true, nerdstats: true } },
  { nodeLlmStreaming: true, splitStreamDeltas: false },
);
console.log("\n" + JSON.stringify(result, null, 2));
```

### TypeScript -- With Jaeger

```typescript
import { Client, syncOtelEnvFromProcess } from "simple-agents-node";
import { config as loadEnv } from "dotenv";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));
loadEnv({ path: join(__dirname, ".env") });

process.env.SIMPLE_AGENTS_TRACING_ENABLED = "true";
process.env.OTEL_EXPORTER_OTLP_ENDPOINT = "http://localhost:4317";
process.env.OTEL_EXPORTER_OTLP_PROTOCOL = "grpc";
process.env.OTEL_SERVICE_NAME = "my-workflow";

syncOtelEnvFromProcess(
  process.env.SIMPLE_AGENTS_TRACING_ENABLED,
  process.env.OTEL_EXPORTER_OTLP_PROTOCOL,
  process.env.OTEL_EXPORTER_OTLP_ENDPOINT,
  process.env.OTEL_EXPORTER_OTLP_HEADERS ?? "",
  process.env.OTEL_SERVICE_NAME,
);

const client = new Client(process.env.WORKFLOW_API_KEY!, process.env.WORKFLOW_API_BASE);

const result = await client.runWorkflow(
  join(__dirname, "workflow.yaml"),
  { messages: [{ role: "user", content: "your input here" }] },
  { telemetry: { enabled: true, nerdstats: true } },
);
console.log(JSON.stringify(result, null, 2));
```

## Execution Flags Reference

Pass at runtime to override/combine with per-node YAML settings:

| Flag | Type | Default | Purpose |
|---|---|---|---|
| `node_llm_streaming` | bool | `true` | Master switch for node streaming. `stream = yaml.stream AND this flag` |
| `split_stream_deltas` | bool | `false` | Emit separate thinking vs output delta events |
| `healing` | bool | `false` | Global healing. `heal = yaml.heal OR this flag` |
| `workflow_streaming` | bool | `false` | Forward token deltas to event sink |

## Validation Checklist

Before outputting any YAML:

- [ ] `id`, `version`, `entry_node` present
- [ ] `entry_node` exists in `nodes`
- [ ] Every node has a unique `id`
- [ ] All `switch` targets and edge targets exist as node IDs
- [ ] Every `llm_call` has `config.output_schema`
- [ ] `output_schema` has `required` and `additionalProperties: false`
- [ ] Switch conditions reference real output paths (`$.nodes.<id>.output.<field>`)
- [ ] Each prompt says `Return JSON only`
- [ ] `edges` cover all intended flow transitions
- [ ] For `custom_worker`, `handler` matches a function in the handler file
- [ ] No ambiguous multi-question prompts in interview/chat flows

## Complete Example: Email Classification Workflow

This real-world example classifies emails into categories with hierarchical routing and custom worker enrichment.

### workflow.yaml

```yaml
id: email-classifier
version: 1.0.0
entry_node: classify_email

nodes:
  - id: classify_email
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
          category:
            type: string
            enum: [hr, finance, education]
          reason:
            type: string
        required: [category, reason]
        additionalProperties: false
      prompt: |
        Classify this email into exactly one category.
        Return JSON only: {"category": "hr" | "finance" | "education", "reason": "..."}

  - id: route_category
    node_type:
      switch:
        branches:
          - condition: '$.nodes.classify_email.output.category == "finance"'
            target: detect_finance_subtype
        default: finalize

  - id: detect_finance_subtype
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
          subtype:
            type: string
            enum: [invoice, reimbursement, tax]
          reason:
            type: string
        required: [subtype, reason]
        additionalProperties: false
      prompt: |
        This email is classified as finance. Determine the subtype.
        Return JSON only: {"subtype": "invoice" | "reimbursement" | "tax", "reason": "..."}

  - id: route_finance
    node_type:
      switch:
        branches:
          - condition: '$.nodes.detect_finance_subtype.output.subtype == "invoice"'
            target: extract_company
        default: finalize

  - id: extract_company
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
          company_name:
            type: string
        required: [company_name]
        additionalProperties: false
      prompt: |
        Extract the seller/vendor company name from this invoice email.
        Return JSON only: {"company_name": "..."}

  - id: lookup_stakeholder
    node_type:
      custom_worker:
        handler: get_seller_name
    config:
      payload:
        company_name: "{{ nodes.extract_company.output.company_name }}"

  - id: finalize
    node_type:
      llm_call:
        model: gpt-4.1-mini
        messages_path: input.messages
        append_prompt_as_user: true
    config:
      output_schema:
        type: object
        properties:
          summary:
            type: string
        required: [summary]
        additionalProperties: false
      prompt: |
        Summarize the classification result.
        Category: {{ nodes.classify_email.output.category }}
        Return JSON only: {"summary": "..."}

edges:
  - from: classify_email
    to: route_category
  - from: detect_finance_subtype
    to: route_finance
  - from: extract_company
    to: lookup_stakeholder
  - from: lookup_stakeholder
    to: finalize
```

### handlers.py

```python
def get_seller_name(context, payload):
    company_name = str(payload.get("company_name", "")).strip().lower()
    stakeholder_map = {
        "google": "Sundar Pichai",
        "microsoft": "Satya Nadella",
        "apple": "Tim Cook",
        "amazon": "Andy Jassy",
    }
    return stakeholder_map.get(company_name, "unknown")
```

### run.py

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
    messages=[
        WorkflowMessage(
            role=WorkflowRole.USER,
            content="We received an invoice from Google for $50,000 for cloud services.",
        ),
    ],
)

result = client.run_workflow(workflow_execution_request_to_mapping(req))
print(json.dumps(result, indent=2))
```

## References

Read these files for reusable patterns and a pre-flight checklist:

- `skills/simpleagents-builder/references/patterns.md` -- Detect->Route->Act, LLM best practices, custom workers, templating, multi-level routing, image input, execution flags
- `skills/simpleagents-builder/references/checklist.md` -- QA checklist to validate YAML before outputting

Runnable skill examples (self-contained YAML + handler + runner):

- `skills/simpleagents-builder/examples/minimal-chat.yaml` -- simplest single-node workflow
- `skills/simpleagents-builder/examples/email-classification.yaml` -- hierarchical classification with custom worker enrichment
- `skills/simpleagents-builder/examples/handlers.py` -- Python custom worker handler
- `skills/simpleagents-builder/examples/run.py` -- Python normal run
- `skills/simpleagents-builder/examples/run_streaming.py` -- Python streaming run
- `skills/simpleagents-builder/examples/run.ts` -- TypeScript run with custom worker dispatch

Full working examples in the repo (source of truth):

- `examples/python-test-simpleAgents/test.yaml` -- full email classification with finance enrichment
- `examples/python-test-simpleAgents/friendly.yaml` -- minimal single-node chat bot
- `examples/python-test-simpleAgents/handlers.py` -- Python custom worker handler
- `examples/python-test-simpleAgents/test-py-simple-agents.py` -- normal Python run
- `examples/python-test-simpleAgents/test-py-simple-agents-streaming.py` -- streaming Python run
- `examples/python-test-simpleAgents/test-py-simple-agents-streaming-langfuse.py` -- streaming with Langfuse
- `examples/python-test-simpleAgents/test-py-simple-agents-invoice-image.py` -- image input (normal)
- `examples/python-test-simpleAgents/test-py-simple-agents-invoice-image-streaming.py` -- image input (streaming)
- `examples/python-test-simpleAgents/test-py-simple-agents-invoice-image-jaegar.py` -- image input with Jaeger
- `examples/napi-test-simpleAgents/test-simple-agents.ts` -- normal TypeScript run
- `examples/napi-test-simpleAgents/test-simple-agents-streaming.ts` -- streaming TypeScript run
- `examples/napi-test-simpleAgents/test-simple-agents-streaming-langfuse.ts` -- streaming with Langfuse
- `examples/napi-test-simpleAgents/test-simple-agents-invoice-image.ts` -- image input (TypeScript)
- `examples/napi-test-simpleAgents/test-simple-agents-invoice-image-jaegar.ts` -- image input with Jaeger
- `examples/napi-test-simpleAgents/handlers.ts` -- TypeScript custom worker dispatch
