from __future__ import annotations

import argparse
import json
import re
from pathlib import Path
from typing import Any

from simple_agents_py import Client, ResponseWithMetadata

from python_email_workflow_demo import load_llm_settings

try:
    import yaml
except ImportError as exc:  # pragma: no cover - dependency guidance
    raise RuntimeError(
        "PyYAML is required. Install with `uv add pyyaml` in examples/."
    ) from exc


CONDITION_RE = re.compile(r'^\$\.(?P<path>[\w\.]+)\s*==\s*"(?P<value>[^"]+)"$')


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Run workflow YAML email example")
    parser.add_argument("workflow", help="YAML workflow file path")
    parser.add_argument(
        "--email",
        default="Please process supply chain replacement, order 9921 arrived damaged.",
        help="Incoming email text",
    )
    return parser.parse_args()


def resolve_workflow_path(raw_path: str) -> Path:
    candidate = Path(raw_path)
    if candidate.exists():
        return candidate

    local = Path(__file__).resolve().parent / raw_path
    if local.exists():
        return local

    raise FileNotFoundError(f"Workflow file not found: {raw_path}")


def load_workflow(path: Path) -> dict[str, Any]:
    with path.open("r", encoding="utf-8") as handle:
        parsed = yaml.safe_load(handle)
    if not isinstance(parsed, dict):
        raise RuntimeError("Workflow YAML must parse to an object")
    return parsed


def resolve_path(root: dict[str, Any], dotted_path: str) -> Any:
    current: Any = root
    for segment in dotted_path.split("."):
        if not isinstance(current, dict) or segment not in current:
            return None
        current = current[segment]
    return current


def evaluate_condition(condition: str, context: dict[str, Any]) -> bool:
    match = CONDITION_RE.match(condition.strip())
    if not match:
        raise RuntimeError(f"Unsupported switch condition format: {condition}")

    left_value = resolve_path(context, match.group("path"))
    right_value = match.group("value")
    return str(left_value) == right_value


def schema_for_node(node_id: str) -> dict[str, Any]:
    if node_id == "classify_top_level":
        return {
            "type": "object",
            "properties": {
                "category": {
                    "type": "string",
                    "enum": [
                        "probation",
                        "termination",
                        "leave_request",
                        "supply_chain_request",
                        "clarification",
                    ],
                },
                "reason": {"type": "string"},
            },
            "required": ["category", "reason"],
            "additionalProperties": False,
        }

    if node_id in {"classify_supply_chain_subtype", "classify_termination_subtype"}:
        enum_values = (
            ["order_assessment", "order_replacement", "clarification"]
            if node_id == "classify_supply_chain_subtype"
            else ["first_time_offense", "repeated_offense", "clarification"]
        )
        return {
            "type": "object",
            "properties": {
                "subtype": {"type": "string", "enum": enum_values},
                "reason": {"type": "string"},
            },
            "required": ["subtype", "reason"],
            "additionalProperties": False,
        }

    raise RuntimeError(f"No schema mapping defined for llm node: {node_id}")


def complete_structured(
    client: Client,
    model: str,
    prompt: str,
    schema: dict[str, Any],
    api_base: str,
) -> dict[str, Any]:
    messages: list[dict[str, object]] = [
        {"role": "system", "content": "You execute workflow classification steps."},
        {"role": "user", "content": prompt},
    ]
    try:
        result = client.complete(
            model, messages, schema=schema, schema_name="workflow_step"
        )
    except Exception as error:
        raise RuntimeError(
            "Failed to call LLM provider. "
            f"Check CUSTOM_API_BASE/CUSTOM_API_KEY/CUSTOM_API_MODEL (base={api_base})."
        ) from error

    if isinstance(result, ResponseWithMetadata):
        raw = result.content
    elif isinstance(result, str):
        raw = result
    else:
        raise RuntimeError(f"Unexpected response type: {type(result).__name__}")

    payload = json.loads(raw)
    if not isinstance(payload, dict):
        raise RuntimeError("Structured response must be an object")
    return payload


