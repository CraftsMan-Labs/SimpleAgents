"""Runtime workflow contract types (``TypedDict``) and optional Pydantic mirrors.

Native :class:`simple_agents_py.simple_agents_py.Client` workflow methods return plain
dicts; their shapes match the ``TypedDict`` definitions here. Pydantic models are for
validation, OpenAPI, and FastAPI responses.
"""

from __future__ import annotations

from typing import (
    Any,
    Literal,
    Mapping,
    TypeAlias,
    TypedDict,
)

from pydantic import BaseModel, ConfigDict, Field

# ---------------------------------------------------------------------------
# Primitive aliases (same wire contract as the YAML workflow runner)
# ---------------------------------------------------------------------------

WorkflowMessageRole: TypeAlias = Literal["system", "user", "assistant", "tool"]
WorkflowPayloadMode: TypeAlias = Literal["full_payload", "redacted_payload"]
WorkflowToolTraceMode: TypeAlias = Literal["full", "redacted", "off"]

# Known event_type strings emitted by the Rust YAML workflow runner (wire format).
# Source of truth: crates/simple-agents-workflow/src/yaml_runner/ (execute.rs,
# client_executor.rs, node_execution.rs). The runner may add new types; use
# ``WorkflowRunnerEventType | str`` where an open-ended union is needed.
WorkflowRunnerEventType: TypeAlias = Literal[
    "workflow_started",
    "workflow_completed",
    "node_started",
    "node_completed",
    "resolved_llm_input",
    "node_stream_delta",
    "node_stream_snapshot",
    "node_stream_thinking_delta",
    "node_stream_output_delta",
    "node_tool_call_requested",
    "node_tool_call_failed",
    "node_tool_call_completed",
    "node_tool_roundtrip_completed",
    "node_healed",
]

JSONValue: TypeAlias = (
    None | bool | int | float | str | list["JSONValue"] | dict[str, "JSONValue"]
)

# ---------------------------------------------------------------------------
# Workflow request / runner dict types
# ---------------------------------------------------------------------------


WorkflowMessageContent: TypeAlias = str | list[dict[str, JSONValue]]


class WorkflowMessage(TypedDict, total=False):
    role: WorkflowMessageRole
    content: WorkflowMessageContent
    name: str
    tool_call_id: str


class WorkflowInput(TypedDict, total=False):
    messages: list[WorkflowMessage]


class WorkflowTelemetryOptions(TypedDict, total=False):
    enabled: bool
    nerdstats: bool
    sample_rate: float
    payload_mode: WorkflowPayloadMode
    retention_days: int
    multi_tenant: bool
    tool_trace_mode: WorkflowToolTraceMode


class WorkflowTraceContextOptions(TypedDict, total=False):
    trace_id: str
    span_id: str
    parent_span_id: str
    traceparent: str
    tracestate: str
    baggage: Mapping[str, str]


class WorkflowTraceTenantOptions(TypedDict, total=False):
    workspace_id: str
    user_id: str
    conversation_id: str
    request_id: str
    run_id: str


class WorkflowTraceOptions(TypedDict, total=False):
    context: WorkflowTraceContextOptions
    tenant: WorkflowTraceTenantOptions


class WorkflowRunOptions(TypedDict, total=False):
    telemetry: WorkflowTelemetryOptions
    trace: WorkflowTraceOptions
    model: str


class WorkflowExecutionFlags(TypedDict, total=False):
    model: str
    healing: bool
    workflow_streaming: bool
    node_llm_streaming: bool
    split_stream_deltas: bool
    debug_stream_parse: bool


class WorkflowExecutionRequest(TypedDict, total=False):
    workflow_path: str
    messages: list[WorkflowMessage]
    context: Mapping[str, JSONValue]
    media: Mapping[str, JSONValue]
    input: Mapping[str, JSONValue]
    execution: WorkflowExecutionFlags
    workflow_options: WorkflowRunOptions


WorkflowNodeKind: TypeAlias = Literal["llm_call", "switch", "custom_worker", "unknown"]


class WorkflowNodeOutputRecord(TypedDict):
    node_id: str
    node_kind: WorkflowNodeKind
    value: JSONValue


class WorkflowStepTiming(TypedDict, total=False):
    node_id: str
    node_kind: str
    model_name: str
    elapsed_ms: int
    prompt_tokens: int
    completion_tokens: int
    total_tokens: int
    reasoning_tokens: int
    tokens_per_second: float


