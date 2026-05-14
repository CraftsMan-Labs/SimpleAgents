"""Stream a YAML workflow with live events and Langfuse OTLP (Pydantic request).

Same as ``test-py-simple-agents-streaming.py``, plus mapping ``LANGFUSE_*`` to
OpenTelemetry export settings for Langfuse.

From ``examples/``: ``uv sync`` (workspace member; local ``simple-agents-py``).

LLM nodes in the YAML should use stream: true if you want token deltas.

**Langfuse:** set ``LANGFUSE_PUBLIC_KEY``, ``LANGFUSE_SECRET_KEY``, and
``LANGFUSE_BASE_URL`` (e.g. ``http://localhost:3000``) in your shell (repo root). This script maps
them to SimpleAgents OTLP settings (``SIMPLE_AGENTS_TRACING_ENABLED``,
``OTEL_EXPORTER_OTLP_*``) as in ``docs/OTEL_CONFIGURATION.md``. Optional:
``OTEL_SERVICE_NAME`` (defaults to ``simple-agents-workflow`` in the runtime).
"""

from __future__ import annotations

import base64
import json
import os
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from example_env import require_env
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
from simple_agents_py.workflow_stream import WorkflowStreamEvent


def configure_langfuse_otel_from_env() -> None:
    """Map ``LANGFUSE_*`` into SimpleAgents OpenTelemetry exporter env (OTLP HTTP → Langfuse)."""
    public = os.environ.get("LANGFUSE_PUBLIC_KEY")
    secret = os.environ.get("LANGFUSE_SECRET_KEY")
    base = (os.environ.get("LANGFUSE_BASE_URL") or "").strip()
    if not public or not secret or not base:
        return

    token = base64.b64encode(f"{public}:{secret}".encode()).decode("ascii")
    endpoint = base.rstrip("/") + "/api/public/otel"
    os.environ["SIMPLE_AGENTS_TRACING_ENABLED"] = "true"
    os.environ["OTEL_EXPORTER_OTLP_PROTOCOL"] = "http/protobuf"
    os.environ["OTEL_EXPORTER_OTLP_ENDPOINT"] = endpoint
    os.environ["OTEL_EXPORTER_OTLP_HEADERS"] = (
        f"Authorization=Basic {token},x-langfuse-ingestion-version=4"
    )


def default_on_event(event: WorkflowStreamEvent) -> None:
    """Print streamed tokens to stdout; log structured snapshots to stderr.

    A ready-made ``on_event`` callback suitable for quick scripts and demos.
    Pass it directly wherever a callback is accepted::

        from simple_agents_py.workflow_stream import default_on_event
        client.stream_workflow(payload, on_event=default_on_event)

    Prints ``node_stream_delta``, ``node_stream_thinking_delta``, and
    ``node_stream_output_delta`` tokens inline on **stdout** (no newline between
    tokens). Emits a single line per ``node_stream_snapshot`` event on **stderr**
    (healing / structured JSON snapshot progress: node id, optional metadata, JSON
    preview). Silently ignores ``workflow_started`` and ``workflow_completed``;
    all other event types are also silently ignored by this handler.
    """
    print(event)


workflow_file = workflows("email-classification", "test.yaml")
# workflow_file = workflows("friendly", "friendly.yaml")


def main() -> None:
    configure_langfuse_otel_from_env()

    client = Client(
        require_env("WORKFLOW_PROVIDER"),
        api_base=require_env("WORKFLOW_API_BASE"),
        api_key=require_env("WORKFLOW_API_KEY"),
    )

    user_input = input("Enter your Input: ")

    req = WorkflowExecutionRequest(
        workflow_path=str(workflow_file),
        messages=[WorkflowMessage(role=WorkflowRole.USER, content=user_input)],
        execution=WorkflowExecutionFlags(
            node_llm_streaming=True,
            split_stream_deltas=False,
        ),
        workflow_options=WorkflowRunOptions(
            telemetry=WorkflowTelemetryConfig(
                enabled=True,
                nerdstats=True,
            ),
        ),
    )

    result = client.stream_workflow(
        req,
        on_event=default_on_event,
    )

    print("\n")
    print(json.dumps(result.to_dict(), indent=2))


if __name__ == "__main__":
    main()
