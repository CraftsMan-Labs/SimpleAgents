"""Contract fixture parity tests for Python bindings."""

from __future__ import annotations

import json
from pathlib import Path


def _fixture() -> dict:
    root = Path(__file__).resolve().parents[3]
    fixture_path = root / "parity-fixtures" / "binding_contract.json"
    return json.loads(fixture_path.read_text(encoding="utf-8"))


def _workflow_fixture() -> dict:
    root = Path(__file__).resolve().parents[3]
    fixture_path = root / "parity-fixtures" / "workflow_dsl_ir_golden.json"
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


def test_python_stub_workflow_methods_accept_workflow_options() -> None:
    root = Path(__file__).resolve().parents[1]
    stub = (root / "simple_agents_py.pyi").read_text(encoding="utf-8")

    assert "def run_email_workflow_yaml(" in stub
    assert (
        "workflow_options: WorkflowRunOptions | Mapping[str, Any] | None = None" in stub
    )
    assert "def run_workflow_yaml(" in stub


def _outgoing_edges_for_node(node: dict) -> list[str]:
    node_type = node["type"]
    if node_type == "start":
        return [node["next"]]
    if node_type in {"llm", "tool", "subgraph"}:
        return [node["next"]] if node.get("next") else []
    if node_type == "condition":
        return [node["on_true"], node["on_false"]]
    if node_type == "loop":
        return [node["body"], node["next"]]
    if node_type in {"batch", "filter", "merge", "map", "reduce"}:
        return [node["next"]]
    if node_type == "parallel":
        return [*node["branches"], node["next"]]
    if node_type == "end":
        return []
    raise AssertionError(f"unsupported node type in fixture: {node_type}")


def test_workflow_dsl_ir_golden_fixture_covers_advanced_nodes_and_wires() -> None:
    fixture = _workflow_fixture()
    dsl_nodes = fixture["workflow_dsl"]["nodes"]
    canonical_nodes = fixture["canonical_ir"]["nodes"]

    dsl_ids = set(dsl_nodes.keys())
    canonical_by_id = {node["id"]: node for node in canonical_nodes}
    assert dsl_ids == set(canonical_by_id.keys())
    assert fixture["workflow_dsl"]["entry"] in canonical_by_id

    required_node_types = set(fixture["required_node_types"])
    canonical_node_types = {node["type"] for node in canonical_nodes}
    assert canonical_node_types == required_node_types

    for wire_expectation in fixture["wire_expectations"]:
        node = canonical_by_id[wire_expectation["node_id"]]
        actual = sorted(_outgoing_edges_for_node(node))
        expected = sorted(wire_expectation["outgoing"])
        assert actual == expected

    for merge_expectation in fixture["merge_source_expectations"]:
        node = canonical_by_id[merge_expectation["node_id"]]
        assert node["type"] == "merge"
        assert sorted(node["sources"]) == sorted(merge_expectation["sources"])
