from __future__ import annotations

import argparse
import json
import os
from pathlib import Path

from dotenv import load_dotenv
from simple_agents_py import Client


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Run all workflow_email YAML workflows via Python SDK"
    )
    parser.add_argument(
        "--email",
        default="Please help with a damaged supply order and draft the right response.",
        help="Input email text used for all workflow runs",
    )
    parser.add_argument(
        "--include-events",
        action="store_true",
        help="(unused – kept for backward compat)",
    )
    return parser.parse_args()


def load_config() -> tuple[str, str, str]:
    load_dotenv(Path(__file__).resolve().parents[2] / ".env")
    load_dotenv()

    provider = os.getenv("WORKFLOW_PROVIDER", "openai")
    api_base = os.getenv("WORKFLOW_API_BASE") or os.getenv("CUSTOM_API_BASE")
    api_key = os.getenv("WORKFLOW_API_KEY") or os.getenv("CUSTOM_API_KEY")
    if api_base is None or api_key is None:
        raise RuntimeError(
            "Set WORKFLOW_API_BASE and WORKFLOW_API_KEY (or CUSTOM_API_BASE/CUSTOM_API_KEY)."
        )
    return provider, api_base, api_key


def find_workflows() -> list[Path]:
    root = Path(__file__).resolve().parents[1]
    return sorted(root.glob("*.yaml"))


def main() -> None:
    args = parse_args()
    provider, api_base, api_key = load_config()
    client = Client(provider, api_base=api_base, api_key=api_key)

    workflows = find_workflows()
    workflow_input = {
        "email_text": args.email,
        "messages": [
            {
                "role": "system",
                "content": "You are a professional assistant for workflow testing.",
            },
            {"role": "user", "content": args.email},
        ],
    }

    summary: list[dict[str, object]] = []
    for workflow in workflows:
        try:
            result = client.run_workflow(
                {
                    "workflow_path": str(workflow),
                    "input": workflow_input,
                }
            )
            summary.append(
                {
                    "workflow": str(workflow),
                    "status": "ok",
                    "terminal_node": result.get("terminal_node"),
                    "total_elapsed_ms": result.get("total_elapsed_ms"),
                }
            )
        except Exception as error:  # noqa: BLE001
            summary.append(
                {
                    "workflow": str(workflow),
                    "status": "error",
                    "error": str(error),
                }
            )

    print(json.dumps(summary, indent=2))


if __name__ == "__main__":
    main()
