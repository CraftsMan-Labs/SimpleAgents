import pytest

API_KEY = "sk-test-1234567890123456"


def test_unknown_provider_raises():
    import simple_agents_py

    with pytest.raises(Exception) as excinfo:
        simple_agents_py.Client("unknown")
    assert "Unknown provider" in str(excinfo.value)


def test_missing_openai_env_raises(monkeypatch):
    import simple_agents_py

    monkeypatch.delenv("OPENAI_API_KEY", raising=False)
    with pytest.raises(Exception) as excinfo:
        simple_agents_py.Client("openai")
    assert "OPENAI_API_KEY" in str(excinfo.value)


def test_healed_json_result_structure():
    import simple_agents_py

    result = simple_agents_py.HealedJsonResult(
        content='{"test": "value"}',
        confidence=0.95,
        was_healed=True,
        flags=["Stripped markdown code fences", "Fixed trailing comma in JSON"],
    )

    assert result.content == '{"test": "value"}'
    assert result.confidence == pytest.approx(0.95)
    assert result.was_healed is True
    assert len(result.flags) == 2
    assert "Stripped markdown code fences" in result.flags
    assert "Fixed trailing comma in JSON" in result.flags


def test_healed_json_result_repr():
    import simple_agents_py

    result = simple_agents_py.HealedJsonResult(
        content='{"test": "value"}',
        confidence=0.95,
        was_healed=False,
        flags=[],
    )

    repr_str = repr(result)
    assert "HealedJsonResult" in repr_str
    assert "confidence=0.95" in repr_str
    assert "flags=0" in repr_str


def test_complete_json_healed_with_mock_provider():
    import simple_agents_py

    class MockProvider:
        def __init__(self):
            self.api_key = "test-key"

    client = simple_agents_py.Client("openai", api_key=API_KEY)

    # Test that the method exists and has the right signature
    assert hasattr(client, "complete_json_healed")

    # We can't make actual API calls in tests, but we can verify the method signature
    # by checking that it accepts the expected parameters
    import inspect

    sig = inspect.signature(client.complete_json_healed)
    params = list(sig.parameters.keys())
    assert "model" in params
    assert "messages" in params
    assert "max_tokens" in params
    assert "temperature" in params
    assert "top_p" in params
