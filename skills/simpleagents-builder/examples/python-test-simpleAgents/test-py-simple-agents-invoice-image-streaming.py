"""Run a YAML workflow with a multimodal invoice image message.

From ``examples/``: ``uv sync`` (workspace member; ``simple-agents-py`` comes from
``examples/pyproject.toml`` → ``../crates/simple-agents-py``).
"""

from __future__ import annotations

import json
import os
import base64
from pathlib import Path

from dotenv import load_dotenv
from simple_agents_py import Client as SimpleAgentsClient
from simple_agents_py.workflow_request import (
    WorkflowExecutionRequest,
    WorkflowMessage,
    WorkflowRole,
)
from simple_agents_py.workflow_stream import WorkflowStreamEvent

load_dotenv()

workflow_file = Path(__file__).resolve().parent / "test.yaml"
image_file = Path(__file__).resolve().parent / "test-invoice.jpeg"

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


def main() -> None:
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
    )

    result = client.stream_workflow(req, on_event=default_on_event)
    print(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()