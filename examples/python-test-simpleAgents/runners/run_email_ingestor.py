"""Stream the email ingestor workflow (Langfuse/Jaeger tracing from repo root ``.env``).

Setup (once)::

    cd examples/python-test-simpleAgents
    uv sync

Run (must use ``uv run``, not plain ``python``)::

    cd examples/python-test-simpleAgents
    uv run python runners/run_email_ingestor.py "Please submit my expense receipts for reimbursement."

Or interactively::

    uv run python runners/run_email_ingestor.py

Environment is loaded automatically from ``<repo-root>/.env`` (see ``example_env.py``).
Required keys: ``WORKFLOW_PROVIDER``, ``WORKFLOW_API_BASE``, ``WORKFLOW_API_KEY``.
"""

from __future__ import annotations

import argparse
import json
import os
import sys
from pathlib import Path

_PACKAGE_ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(_PACKAGE_ROOT))

try:
    from example_env import env_or_default, load_monorepo_root_dotenv, require_env
    from example_paths import workflows
    from simple_agents_py import Client
    from simple_agents_py.workflow_request import (
        WorkflowExecutionFlags,
        WorkflowExecutionRequest,
        WorkflowMessage,
        WorkflowRole,
        WorkflowRunOptions,
        WorkflowTelemetryConfig,
    )
    from simple_agents_py.workflow_stream import default_on_event
except ModuleNotFoundError as exc:
    if exc.name == "simple_agents_py":
        raise SystemExit(
            "simple_agents_py is not installed in this Python environment.\n\n"
            "Install deps, then use uv run:\n"
            "  cd examples/python-test-simpleAgents\n"
            "  uv sync\n"
            "  uv run python runners/run_email_ingestor.py \"your email text here\"\n"
        ) from exc
    raise

WORKFLOW_FILE = workflows("email-ingestor", "email-ingestor.yaml")
_REPO_ROOT = _PACKAGE_ROOT.parent.parent


def configure_jaeger_otel_from_env() -> bool:
    """Enable OTLP export; return False only if ``JAEGER_OTEL`` is explicitly false."""
    if os.environ.get("JAEGER_OTEL", "").strip().lower() in ("0", "false", "no", "off"):
        return False

    os.environ["SIMPLE_AGENTS_TRACING_ENABLED"] = "true"
    if not (os.environ.get("OTEL_EXPORTER_OTLP_ENDPOINT") or "").strip():
        os.environ["OTEL_EXPORTER_OTLP_ENDPOINT"] = "http://localhost:4317"
    if not (os.environ.get("OTEL_EXPORTER_OTLP_PROTOCOL") or "").strip():
        os.environ["OTEL_EXPORTER_OTLP_PROTOCOL"] = "grpc"
    if not (os.environ.get("OTEL_SERVICE_NAME") or "").strip():
        os.environ["OTEL_SERVICE_NAME"] = "simple-agents-email-ingestor"

    print(
        "OTLP tracing: "
        f"endpoint={os.environ['OTEL_EXPORTER_OTLP_ENDPOINT']} "
        f"protocol={os.environ['OTEL_EXPORTER_OTLP_PROTOCOL']} "
        f"service={os.environ.get('OTEL_SERVICE_NAME', '')}",
        file=sys.stderr,
    )
    return True


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Classify an incoming email string with the email-ingestor workflow.",
    )
    parser.add_argument(
        "email",
        nargs="?",
        help="Email text to classify (omit to be prompted interactively)",
    )
    parser.add_argument(
        "--model",
        default=os.environ.get("WORKFLOW_MODEL", "").strip()
        or env_or_default("CUSTOM_API_MODEL", ""),
        help="Override all LLM node models (YAML default: azure/gpt-4.1-mini@eastus2). "
        "Set WORKFLOW_MODEL in repo root .env when your API router rejects the @eastus2 id.",
    )
    return parser.parse_args()


def main() -> None:
    load_monorepo_root_dotenv()
    telemetry_on = configure_jaeger_otel_from_env()
    if not telemetry_on:
        print("OTLP tracing disabled (JAEGER_OTEL=false).", file=sys.stderr)

    env_file = _REPO_ROOT / ".env"
    print(f"Env file: {env_file} ({'found' if env_file.is_file() else 'missing'})", file=sys.stderr)
    print(f"Workflow: {WORKFLOW_FILE}", file=sys.stderr)

    client = Client(
        require_env("WORKFLOW_PROVIDER"),
        api_base=require_env("WORKFLOW_API_BASE"),
        api_key=require_env("WORKFLOW_API_KEY"),
    )
    print(f"API base: {require_env('WORKFLOW_API_BASE')}", file=sys.stderr)

    args = parse_args()
    user_input = (args.email or input("Paste incoming email text: ")).strip()
    if not user_input:
        raise SystemExit("No email text provided.")

    model_override = (args.model or "").strip() or None
    if model_override:
        print(f"Model override: {model_override}", file=sys.stderr)
    else:
        print("Model: azure/gpt-4.1-mini@eastus2 (from workflow YAML)", file=sys.stderr)

    workflow_options = WorkflowRunOptions(
        model=model_override,
        telemetry=WorkflowTelemetryConfig(enabled=True, nerdstats=True),
    )
    if telemetry_on:
        pass
    elif model_override:
        workflow_options = WorkflowRunOptions(model=model_override)
    else:
        workflow_options = None

    req = WorkflowExecutionRequest(
        workflow_path=str(WORKFLOW_FILE),
        messages=[WorkflowMessage(role=WorkflowRole.USER, content=user_input)],
        execution=WorkflowExecutionFlags(
            node_llm_streaming=True,
            split_stream_deltas=False,
        ),
        workflow_options=workflow_options,
    )

    print("\n--- streaming LLM output ---", file=sys.stderr)
    result = client.stream_workflow(req, on_event=default_on_event)

    print("\n\n--- final result ---")
    print(json.dumps(result.to_dict(), indent=2))


if __name__ == "__main__":
    main()
