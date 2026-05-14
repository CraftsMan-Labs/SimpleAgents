from __future__ import annotations

import json
import os
from typing import Iterator, Optional, Tuple, Union

try:
    from dotenv import load_dotenv  # type: ignore[reportMissingImports]
except ImportError:
    def load_dotenv() -> None:
        return None

from simple_agents_py import (
    Client,
    ClientBuilder,
    HealedJsonResult,
    PyStreamIterator,
    ResponseWithMetadata,
    StreamChunk,
    StructuredStreamIterator,
    PyStructuredEvent,
)

load_dotenv()

CompleteResult = Union[
    ResponseWithMetadata,
    HealedJsonResult,
    str,
    Iterator[StreamChunk],
    Iterator[PyStructuredEvent],
]


def expect_response(result: CompleteResult) -> ResponseWithMetadata:
    if isinstance(result, ResponseWithMetadata):
        return result
    raise TypeError(f"Expected ResponseWithMetadata, got {type(result).__name__}")


def expect_json_text(result: CompleteResult) -> str:
    if isinstance(result, str):
        return result
    raise TypeError(f"Expected JSON text, got {type(result).__name__}")


def expect_healed(result: CompleteResult) -> HealedJsonResult:
    if isinstance(result, HealedJsonResult):
        return result
    raise TypeError(f"Expected HealedJsonResult, got {type(result).__name__}")


def expect_stream_chunks(result: CompleteResult) -> Iterator[StreamChunk]:
    if isinstance(result, PyStreamIterator):
        return result
    raise TypeError(f"Expected streaming chunks, got {type(result).__name__}")


def expect_stream_events(result: CompleteResult) -> Iterator[PyStructuredEvent]:
    if isinstance(result, StructuredStreamIterator):
        return result
    raise TypeError(f"Expected structured stream, got {type(result).__name__}")


def load_settings() -> Optional[Tuple[str, str, str]]:
    api_base = os.getenv("CUSTOM_API_BASE")
    api_key = os.getenv("CUSTOM_API_KEY")
    model = os.getenv("CUSTOM_API_MODEL")
    if not api_base or not api_key or not model:
        print(
            "Set CUSTOM_API_BASE, CUSTOM_API_KEY, CUSTOM_API_MODEL to run this example."
        )
        return None
    return api_base, api_key, model


def example_basic_completion(client: Client, model: str) -> None:
    messages: list[dict[str, object]] = [
        {"role": "system", "content": "You are a concise assistant."},
        {"role": "user", "content": "Give me one project idea."},
    ]
    response = expect_response(client.complete(model, messages))
    print("basic_completion:", response.content)


def example_metadata(client: Client, model: str) -> None:
    messages: list[dict[str, object]] = [
        {"role": "user", "content": "Summarize why tests matter."}
    ]
    response = expect_response(client.complete(model, messages, max_tokens=80))
    print("metadata:", response.content)
    print("metadata: usage", response.usage)
    print("metadata: latency_ms", response.latency_ms)


def example_streaming(client: Client, model: str) -> None:
    messages: list[dict[str, object]] = [
        {"role": "user", "content": "Say hello in one sentence."}
    ]
    print("streaming:", end=" ")
    stream = expect_stream_chunks(
        client.complete(model, messages, max_tokens=40, stream=True)
    )
    for chunk in stream:
        if chunk.content:
            print(chunk.content, end="", flush=True)
    print()


def example_structured_json(client: Client, model: str) -> None:
    schema = {
        "type": "object",
        "properties": {
            "ideas": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "title": {"type": "string"},
                        "one_liner": {"type": "string"},
                    },
                    "required": ["title", "one_liner"],
                    "additionalProperties": False,
                },
                "minItems": 2,
            }
        },
        "required": ["ideas"],
        "additionalProperties": False,
    }
    messages: list[dict[str, object]] = [
        {"role": "user", "content": "Give me two project ideas as JSON."}
    ]
    json_text = expect_json_text(
        client.complete(
            model,
            messages,
            schema=schema,
            schema_name="project_ideas",
        )
    )
    print("structured_json:", json.dumps(json.loads(json_text), indent=2))


def example_structured_pydantic(client: Client, model: str) -> None:
    try:
        from pydantic import BaseModel  # type: ignore[reportMissingImports]
    except ImportError:
        print("structured_pydantic: skipped (pydantic not installed)")
        return

    class Person(BaseModel):
        name: str
        age: int

    messages: list[dict[str, object]] = [
        {"role": "user", "content": "Extract name and age: Alice is 28."}
    ]
    json_text = expect_json_text(
        client.complete(
            model,
            messages,
            schema=Person,
            schema_name="person",
        )
    )
    print("structured_pydantic:", json.loads(json_text))


def example_structured_streaming(client: Client, model: str) -> None:
    schema = {
        "type": "object",
        "properties": {
            "name": {"type": "string"},
            "age": {"type": "number"},
        },
        "required": ["name", "age"],
    }
    messages: list[dict[str, object]] = [
        {"role": "user", "content": "Extract name and age: Alice is 28."}
    ]
    stream = expect_stream_events(
        client.complete(
            model,
            messages,
            schema=schema,
            max_tokens=80,
            stream=True,
        )
    )
    for event in stream:
        if event.is_partial:
            print("structured_partial:", event.partial_value)
        else:
            print("structured_complete:", event.value)


def example_healing(client: Client, model: str) -> None:
    messages: list[dict[str, object]] = [
        {
            "role": "user",
            "content": 'Return JSON: {"firstName":"Sam","lastName":"Smith","age":30}',
        }
    ]
    healed = expect_healed(
        client.complete(
            model,
            messages,
            max_tokens=20,
            response_format="json",
            heal=True,
        )
    )
    print("healed JSON:", healed.content)
    print("raw response:", repr(healed.raw_response))
    print("was_healed:", healed.was_healed)
    print("confidence:", healed.confidence)
    print("usage:", healed.usage)


def example_tool_calling(client: Client, model: str) -> None:
    tools = [
        {
            "type": "function",
            "function": {
                "name": "get_weather",
                "description": "Get the weather for a city",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "city": {"type": "string"},
                        "unit": {"type": "string", "enum": ["c", "f"]},
                    },
                    "required": ["city"],
                },
            },
        }
    ]
    messages: list[dict[str, object]] = [
        {"role": "user", "content": "What's the weather in Tokyo?"}
    ]
    response = expect_response(client.complete(model, messages, tools=tools))
    print("tool_calls:", response.tool_calls)


def example_client_builder(api_base: str, api_key: str, model: str) -> None:
    builder = (
        ClientBuilder()
        .add_provider("openai", api_key=api_key, api_base=api_base)
        .with_healing_config({"enabled": True, "min_confidence": 0.7})
    )
    client = builder.build()
    response = expect_response(
        client.complete(model, "Give me a quick checklist.", max_tokens=80)
    )
    print("builder_completion:", response.content)


def main() -> None:
    settings = load_settings()
    if not settings:
        return
    api_base, api_key, model = settings
    client = Client("openai", api_base=api_base, api_key=api_key)

    example_basic_completion(client, model)
    example_metadata(client, model)
    example_streaming(client, model)
    example_structured_json(client, model)
    example_structured_pydantic(client, model)
    example_structured_streaming(client, model)
    example_healing(client, model)
    example_tool_calling(client, model)
    example_client_builder(api_base, api_key, model)


if __name__ == "__main__":
    main()
