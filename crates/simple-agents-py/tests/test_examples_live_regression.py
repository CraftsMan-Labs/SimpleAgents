from __future__ import annotations

import base64
import os
from pathlib import Path
from typing import Any

import pytest  # type: ignore[reportMissingImports]
from simple_agents_py import Client as SimpleAgentsClient
from simple_agents_py.workflow_request import (
    WorkflowExecutionFlags,
    WorkflowExecutionRequest,
    WorkflowMessage,
    WorkflowRole,
)

from repo_dotenv import load_root_dotenv_into

_STREAM_EVENT_TYPES = (
    "node_stream_delta",
    "node_stream_thinking_delta",
    "node_stream_output_delta",
)

_INVOICE_INPUT_TEXT = (
    "Seller Google, 245 Market Street, Suite 800 San Francisco, CA 94105, USA "
    "EIN: 12-3456789 Sales Tax Permit: CA-987654321 Bill To Northwind Retail Inc. "
    "890 Madison Ave New York, NY 10022, USA Invoice Details Invoice Number: INV-2026-104 "
    "Invoice Date: March 26, 2026 Due Date: April 9, 2026 Payment Terms: Net 14 "
    "Description Qty Unit Price Amount Website development services 20 hrs $75.00 $1,500.00 "
    "UI design revisions 5 hrs $60.00 $300.00 Hosting setup fee 1 $120.00 $120.00 "
    "Subtotal: $1,920.00 Sales Tax (8.25%): $158.40 Total Due: $2,078.40 "
    "Payment Method Bank Transfer / ACH Account Name: Google Bank: First National Bank "
    "Notes Thank you for your business. Please include the invoice number with your payment. "
    "Copyable version Seller: Google Buyer: Northwind Retail Inc. Invoice No: INV-2026-104 "
    "Date: March 26, 2026 Due: April 9, 2026 Website development services - $1,500.00 "
    "UI design revisions - $300.00 Hosting setup fee - $120.00 Subtotal: $1,920.00 "
    "Sales Tax: $158.40 Total Due: $2,078.40"
)


def _resolved_live_env() -> dict[str, str]:
    load_root_dotenv_into(os.environ, override=False)
    provider = os.getenv("WORKFLOW_PROVIDER") or os.getenv("CUSTOM_PROVIDER") or "openai"
    api_base = os.getenv("WORKFLOW_API_BASE") or os.getenv("CUSTOM_API_BASE")
    api_key = os.getenv("WORKFLOW_API_KEY") or os.getenv("CUSTOM_API_KEY")
    missing = []
    if not api_base:
        missing.append("WORKFLOW_API_BASE (or CUSTOM_API_BASE)")
    if not api_key:
        missing.append("WORKFLOW_API_KEY (or CUSTOM_API_KEY)")
    if missing:
        pytest.skip(f"Missing live workflow env vars: {', '.join(missing)}")
    assert api_base is not None
    assert api_key is not None
    return {
        "WORKFLOW_PROVIDER": provider,
        "WORKFLOW_API_BASE": api_base,
        "WORKFLOW_API_KEY": api_key,
    }


def _repo_root() -> Path:
    return Path(__file__).resolve().parents[3]


def _examples_dir() -> Path:
    return _repo_root() / "examples" / "python-test-simpleAgents"


def _workflow_path() -> Path:
    return _examples_dir() / "workflows" / "email-classification" / "test.yaml"


def _client() -> SimpleAgentsClient:
    live_env = _resolved_live_env()
    return SimpleAgentsClient(
        live_env["WORKFLOW_PROVIDER"],
        api_base=live_env["WORKFLOW_API_BASE"],
        api_key=live_env["WORKFLOW_API_KEY"],
    )


def _run_workflow(req: WorkflowExecutionRequest) -> Any:
    client = _client()
    try:
        return client.run_workflow(req)
    except Exception as error:
        if "Invalid model name passed in model=" in str(error):
            pytest.skip(
                "Live provider does not support models hardcoded in example workflow YAML"
            )
        raise


def _stream_workflow(
    req: WorkflowExecutionRequest,
    *,
    on_event: Any,
) -> Any:
    client = _client()
    try:
        return client.stream_workflow(req, on_event=on_event)
    except Exception as error:
        if "Invalid model name passed in model=" in str(error):
            pytest.skip(
                "Live provider does not support models hardcoded in example workflow YAML"
            )
        raise


def _assert_output_schema_contract(terminal_output: Any) -> None:
    assert isinstance(terminal_output, dict)
    assert isinstance(terminal_output.get("top_level_category"), str)
    assert isinstance(terminal_output.get("subtype"), str)
    assert isinstance(terminal_output.get("label"), str)
    assert isinstance(terminal_output.get("reason"), str)

    top_level_category = terminal_output["top_level_category"]
    subtype = terminal_output["subtype"]
    label = terminal_output["label"]

    if top_level_category == "hr":
        assert subtype == "general"
        assert label == "hr/general"
        return
    if top_level_category == "education":
        assert subtype == "general"
        assert label == "education/general"
        return
    if top_level_category == "finance":
        assert label.startswith("finance/")
        if subtype == "invoice":
            assert label == "finance/invoice"
            assert isinstance(terminal_output.get("company_name"), str)
            assert isinstance(terminal_output.get("stakeholder_name"), str)
        return

    pytest.fail(f"Unexpected top_level_category value: {top_level_category!r}")


def _assert_workflow_result_shape(result: Any) -> None:
    assert isinstance(result.status, str)
    assert result.status in ("completed", "awaiting_human_input")
    assert isinstance(result.terminal_node, str)
    assert isinstance(result.terminal_output, dict)
    assert isinstance(result.outputs, dict)
    assert isinstance(result.step_timings, list)
    assert isinstance(result.trace, list)
    assert isinstance(result.total_elapsed_ms, int)
    assert isinstance(result.workflow_id, str)
    _assert_output_schema_contract(result.terminal_output)


def test_example_blocking_output_shape() -> None:
    req = WorkflowExecutionRequest(
        workflow_path=str(_workflow_path()),
        messages=[WorkflowMessage(role=WorkflowRole.USER, content=_INVOICE_INPUT_TEXT)],
    )
    result = _run_workflow(req)
    _assert_workflow_result_shape(result)


def test_example_streaming_emits_chunks_and_output_shape() -> None:
    events: list[dict[str, object]] = []

    def on_event(event: dict[str, object]) -> None:
        events.append(event)

    req = WorkflowExecutionRequest(
        workflow_path=str(_workflow_path()),
        messages=[WorkflowMessage(role=WorkflowRole.USER, content=_INVOICE_INPUT_TEXT)],
        execution=WorkflowExecutionFlags(
            node_llm_streaming=True,
            split_stream_deltas=False,
        ),
    )

    result = _stream_workflow(req, on_event=on_event)
    _assert_workflow_result_shape(result)

    event_types = {str(event.get("event_type")) for event in events}
    found = [event_type for event_type in _STREAM_EVENT_TYPES if event_type in event_types]
    assert found, (
        "Expected streamed chunk events; found none of "
        f"{', '.join(_STREAM_EVENT_TYPES)}"
    )


def test_example_invoice_image_output_shape() -> None:
    image_path = _examples_dir() / "assets" / "test-invoice.jpeg"
    if not image_path.exists():
        pytest.skip(f"Missing invoice fixture image: {image_path}")

    b64 = base64.b64encode(image_path.read_bytes()).decode("ascii")
    req = WorkflowExecutionRequest(
        workflow_path=str(_workflow_path()),
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
            )
        ],
    )
    result = _run_workflow(req)
    _assert_workflow_result_shape(result)
