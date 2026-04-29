"""Run a YAML workflow using typed WorkflowExecutionRequest (Pydantic).

From ``examples/``: ``uv sync`` (workspace member; ``simple-agents-py`` comes from
``examples/pyproject.toml`` → ``../crates/simple-agents-py``).
"""

from __future__ import annotations

import json
from pathlib import Path

from dotenv import load_dotenv
from example_env import require_env
from simple_agents_py import Client as SimpleAgentsClient
from simple_agents_py.workflow_payload import workflow_execution_request_to_mapping
from simple_agents_py.workflow_request import (
    WorkflowExecutionRequest,
    WorkflowMessage,
    WorkflowRole,
)

load_dotenv()

workflow_file = Path(__file__).resolve().parent / "test.yaml"


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
