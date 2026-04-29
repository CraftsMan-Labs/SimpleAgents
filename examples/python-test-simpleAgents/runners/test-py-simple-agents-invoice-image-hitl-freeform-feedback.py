"""Invoice image HITL: free-form feedback saved by custom worker."""

from __future__ import annotations

import base64
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from example_env import require_env  # type: ignore[import-not-found]  # noqa: E402
from example_paths import asset, workflows  # type: ignore[import-not-found]  # noqa: E402
from simple_agents_py import Client as SimpleAgentsClient

workflow_file = workflows("invoice-hitl", "freeform-feedback.yaml")
image_file = asset("test-invoice.jpeg")
feedback_store = workflows(
    "invoice-hitl", "reviewer-feedback-log.jsonl"
).resolve()


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
    request_input = {"feedback_store_path": str(feedback_store)}

    initial_request = {
        "workflow_path": str(workflow_file),
        "input": request_input,
        "messages": [
            {
                "role": "user",
                "content": [
                    {
                        "type": "text",
                        "text": "Extract invoice fields from this image.",
                    },
                    {
                        "type": "image_url",
                        "image_url": {"url": f"data:image/jpeg;base64,{b64}"},
                    },
                ],
            }
        ],
    }

    paused = client.run_workflow(initial_request)
    print("Paused output:")
    print(json.dumps(paused, indent=2))

    if paused.get("status") != "awaiting_human_input":
        raise SystemExit("Expected workflow to pause for human text feedback.")

    feedback = input("Reviewer feedback: ").strip()
    resumed = client.run_workflow(
        {
            "workflow_path": str(workflow_file),
            "input": request_input,
            "resume": paused,
            "human_response": feedback,
        }
    )
    print("Final output:")
    print(json.dumps(resumed, indent=2))
    print(f"Feedback persisted to: {feedback_store}")


if __name__ == "__main__":
    main()
