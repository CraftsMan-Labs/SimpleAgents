from __future__ import annotations

import argparse
import json
import re
from pathlib import Path
from typing import Any
from dotenv import load_dotenv

load_dotenv()
from simple_agents_py import Client, ResponseWithMetadata

from common import resolve_workflow_path
from handlers import get_rag_data
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


def interpolate_template(template: str, context: dict[str, Any]) -> str:
    output = template
    while True:
        start = output.find("{{")
        if start < 0:
            break
        end = output.find("}}", start + 2)
        if end < 0:
            break
        expr = output[start + 2 : end].strip()
        value = resolve_path(context, expr)
        if isinstance(value, (dict, list)):
            replacement = json.dumps(value)
        elif value is None:
            replacement = ""
        else:
            replacement = str(value)
        output = output[:start] + replacement + output[end + 2 :]
    return output


def apply_set_globals(
    node: dict[str, Any],
    *,
    email_text: str,
    outputs: dict[str, dict[str, Any]],
    globals_state: dict[str, Any],
) -> None:
    config = node.get("config", {})
    set_globals = config.get("set_globals")
    if not isinstance(set_globals, dict):
        return

    context = {
        "input": {"email_text": email_text},
        "nodes": outputs,
        "globals": globals_state,
    }
    for key, expr in set_globals.items():
        if not isinstance(key, str) or not isinstance(expr, str):
            continue
        globals_state[key] = resolve_path(context, expr)


def apply_update_globals(
    node: dict[str, Any],
    *,
    email_text: str,
    outputs: dict[str, dict[str, Any]],
    globals_state: dict[str, Any],
) -> None:
    config = node.get("config", {})
    update_globals = config.get("update_globals")
    if not isinstance(update_globals, dict):
        return

    context = {
        "input": {"email_text": email_text},
        "nodes": outputs,
        "globals": globals_state,
    }

    for key, spec in update_globals.items():
        if not isinstance(key, str) or not isinstance(spec, dict):
            continue
        op = str(spec.get("op", "")).strip()

        if op == "increment":
            by = spec.get("by", 1)
            try:
                by_num = float(by)
            except (TypeError, ValueError):
                by_num = 1.0
            current = globals_state.get(key, 0)
            try:
                current_num = float(current)
            except (TypeError, ValueError):
                current_num = 0.0
            globals_state[key] = current_num + by_num
            continue

        from_path = spec.get("from")
        if not isinstance(from_path, str):
            continue
        value = resolve_path(context, from_path)

        if op == "set":
            globals_state[key] = value
        elif op == "append":
            existing = globals_state.get(key)
            if not isinstance(existing, list):
                existing = [] if existing is None else [existing]
            existing.append(value)
            globals_state[key] = existing
        elif op == "merge":
            if isinstance(value, dict):
                existing = globals_state.get(key)
                if not isinstance(existing, dict):
                    existing = {}
                existing.update(value)
                globals_state[key] = existing


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

    if node_id == "generate_email_draft":
        return {
            "type": "object",
            "properties": {
                "subject": {"type": "string"},
                "body": {"type": "string"},
            },
            "required": ["subject", "body"],
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


HANDLER_REGISTRY = {
    "GetRagData": get_rag_data,
}


def run_custom_worker_handler(
    handler: str,
    topic: str,
    *,
    email_text: str,
    outputs: dict[str, dict[str, Any]],
    globals_state: dict[str, Any],
) -> dict[str, Any]:
    fn = HANDLER_REGISTRY.get(handler)
    if fn is None:
        raise RuntimeError(f"Unsupported custom worker handler: {handler}")

    return fn(
        topic,
        email_text=email_text,
        context={
            "input": {"email_text": email_text},
            "nodes": outputs,
            "globals": globals_state,
        },
    )


def run_workflow(workflow: dict[str, Any], email_text: str) -> dict[str, Any]:
    nodes = {node["id"]: node for node in workflow.get("nodes", [])}
    if not nodes:
        raise RuntimeError("Workflow has no nodes")

    edges = {edge["from"]: edge["to"] for edge in workflow.get("edges", [])}
    current = workflow.get("entry_node")
    if not isinstance(current, str) or current not in nodes:
        raise RuntimeError("Invalid or missing entry_node")

    provider, api_base, api_key, default_model = load_llm_settings()
    client = Client(provider, api_base=api_base, api_key=api_key)

    outputs: dict[str, dict[str, Any]] = {}
    globals_state: dict[str, Any] = {}
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
            context = {
                "input": {"email_text": email_text},
                "nodes": outputs,
                "globals": globals_state,
            }
            prompt = interpolate_template(str(prompt_template), context)
            schema = schema_for_node(current)
            payload = complete_structured(client, model, prompt, schema, api_base)
            outputs[current] = {"output": payload}
            apply_set_globals(
                node,
                email_text=email_text,
                outputs=outputs,
                globals_state=globals_state,
            )
            apply_update_globals(
                node,
                email_text=email_text,
                outputs=outputs,
                globals_state=globals_state,
            )
            next_node = edges.get(current)
            if next_node is None:
                break
            current = next_node
            continue

        if "switch" in node_type:
            switch = node_type["switch"]
            context = {
                "input": {"email_text": email_text},
                "nodes": outputs,
                "globals": globals_state,
            }
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
            topic = (
                node.get("config", {}).get("payload", {}).get("topic", "clarification")
            )
            outputs[current] = {
                "output": run_custom_worker_handler(
                    str(handler),
                    str(topic),
                    email_text=email_text,
                    outputs=outputs,
                    globals_state=globals_state,
                )
            }
            apply_set_globals(
                node,
                email_text=email_text,
                outputs=outputs,
                globals_state=globals_state,
            )
            apply_update_globals(
                node,
                email_text=email_text,
                outputs=outputs,
                globals_state=globals_state,
            )
            next_node = edges.get(current)
            if next_node is None:
                break
            current = next_node
            continue

        raise RuntimeError(f"Unsupported node type for node '{current}'")

    return {
        "workflow_id": workflow.get("id"),
        "entry_node": workflow.get("entry_node"),
        "email_text": email_text,
        "trace": trace,
        "outputs": outputs,
        "globals": globals_state,
        "terminal_node": trace[-1],
        "terminal_output": outputs.get(trace[-1], {}).get("output"),
    }


def main() -> None:
    args = parse_args()
    workflow_path = resolve_workflow_path(args.workflow, caller_file=__file__)
    workflow = load_workflow(workflow_path)
    result = run_workflow(workflow, args.email)
    print(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()
