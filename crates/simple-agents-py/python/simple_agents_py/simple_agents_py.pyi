from __future__ import annotations

from typing import (
    Any,
    Callable,
    Iterator,
    Literal,
    Mapping,
    Sequence,
    TypedDict,
    overload,
    Never,
)
from enum import Enum

# ---------------------------------------------------------------------------
# Primitive aliases
# ---------------------------------------------------------------------------

WorkflowMessageRole = Literal["system", "user", "assistant", "tool"]
WorkflowPayloadMode = Literal["full_payload", "redacted_payload"]
WorkflowToolTraceMode = Literal["full", "redacted", "off"]

# Known event_type strings emitted by the Rust YAML workflow runner (wire format).
# Source of truth: crates/simple-agents-workflow/src/yaml_runner/ (execute.rs,
# client_executor.rs, node_execution.rs). The runner may add new types; use
# ``WorkflowRunnerEventType | str`` where an open-ended union is needed.
WorkflowRunnerEventType = Literal[
    "workflow_started",
    "workflow_completed",
    "node_started",
    "node_completed",
    "resolved_llm_input",
    "node_stream_delta",
    "node_stream_thinking_delta",
    "node_stream_output_delta",
    "node_tool_call_requested",
    "node_tool_call_failed",
    "node_tool_call_completed",
    "node_tool_roundtrip_completed",
    "node_healed",
]
JSONValue = None | bool | int | float | str | list["JSONValue"] | dict[str, "JSONValue"]

# ---------------------------------------------------------------------------
# Typed message API  (new unified surface)
# ---------------------------------------------------------------------------

class Role(Enum):
    """LLM conversation role."""
    User = "User"
    System = "System"
    Assistant = "Assistant"
    Tool = "Tool"

class ContentPart:
    """A single multimodal content part (text, image, audio, or video)."""
    @staticmethod
    def text(text: str) -> ContentPart: ...
    @staticmethod
    def image_url(url: str) -> ContentPart: ...
    @staticmethod
    def image(media_type: str, data: str) -> ContentPart: ...
    @staticmethod
    def audio(media_type: str, data: str) -> ContentPart: ...
    @staticmethod
    def video(media_type: str, data: str) -> ContentPart: ...

class Message:
    """A typed conversation message."""
    @staticmethod
    def user(content: str) -> Message: ...
    @staticmethod
    def system(content: str) -> Message: ...
    @staticmethod
    def assistant(content: str) -> Message: ...
    @staticmethod
    def user_parts(parts: list[ContentPart]) -> Message: ...
    @property
    def role(self) -> Role: ...

# ---------------------------------------------------------------------------
# Workflow types (dict-based, legacy and options)
# ---------------------------------------------------------------------------

class WorkflowMessage(TypedDict, total=False):
    role: WorkflowMessageRole
    content: str
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

class WorkflowExecutionRequest(TypedDict, total=False):
    workflow_path: str
    messages: list[WorkflowMessage]
    context: Mapping[str, JSONValue]
    media: Mapping[str, JSONValue]
    input: Mapping[str, JSONValue]
    execution: WorkflowExecutionFlags
    workflow_options: WorkflowRunOptions

WorkflowNodeKind = Literal["llm_call", "switch", "custom_worker", "unknown"]

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
# Healing / parsing types
# ---------------------------------------------------------------------------

class ParseResult:
    value: Any
    confidence: float
    was_healed: bool

    def __init__(
        self, value: Any, confidence: float, was_healed: bool, flags: list[str]
    ) -> None: ...
    @property
    def flags(self) -> list[str]: ...

class CoercionResult:
    value: Any
    confidence: float
    was_coerced: bool

    def __init__(
        self, value: Any, confidence: float, was_coerced: bool, flags: list[str]
    ) -> None: ...
    @property
    def flags(self) -> list[str]: ...

class HealedJsonResult:
    content: str
    confidence: float
    was_healed: bool
    raw_response: str
    usage: Any

    def __init__(
        self,
        content: str,
        confidence: float,
        was_healed: bool,
        flags: list[str],
        *,
        raw_response: str | None = None,
        usage: Any | None = None,
    ) -> None: ...
    @property
    def flags(self) -> list[str]: ...

