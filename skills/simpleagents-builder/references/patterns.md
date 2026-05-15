# YAML Workflow Patterns

## 1) Detect -> Route -> Act

The default architecture for any workflow. Taken from `examples/python-test-simpleAgents/test.yaml`.

1. `detect_*` (`llm_call`) classifies input with strict enum schema
2. `route_*` (`switch`) branches deterministically on the output
3. `act_*` (`llm_call` or `custom_worker`) produces the result

```yaml
nodes:
  - id: detect_email_domain
    node_type:
      llm_call:
        model: azure/gpt-4.1-mini
        temperature: 0.7
        messages_path: input.messages
        append_prompt_as_user: true
        stream: true
        heal: true
    config:
      output_schema:
        type: object
        properties:
          domain:
            type: string
            enum: [hr, finance, education]
          reason:
            type: string
        required: [domain, reason]
        additionalProperties: false
      prompt: |
        Classify the email into one domain.
        Return JSON only: {"domain": "hr" | "finance" | "education", "reason": "..."}

  - id: route_email_domain
    node_type:
      switch:
        branches:
          - condition: '$.nodes.detect_email_domain.output.domain == "finance"'
            target: detect_finance_subtype
        default: finalize

edges:
  - from: detect_email_domain
    to: route_email_domain
```

## 2) LLM Node Best Practices

Always set on every `llm_call`:

- `messages_path: input.messages` -- pass conversation history
- `append_prompt_as_user: true` -- inject the node prompt as a user message
- `stream: true` -- enable streaming (controlled at runtime by `node_llm_streaming` flag)
- `heal: true` -- auto-fix truncated JSON
- `stream_json_as_text: false` -- set to `true` only when you want raw text deltas for structured output

Always define `config.output_schema` with:

- `type: object`
- narrow `properties`
- strict `required`
- `additionalProperties: false`

## 3) Custom Worker Pattern

From `examples/python-test-simpleAgents/test.yaml` + `handlers.py`.

Use `custom_worker` for deterministic code (DB lookups, API calls, business logic).

YAML:

```yaml
- id: lookup_invoice_stakeholder
  node_type:
    custom_worker:
      handler: get_seller_name
  config:
    payload:
      company_name: "{{ nodes.extract_invoice_company_name.output.company_name }}"
```

Python handler (`handlers.py` next to the YAML) -- **keyword-only** signature:

```python
def get_seller_name(*, context: dict, payload: dict) -> dict:
    company_name = str(payload.get("company_name", "")).strip().lower()
    stakeholder_map = {
        "google": "Sundar Pichai",
        "microsoft": "Satya Nadella",
        "apple": "Tim Cook",
        "amazon": "Andy Jassy",
    }
    return {"stakeholder": stakeholder_map.get(company_name, "unknown")}
```

TypeScript handler (pass as `customWorkerDispatch` to `runWorkflow`/`streamWorkflow`):

```typescript
export function customWorkerDispatch(req: {
  handler: string;
  payload: unknown;
  context: unknown;
}): string {
  if (req.handler === "get_seller_name") {
    const p = req.payload as Record<string, unknown>;
    const name = String(p.company_name ?? "").trim().toLowerCase();
    const map: Record<string, string> = {
      google: "Sundar Pichai",
      microsoft: "Satya Nadella",
      apple: "Tim Cook",
      amazon: "Andy Jassy",
    };
    return JSON.stringify({ stakeholder: map[name] ?? "unknown" });
  }
  throw new Error(`unknown handler: ${req.handler}`);
}
```

## 4) Templating -- Reference Previous Outputs

In prompts:

```yaml
prompt: |
  Finance subtype reason: {{ nodes.detect_finance_subtype.output.reason }}
  Extracted company: {{ nodes.extract_invoice_company_name.output.company_name }}
  Stakeholder lookup: {{ nodes.lookup_invoice_stakeholder.output }}
```

In custom worker payloads:

