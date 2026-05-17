"""Invoice image HITL example: human approves or rejects extraction."""

from __future__ import annotations

import base64
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from example_env import require_env  # type: ignore[import-not-found]  # noqa: E402
from example_paths import asset, workflows  # type: ignore[import-not-found]  # noqa: E402
from simple_agents_py import Client as SimpleAgentsClient
from simple_agents_py.workflow_request import (
    WorkflowExecutionRequest,
    WorkflowMessage,
    WorkflowRole,
)

workflow_file = workflows("invoice-hitl", "approve-reject.yaml")
image_file = asset("test-invoice.jpeg")


def require_file(path: Path) -> Path:
    if not path.exists():
        raise SystemExit(
            f"Required example asset is missing: {path}\n"
            "Add a small invoice JPEG at that path before running this example."
        )
    return path


def ask_choice() -> str:
    while True:
        raw = input("Approve extraction? [approve/reject]: ").strip().lower()
        if raw in {"approve", "reject"}:
            return raw
        print("Please type 'approve' or 'reject'.")


def main() -> None:
    client = SimpleAgentsClient(
        require_env("WORKFLOW_PROVIDER"),
        api_base=require_env("WORKFLOW_API_BASE"),
        api_key=require_env("WORKFLOW_API_KEY"),
    )

    b64 = base64.b64encode(require_file(image_file).read_bytes()).decode("ascii")

    initial_request = WorkflowExecutionRequest(
        workflow_path=str(workflow_file),
        messages=[
            WorkflowMessage(
                role=WorkflowRole.USER,
                content=[
                    {
                        "type": "text",
                        "text": "Extract structured fields from this invoice image.",
                    },
                    {
                        "type": "image_url",
                        "image_url": {"url": f"data:image/jpeg;base64,{b64}"},
                    },
                ],
            )
        ],
    )

    paused = client.run_workflow(initial_request)
    paused_map = paused.to_dict()
    print("Paused output:")
    print(json.dumps(paused_map, indent=2))

    if paused_map.get("status") != "awaiting_human_input":
        raise SystemExit("Expected workflow to pause for human input.")

    decision = ask_choice()
    resumed = client.run_workflow(
        WorkflowExecutionRequest(
            workflow_path=str(workflow_file),
            resume=paused_map,
            human_response=decision,
        )
    )
    print("Final output:")
    print(json.dumps(resumed.to_dict(), indent=2))


if __name__ == "__main__":
    main()
