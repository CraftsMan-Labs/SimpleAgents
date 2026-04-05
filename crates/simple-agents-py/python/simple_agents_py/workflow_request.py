"""Typed workflow execution request models (Pydantic v2).

Install the optional extra::

    pip install simple-agents-py[pydantic]

Then pass :class:`WorkflowExecutionRequest` to :func:`simple_agents_py.workflow_stream.stream_workflow`
or :func:`simple_agents_py.workflow_stream.run_workflow_request` without hand-written dicts.
"""

from __future__ import annotations

from typing import Any

from pydantic import BaseModel, ConfigDict, Field


class WorkflowMessage(BaseModel):
    """One chat message in ``WorkflowExecutionRequest.messages``."""

    model_config = ConfigDict(extra="allow")

    role: str
    content: str


class WorkflowExecutionFlags(BaseModel):
    """Execution flags for ``WorkflowExecutionRequest.execution``.

    Booleans match Rust ``YamlWorkflowExecutionFlags``. ``model`` is a binding convenience:
    when set, it is merged into ``workflow_options.model`` (same as :class:`WorkflowRunOptions`).
    """

    model: str | None = Field(
        default=None,
        description="Optional default model override; merged into workflow_options.model, not a Rust execution flag.",
    )
    healing: bool = Field(
        default=False,
        description="JSON healing path for structured LLM outputs (Rust: healing).",
    )
    workflow_streaming: bool = Field(
        default=False,
        description="When False with a stream sink, token deltas are not forwarded (Rust: workflow_streaming).",
    )
    node_llm_streaming: bool = Field(
        default=True,
        description="When False, LLM nodes never use provider streaming (Rust: node_llm_streaming).",
    )
    split_stream_deltas: bool = Field(
        default=False,
        description="When True, emit thinking vs output stream events (Rust: split_stream_deltas).",
    )


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

    def to_client_payload(self, *, merge_execution_defaults: bool = False) -> dict[str, Any]:
        """Same mapping as :func:`workflow_payload.workflow_execution_request_to_mapping`.

        When *merge_execution_defaults* is True, ``execution`` is merged with
        :func:`simple_agents_py.workflow_stream.merge_workflow_execution` so every
        boolean flag is explicit on the wire.
        """
        data = self.model_dump(mode="json", exclude_none=True)
        if merge_execution_defaults and isinstance(data.get("execution"), dict):
            from .workflow_stream import merge_workflow_execution

            data["execution"] = merge_workflow_execution(data["execution"])
        return data
