from __future__ import annotations

import json
import os
from datetime import datetime, timezone
from pathlib import Path
from typing import Mapping

from dotenv import load_dotenv


def load_example_env(*, caller_file: str) -> None:
    load_dotenv(Path(caller_file).resolve().parents[1] / ".env")
    load_dotenv()


def load_provider_config(*, caller_file: str) -> tuple[str, str, str]:
    load_example_env(caller_file=caller_file)

    provider = os.getenv("WORKFLOW_PROVIDER", "openai")
    api_base = os.getenv("WORKFLOW_API_BASE") or os.getenv("CUSTOM_API_BASE")
    api_key = os.getenv("WORKFLOW_API_KEY") or os.getenv("CUSTOM_API_KEY")

    if not api_base or not api_key:
        raise RuntimeError(
            "Set WORKFLOW_API_BASE and WORKFLOW_API_KEY (or CUSTOM_API_BASE/CUSTOM_API_KEY)."
        )
    return provider, api_base, api_key


def default_workflow_registry(workflow_path: Path) -> dict[str, str]:
    candidates = {
        "hr_warning_email_subgraph": workflow_path.parent
        / "hr-warning-email-subgraph.yaml",
    }
    registry: dict[str, str] = {}
    for workflow_id, path in candidates.items():
        if path.exists():
            registry[workflow_id] = str(path.resolve())
    return registry


def load_workflow_node_names(workflow_path: Path) -> dict[str, str]:
    try:
        import yaml  # type: ignore
    except Exception:
        return {}

    try:
        raw = workflow_path.read_text(encoding="utf-8")
        parsed = yaml.safe_load(raw)
    except Exception:
        return {}

    if not isinstance(parsed, dict):
        return {}

    nodes = parsed.get("nodes")
    if not isinstance(nodes, list):
        return {}

    names: dict[str, str] = {}
    for node in nodes:
        if not isinstance(node, dict):
            continue
        node_id = node.get("id")
        node_name = node.get("name")
        if isinstance(node_id, str):
            if isinstance(node_name, str) and node_name.strip():
                names[node_id] = node_name.strip()
            else:
                names[node_id] = node_id.replace("_", " ").title()
    return names


def step_display_name(node_id: str | None, node_names: Mapping[str, str]) -> str:
    if node_id is None:
        return "Workflow"
    return node_names.get(node_id, node_id.replace("_", " ").title())


def render_json(value: object, *, indent: int = 2) -> str:
    return json.dumps(value, indent=indent, ensure_ascii=True)


def render_terminal_output(terminal_output: object | None) -> str:
    if terminal_output is None:
        return ""
    if isinstance(terminal_output, str):
        return terminal_output
    return render_json(terminal_output)


def create_chat_trace_file(trace_dir: str | Path, conversation_id: str) -> Path:
    target_dir = Path(trace_dir)
    target_dir.mkdir(parents=True, exist_ok=True)
    session_id = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    return target_dir / f"chat-session-{session_id}-{conversation_id}.jsonl"