class WorkflowLlmNodeMetrics(TypedDict, total=False):
    elapsed_ms: int
    prompt_tokens: int
    completion_tokens: int
    total_tokens: int
    reasoning_tokens: int
    tokens_per_second: float


class WorkflowEvent(TypedDict, total=False):
    event_type: WorkflowRunnerEventType | str
    node_id: str
    step_id: str
    node_kind: str
    streamable: bool
    message: str
    delta: str
    snapshot: JSONValue
    token_kind: str
    is_terminal_node_token: bool
    elapsed_ms: int
    metadata: JSONValue


class WorkflowRunOutput(TypedDict, total=False):
    workflow_id: str
    entry_node: str
    trace: list[str]
    outputs: dict[str, JSONValue]
    terminal_node: str
    terminal_output: JSONValue
    step_timings: list[WorkflowStepTiming]
    llm_node_metrics: dict[str, WorkflowLlmNodeMetrics]
    llm_node_models: dict[str, str]
    total_elapsed_ms: int
    ttft_ms: int
    total_input_tokens: int
    total_output_tokens: int
    total_tokens: int
    total_reasoning_tokens: int
    tokens_per_second: float
    trace_id: str
    metadata: JSONValue
    events: list[WorkflowEvent]


# ---------------------------------------------------------------------------
# Pydantic (optional extra: simple-agents-py[pydantic])
# ---------------------------------------------------------------------------


class WorkflowStreamEventModel(BaseModel):
    """One workflow runner event (parity with :class:`WorkflowEvent`)."""

    model_config = ConfigDict(extra="forbid")

    event_type: str | None = None
    node_id: str | None = None
    step_id: str | None = None
    node_kind: str | None = None
    streamable: bool | None = None
    message: str | None = None
    delta: str | None = None
    snapshot: Any = None
    token_kind: str | None = None
    is_terminal_node_token: bool | None = None
    elapsed_ms: int | None = None
    metadata: Any = None


class WorkflowRunOutputModel(BaseModel):
    """Workflow run result (parity with :class:`WorkflowRunOutput`)."""

    model_config = ConfigDict(extra="forbid")

    workflow_id: str | None = None
    entry_node: str | None = None
    trace: list[str] | None = None
    outputs: dict[str, Any] | None = None
    terminal_node: str | None = None
    terminal_output: Any | None = None
    step_timings: list[dict[str, Any]] | None = None
    llm_node_metrics: dict[str, dict[str, Any]] | None = None
    llm_node_models: dict[str, str] | None = None
    total_elapsed_ms: int | None = None
    ttft_ms: int | None = None
    total_input_tokens: int | None = None
    total_output_tokens: int | None = None
    total_tokens: int | None = None
    total_reasoning_tokens: int | None = None
    tokens_per_second: float | None = None
    trace_id: str | None = None
    metadata: Any | None = None
    events: list[dict[str, Any]] | None = None


class SseWorkflowEventEnvelope(BaseModel):
    """One ``data:`` JSON line in the FastAPI example: streaming runner event."""

    workflow_event: WorkflowStreamEventModel = Field(
        ...,
        description="Same structure as events from ``Client.stream_workflow`` ``on_event``.",
    )


class SseWorkflowResultEnvelope(BaseModel):
    """One ``data:`` JSON line: final workflow output before ``[DONE]``."""

    workflow_result: WorkflowRunOutputModel = Field(
        ...,
        description="Same structure as the return value of ``Client.stream_workflow``.",
    )


class SseStreamErrorEnvelope(BaseModel):
    """Error line before stream end."""

    error: str
    error_type: str


__all__ = [
    "JSONValue",
    "WorkflowMessageRole",
    "WorkflowPayloadMode",
    "WorkflowToolTraceMode",
    "WorkflowRunnerEventType",
    "WorkflowMessage",
    "WorkflowInput",
    "WorkflowTelemetryOptions",
    "WorkflowTraceContextOptions",
    "WorkflowTraceTenantOptions",
    "WorkflowTraceOptions",
    "WorkflowRunOptions",
    "WorkflowExecutionFlags",
    "WorkflowExecutionRequest",
    "WorkflowNodeKind",
    "WorkflowNodeOutputRecord",
    "WorkflowStepTiming",
    "WorkflowLlmNodeMetrics",
    "WorkflowEvent",
    "WorkflowRunOutput",
    "WorkflowStreamEventModel",
    "WorkflowRunOutputModel",
    "SseWorkflowEventEnvelope",
    "SseWorkflowResultEnvelope",
    "SseStreamErrorEnvelope",
]
