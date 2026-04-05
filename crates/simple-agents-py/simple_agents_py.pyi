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

WorkflowMessageRole = Literal["system", "user", "assistant", "tool"]
WorkflowPayloadMode = Literal["full_payload", "redacted_payload"]
WorkflowToolTraceMode = Literal["full", "redacted", "off"]
JSONValue = None | bool | int | float | str | list["JSONValue"] | dict[str, "JSONValue"]

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
    event_type: str
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

class PySchema: ...

class SchemaBuilder:
    def __init__(self) -> None: ...
    def allow_additional_fields(self, allow: bool) -> None: ...
    def field(
        self,
        name: str,
        field_type: str | PySchema,
        required: bool = True,
        aliases: list[str] | None = None,
        default: Any | None = None,
        description: str | None = None,
        stream: str | None = None,
        items: str | PySchema | None = None,
    ) -> None: ...
    def build(self) -> PySchema: ...

class StreamingParser:
    def __init__(self, config: dict[str, Any] | None = None) -> None: ...
    def feed(self, chunk: str) -> None: ...
    def finalize(self) -> ParseResult: ...
    def buffer_len(self) -> int: ...
    def is_empty(self) -> bool: ...
    def clear(self) -> None: ...

class StreamChunk:
    content: str
    finish_reason: str | None
    model: str
    index: int

class PyStreamIterator:
    def __iter__(self) -> Iterator[StreamChunk]: ...
    def __next__(self) -> StreamChunk: ...

class HealedJsonResult:
    content: str
    raw_response: str
    confidence: float
    was_healed: bool
    provider: str | None
    model: str
    finish_reason: str
    created: int | None
    latency_ms: int

    def __init__(
        self,
        content: str,
        confidence: float,
        was_healed: bool,
        flags: list[str],
        raw_response: str | None = None,
        provider: str | None = None,
        model: str | None = None,
        finish_reason: str | None = None,
        created: int | None = None,
        latency_ms: int = 0,
        usage: Any | None = None,
    ) -> None: ...
    @property
    def usage(self) -> Any: ...
    @property
    def flags(self) -> list[str]: ...

class PyStructuredEvent:
    is_partial: bool
    is_complete: bool
    value: Any
    partial_value: Any
    confidence: float
    was_healed: bool

class ResponseWithMetadata:
    content: str
    provider: str | None
    model: str
    finish_reason: str
    created: int | None
    latency_ms: int
    was_healed: bool
    healing_confidence: float | None
    healing_error: str | None
    tool_calls: Any

    @property
    def usage(self) -> Any: ...
    @property
    def flags(self) -> list[str]: ...

class StructuredStreamIterator:
    def __iter__(self) -> Iterator[PyStructuredEvent]: ...
    def __next__(self) -> PyStructuredEvent: ...

