"""Stream a YAML workflow with live events (Pydantic request).

From ``examples/``: ``uv sync`` (workspace member; local ``simple-agents-py``).

LLM nodes in the YAML should use stream: true if you want token deltas.
"""

from __future__ import annotations

import os
from pathlib import Path

from dotenv import load_dotenv
from simple_agents_py import Client
from simple_agents_py.workflow_request import (
    WorkflowExecutionFlags,
    WorkflowExecutionRequest,
    WorkflowMessage,
    WorkflowRole,
)
from simple_agents_py.workflow_stream import WorkflowStreamEvent


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


load_dotenv()

workflow_file = Path(__file__).resolve().parent / "test.yaml"
# workflow_file = Path(__file__).resolve().parent / "friendly.yaml"


def main() -> None:
    client = Client(
        os.environ["WORKFLOW_PROVIDER"],
        api_base=os.environ["WORKFLOW_API_BASE"],
        api_key=os.environ["WORKFLOW_API_KEY"],
    )

    user_input = input("Enter your Input: ")

    req = WorkflowExecutionRequest(
        workflow_path=str(workflow_file),
        messages=[WorkflowMessage(role=WorkflowRole.USER, content=user_input)],
        execution=WorkflowExecutionFlags(
            node_llm_streaming=True,
            split_stream_deltas=False,
        ),
    )

    result = client.stream_workflow(
        req,
        on_event=default_on_event,
    )

    print("\n")
    import json

    print(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()