class PyStructuredEvent:
    is_partial: bool
    is_complete: bool
    value: Any
    partial_value: Any
    confidence: float
    was_healed: bool

# ---------------------------------------------------------------------------
# Streaming types
# ---------------------------------------------------------------------------

class StreamChunk:
    content: str
    finish_reason: str | None
    model: str
    index: int

class PyStreamIterator:
    def __iter__(self) -> Iterator[StreamChunk]: ...
    def __next__(self) -> StreamChunk: ...

class PyStructuredStreamIterator:
    def __iter__(self) -> Iterator[PyStructuredEvent]: ...
    def __next__(self) -> PyStructuredEvent: ...

StructuredStreamIterator: type[PyStructuredStreamIterator]

class StreamingParser:
    def __init__(self) -> None: ...
    def feed(self, chunk: str) -> None: ...
    def try_parse(self) -> ParseResult | None: ...
    def finalize(self) -> ParseResult: ...
    def buffer_len(self) -> int: ...
    def is_empty(self) -> bool: ...
    def clear(self) -> None: ...

class ResponseWithMetadata:
    content: str
    provider: str | None
    model: str
    finish_reason: str
    created: int | None
    latency_ms: int
    tool_calls: Any

    @property
    def usage(self) -> Any: ...

# ---------------------------------------------------------------------------
# ClientBuilder / ProviderConfig
# ---------------------------------------------------------------------------

class ProviderConfig:
    name: str
    api_key: str
    api_base: str | None

    def __init__(self, name: str, api_key: str, api_base: str | None = None) -> None: ...

class ClientBuilder:
    def __init__(self) -> None: ...
    def add_provider(
        self,
        name: str,
        *,
        api_key: str | None = None,
        api_base: str | None = None,
        base_url: str | None = None,
    ) -> ClientBuilder: ...
    def add_provider_config(self, config: ProviderConfig) -> ClientBuilder: ...
    def with_routing(self, mode: str) -> ClientBuilder: ...
    def with_latency_routing(self, config: Mapping[str, object]) -> ClientBuilder: ...
    def with_cost_routing(self, config: Mapping[str, object]) -> ClientBuilder: ...
    def with_fallback_routing(self, config: Mapping[str, object]) -> ClientBuilder: ...
    def with_cache(self, ttl_seconds: int) -> ClientBuilder: ...
    def with_healing_config(self, config: Mapping[str, object]) -> ClientBuilder: ...
    def build(self) -> Client: ...

# ---------------------------------------------------------------------------
# Main Client
# ---------------------------------------------------------------------------

