"""Typed workflow execution request models (Pydantic v2).

Install the optional extra::

    pip install simple-agents-py[pydantic]

Then pass :class:`WorkflowExecutionRequest` to :func:`simple_agents_py.workflow_stream.stream_workflow`
or :func:`simple_agents_py.workflow_stream.run_workflow_request` without hand-written dicts.
"""

from __future__ import annotations

from typing import Any

from pydantic import BaseModel, ConfigDict


class WorkflowMessage(BaseModel):
    """One chat message in ``WorkflowExecutionRequest.messages``."""

    model_config = ConfigDict(extra="allow")

    role: str
    content: str


class WorkflowExecutionFlags(BaseModel):
    """Execution flags; aligns with ``WorkflowExecutionFlags`` in ``simple_agents_py.pyi``."""

    model: str | None = None
    healing: bool = False
    workflow_streaming: bool = False
    node_llm_streaming: bool = True
    split_stream_deltas: bool = False


class WorkflowRunOptions(BaseModel):
    """Telemetry, trace, per-run model override, etc."""

    model_config = ConfigDict(extra="allow")

    model: str | None = None
    telemetry: dict[str, Any] | None = None
    trace: dict[str, Any] | None = None


class WorkflowExecutionRequest(BaseModel):
    """Messages-first workflow request; aligns with ``WorkflowExecutionRequest`` in the ``.pyi``."""

    model_config = ConfigDict(extra="allow")

    workflow_path: str
    messages: list[WorkflowMessage]
    context: dict[str, Any] | None = None
    media: dict[str, Any] | None = None
    input: dict[str, Any] | None = None
    execution: WorkflowExecutionFlags | None = None
    workflow_options: WorkflowRunOptions | None = None

    def to_client_payload(self) -> dict[str, Any]:
        """Same mapping :func:`workflow_payload.workflow_execution_request_to_mapping` produces."""
        return self.model_dump(mode="json", exclude_none=True)
