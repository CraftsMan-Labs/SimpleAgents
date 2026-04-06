"""workflow_execution_request_to_mapping and stream_workflow request coercion."""

from __future__ import annotations

import unittest.mock

import pytest

from simple_agents_py.workflow_payload import workflow_execution_request_to_mapping
from simple_agents_py.workflow_stream import (
    default_workflow_execution_bools,
    merge_workflow_execution,
    stream_workflow,
)


def test_mapping_passthrough() -> None:
    d = {"workflow_path": "w.yaml", "messages": [{"role": "user", "content": "hi"}]}
    assert workflow_execution_request_to_mapping(d) == d


def test_model_dump_object() -> None:
    class M:
        def model_dump(self, mode: str = "python", exclude_none: bool = False) -> dict:
            assert mode == "json"
            assert exclude_none is True
            return {"workflow_path": "x", "messages": [{"role": "user", "content": "a"}]}

    assert workflow_execution_request_to_mapping(M())["workflow_path"] == "x"


def test_invalid_request_type_raises() -> None:
    with pytest.raises(TypeError, match="workflow request"):
        workflow_execution_request_to_mapping(42)


def test_stream_workflow_rejects_hooks_and_on_event() -> None:
    client = unittest.mock.Mock()
    with pytest.raises(ValueError, match="only one"):
        stream_workflow(client, {"workflow_path": "w", "messages": [{"role": "u", "content": "c"}]}, object(), on_event=lambda e: None)


def test_merge_workflow_execution_fills_missing_bools() -> None:
    m = merge_workflow_execution({"split_stream_deltas": True})
    assert m == {
        **default_workflow_execution_bools(),
        "split_stream_deltas": True,
    }


def test_stream_workflow_merges_partial_execution() -> None:
    client = unittest.mock.Mock()
    captured: dict = {}

    def capture_stream(*args: object, **kwargs: object) -> dict:
        captured["payload"] = args[0]
        return {}

    client.stream.side_effect = capture_stream
    stream_workflow(
        client,
        {
            "workflow_path": "w.yaml",
            "messages": [{"role": "user", "content": "hi"}],
            "execution": {"split_stream_deltas": True},
        },
    )
    ex = captured["payload"]["execution"]
    assert ex["split_stream_deltas"] is True
    assert ex["node_llm_streaming"] is True
    assert ex["healing"] is False


def test_pydantic_workflow_execution_request_roundtrip() -> None:
    pytest.importorskip("pydantic")
    from pathlib import Path

    from simple_agents_py.workflow_request import (
        WorkflowExecutionRequest,
        WorkflowInput,
        WorkflowMessage,
        WorkflowRole,
        WorkflowRunOptions,
        WorkflowTelemetryConfig,
    )

    req = WorkflowExecutionRequest(
        workflow_path="demo.yaml",
        messages=[WorkflowMessage(role="user", content="hello")],
        execution=None,
    )
    body = workflow_execution_request_to_mapping(req)
    assert body["workflow_path"] == "demo.yaml"
    assert body["messages"] == [{"role": "user", "content": "hello"}]
    assert "execution" not in body

    path_req = WorkflowExecutionRequest(
        workflow_path=Path("/tmp/w.yaml"),
        messages=[WorkflowMessage(role=WorkflowRole.USER, content="x")],
    )
    assert workflow_execution_request_to_mapping(path_req)["workflow_path"] == "/tmp/w.yaml"

    with_input = WorkflowExecutionRequest(
        workflow_path="w.yaml",
        messages=[WorkflowMessage(role="user", content="a")],
        input=WorkflowInput(email_text="hello"),
    )
    assert workflow_execution_request_to_mapping(with_input)["input"] == {"email_text": "hello"}

    opts = WorkflowRunOptions(
        model="gpt-4o-mini",
        telemetry=WorkflowTelemetryConfig(nerdstats=True),
    )
    req_opts = WorkflowExecutionRequest(
        workflow_path="w.yaml",
        messages=[WorkflowMessage(role="user", content="hi")],
        workflow_options=opts,
    )
    wo = workflow_execution_request_to_mapping(req_opts)["workflow_options"]
    assert wo["model"] == "gpt-4o-mini"
    assert wo["telemetry"] == {"nerdstats": True}


def test_stream_workflow_stream_display_incompatible_with_hooks() -> None:
    client = unittest.mock.Mock()
    with pytest.raises(ValueError, match="stream_display"):
        stream_workflow(
            client,
            {"workflow_path": "w", "messages": [{"role": "user", "content": "c"}]},
            object(),
            stream_display="merged",
        )


def test_stream_workflow_split_sets_split_stream_deltas() -> None:
    client = unittest.mock.Mock()
    captured: dict = {}

    def capture_stream(*args: object, **kwargs: object) -> dict:
        captured["payload"] = args[0]
        return {}

    client.stream.side_effect = capture_stream
    stream_workflow(
        client,
        {"workflow_path": "w.yaml", "messages": [{"role": "user", "content": "hi"}]},
        stream_display="split",
    )
    ex = captured["payload"]["execution"]
    assert ex["split_stream_deltas"] is True
    assert ex["node_llm_streaming"] is True
