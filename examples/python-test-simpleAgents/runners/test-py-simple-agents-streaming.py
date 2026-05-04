"""Stream a YAML workflow with live events (Pydantic request).

From ``examples/``: ``uv sync`` (workspace member; local ``simple-agents-py``).

LLM nodes in the YAML should use stream: true if you want token deltas.
"""

from __future__ import annotations

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


workflow_file = workflows("email-classification", "test.yaml")
# workflow_file = workflows("friendly", "friendly.yaml")


def main() -> None:
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
