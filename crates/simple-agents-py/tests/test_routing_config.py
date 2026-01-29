"""Tests for advanced routing configuration."""

import pytest

API_KEY = "sk-test-1234567890123456"
ANTHROPIC_KEY = "sk-ant-test-1234567890123456"
OPENROUTER_KEY = "sk-or-test-1234567890123456"
LOCAL_KEY = "sk-local-1234567890123456"


def test_latency_routing_with_defaults():
    """Test latency routing with default configuration."""
    from simple_agents_py import ClientBuilder

    client = (
        ClientBuilder()
        .add_provider("openai", api_key=API_KEY)
        .add_provider("anthropic", api_key=ANTHROPIC_KEY)
        .with_routing("latency")
        .build()
    )

    assert client is not None


def test_latency_routing_with_custom_config():
    """Test latency routing with custom configuration."""
    from simple_agents_py import ClientBuilder

    config = {
        "alpha": 0.5,
        "slow_threshold_ms": 3000,
    }

    client = (
        ClientBuilder()
        .add_provider("openai", api_key=API_KEY)
        .add_provider("anthropic", api_key=ANTHROPIC_KEY)
        .with_latency_routing(config)
        .build()
    )

    assert client is not None


def test_latency_routing_alpha_validation():
    """Test that alpha is validated to be between 0.0 and 1.0."""
    from simple_agents_py import ClientBuilder

    with pytest.raises(RuntimeError, match="alpha must be between 0.0 and 1.0"):
        ClientBuilder().add_provider("openai", api_key=API_KEY).with_latency_routing(
            {"alpha": 1.5}
        )

    with pytest.raises(RuntimeError, match="alpha must be between 0.0 and 1.0"):
        ClientBuilder().add_provider("openai", api_key=API_KEY).with_latency_routing(
            {"alpha": -0.1}
        )


def test_latency_routing_partial_config():
    """Test latency routing with partial configuration (uses defaults)."""
    from simple_agents_py import ClientBuilder

    # Only set alpha, use default threshold
    client = (
        ClientBuilder()
        .add_provider("openai", api_key=API_KEY)
        .with_latency_routing({"alpha": 0.3})
        .build()
    )

    assert client is not None

    # Only set threshold, use default alpha
    client = (
        ClientBuilder()
        .add_provider("openai", api_key=API_KEY)
        .with_latency_routing({"slow_threshold_ms": 5000})
        .build()
    )

    assert client is not None


def test_cost_routing_with_provider_costs():
    """Test cost routing with provider costs."""
    from simple_agents_py import ClientBuilder

    config = {
        "provider_costs": {
            "openai": 0.002,
            "anthropic": 0.0003,
        }
    }

    client = (
        ClientBuilder()
        .add_provider("openai", api_key=API_KEY)
        .add_provider("anthropic", api_key=ANTHROPIC_KEY)
        .with_cost_routing(config)
        .build()
    )

    assert client is not None


def test_cost_routing_with_multiple_providers():
    """Test cost routing with multiple providers."""
    from simple_agents_py import ClientBuilder

    config = {
        "provider_costs": {
            "openai": 0.002,
            "anthropic": 0.0003,
            "openrouter": 0.0004,
        }
    }

    client = (
        ClientBuilder()
        .add_provider("openai", api_key=API_KEY)
        .add_provider("anthropic", api_key=ANTHROPIC_KEY)
        .add_provider("openrouter", api_key=OPENROUTER_KEY)
        .with_cost_routing(config)
        .build()
    )

    assert client is not None


def test_cost_routing_missing_provider_costs():
    """Test that provider_costs is required."""
    from simple_agents_py import ClientBuilder

    with pytest.raises(RuntimeError, match="provider_costs is required"):
        ClientBuilder().add_provider("openai", api_key=API_KEY).with_cost_routing({})


def test_cost_routing_invalid_cost():
    """Test that invalid cost values are rejected."""
    from simple_agents_py import ClientBuilder

    with pytest.raises(RuntimeError, match="Invalid cost"):
        ClientBuilder().add_provider("openai", api_key=API_KEY).with_cost_routing(
            {"provider_costs": {"openai": -0.001}}
        )

    with pytest.raises(RuntimeError, match="Invalid cost"):
        ClientBuilder().add_provider("openai", api_key=API_KEY).with_cost_routing(
            {"provider_costs": {"openai": float("inf")}}
        )


def test_fallback_routing_with_defaults():
    """Test fallback routing with default configuration."""
    from simple_agents_py import ClientBuilder

    client = (
        ClientBuilder()
        .add_provider("openai", api_key=API_KEY)
        .add_provider("anthropic", api_key=ANTHROPIC_KEY)
        .with_routing("fallback")
        .build()
    )

    assert client is not None


def test_fallback_routing_with_custom_config():
    """Test fallback routing with custom configuration."""
    from simple_agents_py import ClientBuilder

    config = {
        "retryable_only": False,
    }

    client = (
        ClientBuilder()
        .add_provider("openai", api_key=API_KEY)
        .add_provider("anthropic", api_key=ANTHROPIC_KEY)
        .with_fallback_routing(config)
        .build()
    )

    assert client is not None


def test_fallback_routing_retryable_only_true():
    """Test fallback routing with retryable_only=true."""
    from simple_agents_py import ClientBuilder

    config = {
        "retryable_only": True,
    }

    client = (
        ClientBuilder()
        .add_provider("openai", api_key=API_KEY)
        .with_fallback_routing(config)
        .build()
    )

    assert client is not None