class Client:
    def __init__(
        self,
        provider: str,
        *,
        api_key: str | None = None,
        api_base: str | None = None,
        base_url: str | None = None,
        model: str | None = None,
        api_format: str | None = None,
        timeout_seconds: float | None = None,
    ) -> None: ...

    # --- Direct LLM calls ---

    @overload
    def complete(
        self,
        model: str,
        input: str | Sequence[Mapping[str, object]] | list[Message],
        *,
        max_tokens: int | None = None,
        temperature: float | None = None,
        top_p: float | None = None,
        tools: Sequence[Mapping[str, object]] | None = None,
        tool_choice: object | None = None,
        response_format: str | None = None,
        schema: Mapping[str, object] | type[Any],
        schema_name: str | None = None,
        stream: Literal[True],
        heal: Literal[False] = False,
    ) -> Iterator[PyStructuredEvent]: ...
    @overload
    def complete(
        self,
        model: str,
        input: str | Sequence[Mapping[str, object]] | list[Message],
        *,
        max_tokens: int | None = None,
        temperature: float | None = None,
        top_p: float | None = None,
        tools: Sequence[Mapping[str, object]] | None = None,
        tool_choice: object | None = None,
        response_format: str | None = None,
        schema: None = None,
        schema_name: None = None,
        stream: Literal[True],
        heal: Literal[False] = False,
    ) -> Iterator[StreamChunk]: ...
    @overload
    def complete(
        self,
        model: str,
        input: str | Sequence[Mapping[str, object]] | list[Message],
        *,
        max_tokens: int | None = None,
        temperature: float | None = None,
        top_p: float | None = None,
        tools: Sequence[Mapping[str, object]] | None = None,
        tool_choice: object | None = None,
        response_format: str | None = None,
        schema: Mapping[str, object] | type[Any],
        schema_name: str | None = None,
        stream: Literal[False] = False,
        heal: Literal[False] = False,
    ) -> str: ...
    @overload
    def complete(
        self,
        model: str,
        input: str | Sequence[Mapping[str, object]] | list[Message],
        *,
        max_tokens: int | None = None,
        temperature: float | None = None,
        top_p: float | None = None,
        tools: Sequence[Mapping[str, object]] | None = None,
        tool_choice: object | None = None,
        response_format: str | None = None,
        schema: None = None,
        schema_name: None = None,
        stream: Literal[False] = False,
        heal: Literal[True],
    ) -> HealedJsonResult: ...
    @overload
    def complete(
        self,
        model: str,
        input: str | Sequence[Mapping[str, object]] | list[Message],
        *,
        max_tokens: int | None = None,
        temperature: float | None = None,
        top_p: float | None = None,
        tools: Sequence[Mapping[str, object]] | None = None,
        tool_choice: object | None = None,
        response_format: str | None = None,
        schema: None = None,
        schema_name: None = None,
        stream: Literal[False] = False,
        heal: Literal[False] = False,
    ) -> ResponseWithMetadata: ...
    def complete(
        self,
        model: str,
        input: str | Sequence[Mapping[str, object]] | list[Message],
        max_tokens: int | None = None,
        temperature: float | None = None,
        top_p: float | None = None,
        tools: Sequence[Mapping[str, object]] | None = None,
        tool_choice: object | None = None,
        response_format: str | None = None,
        schema: Mapping[str, object] | type[Any] | None = None,
        schema_name: str | None = None,
        stream: bool = False,
        heal: bool = False,
    ) -> ResponseWithMetadata | HealedJsonResult | str | Iterator[StreamChunk] | Iterator[PyStructuredEvent]: ...

    def stream_complete(
        self,
        model: str,
        input: str | Sequence[Mapping[str, object]] | list[Message],
        *,
        max_tokens: int | None = None,
        temperature: float | None = None,
        top_p: float | None = None,
    ) -> Iterator[StreamChunk]: ...

    # --- Unified workflow API (new) ---

    def run(
        self,
        workflow_path: str,
        messages: list[Message] | list[Mapping[str, object]],
        *,
        tools: Any | None = None,
        options: Mapping[str, Any] | None = None,
    ) -> WorkflowRunOutput: ...

    def stream(
        self,
        workflow_path: str,
        messages: list[Message] | list[Mapping[str, object]],
        *,
        on_event: Callable[[WorkflowEvent], object] | None = None,
        tools: Any | None = None,
        options: Mapping[str, Any] | None = None,
    ) -> WorkflowRunOutput: ...

    def resume(
        self,
        checkpoint: Mapping[str, Any],
        *,
        options: Mapping[str, Any] | None = None,
    ) -> WorkflowRunOutput: ...

    # --- Legacy workflow API (kept for backwards compat) ---

    def run_workflow(
        self,
        request: WorkflowExecutionRequest | Mapping[str, JSONValue],
    ) -> WorkflowRunOutput: ...

    def stream_workflow(
        self,
        request: WorkflowExecutionRequest | Mapping[str, JSONValue],
        on_event: Callable[[WorkflowEvent], object] | None = None,
        include_events_in_output: bool = False,
    ) -> WorkflowRunOutput: ...

    def run_workflow_yaml_stream(
        self,
        workflow_path: str,
        input: WorkflowInput | Mapping[str, JSONValue],
        *,
        on_event: Callable[[WorkflowEvent], object] | None = None,
        workflow_options: WorkflowRunOptions | Mapping[str, JSONValue] | None = None,
    ) -> WorkflowRunOutput: ...

# ---------------------------------------------------------------------------
# Module-level functions
# ---------------------------------------------------------------------------

def heal_json(raw: str) -> ParseResult: ...
def coerce_to_schema(value: Any, schema: dict[str, Any]) -> CoercionResult: ...