class Client:
    def __init__(
        self,
        provider: str,
        api_key: str | None = None,
        api_base: str | None = None,
        healing: bool = True,
        timeout_seconds: int = 30,
    ) -> None: ...
    @overload
    def complete(
        self,
        model: str,
        input: str | Sequence[Mapping[str, object]],
        *,
        max_tokens: int | None = None,
        temperature: float | None = None,
        top_p: float | None = None,
        tools: Sequence[Mapping[str, object]] | None = None,
        tool_choice: object | None = None,
        response_format: str | None = None,
        schema: Mapping[str, object] | type[Any],
        schema_name: str | None = None,
        strict: bool = True,
        stream: Literal[True],
        heal: Literal[False] = False,
    ) -> Iterator[PyStructuredEvent]: ...
    @overload
    def complete(
        self,
        model: str,
        input: str | Sequence[Mapping[str, object]],
        *,
        max_tokens: int | None = None,
        temperature: float | None = None,
        top_p: float | None = None,
        tools: Sequence[Mapping[str, object]] | None = None,
        tool_choice: object | None = None,
        response_format: str | None = None,
        schema: None = None,
        schema_name: str | None = None,
        strict: bool = True,
        stream: Literal[True],
        heal: Literal[False] = False,
    ) -> Iterator[StreamChunk]: ...
    @overload
    def complete(
        self,
        model: str,
        input: str | Sequence[Mapping[str, object]],
        *,
        max_tokens: int | None = None,
        temperature: float | None = None,
        top_p: float | None = None,
        tools: Sequence[Mapping[str, object]] | None = None,
        tool_choice: object | None = None,
        response_format: str | None = None,
        schema: Mapping[str, object] | type[Any] | None = None,
        schema_name: str | None = None,
        strict: bool = True,
        stream: Literal[True],
        heal: Literal[True],
    ) -> Never: ...
    @overload
    def complete(
        self,
        model: str,
        input: str | Sequence[Mapping[str, object]],
        *,
        max_tokens: int | None = None,
        temperature: float | None = None,
        top_p: float | None = None,
        tools: Sequence[Mapping[str, object]] | None = None,
        tool_choice: object | None = None,
        response_format: str | None = None,
        schema: Mapping[str, object] | type[Any],
        schema_name: str | None = None,
        strict: bool = True,
        stream: Literal[False] = False,
        heal: Literal[True],
    ) -> HealedJsonResult: ...
    @overload
    def complete(
        self,
        model: str,
        input: str | Sequence[Mapping[str, object]],
        *,
        max_tokens: int | None = None,
        temperature: float | None = None,
        top_p: float | None = None,
        tools: Sequence[Mapping[str, object]] | None = None,
        tool_choice: object | None = None,
        response_format: str | None = None,
        schema: Mapping[str, object] | type[Any],
        schema_name: str | None = None,
        strict: bool = True,
        stream: Literal[False] = False,
        heal: Literal[False] = False,
    ) -> str: ...
    @overload
    def complete(
        self,
        model: str,
        input: str | Sequence[Mapping[str, object]],
        *,
        max_tokens: int | None = None,
        temperature: float | None = None,
        top_p: float | None = None,
        tools: Sequence[Mapping[str, object]] | None = None,
        tool_choice: object | None = None,
        response_format: Literal["json", "json_object"],
        schema: None = None,
        schema_name: str | None = None,
        strict: bool = True,
        stream: Literal[False] = False,
        heal: Literal[True],
    ) -> HealedJsonResult: ...
    @overload
    def complete(
        self,
        model: str,
        input: str | Sequence[Mapping[str, object]],
        *,
        max_tokens: int | None = None,
        temperature: float | None = None,
        top_p: float | None = None,
        tools: Sequence[Mapping[str, object]] | None = None,
        tool_choice: object | None = None,
        response_format: Literal["json", "json_object"],
        schema: None = None,
        schema_name: str | None = None,
        strict: bool = True,
        stream: Literal[False] = False,
        heal: Literal[False] = False,
    ) -> str: ...
    @overload
    def complete(
        self,
        model: str,
        input: str | Sequence[Mapping[str, object]],
        *,
        max_tokens: int | None = None,
        temperature: float | None = None,
        top_p: float | None = None,
        tools: Sequence[Mapping[str, object]] | None = None,
        tool_choice: object | None = None,
        response_format: Literal["text"] | None = None,
        schema: None = None,
        schema_name: str | None = None,
        strict: bool = True,
        stream: Literal[False] = False,
        heal: bool = False,
    ) -> ResponseWithMetadata: ...
    def complete(
        self,
        model: str,
        input: str | Sequence[Mapping[str, object]],
        max_tokens: int | None = None,
        temperature: float | None = None,
        top_p: float | None = None,
        tools: Sequence[Mapping[str, object]] | None = None,
        tool_choice: object | None = None,
        response_format: str | None = None,
        schema: Mapping[str, object] | type[Any] | None = None,
        schema_name: str | None = None,
        strict: bool = True,
        stream: bool = False,
        heal: bool = False,
    ) -> (
        ResponseWithMetadata
        | HealedJsonResult
        | str
        | Iterator[StreamChunk]
        | Iterator[PyStructuredEvent]
    ): ...
    def run_workflow_yaml(
        self,
        workflow_path: str,
        workflow_input: WorkflowInput | Mapping[str, JSONValue],
        include_events: bool = False,
        workflow_options: WorkflowRunOptions | Mapping[str, JSONValue] | None = None,
    ) -> WorkflowRunOutput: ...
    def run_workflow_yaml_stream(
        self,
        workflow_path: str,
        workflow_input: WorkflowInput | Mapping[str, JSONValue],
        on_event: Callable[[WorkflowEvent], object] | None = None,
        workflow_options: WorkflowRunOptions | Mapping[str, JSONValue] | None = None,
    ) -> WorkflowRunOutput: ...

