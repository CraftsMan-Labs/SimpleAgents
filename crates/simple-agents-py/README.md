# simple-agents-py

[![PyPI](https://img.shields.io/pypi/v/simple-agents-py)](https://pypi.org/project/simple-agents-py/)
[![PyPI - Downloads](https://static.pepy.tech/badge/simple-agents-py/month)](https://pepy.tech/project/simple-agents-py)
[![PyPI - Python Version](https://img.shields.io/pypi/pyversions/simple-agents-py)](https://pypi.org/project/simple-agents-py/)
[![License](https://img.shields.io/pypi/l/simple-agents-py)](https://pypi.org/project/simple-agents-py/)

Python bindings for SimpleAgents (PyO3-based).

## Installation

Install from [PyPI](https://pypi.org/project/simple-agents-py/):

```sh
pip install simple-agents-py
```

Or with uv:

```sh
uv pip install simple-agents-py
```

## Build from Source

```sh
uv build
```

## Publish

```sh
uv publish
```

Set `UV_PUBLISH_TOKEN` or pass `--token` to publish to PyPI.

## Usage

```python
from simple_agents_py import Client

client = Client("openai")
response = client.complete("gpt-4", "Hello from Python!", max_tokens=128, temperature=0.7)
print(response.content)
```

## Feature Guide

### Canonical Environment Contract

Use the same cross-binding env contract used by Go/Node examples:

- `PROVIDER` - `openai`, `anthropic`, or `openrouter`
- `CUSTOM_API_KEY`
- `CUSTOM_API_BASE` (optional)
- `CUSTOM_API_MODEL`

Map these to provider-specific variables when needed:

- OpenAI: `OPENAI_API_KEY`, optional `OPENAI_API_BASE`
- Anthropic: `ANTHROPIC_API_KEY`
- OpenRouter: `OPENROUTER_API_KEY`, optional `OPENROUTER_API_BASE`

### Streaming

```python
from simple_agents_py import Client

client = Client("openai")
messages = [{"role": "user", "content": "Say hello in one sentence."}]
for chunk in client.complete("gpt-4o-mini", messages, max_tokens=64, stream=True):
    if chunk.content:
        print(chunk.content, end="", flush=True)
print()
```

`chunk.finish_reason` uses Rust parity values (`stop`, `length`, `content_filter`, `tool_calls`).

### Structured Output (JSON Mode)

```python
from simple_agents_py import Client
import json

client = Client("openai")
schema = {
    "type": "object",
    "properties": {
        "name": {"type": "string"},
        "age": {"type": "number"},
    },
    "required": ["name", "age"],
}
messages = [{"role": "user", "content": "Extract name and age: Alice is 28."}]
json_text = client.complete(
    "gpt-4o-mini",
    messages,
    schema=schema,
    schema_name="person",
)
print(json.loads(json_text))
```

Pydantic models are accepted too:

```python
from pydantic import BaseModel
from simple_agents_py import Client

class Person(BaseModel):
    name: str
    age: int

client = Client("openai")
messages = [{"role": "user", "content": "Extract name and age: Alice is 28."}]
json_text = client.complete(
    "gpt-4o-mini",
    messages,
    schema=Person,
    schema_name="person",
)
print(json_text)
```

**Healing for Structured Outputs**: Healing is enabled by default for structured outputs, automatically fixing malformed JSON, type mismatches, and missing fields. To disable healing:

```python
client = Client("openai", healing=False)
```

### Structured Streaming

```python
from simple_agents_py import Client

client = Client("openai")
schema = {
    "type": "object",
    "properties": {"name": {"type": "string"}, "age": {"type": "number"}},
    "required": ["name", "age"],
}
messages = [{"role": "user", "content": "Extract name and age: Alice is 28."}]
for event in client.complete(
    "gpt-4o-mini",
    messages,
    schema=schema,
    max_tokens=64,
    stream=True,
):
    if event.is_partial:
        print("partial:", event.partial_value)
    else:
        print("complete:", event.value)
```

### Response Healing

```python
from simple_agents_py import Client

client = Client("openai")
messages = [{"role": "user", "content": "Return JSON: {\"name\":\"Sam\",\"age\":30}"}]
healed = client.complete(
    "gpt-4o-mini",
    messages,
    max_tokens=64,
    response_format="json",
    heal=True,
)
print(healed.content)
print(healed.was_healed, healed.confidence)
print(healed.usage.get("total_tokens"))
```

### Tool Calling

```python
from simple_agents_py import Client

client = Client("openai")
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
response = client.complete("gpt-4o-mini", messages, tools=tools)
print(response.tool_calls)
```

### ClientBuilder (Providers + Healing)

```python
from simple_agents_py import ClientBuilder, ProviderConfig

builder = (
    ClientBuilder()
    .add_provider_config(ProviderConfig("openai", api_key="sk-..."))
    .add_provider("anthropic", api_key="sk-ant-...")
    .with_healing_config(
        {
            "enabled": True,
            "min_confidence": 0.7,
            "fuzzy_match_threshold": 0.85,
        }
    )
)
client = builder.build()
print(client.complete("gpt-4o-mini", "Give me one idea.").content)
```

`ClientBuilder` currently supports:

- `add_provider(...)`
- `add_provider_config(...)`
- `with_healing_config({...})`
- `build()`

## Examples

OpenAI with a short prompt:

```python
from simple_agents_py import Client

client = Client("openai")
response = client.complete("gpt-4o-mini", "Summarize this in one sentence.")
print(response.content)
```

Anthropic with custom tokens and temperature:

```python
from simple_agents_py import Client

client = Client("anthropic")
response = client.complete(
    "claude-3-5-sonnet-20240620",
    "Write a friendly welcome message for a new user.",
    max_tokens=120,
    temperature=0.5,
)
print(response.content)
```

OpenRouter with a specific model:

```python
from simple_agents_py import Client

client = Client("openrouter")
response = client.complete("openai/gpt-4o-mini", "Give me three project ideas.")
print(response.content)
```

OpenRouter with explicit API base and key:

```python
from simple_agents_py import Client

client = Client(
    "openrouter",
    api_base="https://openrouter.ai/api/v1",
    api_key="sk-your-openrouter-key",
)
response = client.complete("openai/gpt-4o-mini", "Give me three project ideas.")
print(response.content)
```

OpenAI with a custom gateway:

```python
from simple_agents_py import Client

client = Client(
    "openai",
    api_base="http://localhost:4000/v1",
    api_key="sk-your-openai-key",
)
response = client.complete("gpt-4o-mini", "Say hello from a local proxy.")
print(response.content)
```

Basic error handling:

```python
from simple_agents_py import Client

try:
    client = Client("openai")
    print(client.complete("gpt-4o-mini", "Hello!").content)
except RuntimeError as exc:
    print(f"Request failed: {exc}")
```

## Notes

- `Client` reads provider configuration from environment variables (e.g. `OPENAI_API_KEY`) when `api_key` is not provided.
- `api_base` lets you override the default API base URL.
- `max_tokens` and `temperature` are optional.

## Tests

```sh
uv pip install -e .[dev]
uv run pytest
```

Layered suites:

- Unit: `uv run pytest tests/test_client_builder.py tests/test_client.py tests/test_direct_healing.py tests/test_healing.py tests/test_routing_config.py tests/test_streaming_parser.py`
- Contract: `uv run pytest tests/test_contract_fixtures.py tests/test_error_mapping_consistency.py`
- Live: `uv run pytest tests/test_integration_openai.py tests/test_streaming.py tests/test_structured_streaming.py` (env-gated)

## Package Metrics & Analytics

### Download Statistics

View download statistics on:
- **PyPI Stats**: [https://pypistats.org/packages/simple-agents-py](https://pypistats.org/packages/simple-agents-py)
- **PePy**: [https://pepy.tech/project/simple-agents-py](https://pepy.tech/project/simple-agents-py)
- **PyPI Package Page**: [https://pypi.org/project/simple-agents-py/](https://pypi.org/project/simple-agents-py/)

The badges above automatically track:
- **Version**: Current version on PyPI
- **Downloads**: Monthly download count (from PePy)
- **Python Versions**: Supported Python versions
- **License**: Package license

### Tracking with shields.io

Most badges use [shields.io](https://shields.io/), while downloads use PePy to avoid upstream rate-limit responses from the PyPI monthly downloads endpoint. Available metrics:
- `pypi/v/simple-agents-py` - Latest version
- `pypi/dw/simple-agents-py` - Weekly downloads
- `static.pepy.tech/badge/simple-agents-py/month` - Monthly downloads
- `pypi/pyversions/simple-agents-py` - Python version support
- `pypi/l/simple-agents-py` - License
- `pypi/status/simple-agents-py` - Development status
- `pypi/format/simple-agents-py` - Package format (wheel/sdist)

### Optional Badges (for future)

Once you set up CI/CD and documentation, you can add:

```markdown
<!-- GitHub Actions build status -->
[![Build](https://github.com/yourusername/SimpleAgents/actions/workflows/python.yml/badge.svg)](https://github.com/yourusername/SimpleAgents/actions/workflows/python.yml)

<!-- Documentation -->
[![Docs](https://img.shields.io/badge/docs-latest-blue)](https://simpleagents.readthedocs.io/)

<!-- Code coverage -->
[![Coverage](https://codecov.io/gh/yourusername/SimpleAgents/branch/main/graph/badge.svg)](https://codecov.io/gh/yourusername/SimpleAgents)

<!-- GitHub stats -->
[![GitHub stars](https://img.shields.io/github/stars/yourusername/SimpleAgents?style=social)](https://github.com/yourusername/SimpleAgents)
[![GitHub forks](https://img.shields.io/github/forks/yourusername/SimpleAgents?style=social)](https://github.com/yourusername/SimpleAgents/fork)
```
