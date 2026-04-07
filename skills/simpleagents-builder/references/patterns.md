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

Python handler (`handlers.py` next to the YAML):

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
    return map[name] ?? "unknown";
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
