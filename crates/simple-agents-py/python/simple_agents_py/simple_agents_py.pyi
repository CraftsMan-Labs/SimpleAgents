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
)
from enum import Enum

from .models import (
    JSONValue,
    WorkflowEventWire,
    WorkflowRunOutputWire,
)
from .eval_request import EvalReport, EvalSuiteRequest
from .workflow_request import WorkflowExecutionRequest

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
# Typed WorkflowRunOutput
# ---------------------------------------------------------------------------

class WorkflowRunStatus:
    COMPLETED: str
    AWAITING_HUMAN_INPUT: str

class HumanRequest:
    @property
    def node_id(self) -> str: ...
    @property
    def input_type(self) -> str: ...
    @property
    def prompt(self) -> str | None: ...
    @property
    def options(self) -> list[dict[str, Any]] | None: ...
    @property
    def form_schema(self) -> Any: ...
    @property
    def form_data(self) -> Any: ...

class WorkflowRunOutput:
    @property
    def status(self) -> str: ...
    @property
    def human_request(self) -> HumanRequest | None: ...
    @property
    def workflow_id(self) -> str: ...
    @property
    def entry_node(self) -> str: ...
    @property
    def trace(self) -> list[str]: ...
    @property
    def outputs(self) -> dict[str, Any]: ...
    @property
    def node_outputs(self) -> dict[str, Any]: ...
    @property
    def globals(self) -> dict[str, Any]: ...
    @property
    def terminal_node(self) -> str: ...
    @property
    def terminal_output(self) -> Any: ...
    @property
    def output(self) -> Any: ...
    @property
    def step_timings(self) -> list[dict[str, Any]]: ...
    @property
    def llm_node_metrics(self) -> dict[str, dict[str, Any]]: ...
    @property
    def llm_node_models(self) -> dict[str, str]: ...
    @property
    def total_elapsed_ms(self) -> int: ...
    @property
    def ttft_ms(self) -> int | None: ...
    @property
    def total_input_tokens(self) -> int: ...
    @property
    def total_output_tokens(self) -> int: ...
    @property
    def total_tokens(self) -> int: ...
    @property
    def total_reasoning_tokens(self) -> int | None: ...
    @property
    def tokens_per_second(self) -> float: ...
    @property
    def trace_id(self) -> str | None: ...
    @property
    def metadata(self) -> Any: ...
    @property
    def events(self) -> list[dict[str, Any]] | None: ...
    def to_dict(self) -> WorkflowRunOutputWire: ...

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
    def with_healing_config(self, config: Mapping[str, object]) -> ClientBuilder: ...
    def build(self) -> Client: ...

# ---------------------------------------------------------------------------
# Main Client
# ---------------------------------------------------------------------------

class ClientCreateRequest(TypedDict, total=False):
    provider: str
    api_key: str | None
    api_base: str | None
    base_url: str | None
    model: str | None
    api_format: str | None
    timeout_seconds: float | None
    retry_attempts: int | None
    retry_strategy: str | None


class Client:
    @overload
    def __init__(
        self,
        request: ClientCreateRequest | Mapping[str, object],
    ) -> None: ...

    @overload
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
        retry_attempts: int | None = None,
        retry_strategy: str | None = None,
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

    def run_workflow(
        self,
        request: WorkflowExecutionRequest,
    ) -> WorkflowRunOutput: ...

    def stream_workflow(
        self,
        request: WorkflowExecutionRequest,
        on_event: Callable[[WorkflowEventWire], object] | None = None,
        include_events_in_output: bool = False,
    ) -> WorkflowRunOutput: ...

    # ``WorkflowRunOutput`` is the Rust pyclass; ``to_dict()`` returns a mapping
    # matching ``WorkflowRunOutputWire`` in ``simple_agents_py.models``.

    def run_eval_suite(
        self,
        request: EvalSuiteRequest | Mapping[str, JSONValue],
    ) -> EvalReport: ...

# ---------------------------------------------------------------------------
# Module-level functions
# ---------------------------------------------------------------------------

def heal_json(raw: str) -> ParseResult: ...
def coerce_to_schema(value: Any, schema: dict[str, Any]) -> CoercionResult: ...
