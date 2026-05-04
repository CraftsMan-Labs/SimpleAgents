"""Invoice image HITL example: review/edit extracted fields in a form-like loop."""

from __future__ import annotations

import base64
import json
import sys
from pathlib import Path
from typing import Any

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from example_env import require_env
from example_paths import asset, workflows
from simple_agents_py import Client as SimpleAgentsClient
from simple_agents_py.workflow_request import (
    WorkflowExecutionRequest,
    WorkflowMessage,
    WorkflowRole,
)

workflow_file = workflows("invoice-hitl", "form-feedback.yaml")
image_file = asset("test-invoice.jpeg")
form_store = workflows("invoice-hitl", "reviewed-form-log.jsonl").resolve()


def require_file(path: Path) -> Path:
    if not path.exists():
        raise SystemExit(
            f"Required example asset is missing: {path}\n"
            "Add a small invoice JPEG at that path before running this example."
        )
    return path


def _prompt_with_default(field: str, current: Any) -> str:
    value = "" if current is None else str(current)
    return input(f"{field} [{value}]: ").strip()


def edit_form_data(current: dict[str, Any]) -> dict[str, Any]:
    edited = dict(current)
    vendor_name = _prompt_with_default("vendor_name", edited.get("vendor_name"))
    if vendor_name:
        edited["vendor_name"] = vendor_name

    invoice_number = _prompt_with_default("invoice_number", edited.get("invoice_number"))
    if invoice_number:
        edited["invoice_number"] = invoice_number

    total_amount = _prompt_with_default("total_amount", edited.get("total_amount"))
    if total_amount:
        try:
            edited["total_amount"] = float(total_amount)
        except ValueError as error:
            raise SystemExit(f"Invalid total_amount '{total_amount}': {error}") from error

    currency = _prompt_with_default("currency", edited.get("currency"))
    if currency:
        edited["currency"] = currency

    due_date = _prompt_with_default("due_date", edited.get("due_date"))
    if due_date:
        edited["due_date"] = due_date

    return edited


def main() -> None:
    client = SimpleAgentsClient(
        require_env("WORKFLOW_PROVIDER"),
        api_base=require_env("WORKFLOW_API_BASE"),
        api_key=require_env("WORKFLOW_API_KEY"),
    )

    b64 = base64.b64encode(require_file(image_file).read_bytes()).decode("ascii")
    request_input = {"form_store_path": str(form_store)}

    initial_request = WorkflowExecutionRequest(
        workflow_path=str(workflow_file),
        input=request_input,
        messages=[
            WorkflowMessage(
                role=WorkflowRole.USER,
                content=[
                    {
                        "type": "text",
                        "text": "Extract invoice fields from this image.",
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
    print("Paused output:")
    print(json.dumps(paused, indent=2))

    if paused.get("status") != "awaiting_human_input":
        raise SystemExit("Expected workflow to pause for form review.")

    human_request = paused.get("human_request") or {}
    form_data = human_request.get("form_data")
    if not isinstance(form_data, dict):
        raise SystemExit("human_request.form_data is missing or invalid.")

    print("\nEdit fields. Press Enter to keep the current value.")
    edited_form = edit_form_data(form_data)
    resumed = client.run_workflow(
        WorkflowExecutionRequest(
            workflow_path=str(workflow_file),
            input=request_input,
            resume=paused,
            human_response=edited_form,
        )
    )

    metadata = (
        resumed.get("outputs", {})
        .get("review_invoice_form", {})
        .get("human_input_metadata", {})
    )
    print("Final output:")
    print(json.dumps(resumed, indent=2))
    print("Form metadata:")
    print(json.dumps(metadata, indent=2))
    print(f"Reviewed form persisted to: {form_store}")


if __name__ == "__main__":
    main()
