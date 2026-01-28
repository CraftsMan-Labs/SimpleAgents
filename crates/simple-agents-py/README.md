# simple-agents-py

[![PyPI](https://img.shields.io/pypi/v/simple-agents-py)](https://pypi.org/project/simple-agents-py/)
[![PyPI - Downloads](https://img.shields.io/pypi/dm/simple-agents-py)](https://pypi.org/project/simple-agents-py/)
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
print(response)
```

## Examples

OpenAI with a short prompt:

```python
from simple_agents_py import Client

client = Client("openai")
text = client.complete("gpt-4o-mini", "Summarize this in one sentence.")
print(text)
```

Anthropic with custom tokens and temperature:

```python
from simple_agents_py import Client

client = Client("anthropic")
text = client.complete(
    "claude-3-5-sonnet-20240620",
    "Write a friendly welcome message for a new user.",
    max_tokens=120,
    temperature=0.5,
)
print(text)
```

OpenRouter with a specific model:

```python
from simple_agents_py import Client

client = Client("openrouter")
text = client.complete("openai/gpt-4o-mini", "Give me three project ideas.")
print(text)
```

OpenRouter with explicit API base and key:

```python
from simple_agents_py import Client

client = Client(
    "openrouter",
    api_base="https://openrouter.ai/api/v1",
    api_key="sk-your-openrouter-key",
)
text = client.complete("openai/gpt-4o-mini", "Give me three project ideas.")
print(text)
```

OpenAI with a custom gateway:

```python
from simple_agents_py import Client

client = Client(
    "openai",
    api_base="http://localhost:4000/v1",
    api_key="sk-your-openai-key",
)
text = client.complete("gpt-4o-mini", "Say hello from a local proxy.")
print(text)
```

Basic error handling:

```python
from simple_agents_py import Client

try:
    client = Client("openai")
    print(client.complete("gpt-4o-mini", "Hello!"))
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

## Package Metrics & Analytics

### Download Statistics

View download statistics on:
- **PyPI Stats**: [https://pypistats.org/packages/simple-agents-py](https://pypistats.org/packages/simple-agents-py)
- **PePy**: [https://pepy.tech/project/simple-agents-py](https://pepy.tech/project/simple-agents-py)
- **PyPI Package Page**: [https://pypi.org/project/simple-agents-py/](https://pypi.org/project/simple-agents-py/)

The badges above automatically track:
- **Version**: Current version on PyPI
- **Downloads**: Monthly download count
- **Python Versions**: Supported Python versions
- **License**: Package license

### Tracking with shields.io

The badges use [shields.io](https://shields.io/) which automatically fetches data from PyPI. Available metrics:
- `pypi/v/simple-agents-py` - Latest version
- `pypi/dm/simple-agents-py` - Monthly downloads
- `pypi/dw/simple-agents-py` - Weekly downloads
- `pypi/dd/simple-agents-py` - Daily downloads
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
