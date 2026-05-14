"""Tests for ClientBuilder."""

import pytest  # type: ignore[reportMissingImports]

API_KEY = "sk-test-1234567890123456"
ANTHROPIC_KEY = "sk-ant-test-1234567890123456"
LOCAL_KEY = "sk-local-1234567890123456"


def test_builder_requires_providers():
    """Test that builder requires at least one provider."""
    from simple_agents_py import ClientBuilder

    builder = ClientBuilder()
    with pytest.raises(
        RuntimeError, match="At least one provider is required"
    ):
        builder.build()


def test_builder_single_provider():
    """Test building client with single provider."""
    from simple_agents_py import ClientBuilder

    client = ClientBuilder().add_provider("openai", api_key=API_KEY).build()
    assert client is not None


def test_builder_multiple_providers():
    """Test building client with multiple providers."""
    from simple_agents_py import ClientBuilder

    client = (
        ClientBuilder()
        .add_provider("openai", api_key=API_KEY)
        .add_provider("anthropic", api_key=ANTHROPIC_KEY)
        .build()
    )
    assert client is not None


def test_builder_healing_config():
    """Test builder with healing configuration."""
    from simple_agents_py import ClientBuilder

    healing_config = {
        "enabled": True,
        "min_confidence": 0.7,
        "fuzzy_match_threshold": 0.85,
    }

    client = (
        ClientBuilder()
        .add_provider("openai", api_key=API_KEY)
        .with_healing_config(healing_config)
        .build()
    )
    assert client is not None


def test_builder_healing_config_partial():
    """Test builder with partial healing configuration."""
    from simple_agents_py import ClientBuilder

    client = (
        ClientBuilder()
        .add_provider("openai", api_key=API_KEY)
        .with_healing_config({"min_confidence": 0.5})
        .build()
    )
    assert client is not None


def test_builder_healing_config_empty():
    """Test builder with empty healing configuration (uses defaults)."""
    from simple_agents_py import ClientBuilder

    client = (
        ClientBuilder()
        .add_provider("openai", api_key=API_KEY)
        .with_healing_config({})
        .build()
    )
    assert client is not None


def test_builder_disable_healing():
    """Test disabling healing."""
    from simple_agents_py import ClientBuilder

    client = (
        ClientBuilder()
        .add_provider("openai", api_key=API_KEY)
        .with_healing_config({"enabled": False})
        .build()
    )
    assert client is not None


def test_builder_full_configuration():
    """Test builder with all currently supported configuration options."""
    from simple_agents_py import ClientBuilder

    healing_config = {
        "enabled": True,
        "min_confidence": 0.8,
        "fuzzy_match_threshold": 0.9,
    }

    client = (
        ClientBuilder()
        .add_provider("openai", api_key=API_KEY)
        .add_provider("anthropic", api_key=ANTHROPIC_KEY)
        .with_healing_config(healing_config)
        .build()
    )
    assert client is not None


def test_builder_repr():
    """Test builder string representation."""
    from simple_agents_py import ClientBuilder

    builder = ClientBuilder().add_provider("openai", api_key=API_KEY)
    repr_str = repr(builder)

    assert "ClientBuilder" in repr_str
    assert "providers=1" in repr_str


def test_builder_with_api_base():
    """Test builder with custom API base."""
    from simple_agents_py import ClientBuilder

    client = (
        ClientBuilder()
        .add_provider(
            "openai", api_key=LOCAL_KEY, api_base="http://localhost:8080/v1"
        )
        .build()
    )
    assert client is not None


def test_builder_with_base_url_alias():
    """Test builder accepts base_url as api_base alias."""
    from simple_agents_py import ClientBuilder

    client = (
        ClientBuilder()
        .add_provider(
            "openai", api_key=LOCAL_KEY, base_url="http://localhost:8080/v1"
        )
        .build()
    )
    assert client is not None


def test_builder_multiple_chained_calls():
    """Test that supported builder methods can be chained."""
    from simple_agents_py import ClientBuilder

    client = (
        ClientBuilder()
        .add_provider("openai", api_key=API_KEY)
        .add_provider("anthropic", api_key=ANTHROPIC_KEY)
        .with_healing_config({"min_confidence": 0.7})
        .build()
    )
    assert client is not None
