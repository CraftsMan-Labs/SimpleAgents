from __future__ import annotations

from typing import Any, Callable, Iterator, Literal, Mapping, Sequence, overload, Never

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
    def run_email_workflow_yaml(
        self,
        workflow_path: str,
        email_text: str,
        include_events: bool = False,
    ) -> dict[str, Any]: ...
    def run_email_workflow_yaml_stream(
        self,
        workflow_path: str,
        email_text: str,
        on_event: Callable[[dict[str, Any]], object] | None = None,
    ) -> dict[str, Any]: ...

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
