from __future__ import annotations

from typing import Any, Iterator, Mapping, Sequence


class ParseResult:
    value: Any
    confidence: float
    was_healed: bool

    def __init__(self, value: Any, confidence: float, was_healed: bool, flags: list[str]) -> None: ...
    @property
    def flags(self) -> list[str]: ...


class CoercionResult:
    value: Any
    confidence: float
    was_coerced: bool

    def __init__(self, value: Any, confidence: float, was_coerced: bool, flags: list[str]) -> None: ...
    @property
    def flags(self) -> list[str]: ...


class PySchema:
    ...


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
    ) -> None: ...

    def complete(
        self,
        model: str,
        prompt: str,
        max_tokens: int | None = None,
        temperature: float | None = None,
    ) -> ResponseWithMetadata: ...

    def complete_messages(
        self,
        model: str,
        messages: Sequence[Mapping[str, object]],
        max_tokens: int | None = None,
        temperature: float | None = None,
        top_p: float | None = None,
    ) -> ResponseWithMetadata: ...
    def complete_with_tools(
        self,
        model: str,
        messages: Sequence[Mapping[str, object]],
        tools: Sequence[Mapping[str, object]],
        tool_choice: object | None = None,
        max_tokens: int | None = None,
        temperature: float | None = None,
        top_p: float | None = None,
    ) -> ResponseWithMetadata: ...

    def complete_json(
        self,
        model: str,
        messages: Sequence[Mapping[str, object]],
        max_tokens: int | None = None,
        temperature: float | None = None,
        top_p: float | None = None,
    ) -> str: ...

    def complete_json_healed(
        self,
        model: str,
        messages: Sequence[Mapping[str, object]],
        max_tokens: int | None = None,
        temperature: float | None = None,
        top_p: float | None = None,
    ) -> HealedJsonResult: ...

    def complete_json_schema(
        self,
        model: str,
        messages: Sequence[Mapping[str, object]],
        schema: Mapping[str, object] | type[Any],
        schema_name: str,
        max_tokens: int | None = None,
        temperature: float | None = None,
        top_p: float | None = None,
        strict: bool = True,
    ) -> str: ...

    def stream(
        self,
        model: str,
        messages: Sequence[Mapping[str, object]],
        max_tokens: int | None = None,
        temperature: float | None = None,
        top_p: float | None = None,
    ) -> Iterator[StreamChunk]: ...

    def stream_structured(
        self,
        model: str,
        messages: Sequence[Mapping[str, object]],
        schema: Mapping[str, object],
        max_tokens: int | None = None,
        temperature: float | None = None,
        top_p: float | None = None,
    ) -> Iterator[PyStructuredEvent]: ...


class ClientBuilder:
    def __init__(self) -> None: ...
    def add_provider(
        self,
        provider: str,
        api_key: str | None = None,
        api_base: str | None = None,
    ) -> ClientBuilder: ...
    def with_routing(self, mode: str) -> ClientBuilder: ...
    def with_latency_routing(self, config: dict[str, Any]) -> ClientBuilder: ...
    def with_cost_routing(self, config: dict[str, Any]) -> ClientBuilder: ...
    def with_fallback_routing(self, config: dict[str, Any]) -> ClientBuilder: ...
    def with_cache(self, ttl_seconds: int) -> ClientBuilder: ...
    def with_healing_config(self, config: dict[str, Any]) -> ClientBuilder: ...
    def add_middleware(self, middleware: object) -> ClientBuilder: ...
    def with_custom_cache(self, cache: object, ttl_seconds: int | None = None) -> ClientBuilder: ...
    def build(self) -> Client: ...


def heal_json(text: str, config: dict[str, Any] | None = None) -> ParseResult: ...
def coerce_to_schema(
    data: Any,
    schema: dict[str, Any] | PySchema,
    config: dict[str, Any] | None = None,
) -> CoercionResult: ...
