from __future__ import annotations

import json
import os
import shutil
import subprocess
from pathlib import Path
from typing import Any

import pytest  # type: ignore[reportMissingImports]

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
    if shutil.which("uv") is None:
        pytest.skip("uv binary not available")
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


def _runners_dir() -> Path:
    return _examples_dir() / "runners"


def _run_example(script_name: str, stdin_text: str | None = None) -> str:
    live_env = _resolved_live_env()
    script_path = _runners_dir() / script_name
    if not script_path.exists():
        pytest.fail(f"Example script not found: {script_path}")

    env = os.environ.copy()
    load_root_dotenv_into(env, override=False)
    env.update(live_env)

    result = subprocess.run(
        [
            "uv",
            "run",
            "--directory",
            str(_examples_dir()),
            "python",
            str(Path("runners") / script_name),
        ],
        cwd=str(_repo_root()),
        input=stdin_text,
        text=True,
        capture_output=True,
        check=False,
        timeout=180,
        env=env,
    )
    if result.returncode != 0:
        combined = f"{result.stdout}\n{result.stderr}"
        if "Invalid model name passed in model=" in combined:
            pytest.skip(
                "Live provider does not support models hardcoded in example workflow YAML"
            )
        pytest.fail(
            f"{script_name} failed with exit code {result.returncode}\n"
            f"stdout:\n{result.stdout}\n"
            f"stderr:\n{result.stderr}"
        )
    return result.stdout


def _extract_trailing_json_block(stdout: str) -> tuple[int, dict[str, Any]]:
    starts = [idx for idx, ch in enumerate(stdout) if ch == "{"]
    for start in reversed(starts):
        candidate = stdout[start:].strip()
        try:
            value = json.loads(candidate)
        except json.JSONDecodeError:
            continue
        if isinstance(value, dict):
            return start, value
    pytest.fail(f"Could not parse trailing JSON object from output tail:\n{stdout[-1500:]}")
    raise AssertionError("unreachable")


def _assert_workflow_result_shape(result: dict[str, Any]) -> None:
    assert isinstance(result.get("terminal_node"), str)
    assert isinstance(result.get("terminal_output"), (dict, str, list))
    assert isinstance(result.get("outputs"), dict)
    assert isinstance(result.get("step_timings"), list)
    assert isinstance(result.get("trace"), list)
    assert isinstance(result.get("total_elapsed_ms"), int)
    assert isinstance(result.get("workflow_id"), str)


def test_example_blocking_output_shape() -> None:
    stdout = _run_example("test-py-simple-agents.py", stdin_text=f"{_INVOICE_INPUT_TEXT}\n")
    _, result = _extract_trailing_json_block(stdout)
    _assert_workflow_result_shape(result)


def test_example_streaming_emits_chunks_and_output_shape() -> None:
    stdout = _run_example(
        "test-py-simple-agents-streaming.py",
        stdin_text=f"{_INVOICE_INPUT_TEXT}\n",
    )
    json_start, result = _extract_trailing_json_block(stdout)
    _assert_workflow_result_shape(result)

    event_positions = {
        event_type: stdout.find(event_type) for event_type in _STREAM_EVENT_TYPES
    }
    found = [event_type for event_type, idx in event_positions.items() if idx != -1]
    assert found, (
        "Expected streamed chunk events in output; found none of "
        f"{', '.join(_STREAM_EVENT_TYPES)}"
    )

    first_event_idx = min(event_positions[event_type] for event_type in found)
    assert first_event_idx < json_start, (
        "Expected at least one streaming chunk event before final JSON result. "
        f"events={found}"
    )


def test_example_invoice_image_output_shape() -> None:
    # Match ``example_paths.asset("test-invoice.jpeg")`` used by the runner script.
    image_path = _examples_dir() / "assets" / "test-invoice.jpeg"
    if not image_path.exists():
        pytest.skip(f"Missing invoice fixture image: {image_path}")

    stdout = _run_example("test-py-simple-agents-invoice-image.py")
    _, result = _extract_trailing_json_block(stdout)
    _assert_workflow_result_shape(result)
