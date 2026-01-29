"""Tests for streaming functionality."""

import sys
import os

# Add the parent directory to the path to import the module
sys.path.insert(
    0, os.path.join(os.path.dirname(__file__), "..", "..", "target", "debug")
)

import pytest


def _require_env():
    api_base = os.getenv("CUSTOM_API_BASE")
    api_key = os.getenv("CUSTOM_API_KEY")
    model = os.getenv("CUSTOM_API_MODEL")
    if not api_base or not api_key or not model:
        pytest.skip("Missing CUSTOM_API_BASE/CUSTOM_API_KEY/CUSTOM_API_MODEL")
    return api_base, api_key, model


class TestStreaming:
    """Test streaming completion functionality."""

    @pytest.fixture
    def client(self):
        """Create a test client."""
        from simple_agents_py import Client

        api_base, api_key, _model = _require_env()
        return Client("openai", api_key=api_key, api_base=api_base)

    @pytest.fixture
    def model(self):
        """Model name for streaming tests."""
        _api_base, _api_key, model = _require_env()
        return model

    def test_stream_basic(self, client, model):
        """Test basic streaming."""
        messages = [{"role": "user", "content": "Say 'Hello'"}]
        chunks = []

        for chunk in client.stream(model, messages, max_tokens=10):
            assert chunk.content is not None
            assert isinstance(chunk.content, str)
            chunks.append(chunk)

        assert len(chunks) > 0
        # At least one chunk should have content
        assert any(c.content for c in chunks)

    def test_stream_with_temperature(self, client, model):
        """Test streaming with temperature parameter."""
        messages = [{"role": "user", "content": "Say 'Hi'"}]

        for chunk in client.stream(model, messages, temperature=0.7):
            assert chunk.model is not None
            break

    def test_stream_chunk_properties(self, client, model):
        """Test StreamChunk properties."""
        messages = [{"role": "user", "content": "Test"}]

        chunks = list(client.stream(model, messages, max_tokens=5))

        # Check that all chunks have expected properties
        for chunk in chunks:
            assert hasattr(chunk, "content")
            assert hasattr(chunk, "finish_reason")
            assert hasattr(chunk, "model")
            assert hasattr(chunk, "index")
            assert isinstance(chunk.index, (int, type(None)))

    def test_stream_empty_messages(self, client, model):
        """Test streaming with empty messages raises error."""
        with pytest.raises(Exception):
            list(client.stream(model, []))

    def test_stream_finish_reason(self, client, model):
        """Test that finish_reason is set on final chunk."""
        messages = [{"role": "user", "content": "Say 'Done'"}]
        chunks = list(client.stream(model, messages, max_tokens=10))

        # Last chunk should have finish_reason
        assert chunks[-1].finish_reason is not None

    def test_stream_top_p(self, client, model):
        """Test streaming with top_p parameter."""
        messages = [{"role": "user", "content": "Test"}]

        for chunk in client.stream(model, messages, top_p=0.9):
            assert chunk.content is not None
            break


class TestStreamChunk:
    """Test StreamChunk class."""

    def test_stream_chunk_repr(self):
        """Test StreamChunk string representation."""
        from simple_agents_py import StreamChunk

        # We can't directly instantiate StreamChunk, so we just test
        # that the class exists
        assert StreamChunk is not None

    def test_stream_chunk_properties_exist(self):
        """Test StreamChunk has expected properties."""
        from simple_agents_py import StreamChunk

        # Check class attributes exist
        assert hasattr(StreamChunk, "__annotations__")


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
