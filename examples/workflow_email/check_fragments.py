from __future__ import annotations

from pathlib import Path
from typing import Any

import yaml


BASE_DIR = Path(__file__).resolve().parent
FRAGMENT_PATH = BASE_DIR / "fragments" / "classification_rag_topics.yaml"
WORKFLOW_PATHS = [
    BASE_DIR / "email-intake-classification.yaml",
    BASE_DIR / "email-unified-chat-intake-classification.yaml",
]


def load_yaml(path: Path) -> dict[str, Any]:
    data = yaml.safe_load(path.read_text(encoding="utf-8"))
    if not isinstance(data, dict):
        raise RuntimeError(f"expected mapping YAML at {path}")
    return data


def node_map(workflow: dict[str, Any]) -> dict[str, dict[str, Any]]:
    nodes = workflow.get("nodes")
    if not isinstance(nodes, list):
        return {}
    out: dict[str, dict[str, Any]] = {}
    for node in nodes:
        if isinstance(node, dict) and isinstance(node.get("id"), str):
            out[node["id"]] = node
    return out


def switch_targets(node: dict[str, Any]) -> dict[str, str]:
    spec = ((node.get("node_type") or {}).get("switch") or {}).get("branches") or []
    mapping: dict[str, str] = {}
    for branch in spec:
        if not isinstance(branch, dict):
            continue
        condition = branch.get("condition")
        target = branch.get("target")
        if isinstance(condition, str) and isinstance(target, str):
            marker = '== "'
            if marker in condition:
                value = condition.split(marker, 1)[1].split('"', 1)[0]
                mapping[value] = target
    default = ((node.get("node_type") or {}).get("switch") or {}).get("default")
    if isinstance(default, str):
        mapping.setdefault("clarification", default)
    return mapping


def rag_topic(node: dict[str, Any]) -> str | None:
    payload = (node.get("config") or {}).get("payload") or {}
    value = payload.get("topic")
    return value if isinstance(value, str) else None


def validate_workflow(workflow_path: Path, fragment: dict[str, Any]) -> list[str]:
    errors: list[str] = []
    workflow = load_yaml(workflow_path)
    nodes = node_map(workflow)

    expected_top = fragment.get("top_level_routes") or {}
    expected_supply = fragment.get("supply_chain_subtype_routes") or {}
    expected_termination = fragment.get("termination_subtype_routes") or {}
    expected_topics = fragment.get("rag_topics") or {}

    top_actual = switch_targets(nodes.get("route_top_level", {}))
    if top_actual != expected_top:
        errors.append(f"{workflow_path.name}: route_top_level mismatch")

    supply_actual = switch_targets(nodes.get("route_supply_chain_subtype", {}))
    if supply_actual != expected_supply:
        errors.append(f"{workflow_path.name}: route_supply_chain_subtype mismatch")

    termination_actual = switch_targets(nodes.get("route_termination_subtype", {}))
    if termination_actual != expected_termination:
        errors.append(f"{workflow_path.name}: route_termination_subtype mismatch")

    for node_id, expected_topic in expected_topics.items():
        actual = rag_topic(nodes.get(node_id, {}))
        if actual != expected_topic:
            errors.append(
                f"{workflow_path.name}: {node_id} topic mismatch (expected {expected_topic}, got {actual})"
            )

    return errors


def main() -> int:
    fragment = load_yaml(FRAGMENT_PATH)
    all_errors: list[str] = []
    for workflow_path in WORKFLOW_PATHS:
        all_errors.extend(validate_workflow(workflow_path, fragment))

    if all_errors:
        for error in all_errors:
            print(error)
        return 1

    print("OK: workflow email variants align with shared fragment mappings")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
