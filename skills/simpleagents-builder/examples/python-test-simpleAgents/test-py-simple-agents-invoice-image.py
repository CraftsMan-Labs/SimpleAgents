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
from simple_agents_py.workflow_payload import workflow_execution_request_to_mapping
from simple_agents_py.workflow_request import (
    WorkflowExecutionRequest,
    WorkflowMessage,
    WorkflowRole,
)

load_dotenv()

workflow_file = Path(__file__).resolve().parent / "test.yaml"
image_file = Path(__file__).resolve().parent / "test-invoice.jpeg"


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

    result = client.run_workflow(workflow_execution_request_to_mapping(req))
    print(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()