class ProviderConfig:
    def __init__(
        self,
        provider: str,
        api_key: str | None = None,
        api_base: str | None = None,
    ) -> None: ...
    @property
    def provider(self) -> str: ...
    @property
    def api_key(self) -> str | None: ...
    @property
    def api_base(self) -> str | None: ...

class RoutingPolicy:
    @staticmethod
    def direct() -> RoutingPolicy: ...
    @staticmethod
    def round_robin() -> RoutingPolicy: ...
    @staticmethod
    def latency(alpha: float = 0.2, slow_threshold_ms: int = 2000) -> RoutingPolicy: ...
    @staticmethod
    def cost(provider_costs: Mapping[str, float]) -> RoutingPolicy: ...
    @staticmethod
    def fallback(retryable_only: bool = True) -> RoutingPolicy: ...
    @property
    def mode(self) -> str: ...

class CacheConfig:
    def __init__(self, ttl_seconds: int) -> None: ...
    @property
    def ttl_seconds(self) -> int: ...

class HealingConfig:
    def __init__(
        self,
        enabled: bool = True,
        min_confidence: float = 0.0,
        fuzzy_match_threshold: float = 0.8,
    ) -> None: ...
    @property
    def enabled(self) -> bool: ...
    @property
    def min_confidence(self) -> float: ...
    @property
    def fuzzy_match_threshold(self) -> float: ...

class ClientBuilder:
    def __init__(self) -> None: ...
    def add_provider(
        self,
        provider: str,
        api_key: str | None = None,
        api_base: str | None = None,
    ) -> ClientBuilder: ...
    def add_provider_config(self, config: ProviderConfig) -> ClientBuilder: ...
    def with_routing(self, mode: str) -> ClientBuilder: ...
    def with_routing_policy(self, policy: RoutingPolicy) -> ClientBuilder: ...
    def with_latency_routing(self, config: dict[str, Any]) -> ClientBuilder: ...
    def with_cost_routing(self, config: dict[str, Any]) -> ClientBuilder: ...
    def with_fallback_routing(self, config: dict[str, Any]) -> ClientBuilder: ...
    def with_cache(self, ttl_seconds: int) -> ClientBuilder: ...
    def with_cache_config(self, config: CacheConfig) -> ClientBuilder: ...
    def with_healing_config(self, config: dict[str, Any]) -> ClientBuilder: ...
    def with_healing(self, config: HealingConfig) -> ClientBuilder: ...
    def add_middleware(self, middleware: object) -> ClientBuilder: ...
    def with_custom_cache(
        self, cache: object, ttl_seconds: int | None = None
    ) -> ClientBuilder: ...
    def build(self) -> Client: ...

def heal_json(text: str, config: dict[str, Any] | None = None) -> ParseResult: ...
def coerce_to_schema(
    data: Any,
    schema: dict[str, Any] | PySchema,
    config: dict[str, Any] | None = None,
) -> CoercionResult: ...
