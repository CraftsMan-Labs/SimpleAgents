"""Tests for ClientBuilder config combinations currently supported."""

import pytest  # type: ignore[reportMissingImports]

API_KEY = "sk-test-1234567890123456"
ANTHROPIC_KEY = "sk-ant-test-1234567890123456"
OPENROUTER_KEY = "sk-or-test-1234567890123456"


def test_add_provider_config_single_provider():
    """Test add_provider_config builds a client."""
    from simple_agents_py import ClientBuilder, ProviderConfig

    client = (
        ClientBuilder()
        .add_provider_config(ProviderConfig("openai", API_KEY, None))
        .build()
    )
    assert client is not None


def test_add_provider_config_multiple_providers():
    """Test add_provider_config supports multiple provider entries."""
    from simple_agents_py import ClientBuilder, ProviderConfig

    client = (
        ClientBuilder()
        .add_provider_config(ProviderConfig("openai", API_KEY, None))
        .add_provider_config(ProviderConfig("anthropic", ANTHROPIC_KEY, None))
        .add_provider_config(
            ProviderConfig("openrouter", OPENROUTER_KEY, None)
        )
        .build()
    )
    assert client is not None


def test_mixed_provider_and_provider_config():
    """Test mixing add_provider and add_provider_config in one chain."""
    from simple_agents_py import ClientBuilder, ProviderConfig

    client = (
        ClientBuilder()
        .add_provider("openai", api_key=API_KEY)
        .add_provider_config(ProviderConfig("anthropic", ANTHROPIC_KEY, None))
        .build()
    )
    assert client is not None


def test_healing_config_with_defaults():
    """Test that omitted healing config still builds."""
    from simple_agents_py import ClientBuilder

    client = ClientBuilder().add_provider("openai", api_key=API_KEY).build()
    assert client is not None


def test_healing_config_enabled_false():
    """Test disabling healing via config."""
    from simple_agents_py import ClientBuilder

    client = (
        ClientBuilder()
        .add_provider("openai", api_key=API_KEY)
        .with_healing_config({"enabled": False})
        .build()
    )
    assert client is not None


def test_healing_config_partial_thresholds():
    """Test partial healing config maps correctly."""
    from simple_agents_py import ClientBuilder

    client = (
        ClientBuilder()
        .add_provider("openai", api_key=API_KEY)
        .with_healing_config({"min_confidence": 0.75})
        .build()
    )
    assert client is not None

    client = (
        ClientBuilder()
        .add_provider("openai", api_key=API_KEY)
        .with_healing_config({"fuzzy_match_threshold": 0.9})
        .build()
    )
    assert client is not None


def test_healing_config_full_settings():
    """Test full healing config dictionary is accepted."""
    from simple_agents_py import ClientBuilder

    client = (
        ClientBuilder()
        .add_provider("openai", api_key=API_KEY)
        .with_healing_config(
            {
                "enabled": True,
                "min_confidence": 0.8,
                "fuzzy_match_threshold": 0.85,
            }
        )
        .build()
    )
    assert client is not None


def test_with_healing_config_requires_dict():
    """Test healing config must be a mapping object."""
    from simple_agents_py import ClientBuilder

    with pytest.raises(TypeError):
        (
            ClientBuilder()
            .add_provider("openai", api_key=API_KEY)
            .with_healing_config(["not", "a", "dict"])
            .build()
        )
