import os

import pytest


def _require_env():
    api_base = os.getenv("CUSTOM_API_BASE")
    api_key = os.getenv("CUSTOM_API_KEY")
    model = os.getenv("CUSTOM_API_MODEL")
    if not api_base or not api_key or not model:
        pytest.skip("Missing CUSTOM_API_BASE/CUSTOM_API_KEY/CUSTOM_API_MODEL")
    return api_base, api_key, model


def _client():
    import simple_agents_py

    api_base, api_key, model = _require_env()
    client = simple_agents_py.Client("openai", api_key=api_key, api_base=api_base)
    return client, model


def test_local_proxy_connection():
    client, model = _client()
    response = client.complete_with_metadata(
        model,
        "Say 'Hello from SimpleAgents!' and nothing else.",
        max_tokens=50,
        temperature=0.7,
    )
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
    prompts = ["Count from 1 to 3.", "What is 2+2?", "Say 'test complete'."]
    for prompt in prompts:
        response = client.complete_with_metadata(
            model, prompt, max_tokens=50, temperature=0.7
        )
        assert response.content


def test_local_proxy_invalid_model():
    client, model = _client()
    bad_model = f"{model}-does-not-exist"
    with pytest.raises(Exception):
        client.complete_with_metadata(bad_model, "Test", max_tokens=20)


def test_local_proxy_temperature_variations():
    client, model = _client()
    for temp in [0.0, 0.5, 1.0]:
        response = client.complete_with_metadata(
            model, "Say hello.", max_tokens=20, temperature=temp
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
    response = client.complete_messages_with_metadata(
        model, messages, max_tokens=30, temperature=0.2
    )
    assert response.content
