from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
from typing import TYPE_CHECKING

from dotenv import load_dotenv
from simple_agents_py import Client

if TYPE_CHECKING:
    from simple_agents_py import WorkflowEvent


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Run YAML workflow with live workflow event streaming"
    )
    parser.add_argument(
        "--workflow",
        default="workflow_email/email-intake-classification.yaml",
        help="Path to workflow YAML file",
    )
    parser.add_argument(
        "--email",
        default="Termination request, second warning already issued.",
        help="Incoming email text",
    )
    return parser.parse_args()


def load_config() -> tuple[str, str, str]:
    load_dotenv(Path(__file__).resolve().parents[1] / ".env")
    load_dotenv()

    provider = os.getenv("WORKFLOW_PROVIDER", "openai")
    api_base = os.getenv("WORKFLOW_API_BASE") or os.getenv("CUSTOM_API_BASE")
    api_key = os.getenv("WORKFLOW_API_KEY") or os.getenv("CUSTOM_API_KEY")

    if not api_base or not api_key:
        raise RuntimeError(
            "Set WORKFLOW_API_BASE and WORKFLOW_API_KEY (or CUSTOM_API_BASE/CUSTOM_API_KEY)."
        )
    return provider, api_base, api_key


def resolve_workflow_path(raw_path: str) -> Path:
    candidate = Path(raw_path)
    if candidate.exists():
        return candidate

    repo_relative = Path(__file__).resolve().parents[2] / raw_path
    if repo_relative.exists():
        return repo_relative

    if raw_path.startswith("examples/"):
        trimmed = raw_path[len("examples/") :]
        fallback = Path(__file__).resolve().parents[1] / trimmed
        if fallback.exists():
            return fallback

    raise FileNotFoundError(f"Workflow file not found: {raw_path}")


def on_event(event: WorkflowEvent) -> None:
    event_type = event.get("event_type")
    node_id = event.get("node_id")
    message = event.get("message")
    delta = event.get("delta")
    metadata = event.get("metadata")

    if event_type == "node_stream_delta" and delta:
        print(f"[stream:{node_id}] {delta}", end="", flush=True)
        return

    if event_type == "node_llm_input_resolved" and isinstance(metadata, dict):
        model = metadata.get("model")
        prompt = metadata.get("prompt")
        bindings = metadata.get("bindings")
        if isinstance(bindings, list):
            sources = [
                str(item.get("source_path"))
                for item in bindings
                if isinstance(item, dict) and item.get("source_path") is not None
            ]
            print(
                f"[llm-input] node={node_id} model={model} sources={','.join(sources)}"
            )
            if isinstance(prompt, str):
                print(f"[llm-prompt:{node_id}]\n{prompt}")
            return

    print(f"[event] type={event_type} node={node_id} msg={message}")


def main() -> None:
    args = parse_args()
    provider, api_base, api_key = load_config()
    client = Client(provider, api_base=api_base, api_key=api_key)

    workflow_path = resolve_workflow_path(args.workflow)
    output = client.run_email_workflow_yaml_stream(
        str(workflow_path), args.email, on_event=on_event
    )

    print("\n\n--- Final Output ---")
    print(json.dumps(output, indent=2))


if __name__ == "__main__":
    main()