def mock_rag(topic: str) -> dict[str, str]:
    data = {
        "probation": (
            "hr_policy/probation.md",
            "Collect manager review, performance evidence, and probation timeline.",
        ),
        "leave_request": (
            "hr_policy/leave.md",
            "Validate leave balance, manager approval, and blackout dates.",
        ),
        "supply_chain_order_assessment": (
            "supply_chain/order_assessment.md",
            "Review order specs, inventory risk, and vendor lead-time guidance.",
        ),
        "supply_chain_order_replacement": (
            "supply_chain/order_replacement.md",
            "Collect order id, damage proof, and replacement SLA policy.",
        ),
        "termination_first_time_offense": (
            "hr_policy/termination_first_offense.md",
            "Validate first-incident criteria and route to HRBP review.",
        ),
        "termination_repeated_offense": (
            "hr_policy/termination_repeated_offense.md",
            "Collect prior warnings and escalation approvals before final action.",
        ),
        "clarification": (
            "shared/request_clarification.md",
            "Request clarifying details before routing.",
        ),
    }
    kb_source, playbook = data.get(topic, data["clarification"])
    return {"kb_source": kb_source, "playbook": playbook}


def run_workflow(workflow: dict[str, Any], email_text: str) -> dict[str, Any]:
    nodes = {node["id"]: node for node in workflow.get("nodes", [])}
    if not nodes:
        raise RuntimeError("Workflow has no nodes")

    edges = {edge["from"]: edge["to"] for edge in workflow.get("edges", [])}
    current = workflow.get("entry_node")
    if not isinstance(current, str) or current not in nodes:
        raise RuntimeError("Invalid or missing entry_node")

    api_base, api_key, default_model = load_llm_settings()
    client = Client("openai", api_base=api_base, api_key=api_key)

    outputs: dict[str, dict[str, Any]] = {}
    trace: list[str] = []

    while True:
        node = nodes[current]
        trace.append(current)
        node_type = node.get("node_type", {})

        if "llm_call" in node_type:
            node_model = node_type["llm_call"].get("model")
            if isinstance(node_model, str) and node_model.strip():
                model = node_model.strip()
            else:
                model = default_model
            prompt_template = node.get("config", {}).get("prompt", "")
            prompt = str(prompt_template).replace("{{ input.email_text }}", email_text)
            schema = schema_for_node(current)
            payload = complete_structured(client, model, prompt, schema, api_base)
            outputs[current] = {"output": payload}
            next_node = edges.get(current)
            if next_node is None:
                break
            current = next_node
            continue

        if "switch" in node_type:
            switch = node_type["switch"]
            context = {"input": {"email_text": email_text}, "nodes": outputs}
            next_node = switch.get("default")
            for branch in switch.get("branches", []):
                condition = branch.get("condition")
                target = branch.get("target")
                if (
                    isinstance(condition, str)
                    and isinstance(target, str)
                    and evaluate_condition(condition, context)
                ):
                    next_node = target
                    break
            if not isinstance(next_node, str):
                raise RuntimeError(f"Switch node '{current}' has no valid next target")
            current = next_node
            continue

        if "custom_worker" in node_type:
            handler = node_type["custom_worker"].get("handler")
            if handler != "GetRagData":
                raise RuntimeError(f"Unsupported custom worker handler: {handler}")
            topic = (
                node.get("config", {}).get("payload", {}).get("topic", "clarification")
            )
            outputs[current] = {"output": mock_rag(str(topic))}
            break

        raise RuntimeError(f"Unsupported node type for node '{current}'")

    return {
        "workflow_id": workflow.get("id"),
        "entry_node": workflow.get("entry_node"),
        "email_text": email_text,
        "trace": trace,
        "outputs": outputs,
        "terminal_node": trace[-1],
        "terminal_output": outputs.get(trace[-1], {}).get("output"),
    }


def main() -> None:
    args = parse_args()
    workflow_path = resolve_workflow_path(args.workflow)
    workflow = load_workflow(workflow_path)
    result = run_workflow(workflow, args.email)
    print(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()
