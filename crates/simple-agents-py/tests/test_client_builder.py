"""Tests for ClientBuilder."""

import pytest

API_KEY = "sk-test-1234567890123456"
ANTHROPIC_KEY = "sk-ant-test-1234567890123456"
OPENROUTER_KEY = "sk-or-test-1234567890123456"
LOCAL_KEY = "sk-local-1234567890123456"


def test_builder_requires_providers():
    """Test that builder requires at least one provider."""
    from simple_agents_py import ClientBuilder

    builder = ClientBuilder()
    with pytest.raises(RuntimeError, match="At least one provider is required"):
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


def test_builder_routing_modes():
    """Test different routing modes."""
    from simple_agents_py import ClientBuilder

    valid_modes = ["direct", "round_robin", "latency", "cost", "fallback"]

    for mode in valid_modes:
        builder = ClientBuilder().add_provider("openai", api_key=API_KEY)
        if mode == "cost":
            builder = builder.with_cost_routing({"provider_costs": {"openai": 0.002}})
        else:
            builder = builder.with_routing(mode)
        client = builder.build()
        assert client is not None


def test_builder_invalid_routing_mode():
    """Test that invalid routing mode raises error."""
    from simple_agents_py import ClientBuilder

    with pytest.raises(RuntimeError, match="Unknown routing mode"):
        ClientBuilder().add_provider("openai", api_key=API_KEY).with_routing(
            "invalid_mode"
        )


def test_builder_with_cache():
    """Test builder with cache configuration."""
    from simple_agents_py import ClientBuilder

    client = (
        ClientBuilder()
        .add_provider("openai", api_key=API_KEY)
        .with_cache(ttl_seconds=300)
        .build()
    )

    assert client is not None


def test_builder_cache_disabled():
    """Test that TTL of 0 disables cache."""
    from simple_agents_py import ClientBuilder

    client = (
        ClientBuilder()
        .add_provider("openai", api_key=API_KEY)
        .with_cache(ttl_seconds=0)
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

    # Only set some config options
    healing_config = {
        "min_confidence": 0.5,
    }

    client = (
        ClientBuilder()
        .add_provider("openai", api_key=API_KEY)
        .with_healing_config(healing_config)
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

    healing_config = {
        "enabled": False,
    }

    client = (
        ClientBuilder()
        .add_provider("openai", api_key=API_KEY)
        .with_healing_config(healing_config)
        .build()
    )

    assert client is not None


def test_builder_full_configuration():
    """Test builder with all configuration options."""
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
        .with_routing("round_robin")
        .with_cache(ttl_seconds=600)
        .with_healing_config(healing_config)
        .build()
    )

    assert client is not None


def test_builder_repr():
    """Test builder string representation."""
    from simple_agents_py import ClientBuilder

    builder = (
        ClientBuilder()
        .add_provider("openai", api_key=API_KEY)
        .with_routing("round_robin")
        .with_cache(ttl_seconds=300)
    )

    repr_str = repr(builder)
    assert "ClientBuilder" in repr_str
    assert "providers=1" in repr_str
    assert "routing=" in repr_str
    assert "cache_ttl=" in repr_str


def test_builder_with_api_base():
    """Test builder with custom API base."""
    from simple_agents_py import ClientBuilder

    client = (
        ClientBuilder()
        .add_provider("openai", api_key=LOCAL_KEY, api_base="http://localhost:8080/v1")
        .build()
    )

    assert client is not None


def test_builder_multiple_chained_calls():
    """Test that builder methods can be chained."""
    from simple_agents_py import ClientBuilder

    client = (
        ClientBuilder()
        .add_provider("openai", api_key=API_KEY)
        .add_provider("anthropic", api_key=ANTHROPIC_KEY)
        .with_routing("round_robin")
        .with_cache(ttl_seconds=300)
        .with_healing_config({"min_confidence": 0.7})
        .build()
    )

    assert client is not None


def test_builder_round_robin_routing():
    """Test round-robin routing configuration."""
    from simple_agents_py import ClientBuilder

    client = (
        ClientBuilder()
        .add_provider("openai", api_key=API_KEY)
        .add_provider("anthropic", api_key=ANTHROPIC_KEY)
        .add_provider("openrouter", api_key=OPENROUTER_KEY)
        .with_routing("round_robin")
        .build()
    )

    assert client is not None


def test_builder_fallback_routing():
    """Test fallback routing configuration."""
    from simple_agents_py import ClientBuilder

    client = (
        ClientBuilder()
        .add_provider("openai", api_key=API_KEY)
        .add_provider("anthropic", api_key=ANTHROPIC_KEY)
        .with_routing("fallback")
        .build()
    )

    assert client is not None


def test_builder_cost_routing():
    """Test cost-based routing configuration."""
    from simple_agents_py import ClientBuilder

    client = (
        ClientBuilder()
        .add_provider("openai", api_key=API_KEY)
        .add_provider("anthropic", api_key=ANTHROPIC_KEY)
        .with_cost_routing({"provider_costs": {"openai": 0.002, "anthropic": 0.0003}})
        .build()
    )

    assert client is not None


def test_builder_latency_routing():
    """Test latency-based routing configuration."""
    from simple_agents_py import ClientBuilder

    client = (
        ClientBuilder()
        .add_provider("openai", api_key=API_KEY)
        .add_provider("anthropic", api_key=ANTHROPIC_KEY)
        .with_routing("latency")
        .build()
    )

    assert client is not None


def test_builder_direct_routing():
    """Test direct routing (uses first provider only)."""
    from simple_agents_py import ClientBuilder

    client = (
        ClientBuilder()
        .add_provider("openai", api_key=API_KEY)
        .add_provider("anthropic", api_key=ANTHROPIC_KEY)  # This won't be used in direct mode
        .with_routing("direct")
        .build()
    )

    assert client is not None
