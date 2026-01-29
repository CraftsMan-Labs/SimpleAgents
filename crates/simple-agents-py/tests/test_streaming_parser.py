"""Tests for StreamingParser."""

import pytest


def test_streaming_parser_basic():
    """Test basic streaming parsing."""
    from simple_agents_py import StreamingParser

    parser = StreamingParser()

    # Feed chunks
    parser.feed('{"name": "Alice", ')
    parser.feed('"age": 30}')

    # Finalize and get result
    result = parser.finalize()

    assert result.value == {"name": "Alice", "age": 30}
    assert result.confidence == 1.0
    assert result.was_healed is False
    assert len(result.flags) == 0


def test_streaming_parser_multiple_chunks():
    """Test streaming with multiple chunks."""
    from simple_agents_py import StreamingParser

    parser = StreamingParser()

    parser.feed('{"user": {"name": "')
    parser.feed('Alice", "age": 30}, ')
    parser.feed('"active": true}')

    result = parser.finalize()

    assert result.value["user"]["name"] == "Alice"
    assert result.value["user"]["age"] == 30
    assert result.value["active"] is True


def test_streaming_parser_with_markdown():
    """Test streaming with markdown code fences."""
    from simple_agents_py import StreamingParser

    parser = StreamingParser()

    parser.feed("```json\n")
    parser.feed('{"name": "Bob"}')
    parser.feed("\n```")

    result = parser.finalize()

    assert result.value == {"name": "Bob"}
    assert result.was_healed is True
    assert any("markdown" in flag.lower() for flag in result.flags)


def test_streaming_parser_with_trailing_comma():
    """Test streaming with trailing comma."""
    from simple_agents_py import StreamingParser

    parser = StreamingParser()

    parser.feed('{"name": "Charlie", "age": 25,')
    parser.feed("}")

    result = parser.finalize()

    assert result.value == {"name": "Charlie", "age": 25}
    assert result.was_healed is True


def test_streaming_parser_with_array():
    """Test streaming with array."""
    from simple_agents_py import StreamingParser

    parser = StreamingParser()

    parser.feed("[")
    parser.feed('{"id": 1}, ')
    parser.feed('{"id": 2}, ')
    parser.feed('{"id": 3}]')

    result = parser.finalize()

    assert len(result.value) == 3
    assert result.value[0]["id"] == 1
    assert result.value[1]["id"] == 2
    assert result.value[2]["id"] == 3


def test_streaming_parser_buffer_operations():
    """Test buffer operations."""
    from simple_agents_py import StreamingParser

    parser = StreamingParser()

    assert parser.is_empty()
    assert parser.buffer_len() == 0

    parser.feed('{"test": "data"}')

    assert not parser.is_empty()
    assert parser.buffer_len() > 0

    parser.clear()

    assert parser.is_empty()
    assert parser.buffer_len() == 0


def test_streaming_parser_double_finalize():
    """Test that finalize can only be called once."""
    from simple_agents_py import StreamingParser

    parser = StreamingParser()

    parser.feed('{"name": "Test"}')

    result = parser.finalize()
    assert result is not None

    # Second finalize should raise an error
    with pytest.raises(RuntimeError, match="Parser already finalized"):
        parser.finalize()


def test_streaming_parser_repr():
    """Test string representation."""
    from simple_agents_py import StreamingParser

    parser = StreamingParser()

    # Before finalize
    assert "StreamingParser" in repr(parser)
    assert "buffer_len=0" in repr(parser)
    assert "finalized=False" in repr(parser)

    parser.feed('{"data": 123}')

    assert "buffer_len=" in repr(parser)

    parser.finalize()

    # After finalize (parser is already consumed)
    parser2 = StreamingParser()
    parser2.feed('{"test": 1}')
    result = parser2.finalize()
    assert "ParseResult" in repr(result)
    assert result.value == {"test": 1}


def test_streaming_parser_empty_finalize():
    """Test finalizing with empty buffer."""
    from simple_agents_py import StreamingParser

    parser = StreamingParser()

    with pytest.raises(RuntimeError, match="Parsing failed"):
        parser.finalize()


def test_streaming_parser_malformed_json():
    """Test streaming with malformed JSON."""
    from simple_agents_py import StreamingParser

    parser = StreamingParser()

    # Malformed JSON with unquoted key
    parser.feed('{name: "Dave"}')

    result = parser.finalize()

    assert result.value["name"] == "Dave"
    assert result.was_healed is True
    assert result.confidence < 1.0
    assert len(result.flags) > 0
