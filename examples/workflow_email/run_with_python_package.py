from __future__ import annotations

import argparse
import json
import os
from pathlib import Path

from dotenv import load_dotenv
from simple_agents_py import Client


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Run YAML workflow via Python package API"
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


def load_config() -> tuple[str, str, str, str]:
    load_dotenv(Path(__file__).resolve().parents[1] / ".env")
    load_dotenv()

    provider = os.getenv("WORKFLOW_PROVIDER", "openai")
    api_base = os.getenv("WORKFLOW_API_BASE") or os.getenv("CUSTOM_API_BASE")
    api_key = os.getenv("WORKFLOW_API_KEY") or os.getenv("CUSTOM_API_KEY")
    model = os.getenv("WORKFLOW_MODEL") or os.getenv("CUSTOM_API_MODEL")

    if not api_base or not api_key or not model:
        raise RuntimeError(
            "Set WORKFLOW_API_BASE, WORKFLOW_API_KEY, WORKFLOW_MODEL (or CUSTOM_API_*)."
        )
    return provider, api_base, api_key, model


def main() -> None:
    args = parse_args()
    provider, api_base, api_key, _model = load_config()
    client = Client(provider, api_base=api_base, api_key=api_key)

    workflow_path = Path(args.workflow)
    if not workflow_path.exists():
        repo_relative = Path(__file__).resolve().parents[2] / args.workflow
        if repo_relative.exists():
            workflow_path = repo_relative
        elif args.workflow.startswith("examples/"):
            trimmed = args.workflow[len("examples/") :]
            trimmed_path = Path(__file__).resolve().parents[1] / trimmed
            if trimmed_path.exists():
                workflow_path = trimmed_path

    result = client.run_email_workflow_yaml(str(workflow_path), args.email)
    print(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()
