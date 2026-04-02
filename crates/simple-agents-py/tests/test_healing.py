import pytest  # type: ignore[reportMissingImports]

API_KEY = "sk-test-1234567890123456"


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


def test_complete_with_healing_signature():
    import simple_agents_py

    class MockProvider:
        def __init__(self):
            self.api_key = "test-key"

    client = simple_agents_py.Client("openai", api_key=API_KEY)

    # Test that the method exists and has the right signature
    assert hasattr(client, "complete")

    # We can't make actual API calls in tests, but we can verify the method signature
    # by checking that it accepts the expected parameters
    import inspect

    sig = inspect.signature(client.complete)
    params = list(sig.parameters.keys())
    assert "model" in params
    assert "input" in params
    assert "max_tokens" in params
    assert "temperature" in params
    assert "top_p" in params
    assert "response_format" in params
    assert "heal" in params