def test_fallback_routing_partial_config():
    """Test fallback routing with partial configuration (uses defaults)."""
    from simple_agents_py import ClientBuilder

    client = (
        ClientBuilder()
        .add_provider("openai", api_key=API_KEY)
        .with_fallback_routing({})
        .build()
    )

    assert client is not None


def test_full_configuration_with_latency_routing():
    """Test full configuration with latency routing."""
    from simple_agents_py import ClientBuilder

    client = (
        ClientBuilder()
        .add_provider("openai", api_key=API_KEY)
        .add_provider("anthropic", api_key=ANTHROPIC_KEY)
        .with_latency_routing({"alpha": 0.3, "slow_threshold_ms": 2500})
        .with_cache(ttl_seconds=300)
        .with_healing_config({"min_confidence": 0.7})
        .build()
    )

    assert client is not None


def test_full_configuration_with_cost_routing():
    """Test full configuration with cost routing."""
    from simple_agents_py import ClientBuilder

    client = (
        ClientBuilder()
        .add_provider("openai", api_key=API_KEY)
        .add_provider("anthropic", api_key=ANTHROPIC_KEY)
        .with_cost_routing({"provider_costs": {"openai": 0.002, "anthropic": 0.0003}})
        .with_cache(ttl_seconds=600)
        .with_healing_config({"fuzzy_match_threshold": 0.9})
        .build()
    )

    assert client is not None


def test_full_configuration_with_fallback_routing():
    """Test full configuration with fallback routing."""
    from simple_agents_py import ClientBuilder

    client = (
        ClientBuilder()
        .add_provider("openai", api_key=API_KEY)
        .add_provider("anthropic", api_key=ANTHROPIC_KEY)
        .with_fallback_routing({"retryable_only": False})
        .with_cache(ttl_seconds=900)
        .with_healing_config({"enabled": False})
        .build()
    )

    assert client is not None


def test_latency_routing_edge_cases():
    """Test latency routing with edge case values."""
    from simple_agents_py import ClientBuilder

    # Minimum valid alpha
    client = (
        ClientBuilder()
        .add_provider("openai", api_key=API_KEY)
        .with_latency_routing({"alpha": 0.0})
        .build()
    )
    assert client is not None

    # Maximum valid alpha
    client = (
        ClientBuilder()
        .add_provider("openai", api_key=API_KEY)
        .with_latency_routing({"alpha": 1.0})
        .build()
    )
    assert client is not None

    # Very large slow threshold
    client = (
        ClientBuilder()
        .add_provider("openai", api_key=API_KEY)
        .with_latency_routing({"slow_threshold_ms": 100000})
        .build()
    )
    assert client is not None


def test_cost_routing_zero_cost():
    """Test cost routing with zero cost (free provider)."""
    from simple_agents_py import ClientBuilder

    config = {
        "provider_costs": {
            "openai": 0.002,
            "local": 0.0,  # Free provider
        }
    }

    client = (
        ClientBuilder()
        .add_provider("openai", api_key=API_KEY)
        .add_provider("openai", api_key=LOCAL_KEY, api_base="http://localhost:8080/v1")
        .with_cost_routing(config)
        .build()
    )

    assert client is not None


def test_chaining_routing_methods():
    """Test that routing methods can be called in chain."""
    from simple_agents_py import ClientBuilder

    # Start with one routing mode, then switch to another
    builder = (
        ClientBuilder()
        .add_provider("openai", api_key=API_KEY)
        .add_provider("anthropic", api_key=ANTHROPIC_KEY)
    )

    # First set round_robin (gets overridden)
    builder = builder.with_routing("round_robin")

    # Then override with cost routing
    builder = builder.with_cost_routing(
        {"provider_costs": {"openai": 0.002, "anthropic": 0.0003}}
    )

    client = builder.build()
    assert client is not None


def test_latency_with_healing():
    """Test latency routing combined with healing config."""
    from simple_agents_py import ClientBuilder

    healing_config = {
        "enabled": True,
        "min_confidence": 0.8,
        "fuzzy_match_threshold": 0.85,
    }

    client = (
        ClientBuilder()
        .add_provider("openai", api_key=API_KEY)
        .add_provider("anthropic", api_key=ANTHROPIC_KEY)
        .with_latency_routing({"alpha": 0.4, "slow_threshold_ms": 1500})
        .with_healing_config(healing_config)
        .build()
    )

    assert client is not None


def test_cost_with_cache():
    """Test cost routing combined with caching."""
    from simple_agents_py import ClientBuilder

    client = (
        ClientBuilder()
        .add_provider("openai", api_key=API_KEY)
        .add_provider("anthropic", api_key=ANTHROPIC_KEY)
        .with_cost_routing({"provider_costs": {"openai": 0.002, "anthropic": 0.0003}})
        .with_cache(ttl_seconds=1200)
        .build()
    )

    assert client is not None


def test_fallback_with_all_features():
    """Test fallback routing with all features."""
    from simple_agents_py import ClientBuilder

    client = (
        ClientBuilder()
        .add_provider("openai", api_key=API_KEY)
        .add_provider("anthropic", api_key=ANTHROPIC_KEY)
        .add_provider("openrouter", api_key=OPENROUTER_KEY)
        .with_fallback_routing({"retryable_only": True})
        .with_cache(ttl_seconds=1800)
        .with_healing_config({"enabled": True, "min_confidence": 0.75})
        .build()
    )

    assert client is not None
