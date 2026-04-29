"""Run a YAML workflow using typed WorkflowExecutionRequest (Pydantic).

From ``examples/``: ``uv sync`` (workspace member; ``simple-agents-py`` comes from
``examples/pyproject.toml`` → ``../crates/simple-agents-py``).
"""

from __future__ import annotations

import sys
from pathlib import Path

# Run as `python runners/<script>.py` — keep package root importable.
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import json

from example_env import require_env
from example_paths import workflows
from simple_agents_py import Client as SimpleAgentsClient
from simple_agents_py.workflow_payload import workflow_execution_request_to_mapping
from simple_agents_py.workflow_request import (
    WorkflowExecutionRequest,
    WorkflowMessage,
    WorkflowRole,
)

workflow_file = workflows("email-classification", "test.yaml")


def main() -> None:
    client = SimpleAgentsClient(
        require_env("WORKFLOW_PROVIDER"),
        api_base=require_env("WORKFLOW_API_BASE"),
        api_key=require_env("WORKFLOW_API_KEY"),
    )

    user_input = input("Enter your Input: ")

    req = WorkflowExecutionRequest(
        workflow_path=str(workflow_file),
        messages=[
            WorkflowMessage(role=WorkflowRole.USER, content=user_input),
        ],
    )

    result = client.run_workflow(workflow_execution_request_to_mapping(req))
    print(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()
