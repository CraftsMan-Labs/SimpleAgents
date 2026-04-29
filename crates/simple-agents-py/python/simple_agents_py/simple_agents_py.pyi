from __future__ import annotations

from typing import (
    Any,
    Callable,
    Iterator,
    Literal,
    Mapping,
    Sequence,
    overload,
    Never,
)
from enum import Enum

from .models import (
    JSONValue,
    WorkflowEvent,
    WorkflowExecutionRequest,
    WorkflowInput,
    WorkflowRunOptions,
    WorkflowRunOutput,
)
from .eval_request import EvalReport, EvalSuiteRequest

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
    coerced_value: Any
    coerced_confidence: float | None
    coercion_flags: list[str]

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
        send_schema: bool | None = None,
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

    # --- Workflow APIs ---

    def resume(
        self,
        checkpoint: Mapping[str, Any],
        *,
        options: Mapping[str, Any] | None = None,
    ) -> WorkflowRunOutput: ...

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

    def run_eval_suite(
        self,
        request: EvalSuiteRequest | Mapping[str, JSONValue],
    ) -> EvalReport: ...

# ---------------------------------------------------------------------------
# Module-level functions
# ---------------------------------------------------------------------------

def heal_json(raw: str) -> ParseResult: ...
def coerce_to_schema(value: Any, schema: dict[str, Any]) -> CoercionResult: ...
