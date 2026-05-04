# SimpleAgents FAQ

Frequently asked questions about SimpleAgents installation, configuration, usage, and best practices.

## Table of Contents

- [Getting Started](#getting-started)
- [Installation & Setup](#installation--setup)
- [Configuration](#configuration)
- [YAML Workflows](#yaml-workflows)
- [Language Bindings](#language-bindings)
- [Custom Workers](#custom-workers)
- [Streaming & Real-time](#streaming--real-time)
- [Observability & Tracing](#observability--tracing)
- [Production & Deployment](#production--deployment)
- [Troubleshooting](#troubleshooting)

---

## Getting Started

### Q: What is SimpleAgents?

**A:** SimpleAgents is a YAML workflow engine for AI products. It lets you define LLM-powered workflows as configuration files rather than code. You describe nodes (LLM calls, routing, custom workers), edges (connections), and schemas in YAML, then run them in Python, TypeScript, or Rust.

**Key philosophy:** Every agentic SaaS is a config. Build production-ready AI workflows with minimal code.

### Q: Who should use SimpleAgents?

**A:** SimpleAgents is ideal for:

- **Teams building structured AI flows** - classifiers, support bots, document processors, intake systems
- **Developers who want deterministic routing** with LLM-powered decision making
- **Engineers shipping fast** without building framework glue from scratch
- **Multi-language teams** needing the same workflow to run in Python and TypeScript

**Not for:** Single prompt calls without workflow logic (use the lower-level bindings directly).

### Q: How does SimpleAgents compare to LangChain/LlamaIndex?

**A:** 

| Aspect | SimpleAgents | LangChain/LlamaIndex |
|--------|--------------|---------------------|
| **Approach** | YAML-first configuration | Code-first framework |
| **Complexity** | Minimal boilerplate | More abstraction layers |
| **Language** | Rust core, Python/Node/Go bindings | Python-first |
| **Workflows** | Built-in graph execution | Chain-based, requires more setup |
| **Structured Output** | Native JSON healing & coercion | Requires additional setup |
| **Observability** | OpenTelemetry built-in | Requires integration |

SimpleAgents trades some flexibility for shipping speed and consistency.

### Q: Is SimpleAgents production-ready?

**A:** Yes. SimpleAgents is designed for production with:

- **Rust core** for performance and reliability
- **Built-in resilience** (retries, timeouts, fallbacks)
- **Observability** via OpenTelemetry (Langfuse, Jaeger)
- **Structured output validation** and JSON healing
- **Streaming support** for real-time applications
- **Multi-provider support** (OpenAI, Anthropic, Azure, OpenRouter, etc.)

---

## Installation & Setup

### Q: What are the system requirements?

**A:**

**For end users:**
- Python >=3.9 (for Python bindings)
- Node.js >=18 (for Node/TypeScript bindings)
- Any OpenAI-compatible API key

**For contributors:**
- Rust 1.75+
- Cargo
- Make

### Q: How do I install SimpleAgents?

**A:**

**Python:**
```bash
pip install simple-agents-py python-dotenv
```

**Node/TypeScript:**
```bash
npm install simple-agents-node dotenv
# or
bun add simple-agents-node dotenv
```

**Rust (direct):**
```bash
cargo add simple-agents-workflow
```

### Q: Do I need to install Rust to use SimpleAgents?

**A:** No. The Python and Node packages include pre-compiled Rust binaries. You only need Rust if:

- Contributing to the core
- Building from source
- Using the WASM bindings in a browser

### Q: Can I use SimpleAgents in a browser?

**A:** Yes! Use the WASM bindings:

```bash
npm install simple-agents-wasm
```

See `docs/BINDINGS_WASM.md` for details.

### Q: What providers are supported?

**A:** Any OpenAI-compatible provider:

- **OpenAI** (native)
- **Anthropic Claude** (via API)
- **Azure OpenAI**
- **OpenRouter**
- **Requesty**
- **Local servers** (vLLM, Ollama, llama.cpp)
- **Custom OpenAI-compatible endpoints**

---

## Configuration

### Q: How do I configure API keys?

**A:** Two approaches:

**1. Environment variables (recommended for development):**
```bash
# .env file
PROVIDER=openai
CUSTOM_API_BASE=https://api.openai.com/v1
CUSTOM_API_KEY=sk-your-key-here
# Optional: CUSTOM_API_MODEL=gpt-4.1-mini
```

**2. Explicit configuration (recommended for production):**
```python
from simple_agents_py import Client

client = Client(
    provider="openai",
    api_base="https://api.openai.com/v1",
    api_key="sk-your-key-here"
)
```

### Q: Can I use different providers for different nodes?

**A:** Yes! Each `llm_call` node can specify its own model:

```yaml
nodes:
  - id: classify
    node_type:
      llm_call:
        model: gpt-4.1-mini  # OpenAI
  - id: generate
    node_type:
      llm_call:
        model: anthropic/claude-3.5-sonnet  # Anthropic via OpenRouter
```

### Q: How do I use Azure OpenAI?

**A:**

```python
client = Client(
    provider="openai",
    api_base="https://your-resource.openai.azure.com/openai/deployments/your-deployment",
    api_key="your-azure-key"
)
```

Or set environment variables:
```bash
PROVIDER=openai
CUSTOM_API_BASE=https://your-resource.openai.azure.com/openai/deployments/your-deployment
CUSTOM_API_KEY=your-azure-key
```

### Q: How do I use local models (Ollama, vLLM)?

**A:**

**Ollama:**
```bash
# Start Ollama with OpenAI compatibility
OLLAMA_ORIGINS="*" ollama serve
```

```python
client = Client(
    provider="openai",
    api_base="http://localhost:11434/v1",
    api_key="ollama"
)
```

**vLLM:**
```bash
# Start vLLM
python -m vllm.entrypoints.openai.api_server --model meta-llama/Llama-2-70b-chat-hf
```

```python
client = Client(
    provider="openai",
    api_base="http://localhost:8000/v1",
    api_key="dummy-key"
)
```

---

## YAML Workflows

### Q: What are the three node types?

**A:**

| Type | Purpose | Example |
|------|---------|---------|
| `llm_call` | Call an LLM with structured output | Classification, generation, extraction |
| `switch` | Route based on previous node output | If billing → go here, if support → go there |
| `custom_worker` | Run your code | Database lookup, API call, business logic |

### Q: How do I create a simple workflow?

**A:** 

**1. Create `workflow.yaml`:**
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
    config:
      output_schema:
        type: object
        properties:
          answer:
            type: string
        required: [answer]
      prompt: |
        Answer the user's question concisely.
        Return JSON only: {"answer": "..."}
```

**2. Run it (Python):**
```python
from simple_agents_py import Client
from simple_agents_py.workflow_request import WorkflowExecutionRequest, WorkflowMessage, WorkflowRole

client = Client("openai")
req = WorkflowExecutionRequest(
    workflow_path="workflow.yaml",
    messages=[WorkflowMessage(role=WorkflowRole.USER, content="What is 2+2?")]
)
result = client.run_workflow(req)
print(result)
```

### Q: How does routing work?

**A:** Use `switch` nodes with JSONPath conditions:

```yaml
nodes:
  - id: classify
    node_type:
      llm_call:
        model: gpt-4.1-mini
    config:
      output_schema:
        type: object
        properties:
          category:
            type: string
            enum: [billing, support, sales]
      prompt: Classify the user message into one category.

  - id: route
    node_type:
      switch:
        branches:
          - condition: '$.nodes.classify.output.category == "billing"'
            target: handle_billing
          - condition: '$.nodes.classify.output.category == "support"'
            target: handle_support
        default: handle_sales
```

### Q: What is JSON healing?

**A:** LLMs sometimes return malformed JSON (trailing commas, markdown fences, missing quotes). JSON healing automatically fixes these issues:

```yaml
nodes:
  - id: extract
    node_type:
      llm_call:
        model: gpt-4.1-mini
        heal: true  # Enable auto-healing
```

**What it fixes:**
- Markdown code fences (```json...```)
- Trailing commas
- Single quotes → double quotes
- Missing closing brackets/braces
- Truncated JSON (partial responses)

### Q: How do I reference previous node outputs?

**A:** Use templating with `{{ }}` syntax:

```yaml
nodes:
  - id: extract_company
    node_type:
      llm_call:
        model: gpt-4.1-mini
    config:
      output_schema:
        type: object
        properties:
          company_name:
            type: string

  - id: lookup
    node_type:
      custom_worker:
        handler: get_company_info
    config:
      payload:
        company: "{{ nodes.extract_company.output.company_name }}"
        category: "{{ nodes.classify.output.category }}"
```

### Q: Can I use images in workflows?

**A:** Yes! Both Python and TypeScript support multimodal input:

**Python:**
```python
import base64
from pathlib import Path

b64 = base64.b64encode(Path("invoice.jpeg").read_bytes()).decode("ascii")

messages = [
    WorkflowMessage(
        role=WorkflowRole.USER,
        content=[
            {"type": "text", "text": "Describe this image."},
            {"type": "image_url", "image_url": {"url": f"data:image/jpeg;base64,{b64}"}},
        ],
    ),
]
```

**TypeScript:**
```typescript
import { readFileSync } from "node:fs";

const b64 = readFileSync("invoice.jpeg").toString("base64");

const messages = [
  {
    role: "user",
    content: [
      { type: "text", text: "Describe this image." },
      { type: "image", mediaType: "image/jpeg", data: b64 },
    ],
  },
];
```

---

## Language Bindings

### Q: What's the difference between Python and Node bindings?

**A:** Feature parity is maintained, but there are API style differences:

| Feature | Python | Node/TypeScript |
|---------|--------|-----------------|
| **Request building** | Pydantic models | Plain objects |
| **Streaming** | Iterator | Callback-based |
| **Custom workers** | `handlers.py` file | `customWorkerDispatch` callback |
| **Async** | Sync by default | Promise-based |

### Q: Can I use the same YAML workflow in Python and Node?

**A:** Yes! Workflows are language-agnostic. Define once, run anywhere:

```yaml
# workflow.yaml - works in both Python and Node
id: classifier
version: 1.0.0
entry_node: classify
nodes:
  - id: classify
    node_type:
      llm_call:
        model: gpt-4.1-mini
```

### Q: How do I add optional dependencies?

**A:**

**Python (Pydantic support):**
```bash
pip install simple-agents-py[pydantic]
```

This enables `WorkflowExecutionRequest`, `WorkflowMessage`, etc. as Pydantic models.

---

## Custom Workers

### Q: What is a custom worker?

**A:** A `custom_worker` node lets you run your own code inside the workflow graph - database lookups, API calls, business logic, etc.

### Q: How do I write a custom worker in Python?

**A:**

**1. Create `handlers.py` next to your YAML:**
```python
def lookup_company(*, context: dict, payload: dict):
    """
    Handler signature: (*, context, payload)
    
    - context: execution context with input, nodes, globals, trace
    - payload: resolved config.payload from YAML
    """
    company_name = payload.get("company_name", "unknown")
    
    # Your business logic here
    return {
        "company_name": company_name,
        "found": True,
        "industry": "Technology"
    }
```

**2. Reference in YAML:**
```yaml
nodes:
  - id: lookup
    node_type:
      custom_worker:
        handler: lookup_company
        handler_file: handlers.py  # optional, defaults to handlers.py
    config:
      payload:
        company_name: "{{ nodes.extract.output.company_name }}"
```

### Q: How do I write a custom worker in TypeScript?

**A:**

```typescript
import { Client } from "simple-agents-node";

function customWorkerDispatch(req: {
  handler: string;
  payload: unknown;
  context: unknown;
}): string {
  if (req.handler === "lookup_company") {
    const payload = req.payload as Record<string, unknown>;
    const result = {
      company_name: payload.company_name || "unknown",
      found: true,
    };
    return JSON.stringify(result);
  }
  throw new Error(`unknown handler: ${req.handler}`);
}

// Pass to workflow execution
const result = await client.runWorkflow(
  workflowPath,
  input,
  undefined,
  undefined,
  customWorkerDispatch
);
```

**Important:** TypeScript custom workers run synchronously. Return JSON-serializable values only.

### Q: Can custom workers be async?

**A:** 

- **Python:** No, handlers must be synchronous functions. The executor calls handlers directly and immediately serializes the return value, so `async def` would return a coroutine object that is never awaited.
- **TypeScript:** No, currently synchronous only (workaround: pre-fetch data before workflow execution)

### Q: How do I share data between nodes?

**A:** Use `context.nodes` to access previous outputs:

```python
def my_handler(*, context, payload):
    # Access previous node outputs
    classify_output = context["nodes"]["classify"]["output"]
    category = classify_output["category"]
    
    # Access original input
    messages = context["input"]["messages"]
    
    return {"result": f"Category was: {category}"}
```

---

## Streaming & Real-time

### Q: How do I enable streaming?

**A:** Two levels of streaming:

**1. Node-level streaming (YAML):**
```yaml
nodes:
  - id: generate
    node_type:
      llm_call:
        model: gpt-4.1-mini
        stream: true
```

**2. Runtime streaming (Python/Node):**
```python
req = WorkflowExecutionRequest(
    workflow_path="workflow.yaml",
    messages=[...],
    execution=WorkflowExecutionFlags(
        node_llm_streaming=True,
    ),
)

result = client.stream_workflow(
    req,
    on_event=lambda event: print(event),
)
```

### Q: What's the difference between `stream` and `heal`?

**A:** They serve different purposes and can't be combined at the node level:

| Mode | Use When | Behavior |
|------|----------|----------|
| `stream: true` | You want real-time token delivery | Tokens arrive as they're generated |
| `heal: true` | LLM might return malformed JSON | Auto-fixes JSON errors (best-effort) |

**Note:** `stream=True` and `heal=True` cannot both be true on the same node. Choose based on your needs:
- Interactive UI → Use streaming
- Reliable data extraction → Use healing

### Q: How do I stream structured JSON?

**A:** Use `stream_json_as_text: true`:

```yaml
nodes:
  - id: generate
    node_type:
      llm_call:
        model: gpt-4.1-mini
        stream: true
        stream_json_as_text: true
    config:
      output_schema:
        type: object
        properties:
          response:
            type: string
```

This streams raw text deltas instead of parsed JSON snapshots.

### Q: How do I handle streaming events?

**A:** Event types you can handle:

```python
def on_event(event: dict):
    event_type = event.get("event_type")
    
    if event_type == "node_stream_delta":
        # Raw token delta
        print(event.get("delta", ""), end="", flush=True)
    
    elif event_type == "node_stream_thinking_delta":
        # Reasoning/thinking tokens (if model supports it)
        print(f"[Thinking: {event.get('delta')}]")
    
    elif event_type == "node_stream_output_delta":
        # Output tokens (split from thinking)
        print(event.get("delta", ""), end="", flush=True)
    
    elif event_type == "node_llm_input_resolved":
        # LLM input details before calling
        print(f"Calling {event.get('model')} with prompt: {event.get('prompt')}")
    
    elif event_type == "workflow_completed":
        # Final completion
        print("Workflow done!")
```

---

## Observability & Tracing

### Q: How do I set up Langfuse observability?

**A:**

**Python:**
```python
import base64, os

# Setup OpenTelemetry for Langfuse
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

# Enable in request
req = WorkflowExecutionRequest(
    workflow_path="workflow.yaml",
    messages=[...],
    workflow_options=WorkflowRunOptions(
        telemetry=WorkflowTelemetryConfig(enabled=True, nerdstats=True),
    ),
)
```

**Environment variables:**
```bash
LANGFUSE_PUBLIC_KEY=pk-lf-...
LANGFUSE_SECRET_KEY=sk-lf-...
LANGFUSE_BASE_URL=http://localhost:3000
```

### Q: How do I set up Jaeger tracing?

**A:**

```bash
# .env
SIMPLE_AGENTS_TRACING_ENABLED=true
OTEL_EXPORTER_OTLP_ENDPOINT=http://localhost:4317
OTEL_EXPORTER_OTLP_PROTOCOL=grpc
OTEL_SERVICE_NAME=my-workflow-service
```

View traces at `http://localhost:16686` (Jaeger UI).

### Q: What is "nerdstats"?

**A:** Detailed performance metrics:

```python
telemetry=WorkflowTelemetryConfig(nerdstats=True)
```

This includes:
- Per-node execution time
- Token usage (input/output)
- Tokens per second (TPS)
- Latency breakdown
- Step-by-step timings

### Q: Can I sample traces?

**A:** Yes, use deterministic sampling:

```python
workflow_options={
    "telemetry": {
        "enabled": True,
        "sample_rate": 0.1  # 10% of traces
    }
}
```

Sampling is deterministic per trace ID.

### Q: How do I add custom trace attributes?

**A:**

```python
workflow_options={
    "trace": {
        "tenant": {
            "conversation_id": "<uuid>",
            "user_id": "user-123",
            "session_id": "session-456"
        }
    }
}
```

---

## Production & Deployment

### Q: What's the recommended deployment approach?

**A:** SimpleAgents can run:

1. **As a library** - Import in your Python/Node app
2. **As a service** - Wrap workflows in FastAPI/Express endpoints
3. **Serverless** - Deploy to Lambda/Cloud Functions (cold start ~100-200ms)

**Example FastAPI service:**
```python
from fastapi import FastAPI
from simple_agents_py import Client
from simple_agents_py.workflow_request import WorkflowExecutionRequest, WorkflowMessage, WorkflowRole

app = FastAPI()
client = Client("openai")

@app.post("/classify")
async def classify_email(text: str):
    req = WorkflowExecutionRequest(
        workflow_path="classifier.yaml",
        messages=[WorkflowMessage(role=WorkflowRole.USER, content=text)]
    )
    return client.run_workflow(req)
```

### Q: How do I handle rate limits?

**A:** SimpleAgents includes built-in resilience:

```yaml
nodes:
  - id: generate
    node_type:
      llm_call:
        model: gpt-4.1-mini
        # Built-in retry with exponential backoff
        # Respects 429 responses automatically
```

For custom handling, wrap in your own retry logic or use a circuit breaker.

### Q: How do I scale horizontally?

**A:** SimpleAgents is stateless:

1. Deploy multiple instances behind a load balancer
2. Store workflow YAMLs in a shared location (S3, configmap)
3. Each instance loads YAMLs independently

**For stateful workflows** (conversations):
- Store conversation history externally (Redis, database)
- Pass history in `messages` on each request

### Q: Are there any security best practices?

**A:**

1. **Never commit API keys** - Use environment variables or secret managers
2. **Validate custom worker inputs** - Sanitize `payload` data
3. **Use least-privilege** - Separate API keys per environment
4. **Enable tracing** - Audit workflow execution
5. **Validate schemas** - Strict output schemas prevent injection

```yaml
config:
  output_schema:
    type: object
    properties:
      response:
        type: string
        maxLength: 1000  # Limit output size
    additionalProperties: false  # Reject unexpected fields
```

---

## Troubleshooting

### Q: Where can I find more help?

**A:**

- **Documentation:** `docs/` directory in the repo
- **Examples:** `examples/` directory with runnable samples
- **Playground:** https://yamslam.craftsmanlabs.net/playground
- **Full Docs:** https://docs.simpleagents.craftsmanlabs.net/
- **Issues:** https://github.com/CraftsMan-Labs/SimpleAgents/issues

### Q: Common error: "API key not found"

**A:** Check:
1. `.env` file is loaded: `load_dotenv()` in Python, `dotenv.config()` in Node
2. Environment variable names match the provider:
   - OpenAI: `OPENAI_API_KEY`
   - Anthropic: `ANTHROPIC_API_KEY`
   - Generic: `CUSTOM_API_KEY`
3. API key is not empty or whitespace

### Q: Common error: "Invalid JSON from LLM"

**A:** Enable healing:

```yaml
node_type:
  llm_call:
    model: gpt-4.1-mini
    heal: true
```

Or adjust your prompt to be more explicit:
```yaml
prompt: |
  Return valid JSON only. No markdown, no explanation.
  Format: {"field": "value"}
```

### Q: Common error: "Handler not found" (custom workers)

**A:** 

- **Python:** Ensure `handlers.py` exists next to your YAML file and the function name matches exactly
- **TypeScript:** Ensure you pass `customWorkerDispatch` callback to `runWorkflow`
- Check function signature uses `*, context, payload` (not positional args)

---

## Still Have Questions?

- 📖 Read the full docs: `docs/DOCS_MAP.md`
- 🔍 Check examples: `examples/`
- 🐛 Report issues: https://github.com/CraftsMan-Labs/SimpleAgents/issues
- 🌟 Star the repo if you find it helpful!
