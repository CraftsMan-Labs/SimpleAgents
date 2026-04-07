"""Pydantic models for workflow stream events (parity with TypedDicts in ``models``)."""

from __future__ import annotations

from simple_agents_py.models import (
    SseWorkflowEventEnvelope,
    WorkflowStreamEventModel,
)


def test_workflow_stream_event_model_accepts_runner_event_dict() -> None:
    raw = {
        "event_type": "node_stream_delta",
        "node_id": "n1",
        "delta": "hi",
    }
    m = WorkflowStreamEventModel.model_validate(raw)
    assert m.event_type == "node_stream_delta"
    assert m.delta == "hi"


def test_sse_envelope_roundtrip() -> None:
    env = SseWorkflowEventEnvelope(
        workflow_event=WorkflowStreamEventModel(
            event_type="node_stream_delta",
            delta="x",
        )
    )
    d = env.model_dump(mode="json")
    assert d["workflow_event"]["delta"] == "x"