```yaml
config:
  payload:
    company_name: "{{ nodes.extract_invoice_company_name.output.company_name }}"
```

Reference `input.*` for workflow-level input:

```yaml
prompt: |
  SPEC TEXT: {{ input.spec_text }}
  Bundle version: {{ input.bundle_version }}
```

## 5) Hierarchical Classification (Multi-Level Routing)

From `examples/python-test-simpleAgents/test.yaml`. Classify at top level, then sub-classify within a branch:

```
detect_email_domain -> route_email_domain
                         |-- "hr" -> finalize_hr
                         |-- "finance" -> detect_finance_subtype -> route_finance_subtype
                         |                                           |-- "invoice" -> extract_company -> lookup_stakeholder -> finalize_invoice
                         |                                           |-- default -> finalize_finance
                         |-- default -> finalize_education
```

## 6) Image/Multimodal Input

No YAML changes needed. Images are passed as multimodal message content from the runner:

Python:

```python
WorkflowMessage(
    role=WorkflowRole.USER,
    content=[
        {"type": "text", "text": "Classify this invoice."},
        {"type": "image_url", "image_url": {"url": f"data:image/jpeg;base64,{b64}"}},
    ],
)
```

TypeScript:

```typescript
const messages: MessageInput[] = [{
  role: "user",
  content: [
    { type: "text", text: "Classify this invoice." },
    { type: "image", mediaType: "image/jpeg", data: b64 },
  ],
}];
```

## 7) Execution Flags (Runtime)

Control streaming and healing at the request level:

| Flag | Default | What it does |
|---|---|---|
| `node_llm_streaming` | `true` | Master switch: `stream = yaml.stream AND this flag` |
| `split_stream_deltas` | `false` | Separate thinking vs output deltas |
| `healing` | `false` | Global healing: `heal = yaml.heal OR this flag` |
| `workflow_streaming` | `false` | Forward token deltas to event sink |
| `debug_stream_parse` | `false` | Append partial LLM text to JSON parse errors |

Python:

```python
WorkflowExecutionFlags(
    node_llm_streaming=True,
    split_stream_deltas=False,
)
```

TypeScript:

```typescript
const executionFlags = {
  nodeLlmStreaming: true,
  splitStreamDeltas: false,
};
```

## 8) Human Input (HITL) Patterns

Use `human_input` nodes to pause the workflow for human approval, review, or feedback. The workflow returns `status: "awaiting_human_input"` with a `human_request` object. The host resumes with `resume` + `human_response`.

### Choice (approve/reject)

```yaml
- id: review_extraction
  node_type:
    human_input:
      input_type: choice
      prompt: |
        Review extracted invoice:
        vendor={{ nodes.extract_invoice.output.vendor_name }},
        total={{ nodes.extract_invoice.output.total_amount }}.
        Approve or reject?
      options:
        - value: approve
          label: Approve
        - value: reject
          label: Reject
```

Route on choice output with a switch:

```yaml
- id: route_review
  node_type:
    switch:
      branches:
        - condition: '$.nodes.review_extraction.output == "approve"'
          target: handle_approval
      default: handle_rejection
```

### Text (free-form)

```yaml
- id: collect_follow_up
  node_type:
    human_input:
      input_type: text
      prompt: |
        {{ input.follow_up_instruction }}
        Target question ids: {{ input.unclear_question_ids_json }}
```

### Form (structured fields)

```yaml
- id: collect_initial_answers
  node_type:
    human_input:
      input_type: form
      prompt: |
        Answer each question clearly.
        Questions (JSON): {{ nodes.draft_questions.output.questions }}
      form_schema:
        type: object
        properties:
          answer_q01: { type: string }
          answer_q02: { type: string }
        required: [answer_q01, answer_q02]
        additionalProperties: false
```

### Resume protocol (Python)

