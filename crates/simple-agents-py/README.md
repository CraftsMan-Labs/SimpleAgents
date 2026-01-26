# simple-agents-py

Python bindings for SimpleAgents (PyO3-based).

## Build

```sh
maturin build -m crates/simple-agents-py/Cargo.toml
```

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
