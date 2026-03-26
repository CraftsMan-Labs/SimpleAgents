# Workflow Quickstart

This guide gets you from zero to a running YAML workflow in the fewest possible steps.
By the end, you will have a workflow file, one API key configured, and one working run.

## Start Here

If you just want to get YAML running, do this:

1. Copy the example workflow below into `my-workflow.yaml`.
2. Export `OPENAI_API_KEY`.
3. Run one validation command.
4. Run one example command that executes a workflow.

After that, move to [YAML Workflow System Guide](/YAML_WORKFLOW_SYSTEM) for branching, workers, globals, and debugging.

## Prerequisites

- Rust toolchain installed
- Repository cloned locally
- One provider API key available
- `uv` installed for the Python example runner

## Step 1: Create a minimal workflow

Create `my-workflow.yaml`:

```yaml
id: hello-workflow
version: 1.0.0
entry_node: draft_reply

nodes:
  - id: draft_reply
    node_type:
      llm_call:
        model: gpt-4.1
        messages_path: input.messages
        append_prompt_as_user: true
    config:
      output_schema:
        type: object
        properties:
          reply:
            type: string
        required: [reply]
        additionalProperties: false
      prompt: |
        Write a short professional reply to the user's request.
        Return JSON only.

        {
          "reply": "..."
        }
```

Why this is the recommended first workflow:

- one node only
- no routing yet
- strict JSON output
- easy to extend later

## Step 2: Set your API key

```bash
export OPENAI_API_KEY="<your-api-key>"
```

## Step 3: Validate the workflow shape

This does not call the model. It only confirms the YAML parses as a workflow graph.

```bash
cargo run -p simple-agents-cli -- workflow mermaid my-workflow.yaml
```

If the file is valid, you will get Mermaid graph output for the workflow.

## Step 4: Run a working workflow example

The repository already includes runnable workflow examples. Use one first so you can verify your environment before wiring your own runner.

```bash
uv run --directory examples python workflow_email/run_with_python_package.py \
  --workflow examples/workflow_email/hr-warning-email-subgraph.yaml \
  --email "Please draft a warning email for repeated tardiness."
```

What success looks like:

- workflow completes without YAML validation errors
- terminal output is printed
- timing information is printed

## Step 5: Use the pattern for your own workflow

Once the example works, keep this same build order for your own YAML:

1. Start with one `llm_call` node.
2. Add `config.output_schema` before adding more logic.
3. Add a `switch` node only when you need branching.
4. Add `custom_worker` only when deterministic external logic is required.

## The Simplest Mental Model

- `llm_call` generates structured output
- `switch` routes based on previous output
- `custom_worker` runs deterministic code outside the model

If you remember only one workflow pattern, use this one:

`classifier -> switch -> action`

## Common First Steps After Quickstart

- Want branching: go to [YAML Workflow System Guide](/YAML_WORKFLOW_SYSTEM)
- Want full examples: go to [Examples](/EXAMPLES)
- Want troubleshooting help: go to [Troubleshooting](/TROUBLESHOOTING)

## Common Setup Problems

### Missing API key

```bash
echo "$OPENAI_API_KEY"
```

If this prints nothing, export the key in the same shell where you run the example.

### Want a smaller starter YAML

Use `examples/workflow_email/hr-warning-email-subgraph.yaml` as the baseline example. It is shorter and easier to understand than the multi-node chat workflows.

### Want to inspect workflow wiring first

```bash
cargo run -p simple-agents-cli -- workflow mermaid examples/workflow_email/hr-warning-email-subgraph.yaml
```
