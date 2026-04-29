"""Typed workflow execution request models (Pydantic v2).

Install the optional extra::

    pip install simple-agents-py[pydantic]

Then pass :class:`WorkflowExecutionRequest` to :func:`simple_agents_py.workflow_stream.stream_workflow`
or directly to ``Client.run_workflow`` / ``Client.stream_workflow`` without hand-written dicts.
"""

from __future__ import annotations

from enum import Enum
from typing import Annotated, Any, TypeAlias

from pydantic import BaseModel, BeforeValidator, ConfigDict, Field, model_validator

from ._path_utils import coerce_path


def _coerce_workflow_path(value: Any) -> str:
    return coerce_path(value, field_name="workflow_path")


WorkflowPath = Annotated[str, BeforeValidator(_coerce_workflow_path)]


class WorkflowRole(str, Enum):
    """Chat message role (OpenAI-style)."""

    SYSTEM = "system"
    USER = "user"
    ASSISTANT = "assistant"
    TOOL = "tool"


class WorkflowMessage(BaseModel):
    """One chat message in ``WorkflowExecutionRequest.messages``.

    For multimodal content (images, audio, video), pass ``content`` as a list
    of dict parts matching the wire schema, for example::

        WorkflowMessage(role="user", content=[
            {"type": "text", "text": "Describe this image."},
            {"type": "image_url", "image_url": {"url": "data:image/jpeg;base64,..."}},
        ])
    """

    model_config = ConfigDict(extra="forbid")

    role: WorkflowRole | str
    content: str | list[dict[str, Any]]
    name: str | None = None
    tool_call_id: str | None = None

    @model_validator(mode="after")
    def _content_not_empty_list(self) -> "WorkflowMessage":
        if isinstance(self.content, list) and len(self.content) == 0:
            raise ValueError("content list must not be empty")
        return self


class WorkflowExecutionFlags(BaseModel):
    """Execution flags for ``WorkflowExecutionRequest.execution``.

    Booleans match Rust ``YamlWorkflowExecutionFlags``. ``model`` is a binding convenience:
    when set, it is merged into ``workflow_options.model`` (same as :class:`WorkflowRunOptions`).
    """

    model_config = ConfigDict(extra="forbid")

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
    debug_stream_parse: bool = Field(
        default=False,
        description="When True (or env SIMPLE_AGENTS_DEBUG_STREAM_PARSE), append partial LLM text to structured JSON parse errors (Rust: debug_stream_parse).",
    )


class WorkflowTraceContext(BaseModel):
    """Upstream trace propagation (JSON field names match Go ``WorkflowTraceContext``)."""

    model_config = ConfigDict(extra="forbid")

    trace_id: str | None = None
    span_id: str | None = None
    parent_span_id: str | None = None
    traceparent: str | None = None
    tracestate: str | None = None
    baggage: dict[str, str] | None = None


class WorkflowTraceTenant(BaseModel):
    """Multi-tenant correlation (JSON field names match Go ``WorkflowTraceTenant``)."""

    model_config = ConfigDict(extra="forbid")

    workspace_id: str | None = None
    user_id: str | None = None
    conversation_id: str | None = None
    request_id: str | None = None
    run_id: str | None = None


class WorkflowTraceConfig(BaseModel):
    """Nested trace config for ``WorkflowRunOptions.trace``."""

    model_config = ConfigDict(extra="forbid")

    context: WorkflowTraceContext | None = None
    tenant: WorkflowTraceTenant | None = None


class WorkflowTelemetryConfig(BaseModel):
    """Telemetry flags for ``WorkflowRunOptions.telemetry`` (matches Go ``WorkflowTelemetryConfig``)."""

    model_config = ConfigDict(extra="forbid")

    enabled: bool | None = None
    nerdstats: bool | None = None
    sample_rate: float | None = Field(default=None, ge=0.0, le=1.0)
    payload_mode: str | None = None
    retention_days: int | None = Field(default=None, ge=0)
    multi_tenant: bool | None = None
    tool_trace_mode: str | None = None


class WorkflowRunOptions(BaseModel):
    """Per-run options (matches Rust ``YamlWorkflowRunOptions``: unknown keys are rejected server-side)."""

    model_config = ConfigDict(extra="forbid")

    model: str | None = None
    telemetry: WorkflowTelemetryConfig | None = None
    trace: WorkflowTraceConfig | None = None


WorkflowInput: TypeAlias = dict[str, Any]
"""Explicit arbitrary workflow payload map, e.g. ``WorkflowInput(email_text="hello")``."""


class WorkflowExecutionRequest(BaseModel):
    """Messages-first workflow request; aligns with ``WorkflowExecutionRequest`` in the ``.pyi``."""

    model_config = ConfigDict(extra="forbid")

    workflow_path: WorkflowPath
    messages: list[WorkflowMessage]
    context: dict[str, Any] | None = None
    media: dict[str, Any] | None = None
    input: WorkflowInput | None = None
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


__all__ = [
    "WorkflowExecutionFlags",
    "WorkflowExecutionRequest",
    "WorkflowInput",
    "WorkflowMessage",
    "WorkflowPath",
    "WorkflowRole",
    "WorkflowRunOptions",
    "WorkflowTelemetryConfig",
    "WorkflowTraceConfig",
    "WorkflowTraceContext",
    "WorkflowTraceTenant",
]
