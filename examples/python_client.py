import json
import os
from typing import Optional, Tuple

from simple_agents_py import Client, ClientBuilder


def load_settings() -> Optional[Tuple[str, str, str]]:
    api_base = os.getenv("CUSTOM_API_BASE")
    api_key = os.getenv("CUSTOM_API_KEY")
    model = os.getenv("CUSTOM_API_MODEL")
    if not api_base or not api_key or not model:
        print("Set CUSTOM_API_BASE, CUSTOM_API_KEY, CUSTOM_API_MODEL to run this example.")
        return None
    return api_base, api_key, model


def example_basic_completion(client: Client, model: str) -> None:
    messages = [
        {"role": "system", "content": "You are a concise assistant."},
        {"role": "user", "content": "Give me one project idea."},
    ]
    text = client.complete_messages(model, messages)
    print("basic_completion:", text)


def example_metadata(client: Client, model: str) -> None:
    response = client.complete_messages_with_metadata(
        model,
        [{"role": "user", "content": "Summarize why tests matter."}],
        max_tokens=80,
    )
    print("metadata:", response.content)
    print("metadata: usage", response.usage)
    print("metadata: latency_ms", response.latency_ms)


def example_streaming(client: Client, model: str) -> None:
    messages = [{"role": "user", "content": "Say hello in one sentence."}]
    print("streaming:", end=" ")
    for chunk in client.stream(model, messages, max_tokens=40):
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
    messages = [{"role": "user", "content": "Give me two project ideas as JSON."}]
    json_text = client.complete_json_schema(model, messages, schema, "project_ideas")
    print("structured_json:", json.dumps(json.loads(json_text), indent=2))


def example_structured_streaming(client: Client, model: str) -> None:
    schema = {
        "type": "object",
        "properties": {
            "name": {"type": "string"},
            "age": {"type": "number"},
        },
        "required": ["name", "age"],
    }
    messages = [
        {"role": "user", "content": "Extract name and age: Alice is 28."}
    ]
    for event in client.stream_structured(model, messages, schema, max_tokens=80):
        if event.is_partial:
            print("structured_partial:", event.partial_value)
        else:
            print("structured_complete:", event.value)


def example_healing(client: Client, model: str) -> None:
    messages = [{"role": "user", "content": "Return JSON: {\"name\":\"Sam\",\"age\":30}"}]
    healed = client.complete_json_healed(model, messages, max_tokens=80)
    print("healed:", healed.content)
    print("healed: was_healed", healed.was_healed)


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
    messages = [{"role": "user", "content": "What's the weather in Tokyo?"}]
    response = client.complete_with_tools(model, messages, tools)
    print("tool_calls:", response.tool_calls)


def example_client_builder(api_base: str, api_key: str, model: str) -> None:
    builder = (
        ClientBuilder()
        .add_provider("openai", api_key=api_key, api_base=api_base)
        .with_routing("direct")
        .with_cache(ttl_seconds=60)
        .with_healing_config({"enabled": True, "min_confidence": 0.7})
    )
    client = builder.build()
    text = client.complete(model, "Give me a quick checklist.", max_tokens=80)
    print("builder_completion:", text)


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
    example_structured_streaming(client, model)
    example_healing(client, model)
    example_tool_calling(client, model)
    example_client_builder(api_base, api_key, model)


if __name__ == "__main__":
    main()
