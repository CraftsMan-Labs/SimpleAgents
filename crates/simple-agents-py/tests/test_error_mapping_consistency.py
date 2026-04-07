"""Error mapping consistency checks for Python bindings."""

from __future__ import annotations

import pytest  # type: ignore[reportMissingImports]


def test_unknown_provider_error_is_runtime_error() -> None:
    import simple_agents_py

    with pytest.raises(RuntimeError) as excinfo:
        simple_agents_py.Client("not-a-provider")
    assert "Unknown provider" in str(excinfo.value)


def test_empty_prompt_error_is_runtime_error() -> None:
    import simple_agents_py

    client = simple_agents_py.Client(
        "openai",
        api_key="test-key-00000000000000",
        api_base="http://localhost:1/v1",
    )
    with pytest.raises(RuntimeError) as excinfo:
        client.complete("gpt-4o-mini", "")
    assert "prompt cannot be empty" in str(excinfo.value)


def test_stream_and_heal_no_longer_raises_conflict() -> None:
    """stream=True with heal=True was previously rejected; the streaming path is used now."""
    import simple_agents_py

    client = simple_agents_py.Client(
        "openai",
        api_key="test-key-00000000000000",
        api_base="http://localhost:1/v1",
    )
    with pytest.raises(RuntimeError) as excinfo:
        client.complete(
            "gpt-4o-mini",
            [{"role": "user", "content": "hello"}],
            stream=True,
            heal=True,
        )
    assert "heal is not supported with stream=True" not in str(excinfo.value)
