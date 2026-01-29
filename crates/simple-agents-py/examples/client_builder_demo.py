"""Demo of ClientBuilder for multi-provider support."""

from simple_agents_py import ClientBuilder


def demo_basic_builder():
    """Basic builder usage with single provider."""
    print("=== Basic Builder Usage ===\n")

    client = ClientBuilder().add_provider("openai", api_key="sk-test-key").build()

    print("Client created successfully with single provider")
    print(f"Client type: {type(client).__name__}\n")


def demo_multi_provider():
    """Builder with multiple providers."""
    print("=== Multi-Provider Setup ===\n")

    client = (
        ClientBuilder()
        .add_provider("openai", api_key="sk-openai-key")
        .add_provider("anthropic", api_key="sk-ant-anthropic-key")
        .add_provider("openrouter", api_key="sk-or-key")
        .build()
    )

    print("Client created with 3 providers:")
    print("  - OpenAI (for GPT models)")
    print("  - Anthropic (for Claude models)")
    print("  - OpenRouter (for multi-model access)")
    print(f"Client type: {type(client).__name__}\n")


def demo_round_robin_routing():
    """Round-robin routing across providers."""
    print("=== Round-Robin Routing ===\n")

    client = (
        ClientBuilder()
        .add_provider("openai", api_key="sk-1")
        .add_provider("anthropic", api_key="sk-2")
        .with_routing("round_robin")
        .build()
    )

    print("Client configured with round-robin routing:")
    print("  - Request 1 -> OpenAI")
    print("  - Request 2 -> Anthropic")
    print("  - Request 3 -> OpenAI")
    print("  - (and so on...)")
    print(f"\nClient type: {type(client).__name__}\n")


def demo_fallback_routing():
    """Fallback routing configuration."""
    print("=== Fallback Routing ===\n")

    client = (
        ClientBuilder()
        .add_provider("openai", api_key="sk-primary")
        .add_provider("anthropic", api_key="sk-backup")
        .with_routing("fallback")
        .build()
    )

    print("Client configured with fallback routing:")
    print("  - Primary: OpenAI")
    print("  - Backup: Anthropic (used if OpenAI fails)")
    print(f"\nClient type: {type(client).__name__}\n")


def demo_latency_routing():
    """Latency-based routing configuration."""
    print("=== Latency-Based Routing ===\n")

    client = (
        ClientBuilder()
        .add_provider("openai", api_key="sk-1")
        .add_provider("anthropic", api_key="sk-2")
        .with_routing("latency")
        .build()
    )

    print("Client configured with latency-based routing:")
    print("  - Routes requests to the fastest provider")
    print("  - Automatically tracks response times")
    print("  - Adapts to changing conditions")
    print(f"\nClient type: {type(client).__name__}\n")


def demo_cost_routing():
    """Cost-based routing configuration."""
    print("=== Cost-Based Routing ===\n")

    client = (
        ClientBuilder()
        .add_provider("openai", api_key="sk-1")
        .add_provider("anthropic", api_key="sk-2")
        .with_routing("cost")
        .build()
    )

    print("Client configured with cost-based routing:")
    print("  - Routes requests to the cheapest provider")
    print("  - Considers per-token costs")
    print("  - Optimizes for cost efficiency")
    print(f"\nClient type: {type(client).__name__}\n")


def demo_with_cache():
    """Cache configuration."""
    print("=== Cache Configuration ===\n")

    client = (
        ClientBuilder()
        .add_provider("openai", api_key="sk-test")
        .with_cache(ttl_seconds=300)
        .build()
    )

    print("Client configured with cache:")
    print("  - Cache TTL: 300 seconds (5 minutes)")
    print("  - Identical requests within TTL return cached responses")
    print("  - Reduces API costs and improves latency")
    print(f"\nClient type: {type(client).__name__}\n")


def demo_healing_config():
    """Healing configuration."""
    print("=== Healing Configuration ===\n")

    healing_config = {
        "enabled": True,
        "min_confidence": 0.7,
        "fuzzy_match_threshold": 0.85,
    }

    client = (
        ClientBuilder()
        .add_provider("openai", api_key="sk-test")
        .with_healing_config(healing_config)
        .build()
    )

    print("Client configured with custom healing settings:")
    print("  - Healing: Enabled")
    print("  - Minimum confidence: 0.7")
    print("  - Fuzzy match threshold: 0.85")
    print("  - Will fix common JSON issues automatically")
    print(f"\nClient type: {type(client).__name__}\n")


