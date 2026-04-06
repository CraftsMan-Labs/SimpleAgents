"""Typed workflow execution request models (Pydantic v2).

Install the optional extra::

    pip install simple-agents-py[pydantic]

Then pass :class:`WorkflowExecutionRequest` to :func:`simple_agents_py.workflow_stream.stream_workflow`
or :func:`simple_agents_py.workflow_stream.run_workflow_request` without hand-written dicts.
"""

from __future__ import annotations

from enum import Enum
from pathlib import Path
from typing import Annotated, Any

from pydantic import BaseModel, BeforeValidator, ConfigDict, Field


def _coerce_workflow_path(value: Any) -> str:
    if isinstance(value, str):
        return value
    if isinstance(value, Path):
        return str(value)
    fspath = getattr(value, "__fspath__", None)
    if callable(fspath):
        return str(fspath())
    raise TypeError(
        "workflow_path must be str, pathlib.Path, or os.PathLike[str], "
        f"not {type(value).__name__}"
    )


WorkflowPath = Annotated[str, BeforeValidator(_coerce_workflow_path)]


class WorkflowRole(str, Enum):
    """Chat message role (OpenAI-style)."""

    SYSTEM = "system"
    USER = "user"
    ASSISTANT = "assistant"
    TOOL = "tool"


class WorkflowMessage(BaseModel):
    """One chat message in ``WorkflowExecutionRequest.messages``."""

    model_config = ConfigDict(extra="allow")

    role: WorkflowRole | str
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


class WorkflowInput(BaseModel):
    """Arbitrary workflow input fields (e.g. ``email_text``). Use keyword args, not a dict."""

    model_config = ConfigDict(extra="allow")


class WorkflowExecutionRequest(BaseModel):
    """Messages-first workflow request; aligns with ``WorkflowExecutionRequest`` in the ``.pyi``."""

    model_config = ConfigDict(extra="allow")

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
