"""
Workflow stream hooks + split_stream_deltas without mutating os.environ.

Requires ``pip install simple-agents-py[pydantic]`` (or ``[dev]`` in this repo), a provider
API key, and a resolvable ``workflow_path``.
"""

from __future__ import annotations

import os
import sys
from pathlib import Path
from typing import Any, Mapping

from simple_agents_py import Client
from simple_agents_py.workflow_request import (
    WorkflowExecutionFlags,
    WorkflowExecutionRequest,
    WorkflowInput,
    WorkflowMessage,
    WorkflowRole,
    WorkflowRunOptions,
    WorkflowTelemetryConfig,
)
from simple_agents_py.workflow_stream import stream_workflow


class Hooks:
    """Structured hooks; implement a subset or add ``on_event`` for a catch-all."""

    def on_workflow_started(self, event: Mapping[str, Any]) -> None:
        print("[start]", event.get("workflow_id", ""))

    def on_stream_output_delta(self, event: Mapping[str, Any]) -> None:
        delta = event.get("delta")
        if isinstance(delta, str):
            print(delta, end="", flush=True)

    def on_workflow_completed(self, event: Mapping[str, Any]) -> None:
        print("\n[completed]", event.get("event_type", ""))


def main() -> None:
    workflow_path = Path(
        os.environ.get(
            "SIMPLE_AGENTS_DEMO_WORKFLOW",
            "examples/workflow_email/email-intake-classification.yaml",
        )
    )
    if not workflow_path.is_file():
        print(
            f"Set SIMPLE_AGENTS_DEMO_WORKFLOW to a YAML file (missing: {workflow_path})",
            file=sys.stderr,
        )
        sys.exit(1)

    client = Client("openai")
    request = WorkflowExecutionRequest(
        workflow_path=workflow_path,
        messages=[
            WorkflowMessage(role=WorkflowRole.USER, content="Classify: short resignation note."),
        ],
        input=WorkflowInput(
            email_text="I am resigning effective Friday. Thanks for everything.",
        ),
        execution=WorkflowExecutionFlags(
            workflow_streaming=True,
            node_llm_streaming=True,
            split_stream_deltas=True,
        ),
        workflow_options=WorkflowRunOptions(
            telemetry=WorkflowTelemetryConfig(nerdstats=True),
        ),
    )
    result = stream_workflow(client, request, Hooks())
    print("\nterminal:", result.get("terminal_output"))


if __name__ == "__main__":
    main()