def demo_disable_healing():
    """Disabling healing."""
    print("=== Disable Healing ===\n")

    healing_config = {
        "enabled": False,
    }

    client = (
        ClientBuilder()
        .add_provider("openai", api_key="sk-test")
        .with_healing_config(healing_config)
        .build()
    )

    print("Client configured with healing disabled:")
    print("  - Healing: Disabled")
    print("  - Returns raw LLM responses")
    print("  - Useful for debugging or when LLM always returns perfect JSON")
    print(f"\nClient type: {type(client).__name__}\n")


def demo_full_configuration():
    """Full configuration example."""
    print("=== Full Configuration Example ===\n")

    healing_config = {
        "enabled": True,
        "min_confidence": 0.8,
        "fuzzy_match_threshold": 0.9,
    }

    client = (
        ClientBuilder()
        .add_provider("openai", api_key="sk-openai")
        .add_provider("anthropic", api_key="sk-ant")
        .with_routing("round_robin")
        .with_cache(ttl_seconds=600)
        .with_healing_config(healing_config)
        .build()
    )

    print("Client with full configuration:")
    print("  Providers:")
    print("    - OpenAI")
    print("    - Anthropic")
    print("  Routing: Round-robin")
    print("  Cache: 600 seconds (10 minutes)")
    print("  Healing:")
    print("    - Enabled: Yes")
    print("    - Min confidence: 0.8")
    print("    - Fuzzy threshold: 0.9")
    print(f"\nClient type: {type(client).__name__}\n")


def demo_custom_api_base():
    """Custom API base URL."""
    print("=== Custom API Base URL ===\n")

    client = (
        ClientBuilder()
        .add_provider(
            "openai",
            api_key="sk-test",
            api_base="http://localhost:8080/v1",
        )
        .build()
    )

    print("Client configured with custom API base:")
    print("  - Provider: OpenAI")
    print("  - API Base: http://localhost:8080/v1")
    print("  - Useful for local models or custom gateways")
    print(f"\nClient type: {type(client).__name__}\n")


def demo_builder_repr():
    """Builder string representation."""
    print("=== Builder String Representation ===\n")

    builder = (
        ClientBuilder()
        .add_provider("openai", api_key="sk-test")
        .add_provider("anthropic", api_key="sk-ant")
        .with_routing("round_robin")
        .with_cache(ttl_seconds=300)
    )

    print("Builder representation:")
    print(f"  {repr(builder)}\n")


def demo_comparison():
    """Comparison: Old Client vs ClientBuilder."""
    print("=== Comparison: Old vs New ===\n")

    # Old way (single provider only)
    # client = Client("openai", api_key="sk-test")

    # New way (multi-provider with configuration)
    new_client = (
        ClientBuilder()
        .add_provider("openai", api_key="sk-test")
        .with_routing("direct")
        .build()
    )

    print("Old Client class:")
    print("  - Single provider only")
    print("  - Simple configuration")
    print()
    print("New ClientBuilder:")
    print("  - Multiple providers supported")
    print("  - Advanced routing (round-robin, latency, cost, fallback)")
    print("  - Built-in caching")
    print("  - Configurable healing")
    print("  - Custom API base URLs")
    print()
    print("Both provide same basic functionality (complete, complete_json, etc.)")
    print(f"Client type: {type(new_client).__name__}\n")


def main():
    """Run all demos."""
    demo_basic_builder()
    demo_multi_provider()
    demo_round_robin_routing()
    demo_fallback_routing()
    demo_latency_routing()
    demo_cost_routing()
    demo_with_cache()
    demo_healing_config()
    demo_disable_healing()
    demo_full_configuration()
    demo_custom_api_base()
    demo_builder_repr()
    demo_comparison()

    print("=" * 60)
    print("All ClientBuilder demos completed successfully!")
    print("=" * 60)


if __name__ == "__main__":
    main()
