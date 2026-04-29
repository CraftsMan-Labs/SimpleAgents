"""Error mapping consistency checks for Python bindings."""

from __future__ import annotations

import pytest  # type: ignore[reportMissingImports]


def test_unknown_provider_error_is_runtime_error() -> None:
    import simple_agents_py

    with pytest.raises(RuntimeError) as excinfo:
        simple_agents_py.Client("not-a-provider")
    assert "Unknown provider" in str(excinfo.value)


def test_client_accepts_timeout_and_retry_options() -> None:
    import simple_agents_py

    client = simple_agents_py.Client(
        "openai",
        api_key="test-key-00000000000000",
        api_base="http://localhost:1/v1",
        timeout_seconds=60,
        retry_attempts=2,
        retry_strategy="exponential",
    )
    assert client is not None


def test_client_accepts_typed_request_mapping() -> None:
    import simple_agents_py

    client = simple_agents_py.Client(
        {
            "provider": "openai",
            "api_key": "test-key-00000000000000",
            "api_base": "http://localhost:1/v1",
            "timeout_seconds": 60,
            "retry_attempts": 2,
            "retry_strategy": "exponential",
        }
    )
    assert client is not None


def test_client_rejects_unknown_request_fields() -> None:
    import simple_agents_py

    with pytest.raises(ValueError) as excinfo:
        simple_agents_py.Client(
            {
                "provider": "openai",
                "api_key": "test-key-00000000000000",
                "api_base": "http://localhost:1/v1",
                "unexpected": True,
            }
        )
    assert "unknown field" in str(excinfo.value)


@pytest.mark.parametrize(
    ("kwargs", "message"),
    [
        (
            {"timeout_seconds": 0},
            "timeout_seconds must be a positive finite number",
        ),
        (
            {"retry_attempts": 0},
            "retry_attempts must be greater than or equal to 1",
        ),
        ({"retry_strategy": "linear"}, "unknown retry_strategy"),
    ],
)
def test_client_rejects_invalid_timeout_and_retry_options(
    kwargs, message: str
) -> None:
    import simple_agents_py

    with pytest.raises(ValueError) as excinfo:
        simple_agents_py.Client(
            "openai",
            api_key="test-key-00000000000000",
            api_base="http://localhost:1/v1",
            **kwargs,
        )
    assert message in str(excinfo.value)


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
    """stream=True with heal=True was previously rejected."""
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
