# YAML Workflow QA Checklist

## Structure

- [ ] `id`, `version`, `entry_node` present
- [ ] `entry_node` exists in `nodes`
- [ ] every node has unique `id`
- [ ] all edge and switch targets exist

## LLM Nodes

- [ ] every `llm_call` includes `config.output_schema`
- [ ] schema has explicit `required`
- [ ] `additionalProperties: false` for routing-critical outputs
- [ ] prompt says `Return JSON only`

## Routing

- [ ] switch conditions are deterministic (`==` / `!=`)
- [ ] conditions reference real output paths
- [ ] default branch is intentional

## Behavior

- [ ] one-question-at-a-time for interview/chat flows
- [ ] hard policy rules are explicit in prompts
- [ ] node responsibility is single-purpose

## Readability

- [ ] node names describe intent
- [ ] prompts avoid hidden assumptions
- [ ] enum states are concise and stable

## Custom workers

- [ ] `config.payload` contains every node-local value the handler needs (or the handler intentionally reads `context["input"]` / `context["nodes"]` instead)
- [ ] For **Python** execution, handler name matches YAML `handler` and the function uses `*, context, payload`
- [ ] If the workflow must run on **Node/Go** as shipped, avoid `custom_worker` or handle workers outside the YAML runner (no built-in executor)
