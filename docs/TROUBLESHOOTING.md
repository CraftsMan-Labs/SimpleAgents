# Troubleshooting Guide

Common issues, error patterns, and solutions for SimpleAgents core and binding workflows.

## Table of Contents

- [Environment & Credential Issues](#environment--credential-issues)
- [Installation & Build Issues](#installation--build-issues)
- [YAML Workflow Errors](#yaml-workflow-errors)
- [Language Binding Issues](#language-binding-issues)
- [Custom Worker Errors](#custom-worker-errors)
- [Streaming Issues](#streaming-issues)
- [Observability & Tracing Issues](#observability--tracing-issues)
- [Performance Issues](#performance-issues)
- [Provider-Specific Issues](#provider-specific-issues)
- [Debugging Techniques](#debugging-techniques)
- [Getting Help](#getting-help)

---

## Environment & Credential Issues

### Issue: "API key not found" or authentication errors

**Symptoms:**
```text
RuntimeError: API key not found for provider 'openai'
Error: Missing required environment variable WORKFLOW_API_KEY
```text

**Solutions:**

1. **Verify .env file is loaded:**
   ```python
   from dotenv import load_dotenv
   load_dotenv()  # Must be called before creating Client
   ```

2. **Check environment variable names:**
   ```bash
   # For workflow runner
   WORKFLOW_PROVIDER=openai
   WORKFLOW_API_BASE=https://api.openai.com/v1
   WORKFLOW_API_KEY=sk-your-key
   
   # For direct provider usage
   OPENAI_API_KEY=sk-your-key
   ANTHROPIC_API_KEY=sk-ant-...
   ```

3. **Verify the key is set:**
   ```bash
   echo $WORKFLOW_API_KEY  # Should print your key
   ```

4. **For Docker/containerized environments:**
   ```bash
   # Pass env vars explicitly
   docker run -e WORKFLOW_API_KEY=$WORKFLOW_API_KEY myapp
   ```

### Issue: "Provider not found" errors

**Symptoms:**
```text
Error: Unknown provider 'azure'
RuntimeError: Provider 'custom' is not supported
```text

**Solutions:**

Use `openai` as the provider with a custom base URL:
```python
# Correct - Azure OpenAI
client = Client(
    provider="openai",
    api_base="https://your-resource.openai.azure.com/...",
    api_key="your-azure-key"
)
```

### Issue: Live tests are skipped

**Symptoms:**
```text
SKIPPED: Live tests require API credentials
```

**Solutions:**

Set required environment variables:
```bash
export PROVIDER=openai
export CUSTOM_API_MODEL=gpt-4.1-mini
export CUSTOM_API_KEY=sk-your-key
export CUSTOM_API_BASE=https://api.openai.com/v1  # optional
```

For Node tests specifically:
```bash
cd crates/simple-agents-napi
npm run test:live  # Will skip without env vars by design
```

---

## Installation & Build Issues

### Issue: Python bindings fail to install

**Symptoms:**
```text
ERROR: Could not build wheels for simple-agents-py
ImportError: cannot import name 'Client' from 'simple_agents_py'
```

**Solutions:**

1. **Ensure Python 3.8+:**
   ```bash
   python --version  # Should be 3.8 or higher
   ```

2. **Use uv (recommended):**
   ```bash
   uv pip install simple-agents-py
   ```

3. **For development builds:**
   ```bash
   cd crates/simple-agents-py
   uv build
   uv pip install -e .
   ```

4. **If using pip:**
   ```bash
   pip install --upgrade pip
   pip install simple-agents-py
   ```

### Issue: Stale Python bindings cache

**Symptoms:**
```text
TypeError: argument 'prompt': 'list' object cannot be converted to 'PyString'
AttributeError: 'Client' object has no attribute 'run_workflow'
```

**Solution:**
Clear uv cache and rebuild:
```bash
cd examples
rm -rf .venv .uv-cache
UV_CACHE_DIR=$PWD/.uv-cache uv run --no-cache python python_client.py
```

### Issue: Node bindings fail to build

**Symptoms:**
```text
Error: Cannot find module 'simple-agents-node'
napi:Error: Failed to load native addon
```

**Solutions:**

1. **Install dependencies:**
   ```bash
   cd crates/simple-agents-napi
   npm ci
   ```

2. **Rebuild the native module:**
   ```bash
   npm run build
   # or
   npm run build:debug
   ```

3. **Verify Node version:**
   ```bash
   node --version  # Should be 16+
   ```

### Issue: Rust build failures

**Symptoms:**
```text
error: linking with `cc` failed
error: could not compile `simple-agents-core`
```

**Solutions:**

1. **Ensure Rust 1.75+:**
   ```bash
   rustc --version
   rustup update
   ```

2. **Install system dependencies:**
   ```bash
   # Ubuntu/Debian
   sudo apt-get install build-essential libssl-dev pkg-config
   
   # macOS
   xcode-select --install
   ```

3. **Clean and rebuild:**
   ```bash
   cargo clean
   cargo build --all
   ```

### Issue: WASM bindings build fails

**Symptoms:**
```text
error: wasm-bindgen not found
error: failed to run custom build command for `simple-agents-wasm`
```

**Solutions:**

Install wasm-bindgen-cli:
```bash
# Makefile target does this automatically:
make ensure-wasm-bindgen

# Or manually:
cargo install wasm-bindgen-cli --version 0.2.117
```

---

## YAML Workflow Errors

### Issue: "Invalid YAML syntax"

**Symptoms:**
```text
Error: failed to parse workflow yaml: invalid type
RuntimeError: YamlError: mapping values are not allowed here
```

**Solutions:**

1. **Validate YAML syntax:**
   ```bash
   # Use yamllint or online validator
   yamllint workflow.yaml
   ```

2. **Common YAML mistakes:**
   ```yaml
   # WRONG - Missing space after colon
   model:gpt-4.1-mini
   
   # CORRECT
   model: gpt-4.1-mini
   
   # WRONG - Tab characters instead of spaces
   nodes:
   	- id: classify
   
   # CORRECT - Use spaces only
   nodes:
     - id: classify
   ```

3. **Use quotes for strings with special characters:**
   ```yaml
   prompt: |
     This is a multi-line string
     with "quotes" and 'apostrophes'
   ```

### Issue: "Node not found" or "Invalid node reference"

**Symptoms:**
```text
Error: Node 'classify' not found in workflow
RuntimeError: Invalid node reference: extract_company
```

**Solutions:**

1. **Check node ID spelling:**
   ```yaml
   nodes:
     - id: classify  # Defined as 'classify'
     - id: route
       node_type:
         switch:
           branches:
             - condition: '$.nodes.clasify.output.category == "billing"'  # WRONG - typo
               target: handle_billing
   ```

2. **Ensure entry_node exists:**
   ```yaml
   id: my-workflow
   version: 1.0.0
   entry_node: classify  # Must match a node id
   
   nodes:
     - id: classify  # This must exist
   ```

3. **Verify edge references:**
   ```yaml
   edges:
     - from: classify  # Must be a valid node id
       to: route       # Must be a valid node id
   ```

### Issue: "Invalid JSONPath expression"

**Symptoms:**
```text
Error: Failed to evaluate condition: Invalid JSONPath
RuntimeError: JSONPath error: path not found
```

**Solutions:**

1. **Test JSONPath syntax:**
   ```yaml
   # WRONG - Missing $ prefix
   condition: 'nodes.classify.output.category == "billing"'
   
   # CORRECT
   condition: '$.nodes.classify.output.category == "billing"'
   ```

2. **Common JSONPath patterns:**
   ```yaml
   # Access node output
   "{{ nodes.node_id.output.field }}"
   
   # Access input messages
   "{{ input.messages[0].content }}"
   
   # Access globals
   "{{ globals.config_value }}"
   
   # In switch conditions
   condition: '$.nodes.previous.output.status == "success"'
   ```

3. **Use quotes for string comparisons:**
   ```yaml
   condition: '$.nodes.classify.output.category == "billing"'  # Quotes required
   condition: '$.nodes.score.output.value > 0.5'               # Numbers ok without
   ```

### Issue: Schema validation failures

**Symptoms:**
```text
Error: Schema validation failed: missing required field 'response'
Error: additionalProperties 'extra_field' not allowed
```

**Solutions:**

1. **Use strict schemas with additionalProperties: false:**
   ```yaml
   config:
     output_schema:
       type: object
       properties:
         response:
           type: string
         confidence:
           type: number
       required: [response]
       additionalProperties: false  # Rejects unexpected fields
   ```

2. **Enable healing for malformed JSON:**
   ```yaml
   node_type:
     llm_call:
       model: gpt-4.1-mini
       heal: true  # Auto-fixes common JSON errors
   ```

3. **Improve the prompt:**
   ```yaml
   prompt: |
     Return ONLY valid JSON. No markdown, no explanation.
     Required fields: response (string), confidence (number between 0-1)
     Example: {"response": "Hello", "confidence": 0.95}
   ```

### Issue: "Circular dependency detected"

**Symptoms:**
```text
Error: Circular dependency in workflow graph
RuntimeError: Cycle detected: classify -> route -> classify
```

**Solutions:**

Check your edges for loops:
```yaml
# WRONG - Creates a cycle
edges:
  - from: classify
    to: route
  - from: route
    to: classify  # Loops back!

# CORRECT - DAG structure
edges:
  - from: classify
    to: route
  - from: route
    to: handle_billing
  - from: route
    to: handle_support
```

Workflows must be directed acyclic graphs (DAGs).

---

## Language Binding Issues

### Issue: Python binding contract failures

**Symptoms:**
```text
FAILED tests/test_binding_contract.py::test_contract - AssertionError
```

**Solutions:**

1. **Run contract tests to identify mismatch:**
   ```bash
   ./scripts/run-binding-contracts.sh
   ```

2. **Check fixture file:**
   ```bash
   cat parity-fixtures/binding_contract.json
   ```

3. **Common causes:**
   - Binding API changed without fixture update
   - Symbol missing in generated declarations
   - Streaming event shape changed

4. **Update fixtures if needed:**
   ```bash
   # After intentional API changes
   ./scripts/update-binding-contracts.sh
   ```

### Issue: Node binding contract failures

**Symptoms:**
```text
not ok 1 - Contract test: complete should return ResponseWithMetadata
TypeError: Cannot read property 'content' of undefined
```

**Solutions:**

1. **Rebuild the addon:**
   ```bash
   cd crates/simple-agents-napi
   npm run build
   ```

2. **Run TypeScript checks:**
   ```bash
   make node-typecheck
   ```

3. **Check declaration file:**
   ```bash
   cat crates/simple-agents-napi/index.d.ts
   ```

### Issue: Pydantic validation errors (Python)

**Symptoms:**
```text
pydantic.error_wrappers.ValidationError: 1 validation error for WorkflowExecutionRequest
field required (type=value_error.missing)
```

**Solutions:**

1. **Install pydantic extra:**
   ```bash
   pip install simple-agents-py[pydantic]
   ```

2. **Check field names:**
   ```python
   from simple_agents_py.workflow_request import WorkflowExecutionRequest, WorkflowMessage, WorkflowRole
   
   # CORRECT
   req = WorkflowExecutionRequest(
       workflow_path="workflow.yaml",  # Note: underscore, not camelCase
       messages=[WorkflowMessage(role=WorkflowRole.USER, content="Hello")]
   )
   ```

3. **Use dict-based requests as fallback:**
   ```python
   # Skip Pydantic models entirely
   req = {
       "workflow_path": "workflow.yaml",
       "messages": [{"role": "user", "content": "Hello"}]
   }
   ```

### Issue: TypeScript type errors

**Symptoms:**
```text
error TS2345: Argument of type 'string' is not assignable to parameter of type 'MessageInput[]'
```

**Solutions:**

1. **Check import paths:**
   ```typescript
   // CORRECT
   import { Client } from "simple-agents-node";
   import type { MessageInput } from "simple-agents-node";
   
   // For workflow events
   import { parseWorkflowEvent } from "simple-agents-node/workflow_event";
   ```

2. **Use proper message format:**
   ```typescript
   const messages: MessageInput[] = [
     { role: "user", content: "Hello" }  // Content should be string or array
   ];
   ```

3. **Run type checker:**
   ```bash
   make node-typecheck
   ```

---

## Custom Worker Errors

### Issue: "Handler not found" (Python)

**Symptoms:**
```text
RuntimeError: Custom worker handler 'lookup_company' not found
Error: No module named 'handlers'
```

**Solutions:**

1. **Ensure handlers.py exists:**
   ```bash
   ls -la handlers.py  # Must be in same directory as workflow.yaml
   ```

2. **Check function name matches exactly:**
   ```python
   # handlers.py
   def lookup_company(*, context, payload):  # Must match YAML exactly
       ...
   ```

   ```yaml
   # workflow.yaml
   node_type:
     custom_worker:
       handler: lookup_company  # Must match function name
       handler_file: handlers.py  # Optional, defaults to handlers.py
   ```

3. **Verify function signature:**
   ```python
   # CORRECT - Keyword-only args
   def my_handler(*, context, payload):
       ...
   
   # WRONG - Positional args
   def my_handler(context, payload):
       ...
   ```

4. **Check for import errors in handlers.py:**
   ```bash
   python -c "import handlers"
   ```

### Issue: "Handler not found" (TypeScript)

**Symptoms:**
```text
Error: custom_worker requires customWorkerDispatch callback
Error: unknown custom worker handler: lookup_company
```

**Solutions:**

1. **Always pass the dispatch callback:**
   ```typescript
   function customWorkerDispatch(req: {
     handler: string;
     payload: unknown;
     context: unknown;
   }): string {
     if (req.handler === "lookup_company") {
       return JSON.stringify({ result: "found" });
     }
     throw new Error(`unknown handler: ${req.handler}`);
   }
   
   // Pass as LAST argument
   const result = await client.runWorkflow(
     workflowPath,
     input,
     undefined,  // workflowOptions
     undefined,  // executionFlags
     customWorkerDispatch  // REQUIRED for custom_worker nodes
   );
   ```

2. **Use executeWorkflowYaml (recommended):**
   ```typescript
   import { Client } from "simple-agents-node";
   
   const client = new Client("openai");
   
   // This signature is cleaner
   const result = client.executeWorkflowYaml({
     workflowPath: "workflow.yaml",
     messages: [...],
     customWorkerDispatch: myDispatch,  // Built into request object
   });
   ```

### Issue: Custom worker returns wrong format

**Symptoms:**
```text
Error: Custom worker output is not valid JSON
RuntimeError: Failed to parse custom worker result
```

**Solutions:**

1. **Return JSON-serializable data:**
   ```python
   # CORRECT
   def my_handler(*, context, payload):
       return {
           "status": "success",
           "data": {"key": "value"}
       }
   
   # WRONG - Returns non-serializable object
   def my_handler(*, context, payload):
       return SomeCustomClass()  # Don't return custom objects
   ```

2. **TypeScript - Return JSON string:**
   ```typescript
   function dispatch(req) {
     if (req.handler === "my_handler") {
       return JSON.stringify({ result: "data" });  // Must return string
     }
   }
   ```

3. **Handle errors gracefully:**
   ```python
   def my_handler(*, context, payload):
       try:
           result = risky_operation()
           return {"success": True, "result": result}
       except Exception as e:
           return {"success": False, "error": str(e)}
   ```

### Issue: TypeError in custom worker (Python)

**Symptoms:**
```text
TypeError: my_handler() got an unexpected keyword argument 'context'
```

**Solution:**

Update to new signature:
```python
# OLD (deprecated)
def my_handler(input, nodes, globals):
    ...

# NEW (current)
def my_handler(*, context, payload):
    input_data = context["input"]
    nodes = context["nodes"]
    globals_ = context["globals"]
    ...
```

---

## Streaming Issues

### Issue: Streaming not working

**Symptoms:**
- No events received
- `on_event` callback never fires
- Workflow completes without streaming

**Solutions:**

1. **Enable streaming at both levels:**
   ```yaml
   # YAML level
   nodes:
     - id: generate
       node_type:
         llm_call:
           model: gpt-4.1-mini
           stream: true  # Enable for this node
   ```

   ```python
   # Runtime level
   execution=WorkflowExecutionFlags(
       node_llm_streaming=True,  # Master switch
   )
   ```

2. **Check streaming logic:**
   - Stream = YAML `stream: true` AND runtime `node_llm_streaming: true`
   - Both must be enabled

3. **Use proper event handling:**
   ```python
   def on_event(event):
       event_type = event.get("event_type")
       if event_type == "node_stream_delta":
           print(event.get("delta", ""), end="")
       elif event_type == "workflow_error":
           print(f"Error: {event}")
   
   result = client.stream_workflow(request, on_event=on_event)
   ```

### Issue: "stream and heal cannot both be true"

**Symptoms:**
```text
ValidationError: stream=True and heal=True cannot both be enabled
RuntimeError: Node config conflict: streaming and healing are mutually exclusive
```

**Solution:**

Choose one mode per node:
```yaml
# For real-time UI feedback
nodes:
  - id: chat
    node_type:
      llm_call:
        model: gpt-4.1-mini
        stream: true
        heal: false

# For reliable data extraction
nodes:
  - id: extract
    node_type:
      llm_call:
        model: gpt-4.1-mini
        stream: false
        heal: true
```

**Workaround for both:**
Use separate nodes - one for streaming (user-facing) and one for healing (data extraction).

### Issue: Streaming structured output is garbled

**Symptoms:**
- Partial JSON looks corrupted
- Events show incomplete/malformed data
- Parsing fails mid-stream

**Solutions:**

1. **Use stream_json_as_text:**
   ```yaml
   node_type:
     llm_call:
       model: gpt-4.1-mini
       stream: true
       stream_json_as_text: true  # Stream as text, not parsed JSON
   ```

2. **Parse incrementally:**
   ```python
   from simple_agents_py import StreamingParser
   
   parser = StreamingParser()
   
   def on_event(event):
       if event.get("event_type") == "node_stream_delta":
           parser.feed(event.get("delta", ""))
           partial = parser.try_parse()
           if partial:
               print(f"Partial: {partial.value}")
   ```

---

## Observability & Tracing Issues

### Issue: Traces not appearing in Langfuse/Jaeger

**Symptoms:**
- No traces visible in UI
- Workflow runs but no observability data

**Solutions:**

1. **Verify environment variables:**
   ```bash
   echo $SIMPLE_AGENTS_TRACING_ENABLED  # Should be "true"
   echo $OTEL_EXPORTER_OTLP_ENDPOINT    # Should be set
   ```

2. **Check endpoint connectivity:**
   ```bash
   curl $OTEL_EXPORTER_OTLP_ENDPOINT
   ```

3. **Enable telemetry in request:**
   ```python
   workflow_options=WorkflowRunOptions(
       telemetry=WorkflowTelemetryConfig(enabled=True)
   )
   ```

4. **For Langfuse specifically:**
   ```python
   import base64
   
   # Verify token is correct
   token = base64.b64encode(f"{public}:{secret}".encode()).decode("ascii")
   print(f"Token length: {len(token)}")  # Should be > 0
   ```

### Issue: "OTEL exporter failed"

**Symptoms:**
```text
ERROR opentelemetry_otlp: Export failed: connection refused
Warning: Failed to export spans
```

**Solutions:**

1. **Verify collector is running:**
   ```bash
   # For Jaeger
   docker run -d --name jaeger \
     -e COLLECTOR_OTLP_ENABLED=true \
     -p 16686:16686 \
     -p 4317:4317 \
     jaegertracing/all-in-one:latest
   
   # Check if running
   curl http://localhost:16686
   ```

2. **Match protocol to endpoint:**
   ```bash
   # gRPC endpoint
   OTEL_EXPORTER_OTLP_PROTOCOL=grpc
   OTEL_EXPORTER_OTLP_ENDPOINT=http://localhost:4317
   
   # HTTP endpoint
   OTEL_EXPORTER_OTLP_PROTOCOL=http/protobuf
   OTEL_EXPORTER_OTLP_ENDPOINT=http://localhost:4318
   ```

3. **Check headers format:**
   ```bash
   # Correct format
   OTEL_EXPORTER_OTLP_HEADERS="Authorization=Bearer token,header2=value"
   ```

### Issue: High cardinality traces

**Symptoms:**
- Langfuse/Jaeger UI slow
- Too many unique traces
- Storage growing rapidly

**Solutions:**

1. **Enable sampling:**
   ```python
   workflow_options={
       "telemetry": {
           "enabled": True,
           "sample_rate": 0.1  # Only 10% of traces
       }
   }
   ```

2. **Disable nerdstats if not needed:**
   ```python
   telemetry=WorkflowTelemetryConfig(
       enabled=True,
       nerdstats=False  # Reduces span count
   )
   ```

---

## Performance Issues

### Issue: Workflows are slow

**Symptoms:**
- High latency (>2s for simple workflows)
- Slow step execution
- Poor throughput

**Solutions:**

1. **Check LLM model:**
   ```yaml
   # Faster models
   model: gpt-4.1-mini  # Faster than gpt-4
   model: gpt-3.5-turbo  # Fastest OpenAI model
   ```

2. **Enable streaming for user-facing workflows:**
   ```yaml
   stream: true  # Reduces time-to-first-token
   ```

3. **Profile with nerdstats:**
   ```python
   telemetry=WorkflowTelemetryConfig(nerdstats=True)
   # Check step_timings in result for slow nodes
   ```

4. **Parallelize independent nodes:**
   ```yaml
   # These nodes run in parallel if no dependencies
   nodes:
     - id: extract_a
       ...
     - id: extract_b
       ...
   
   edges: []  # No dependencies = parallel execution
   ```

### Issue: High memory usage

**Symptoms:**
- OOM errors
- Memory growing over time
- Rust panics on allocation

**Solutions:**

1. **Limit output size:**
   ```yaml
   node_type:
     llm_call:
       model: gpt-4.1-mini
       max_tokens: 500  # Limit response size
   ```

2. **Process large workflows in chunks:**
   ```python
   # Don't load all data at once
   for chunk in batches:
       result = client.run_workflow(
           WorkflowExecutionRequest(
               workflow_path="process.yaml",
               messages=[{"role": "user", "content": chunk}]
           )
       )
   ```

3. **Check for memory leaks in custom workers:**
   ```python
   def my_handler(*, context, payload):
       # Clear large objects explicitly
       result = expensive_operation()
       del payload["large_data"]  # Free memory
       return result
   ```

---

## Provider-Specific Issues

### Issue: Azure OpenAI authentication errors

**Symptoms:**
```text
Error: 401 - Authentication failed
Error: Deployment not found
```

**Solutions:**

1. **Use correct endpoint format:**
   ```python
   api_base = "https://your-resource.openai.azure.com/openai/deployments/your-deployment-name"
   # NOT just the base resource URL
   ```

2. **Set API version in headers if needed:**
   ```python
   # SimpleAgents handles this, but if overriding:
   headers = {"api-version": "2024-02-01"}
   ```

3. **Verify deployment name matches:**
   ```bash
   # In Azure Portal, check deployment name
   # Must match exactly (case-sensitive)
   ```

### Issue: Local models (Ollama/vLLM) not working

**Symptoms:**
```text
Error: Connection refused
Error: 404 Not Found
Model not found
```

**Solutions:**

1. **Verify server is running:**
   ```bash
   # Ollama
   curl http://localhost:11434/api/tags
   
   # vLLM
   curl http://localhost:8000/v1/models
   ```

2. **Use correct base URL:**
   ```python
   # Ollama OpenAI-compatible endpoint
   api_base = "http://localhost:11434/v1"
   
   # vLLM
   api_base = "http://localhost:8000/v1"
   ```

3. **Model name format:**
   ```yaml
   # Ollama - use model tag name
   model: llama2
   
   # vLLM - use full path or simplified name
   model: meta-llama/Llama-2-70b-chat-hf
   ```

4. **Check CORS (Ollama):**
   ```bash
   OLLAMA_ORIGINS="*" ollama serve
   ```

### Issue: Rate limiting from provider

**Symptoms:**
```text
Error: 429 Too Many Requests
Error: Rate limit exceeded
```

**Solutions:**

1. **SimpleAgents has built-in retries:**
   ```yaml
   # Automatic retry with exponential backoff
   node_type:
     llm_call:
       model: gpt-4.1-mini
   ```

2. **Add delays between batches:**
   ```python
   import time
   
   for item in items:
       result = client.run_workflow(...)
       time.sleep(0.5)  # Rate limit yourself
   ```

3. **Use slower tier:**
   ```yaml
   model: gpt-3.5-turbo  # Higher rate limits than GPT-4
   ```

---

## Debugging Techniques

### Enable Debug Logging

**Rust:**
```bash
RUST_LOG=debug cargo run
RUST_LOG=simple_agents=trace cargo test
```

**Python:**
```python
import logging
logging.basicConfig(level=logging.DEBUG)
```

**Tracing subscriber:**
```rust
use tracing_subscriber;
tracing_subscriber::fmt::init();
```

### Inspect Workflow Events

```python
def debug_on_event(event):
    """Print all event details for debugging"""
    print(f"\n=== Event: {event.get('event_type')} ===")
    for key, value in event.items():
        print(f"  {key}: {value}")

result = client.stream_workflow(request, on_event=debug_on_event)
```

### Validate YAML Without Running

```python
import yaml

# Load and validate structure
with open("workflow.yaml") as f:
    data = yaml.safe_load(f)
    
# Check required fields
assert "id" in data, "Missing workflow id"
assert "entry_node" in data, "Missing entry_node"
assert "nodes" in data, "Missing nodes"

# Check node references
node_ids = {n["id"] for n in data["nodes"]}
assert data["entry_node"] in node_ids, "entry_node not in nodes"

for edge in data.get("edges", []):
    assert edge["from"] in node_ids, f"Edge from '{edge['from']}' not found"
    assert edge["to"] in node_ids, f"Edge to '{edge['to']}' not found"
```

### Test Custom Workers in Isolation

```python
# Test handler independently
def test_handler():
    context = {
        "input": {"messages": [{"role": "user", "content": "test"}]},
        "nodes": {},
        "globals": {}
    }
    payload = {"company_name": "Test Corp"}
    
    result = lookup_company(context=context, payload=payload)
    print(f"Result: {result}")
    return result

test_handler()
```

---

## Getting Help

### Before Reporting an Issue

1. **Search existing issues:** https://github.com/CraftsMan-Labs/SimpleAgents/issues
2. **Check this troubleshooting guide** for your specific error
3. **Run validation commands:**
   ```bash
   make check-publish  # Full validation suite
   make test           # Run all tests
   ./scripts/run-binding-contracts.sh  # Check binding parity
   ```

### Information to Include

When reporting issues, include:

1. **Environment:**
   ```bash
   python --version  # or node --version
   rustc --version
   pip show simple-agents-py  # or npm list simple-agents-node
   ```

2. **Minimal reproduction:**
   - Smallest YAML/workflow that triggers the issue
   - Minimal code to reproduce

3. **Error output:**
   - Full error message with stack trace
   - Log output with `RUST_LOG=debug`

4. **Configuration:**
   - Redacted .env (remove API keys)
   - Workflow YAML (sanitized)

### Where to Report

- **Bugs:** https://github.com/CraftsMan-Labs/SimpleAgents/issues
- **Documentation:** Same as above with label `documentation`
- **Feature requests:** Issues with label `enhancement`

### Community Resources

- **Examples:** `examples/` directory in repo
- **Playground:** https://yamslam.craftsmanlabs.net/playground
- **Full Docs:** https://docs.simpleagents.craftsmanlabs.net/

---

## Quick Reference: Common Error Codes

| Error | Meaning | Quick Fix |
|-------|---------|-----------|
| `API key not found` | Missing credentials | Check .env file, verify `load_dotenv()` |
| `Invalid YAML` | Syntax error | Use yamllint, check indentation |
| `Node not found` | Invalid reference | Check node ID spelling |
| `Handler not found` | Custom worker missing | Verify handlers.py exists, function name matches |
| `Circular dependency` | Cycle in graph | Remove loops, ensure DAG structure |
| `Schema validation failed` | Output doesn't match schema | Enable healing, adjust prompt, check schema |
| `Stream + heal conflict` | Both enabled | Choose one per node |
| `429 Rate limit` | Too many requests | Add delays, use retries, check tier |
| `401 Unauthorized` | Bad API key | Verify key, check base URL |
| `Connection refused` | Server not running | Check local model server |

---

*Last updated: 2026-04-20*

For the latest troubleshooting information, check the [GitHub repository](https://github.com/CraftsMan-Labs/SimpleAgents).
