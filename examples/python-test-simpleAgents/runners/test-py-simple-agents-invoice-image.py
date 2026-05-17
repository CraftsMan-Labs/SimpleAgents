"""Run a YAML workflow with a multimodal invoice image message.

From ``examples/``: ``uv sync`` (workspace member; ``simple-agents-py`` comes from
``examples/pyproject.toml`` → ``../crates/simple-agents-py``).
"""

from __future__ import annotations

import base64
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from example_env import require_env
from example_paths import asset, workflows
from simple_agents_py import Client as SimpleAgentsClient
from simple_agents_py.workflow_request import (
    WorkflowExecutionRequest,
    WorkflowMessage,
    WorkflowRole,
)

workflow_file = workflows("email-classification", "test.yaml")
image_file = asset("test-invoice.jpeg")


def require_file(path: Path) -> Path:
    if not path.exists():
        raise SystemExit(
            f"Required example asset is missing: {path}\n"
            "Add a small invoice JPEG at that path before running this example."
        )
    return path


def main() -> None:
    client = SimpleAgentsClient(
        require_env("WORKFLOW_PROVIDER"),
        api_base=require_env("WORKFLOW_API_BASE"),
        api_key=require_env("WORKFLOW_API_KEY"),
    )

    b64 = base64.b64encode(require_file(image_file).read_bytes()).decode("ascii")

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

    result = client.run_workflow(req)
    print(json.dumps(result.to_dict(), indent=2))


if __name__ == "__main__":
    main()