```python
result = client.run_workflow(initial_request)
result_map = result.to_dict()

while result_map.get("status") == "awaiting_human_input":
    hr = result_map.get("human_request") or {}
    # Collect response based on hr["input_type"]
    response = collect_human_input(hr)

    result = client.run_workflow(
        WorkflowExecutionRequest(
            workflow_path=initial_request.workflow_path,
            messages=list(initial_request.messages),
            input=initial_request.input,
            resume=result_map,
            human_response=response,
        )
    )
    result_map = result.to_dict()
```

### End node

Explicitly terminate a branch (no further edges needed):

```yaml
- id: pipeline_complete
  node_type:
    end: {}

edges:
  - from: last_node
    to: pipeline_complete
```

## 9) Eval Suite Pattern

Test workflows with golden datasets using `EvalSuiteRequest` + evaluator callbacks.

### Dataset format (JSONL)

```json
{"id": "case_001", "input": {"messages": [{"role": "user", "content": "Invoice from Google for $50k"}]}, "expected_output": {"terminal_output": {"category": "finance"}}}
{"id": "case_002", "input": {"messages": [{"role": "user", "content": "New hire onboarding"}]}, "expected_output": {"terminal_output": {"category": "hr"}}}
```

### Evaluator callback

```python
from simple_agents_py.eval_request import EvalCase, EvalResult
from simple_agents_py.evals import run_eval_suite

def my_evaluator(case: EvalCase) -> EvalResult:
    expected = case.expected_output.get("terminal_output")
    actual = case.actual_output.get("terminal_output")
    if expected == actual:
        return EvalResult.passed_result(id="exact_match")
    return EvalResult.failed("mismatch", id="exact_match", expected=expected, actual=actual)

report = run_eval_suite(
    client,
    workflow_path="workflow.yaml",
    dataset_path="eval_dataset.jsonl",
    evaluator=my_evaluator,
)
print(f"Pass rate: {report.summary.pass_rate:.0%}")
```

### Built-in evaluators

- `terminal_output_exact` -- exact match on `terminal_output`
- `terminal_node_exact` -- exact match on `terminal_node` (routing correctness)
- `output_subset` -- expected is a subset of actual (partial match)

## 10) Multi-Workflow Pipeline Pattern

Chain multiple YAML workflows with host-driven orchestration. Each workflow runs to completion (draining HITL pauses), then passes structured output to the next.

```
WF1 (interview, form HITL)
  → host extracts review, answers
  → if needs_clarification → WF2 loop (clarify, text HITL, up to N rounds)
  → WF3 (usefulness assessment, no HITL)
```

Key: each workflow is a standalone YAML. The host Python script:
1. Builds `WorkflowExecutionRequest` with `input=` carrying prior outputs
2. Calls `run_workflow_until_complete()` (drains HITL)
3. Extracts node outputs from `result["outputs"][node_id]["output"]`
4. Feeds extracted data into the next workflow's `input`

## 11) Typed API Surface (Python)

**Requests:** Pass `WorkflowExecutionRequest` (Pydantic) directly to `Client.run_workflow` / `stream_workflow`. Plain dicts are rejected.

**Responses:** `Client.run_workflow` returns `WorkflowRunOutput` (Rust pyclass). Use `.to_dict()` for `WorkflowRunOutputWire` (TypedDict, JSON-safe).

Key properties on `WorkflowRunOutput`:
- `.status` -- `"completed"` or `"awaiting_human_input"`
- `.human_request` -- `HumanRequest` pyclass (node_id, input_type, prompt, options, form_schema, form_data)
- `.output` -- terminal node output (shortcut)
- `.outputs` -- all node outputs
- `.terminal_node` -- ID of the last executed node
- `.step_timings` -- per-node timing and token counts
- `.total_elapsed_ms`, `.total_tokens`, `.trace_id`

**Streaming:** `Client.stream_workflow(request, on_event=callback)` returns `WorkflowRunOutput`. Use `default_on_event` for quick demos, or `workflow_event_callback(hooks)` for structured dispatch.

**Evals:** `run_eval_suite(client, workflow_path=..., dataset_path=..., evaluator=fn)` returns `EvalReport`.
