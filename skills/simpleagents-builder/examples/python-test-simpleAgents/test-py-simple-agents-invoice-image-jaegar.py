"""Run a YAML workflow with a multimodal invoice image message (Jaeger OTLP).

From ``examples/``: ``uv sync`` (workspace member; ``simple-agents-py`` comes from
``examples/pyproject.toml`` → ``../crates/simple-agents-py``).

Uses standard OTLP environment variables (``SIMPLE_AGENTS_TRACING_ENABLED``,
``OTEL_EXPORTER_OTLP_*``, ``OTEL_SERVICE_NAME``) — see ``docs/OTEL_CONFIGURATION.md``.
This script enables tracing and applies Jaeger-friendly defaults (gRPC to
``http://localhost:4317``) unless ``JAEGER_OTEL=false``.

**Finding traces:** Jaeger UI (e.g. http://localhost:16686) → service
``simple-agents-workflow-invoice-image-jaeger`` (or your ``OTEL_SERVICE_NAME``) → Search.
"""

from __future__ import annotations

import json
import os
import sys
import base64
from pathlib import Path

from dotenv import load_dotenv
from simple_agents_py import Client as SimpleAgentsClient
from simple_agents_py.workflow_request import (
    WorkflowExecutionRequest,
    WorkflowMessage,
    WorkflowRole,
    WorkflowRunOptions,
    WorkflowTelemetryConfig,
)

load_dotenv()

workflow_file = Path(__file__).resolve().parent / "test.yaml"
image_file = Path(__file__).resolve().parent / "test-invoice.jpeg"


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
        os.environ["OTEL_SERVICE_NAME"] = "simple-agents-workflow-invoice-image-jaeger"

    print(
        "Jaeger OTLP: "
        f"endpoint={os.environ['OTEL_EXPORTER_OTLP_ENDPOINT']} "
        f"protocol={os.environ['OTEL_EXPORTER_OTLP_PROTOCOL']} "
        f"service={os.environ.get('OTEL_SERVICE_NAME', '')}",
        file=sys.stderr,
    )
    return True


def main() -> None:
    telemetry_on = configure_jaeger_otel_from_env()
    if not telemetry_on:
        print("Jaeger OTLP disabled (JAEGER_OTEL=false).", file=sys.stderr)

    client = SimpleAgentsClient(
        os.environ["WORKFLOW_PROVIDER"],
        api_base=os.environ["WORKFLOW_API_BASE"],
        api_key=os.environ["WORKFLOW_API_KEY"],
    )

    b64 = base64.b64encode(image_file.read_bytes()).decode("ascii")

    req = WorkflowExecutionRequest(
        workflow_path=str(workflow_file),
        messages=[
            WorkflowMessage(
                role=WorkflowRole.USER,
                content=[
                    {
                        "type": "text",
                        "text": "Invoice image. Classify and route this per workflow.",
                    },
                    {
                        "type": "image_url",
                        "image_url": {"url": f"data:image/jpeg;base64,{b64}"},
                    },
                ],
            ),
        ],
        workflow_options=WorkflowRunOptions(
            telemetry=WorkflowTelemetryConfig(
                enabled=True,
                nerdstats=True,
            ),
        )
        if telemetry_on
        else None,
    )

    result = client.run_workflow(req)
    print(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()
