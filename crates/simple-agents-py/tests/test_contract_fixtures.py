"""Contract fixture parity tests for Python bindings."""

from __future__ import annotations

import json
from pathlib import Path


def _fixture() -> dict:
    root = Path(__file__).resolve().parents[3]
    fixture_path = root / "parity-fixtures" / "binding_contract.json"
    return json.loads(fixture_path.read_text(encoding="utf-8"))


def test_python_stub_contains_shared_contract_symbols() -> None:
    fixture = _fixture()
    root = Path(__file__).resolve().parents[1]
    stub = (root / "simple_agents_py.pyi").read_text(encoding="utf-8")

    for symbol in fixture["python"]["required_type_symbols"]:
        assert symbol in stub, f"simple_agents_py.pyi should include: {symbol}"

    for symbol in fixture["python"]["required_api_symbols"]:
        assert symbol in stub, f"simple_agents_py.pyi should include: {symbol}"


def test_shared_fixture_cases_are_present_and_stable() -> None:
    fixture = _fixture()
    shared_cases = fixture["shared_cases"]

    assert "request" in shared_cases
    assert "response" in shared_cases
    assert "healing" in shared_cases
    assert "streaming" in shared_cases
    assert "tool_call" in shared_cases

    assert shared_cases["request"]["completion_modes"] == [
        "standard",
        "healed_json",
        "schema",
    ]
    assert shared_cases["streaming"]["event_types"] == ["delta", "error", "done"]
    assert "tool_calls" in shared_cases["streaming"]["finish_reasons"]
