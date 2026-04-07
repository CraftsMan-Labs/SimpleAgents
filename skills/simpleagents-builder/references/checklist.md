# YAML Workflow QA Checklist

## Structure

- [ ] `id`, `version`, `entry_node` present
- [ ] `entry_node` exists in `nodes`
- [ ] every node has unique `id`
- [ ] all edge `from`/`to` and switch targets exist as node IDs
- [ ] `edges` cover all intended flow transitions

## LLM Nodes

- [ ] every `llm_call` includes `config.output_schema`
- [ ] schema has explicit `required`
- [ ] `additionalProperties: false` for routing-critical outputs
- [ ] prompt says `Return JSON only`
- [ ] `messages_path: input.messages` set (for conversation history)
- [ ] `append_prompt_as_user: true` set
- [ ] `stream: true` set (streaming controlled at runtime by `node_llm_streaming` flag)
- [ ] `heal: true` set (auto-fix truncated JSON)

## Routing

- [ ] switch conditions are deterministic (`==` / `!=`)
- [ ] conditions reference real output paths (`$.nodes.<id>.output.<field>`)
- [ ] default branch is intentional

## Custom Workers

- [ ] `handler` matches the actual function name
- [ ] `handler_file` specified if handler is not in the default `handlers.py` next to the YAML
- [ ] `config.payload` contains every value the handler needs (with `{{ }}` templates for node outputs)
- [ ] **Python**: handler signature is `def handler_name(context, payload):` (positional args)
- [ ] **TypeScript**: `customWorkerDispatch` function passed to `runWorkflow`/`streamWorkflow` as last argument
- [ ] handler returns a JSON-serializable value

## Behavior

- [ ] one-question-at-a-time for interview/chat flows
- [ ] hard policy rules are explicit in prompts
- [ ] each node is single-responsibility
- [ ] no ambiguous multi-question prompts

## Runner Code

- [ ] `.env` file has `WORKFLOW_PROVIDER`, `WORKFLOW_API_BASE`, `WORKFLOW_API_KEY`
- [ ] `load_dotenv()` called before creating client
- [ ] `workflow_path` uses `Path(...).resolve()` for absolute path
- [ ] for streaming: `WorkflowExecutionFlags(node_llm_streaming=True)` set
- [ ] for images: `content` is a list with `text` + `image_url` parts
- [ ] for Langfuse: OTLP env vars set + `WorkflowTelemetryConfig(enabled=True)` passed
- [ ] for Jaeger: `OTEL_EXPORTER_OTLP_ENDPOINT=http://localhost:4317` + `grpc` protocol
