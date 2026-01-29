import pytest


def test_heal_json_basic():
    import simple_agents_py

    # Test markdown stripping
    result = simple_agents_py.heal_json('```json\n{"key": "value"}\n```')

    assert result.was_healed is True
    assert result.confidence > 0.9
    assert result.value == {"key": "value"}
    assert any("Stripped markdown" in flag for flag in result.flags)


def test_heal_json_trailing_comma():
    import simple_agents_py

    # Test trailing comma fix
    result = simple_agents_py.heal_json('{"key": "value",}')

    assert result.was_healed is True
    assert result.confidence > 0.9
    assert result.value == {"key": "value"}
    assert any("trailing comma" in flag.lower() for flag in result.flags)


def test_heal_json_perfect():
    import simple_agents_py

    # Perfect JSON - no healing needed
    result = simple_agents_py.heal_json('{"key": "value", "number": 42}')

    assert result.was_healed is False
    assert result.confidence == 1.0
    assert result.value == {"key": "value", "number": 42}
    assert len(result.flags) == 0


def test_coerce_to_schema_type_coercion():
    import simple_agents_py

    # Coerce string to number
    result = simple_agents_py.coerce_to_schema(
        {"age": "30", "score": "95.5"},
        {
            "type": "object",
            "properties": {
                "age": {"type": "integer"},
                "score": {"type": "number"},
            },
        },
    )

    assert result.was_coerced is True
    assert result.value == {"age": 30, "score": 95.5}
    assert any("Coerced type" in f for f in result.flags)


def test_coerce_to_schema_perfect():
    import simple_agents_py

    # Perfect match - no coercion needed
    result = simple_agents_py.coerce_to_schema(
        {"name": "Alice", "age": 30},
        {
            "type": "object",
            "properties": {
                "name": {"type": "string"},
                "age": {"type": "integer"},
            },
        },
    )

    assert result.was_coerced is False
    assert result.confidence == 1.0
    assert result.value == {"name": "Alice", "age": 30}


def test_parse_result_repr():
    import simple_agents_py

    result = simple_agents_py.ParseResult(
        value={"test": "value"},
        confidence=0.95,
        was_healed=True,
        flags=["Test flag"],
    )

    repr_str = repr(result)
    assert "ParseResult" in repr_str
    assert "confidence=0.95" in repr_str
    assert "flags=1" in repr_str


def test_coercion_result_repr():
    import simple_agents_py

    result = simple_agents_py.CoercionResult(
        value={"test": "value"},
        confidence=0.95,
        was_coerced=True,
        flags=["Test flag"],
    )

    repr_str = repr(result)
    assert "CoercionResult" in repr_str
    assert "confidence=0.95" in repr_str
    assert "flags=1" in repr_str
