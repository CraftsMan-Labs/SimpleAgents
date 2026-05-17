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
- [ ] `config.user_input_prompt` set (use `config.node_system_prompt` when needed)
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
- [ ] **Python**: handler signature uses keyword-only args: `def handler_name(*, context: dict, payload: dict):`
- [ ] **TypeScript**: `customWorkerDispatch` function passed to `runWorkflow`/`streamWorkflow` as last argument
- [ ] handler returns a JSON-serializable value

## Human Input (HITL)

- [ ] `input_type` is one of `choice`, `text`, `form`
- [ ] for `choice`: `options` list has both `value` and `label` on every entry
- [ ] for `form`: `form_schema` defines all expected fields with types and `required`
- [ ] `prompt` uses `{{ }}` templates to show prior node outputs to the human
- [ ] a `switch` node after a `choice` HITL routes on `$.nodes.<hitl_id>.output == "value"`
- [ ] the runner code handles the `awaiting_human_input` status in a loop
- [ ] resume request includes `resume=result.to_dict()` + `human_response=...`
- [ ] resume request includes the same `messages` and `input` as the initial request

## End Nodes

- [ ] terminal branches end with either an edge to another node or an explicit `end: {}` node
- [ ] `end` nodes have no outgoing edges

## Behavior

- [ ] one-question-at-a-time for interview/chat flows
- [ ] hard policy rules are explicit in prompts
- [ ] each node is single-responsibility
- [ ] no ambiguous multi-question prompts

## Runner Code (Python)

- [ ] `.env` file has `WORKFLOW_PROVIDER`, `WORKFLOW_API_BASE`, `WORKFLOW_API_KEY`
- [ ] `load_dotenv()` called before creating client
- [ ] `workflow_path` uses `Path(...).resolve()` for absolute path
- [ ] `WorkflowExecutionRequest` passed directly to `Client.run_workflow` (no dict conversion)
- [ ] for streaming: `WorkflowExecutionFlags(node_llm_streaming=True)` set
- [ ] for images: `content` is a list with `text` + `image_url` parts
- [ ] for Langfuse: OTLP env vars set + `WorkflowTelemetryConfig(enabled=True)` passed
- [ ] for Jaeger: `OTEL_EXPORTER_OTLP_ENDPOINT=http://localhost:4317` + `grpc` protocol
- [ ] for HITL: `while result_map.get("status") == "awaiting_human_input"` loop drains pauses
- [ ] response is `WorkflowRunOutput` pyclass; `.to_dict()` for JSON-safe wire format

## Eval Suite

- [ ] dataset is `.jsonl` with one JSON object per line
- [ ] each record has `id`, `input`, `expected_output`
- [ ] record `id` values are unique
- [ ] `input` has `messages` list matching what the workflow expects
- [ ] evaluator callback signature: `(case: EvalCase) -> EvalResult | dict | bool`
- [ ] evaluator returns `EvalResult.passed_result()`, `.failed()`, or `.errored()`
- [ ] `run_eval_suite` called with `workflow_path`, `dataset_path`, and `evaluator`
