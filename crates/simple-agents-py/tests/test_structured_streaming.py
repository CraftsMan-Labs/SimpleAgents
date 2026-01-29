"""Tests for structured streaming functionality."""

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


class TestStructuredStreaming:
    """Test structured streaming completion functionality."""

    @pytest.fixture
    def client(self):
        """Create a test client."""
        from simple_agents_py import Client

        api_base, api_key, _model = _require_env()
        return Client("openai", api_key=api_key, api_base=api_base)

    @pytest.fixture
    def model(self):
        """Model name for structured streaming tests."""
        _api_base, _api_key, model = _require_env()
        return model

    @pytest.fixture
    def simple_schema(self):
        """A simple JSON schema for testing."""
        return {
            "type": "object",
            "properties": {"name": {"type": "string"}, "age": {"type": "number"}},
            "required": ["name", "age"],
        }

    @pytest.fixture
    def array_schema(self):
        """A schema with an array for testing."""
        return {
            "type": "object",
            "properties": {"items": {"type": "array", "items": {"type": "string"}}},
            "required": ["items"],
        }

    def test_stream_structured_basic(self, client, model, simple_schema):
        """Test basic structured streaming."""
        messages = [
            {
                "role": "user",
                "content": "Extract name and age from: John is 30 years old",
            }
        ]
        events = []

        for event in client.stream_structured(
            model, messages, simple_schema, max_tokens=50
        ):
            assert event.is_partial or event.is_complete
            assert hasattr(event, "value")
            assert hasattr(event, "partial_value")
            assert hasattr(event, "confidence")
            events.append(event)

        assert len(events) > 0
        # Should have at least one complete event
        assert any(e.is_complete for e in events)

    def test_stream_structured_complete_event(self, client, model, simple_schema):
        """Test that complete event has correct properties."""
        messages = [{"role": "user", "content": "Create a person named Alice, age 25"}]

        events = list(
            client.stream_structured(
                model, messages, simple_schema, max_tokens=50
            )
        )

        # Find the complete event
        complete_events = [e for e in events if e.is_complete]
        assert len(complete_events) >= 1

        event = complete_events[-1]
        assert event.is_complete
        assert not event.is_partial
        assert event.confidence > 0.0
        assert event.confidence <= 1.0

    def test_stream_structured_with_temperature(self, client, model, simple_schema):
        """Test structured streaming with temperature parameter."""
        messages = [{"role": "user", "content": "Create a person"}]

        for event in client.stream_structured(
            model, messages, simple_schema, temperature=0.7
        ):
            assert hasattr(event, "value")
            if event.is_complete:
                break

    def test_stream_structured_event_properties(self, client, model, simple_schema):
        """Test PyStructuredEvent properties."""
        messages = [{"role": "user", "content": "Test"}]

        events = list(
            client.stream_structured(
                model, messages, simple_schema, max_tokens=30
            )
        )

        # Check that all events have expected properties
        for event in events:
            assert hasattr(event, "is_partial")
            assert hasattr(event, "is_complete")
            assert hasattr(event, "value")
            assert hasattr(event, "partial_value")
            assert hasattr(event, "confidence")
            assert hasattr(event, "was_healed")
            assert isinstance(event.confidence, (int, float))
            assert isinstance(event.was_healed, bool)

    def test_stream_structured_empty_messages(self, client, model, simple_schema):
        """Test structured streaming with empty messages raises error."""
        with pytest.raises(Exception):
            list(client.stream_structured(model, [], simple_schema))

    def test_stream_structured_invalid_schema(self, client, model):
        """Test structured streaming with invalid schema."""
        messages = [{"role": "user", "content": "Test"}]
        # Non-dict schema should raise error
        with pytest.raises(Exception):
            list(client.stream_structured(model, messages, "not a dict"))

    def test_stream_structured_array_output(self, client, model, array_schema):
        """Test structured streaming with array schema."""
        messages = [{"role": "user", "content": "List three fruits"}]

        events = list(client.stream_structured(model, messages, array_schema, max_tokens=50))

        # Should have at least one complete event
        complete_events = [e for e in events if e.is_complete]
        assert len(complete_events) >= 1

    def test_stream_structured_top_p(self, client, model, simple_schema):
        """Test structured streaming with top_p parameter."""
        messages = [{"role": "user", "content": "Create a person"}]

        for event in client.stream_structured(model, messages, simple_schema, top_p=0.9):
            if event.is_complete:
                break


class TestPyStructuredEvent:
    """Test PyStructuredEvent class."""

    def test_structured_event_repr(self):
        """Test PyStructuredEvent string representation."""
        from simple_agents_py import PyStructuredEvent

        # We can't directly instantiate PyStructuredEvent, so we just test
        # that the class exists
        assert PyStructuredEvent is not None

    def test_structured_event_properties_exist(self):
        """Test PyStructuredEvent has expected properties."""
        from simple_agents_py import PyStructuredEvent

        # Check class attributes exist
        assert hasattr(PyStructuredEvent, "__annotations__")


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
