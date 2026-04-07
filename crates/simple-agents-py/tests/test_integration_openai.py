from __future__ import annotations

import os
from pathlib import Path
from typing import TYPE_CHECKING, Tuple

import pytest  # type: ignore[reportMissingImports]
from simple_agents_py import ResponseWithMetadata

if TYPE_CHECKING:
    import simple_agents_py


def _require_env() -> Tuple[str, str, str]:
    api_base = os.getenv("CUSTOM_API_BASE")
    api_key = os.getenv("CUSTOM_API_KEY")
    model = os.getenv("CUSTOM_API_MODEL")
    if not api_base or not api_key or not model:
        pytest.skip("Missing CUSTOM_API_BASE/CUSTOM_API_KEY/CUSTOM_API_MODEL")
    assert api_base is not None
    assert api_key is not None
    assert model is not None
    return api_base, api_key, model


def _client() -> Tuple[simple_agents_py.Client, str]:
    import simple_agents_py

    api_base, api_key, model = _require_env()
    client = simple_agents_py.Client("openai", api_key=api_key, api_base=api_base)
    return client, model


def _expect_response(result: object) -> ResponseWithMetadata:
    if isinstance(result, ResponseWithMetadata):
        return result
    raise TypeError(f"Expected ResponseWithMetadata, got {type(result).__name__}")


def test_local_proxy_connection():
    client, model = _client()
    response = client.complete(
        model,
        "Say 'Hello from SimpleAgents!' and nothing else.",
        max_tokens=50,
        temperature=0.7,
    )
    response = _expect_response(response)
    assert response.content
    assert response.model
    assert response.usage["prompt_tokens"] > 0
    assert response.usage["completion_tokens"] > 0
    assert (
        response.usage["total_tokens"]
        == response.usage["prompt_tokens"] + response.usage["completion_tokens"]
    )


def test_local_proxy_multiple_requests():
    client, model = _client()
    # Use simple, safe prompts to avoid triggering content filters
    prompts = ["List three colors.", "Name a programming language.", "Say hello world."]
    for prompt in prompts:
        response = _expect_response(
            client.complete(model, prompt, max_tokens=50, temperature=0.7)
        )
        assert response.content


def test_local_proxy_invalid_model():
    client, model = _client()
    bad_model = f"{model}-does-not-exist"
    with pytest.raises(Exception):
        client.complete(bad_model, "Test", max_tokens=20)


def test_local_proxy_temperature_variations():
    client, model = _client()
    for temp in [0.0, 0.5, 1.0]:
        response = _expect_response(
            client.complete(model, "Say hello.", max_tokens=20, temperature=temp)
        )
        assert response.content


def test_local_proxy_conversation():
    client, model = _client()
    messages = [
        {"role": "system", "content": "You are a helpful assistant."},
        {"role": "user", "content": "What is the capital of France?"},
        {"role": "assistant", "content": "The capital is Paris."},
        {"role": "user", "content": "And Germany?"},
    ]
    response = _expect_response(
        client.complete(model, messages, max_tokens=30, temperature=0.2)
    )
    assert response.content


def test_local_proxy_workflow_stream_includes_nerdstats(tmp_path: Path) -> None:
    client, model = _client()

    workflow_path = tmp_path / "live-workflow-stream-nerdstats.yaml"
    workflow_path.write_text(
        f"""id: live-workflow-stream-nerdstats-test
version: 1.0.0
entry_node: classify
nodes:
  - id: classify
    node_type:
      llm_call:
        model: {model}
        messages_path: input.messages
        append_prompt_as_user: true
        stream: true
        stream_json_as_text: true
        heal: true
    config:
      prompt: |
        Return only this JSON object with no extra text:
        {{
          \"state\": \"capabilities_query\",
          \"reason\": \"short\"
        }}
      output_schema:
        type: object
        required: [state, reason]
        properties:
          state:
            type: string
          reason:
            type: string
""",
        encoding="utf-8",
    )

    events: list[dict[str, object]] = []

    def on_event(event: dict[str, object]) -> None:
        events.append(event)

    result = client.stream_workflow(
        {
            "workflow_path": str(workflow_path),
            "messages": [{"role": "user", "content": "Hi"}],
            "workflow_options": {"telemetry": {"nerdstats": True}},
        },
        on_event=on_event,
    )

    assert isinstance(result, dict)
    assert isinstance(result.get("terminal_node"), str)

    completed_events = [
        event for event in events if event.get("event_type") == "workflow_completed"
    ]
    assert completed_events, "expected workflow_completed event"
    metadata = completed_events[-1].get("metadata")
    assert isinstance(metadata, dict)
    nerdstats = metadata.get("nerdstats")
    assert isinstance(nerdstats, dict)
    assert "total_elapsed_ms" in nerdstats
    assert "token_metrics_available" in nerdstats
    assert nerdstats.get("ttft_ms") is None or isinstance(nerdstats.get("ttft_ms"), int)
