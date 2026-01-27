# simple-agents-py

Python bindings for SimpleAgents (PyO3-based).

## Build

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

## Notes

- `Client` reads provider configuration from environment variables (e.g. `OPENAI_API_KEY`).
- `max_tokens` and `temperature` are optional.

## Tests

```sh
uv pip install -e .[dev]
uv run pytest
```
