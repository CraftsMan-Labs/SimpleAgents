"""Demo of advanced routing configuration."""

from simple_agents_py import ClientBuilder


def demo_latency_routing_defaults():
    """Latency routing with default configuration."""
    print("=== Latency Routing (Defaults) ===\n")

    client = (
        ClientBuilder()
        .add_provider("openai", api_key="sk-test-1")
        .add_provider("anthropic", api_key="sk-ant-test-2")
        .with_routing("latency")
        .build()
    )

    print("Client configured with latency-based routing:")
    print("  - Tracks response times from each provider")
    print("  - Routes to fastest provider")
    print("  - Alpha (smoothing factor): 0.2 (default)")
    print("  - Slow threshold: 2000ms (default)")
    print(f"\nClient type: {type(client).__name__}\n")


def demo_latency_routing_custom():
    """Latency routing with custom configuration."""
    print("=== Latency Routing (Custom Config) ===\n")

    config = {
        "alpha": 0.5,  # More aggressive adaptation
        "slow_threshold_ms": 3000,  # More tolerant of slow providers
    }

    client = (
        ClientBuilder()
        .add_provider("openai", api_key="sk-test-1")
        .add_provider("anthropic", api_key="sk-ant-test-2")
        .with_latency_routing(config)
        .build()
    )

    print("Client configured with custom latency routing:")
    print("  - Alpha: 0.5 (higher = adapts faster)")
    print("  - Slow threshold: 3000ms (slower providers tolerated)")
    print("  - Adapts to changing network conditions")
    print(f"\nClient type: {type(client).__name__}\n")


def demo_latency_aggressive():
    """Latency routing with aggressive settings."""
    print("=== Latency Routing (Aggressive) ===\n")

    config = {
        "alpha": 0.9,  # Very responsive to recent changes
        "slow_threshold_ms": 1000,  # Strict threshold
    }

    client = (
        ClientBuilder()
        .add_provider("openai", api_key="sk-test-1")
        .add_provider("anthropic", api_key="sk-ant-test-2")
        .with_latency_routing(config)
        .build()
    )

    print("Client configured with aggressive latency routing:")
    print("  - Alpha: 0.9 (very fast adaptation)")
    print("  - Slow threshold: 1000ms (strict performance requirement)")
    print("  - Quickly switches to faster provider")
    print(f"\nClient type: {type(client).__name__}\n")


def demo_latency_conservative():
    """Latency routing with conservative settings."""
    print("=== Latency Routing (Conservative) ===\n")

    config = {
        "alpha": 0.1,  # Slow to adapt
        "slow_threshold_ms": 5000,  # Very tolerant
    }

    client = (
        ClientBuilder()
        .add_provider("openai", api_key="sk-test-1")
        .add_provider("anthropic", api_key="sk-ant-test-2")
        .with_latency_routing(config)
        .build()
    )

    print("Client configured with conservative latency routing:")
    print("  - Alpha: 0.1 (slow to adapt, stable)")
    print("  - Slow threshold: 5000ms (very tolerant)")
    print("  - Avoids frequent provider switching")
    print(f"\nClient type: {type(client).__name__}\n")


def demo_cost_routing():
    """Cost routing with provider costs."""
    print("=== Cost Routing ===\n")

    config = {
        "provider_costs": {
            "openai": 0.002,  # $0.002 per 1k tokens
            "anthropic": 0.0003,  # $0.0003 per 1k tokens
        }
    }

    client = (
        ClientBuilder()
        .add_provider("openai", api_key="sk-test-1")
        .add_provider("anthropic", api_key="sk-ant-test-2")
        .with_cost_routing(config)
        .build()
    )

    print("Client configured with cost-based routing:")
    print("  - OpenAI: $0.002 per 1k tokens")
    print("  - Anthropic: $0.0003 per 1k tokens")
    print("  - Routes to cheapest provider (Anthropic)")
    print("  - Optimizes for cost efficiency")
    print(f"\nClient type: {type(client).__name__}\n")


def demo_cost_routing_multiple():
    """Cost routing with multiple providers."""
    print("=== Cost Routing (Multiple Providers) ===\n")

    config = {
        "provider_costs": {
            "openai": 0.002,
            "anthropic": 0.0003,
            "openrouter": 0.001,  # Competitive pricing
        }
    }

    client = (
        ClientBuilder()
        .add_provider("openai", api_key="sk-test-1")
        .add_provider("anthropic", api_key="sk-ant-test-2")
        .add_provider("openrouter", api_key="sk-or-test-3")
        .with_cost_routing(config)
        .build()
    )

    print("Client configured with cost routing (3 providers):")
    print("  - OpenAI: $0.002 per 1k tokens")
    print("  - Anthropic: $0.0003 per 1k tokens (cheapest)")
    print("  - OpenRouter: $0.001 per 1k tokens")
    print("  - Routes to Anthropic (lowest cost)")
    print(f"\nClient type: {type(client).__name__}\n")


def demo_cost_routing_with_cache():
    """Cost routing combined with caching."""
    print("=== Cost Routing + Cache ===\n")

    config = {
        "provider_costs": {
            "openai": 0.002,
            "anthropic": 0.0003,
        }
    }

    client = (
        ClientBuilder()
        .add_provider("openai", api_key="sk-test-1")
        .add_provider("anthropic", api_key="sk-ant-test-2")
        .with_cost_routing(config)
        .with_cache(ttl_seconds=300)
        .build()
    )

    print("Client configured with cost routing and cache:")
    print("  - Routes to cheapest provider")
    print("  - Cache TTL: 300 seconds")
    print("  - Reduces API costs further via caching")
    print(f"\nClient type: {type(client).__name__}\n")


def demo_fallback_routing_defaults():
    """Fallback routing with default configuration."""
    print("=== Fallback Routing (Defaults) ===\n")

    client = (
        ClientBuilder()
        .add_provider("openai", api_key="sk-primary")
        .add_provider("anthropic", api_key="sk-backup")
        .with_routing("fallback")
        .build()
    )

    print("Client configured with fallback routing:")
    print("  - Primary: OpenAI")
    print("  - Backup: Anthropic")
    print("  - Fallback on: Rate limits, timeouts, server errors")
    print("  - No fallback on: Invalid API key, auth errors")
    print(f"\nClient type: {type(client).__name__}\n")


def demo_fallback_routing_all_errors():
    """Fallback routing on all errors."""
    print("=== Fallback Routing (All Errors) ===\n")

    config = {
        "retryable_only": False,  # Fallback on all errors
    }

    client = (
        ClientBuilder()
        .add_provider("openai", api_key="sk-test-1")
        .add_provider("anthropic", api_key="sk-test-2")
        .with_fallback_routing(config)
        .build()
    )

    print("Client configured with fallback routing (all errors):")
    print("  - Primary: OpenAI")
    print("  - Backup: Anthropic")
    print("  - Fallback on: ALL errors")
    print("  - Maximum reliability")
    print(f"\nClient type: {type(client).__name__}\n")


def demo_fallback_routing_multiple():
    """Fallback routing with multiple providers."""
    print("=== Fallback Routing (Multiple Providers) ===\n")

    config = {
        "retryable_only": True,
    }

    client = (
        ClientBuilder()
        .add_provider("openai", api_key="sk-test-1")
        .add_provider("anthropic", api_key="sk-test-2")
        .add_provider("openrouter", api_key="sk-test-3")
        .with_fallback_routing(config)
        .build()
    )

    print("Client configured with fallback routing (3 providers):")
    print("  - Primary: OpenAI")
    print("  - Secondary: Anthropic")
    print("  - Tertiary: OpenRouter")
    print("  - Tries each in order on retryable errors")
    print(f"\nClient type: {type(client).__name__}\n")


def demo_complete_configuration():
    """Complete configuration with all features."""
    print("=== Complete Configuration ===\n")

    routing_config = {
        "alpha": 0.3,
        "slow_threshold_ms": 2500,
    }

    healing_config = {
        "enabled": True,
        "min_confidence": 0.75,
        "fuzzy_match_threshold": 0.88,
    }

    client = (
        ClientBuilder()
        .add_provider("openai", api_key="sk-openai")
        .add_provider("anthropic", api_key="sk-ant")
        .add_provider("openrouter", api_key="sk-or")
        .with_latency_routing(routing_config)
        .with_cache(ttl_seconds=600)
        .with_healing_config(healing_config)
        .build()
    )

    print("Client with complete configuration:")
    print("  Providers:")
    print("    - OpenAI")
    print("    - Anthropic")
    print("    - OpenRouter")
    print("  Routing: Latency-based")
    print("    - Alpha: 0.3")
    print("    - Slow threshold: 2500ms")
    print("  Cache:")
    print("    - TTL: 600 seconds (10 minutes)")
    print("  Healing:")
    print("    - Enabled: Yes")
    print("    - Min confidence: 0.75")
    print("    - Fuzzy threshold: 0.88")
    print(f"\nClient type: {type(client).__name__}\n")


def demo_comparison_routing_modes():
    """Compare all routing modes."""
    print("=== Routing Mode Comparison ===\n")

    # Direct routing
    direct = (
        ClientBuilder()
        .add_provider("openai", api_key="sk-test")
        .with_routing("direct")
        .build()
    )
    print("Direct routing:")
    print("  - Uses first provider only")
    print("  - No load balancing")
    print("  - Use when: You know which provider to use")

    # Round-robin routing
    round_robin = (
        ClientBuilder()
        .add_provider("openai", api_key="sk-test")
        .add_provider("anthropic", api_key="sk-test")
        .with_routing("round_robin")
        .build()
    )
    print("\nRound-robin routing:")
    print("  - Distributes requests evenly")
    print("  - Simple load balancing")
    print("  - Use when: All providers are similar")

    # Latency routing
    latency = (
        ClientBuilder()
        .add_provider("openai", api_key="sk-test")
        .add_provider("anthropic", api_key="sk-test")
        .with_routing("latency")
        .build()
    )
    print("\nLatency routing:")
    print("  - Routes to fastest provider")
    print("  - Adapts to network conditions")
    print("  - Use when: Speed is priority")

    # Cost routing
    cost = (
        ClientBuilder()
        .add_provider("openai", api_key="sk-test")
        .add_provider("anthropic", api_key="sk-test")
        .with_routing("cost")
        .build()
    )
    print("\nCost routing:")
    print("  - Routes to cheapest provider")
    print("  - Minimizes spend")
    print("  - Use when: Budget is priority")

    # Fallback routing
    fallback = (
        ClientBuilder()
        .add_provider("openai", api_key="sk-test")
        .add_provider("anthropic", api_key="sk-test")
        .with_routing("fallback")
        .build()
    )
    print("\nFallback routing:")
    print("  - Tries providers in order")
    print("  - Handles failures gracefully")
    print("  - Use when: Reliability is priority")

    print(f"\nAll clients created successfully!\n")


def demo_latency_vs_cost():
    """Compare latency vs cost routing."""
    print("=== Latency vs Cost Routing ===\n")

    # Latency routing
    latency_client = (
        ClientBuilder()
        .add_provider("openai", api_key="sk-fast")
        .add_provider("anthropic", api_key="sk-slow")
        .with_latency_routing({"alpha": 0.3, "slow_threshold_ms": 2000})
        .build()
    )

    print("Latency-based routing:")
    print("  Goal: Minimize response time")
    print("  Strategy: Track actual latency, pick fastest")
    print("  Best for: Real-time applications, user-facing features")
    print("  Trade-off: May use more expensive provider")

    # Cost routing
    cost_config = {
        "provider_costs": {
            "openai": 0.002,
            "anthropic": 0.0003,
        }
    }
    cost_client = (
        ClientBuilder()
        .add_provider("openai", api_key="sk-fast")
        .add_provider("anthropic", api_key="sk-slow")
        .with_cost_routing(cost_config)
        .build()
    )

    print("\nCost-based routing:")
    print("  Goal: Minimize spend")
    print("  Strategy: Use configured token costs")
    print("  Best for: Batch processing, cost-sensitive apps")
    print("  Trade-off: May be slower")

    print("\nChoose based on your priorities!")
    print()


def demo_production_setup():
    """Production-ready configuration."""
    print("=== Production Setup ===\n")

    routing_config = {
        "alpha": 0.2,  # Balanced adaptation
        "slow_threshold_ms": 3000,  # Reasonable threshold
    }

    healing_config = {
        "enabled": True,
        "min_confidence": 0.8,  # High quality threshold
        "fuzzy_match_threshold": 0.85,
    }

    client = (
        ClientBuilder()
        .add_provider("openai", api_key="sk-prod-openai")
        .add_provider("anthropic", api_key="sk-prod-ant")
        .add_provider("openrouter", api_key="sk-prod-or")
        .with_latency_routing(routing_config)
        .with_cache(ttl_seconds=900)  # 15 minutes
        .with_healing_config(healing_config)
        .build()
    )

    print("Production configuration:")
    print("  Providers: OpenAI, Anthropic, OpenRouter")
    print("  Routing: Latency-based (balanced settings)")
    print("  Cache: 15 minutes")
    print("  Healing: Enabled with high confidence threshold")
    print("  ✓ High availability")
    print("  ✓ Low latency")
    print("  ✓ Cost control via caching")
    print("  ✓ Quality output via healing")
    print(f"\nClient type: {type(client).__name__}\n")


def main():
    """Run all routing configuration demos."""
    demo_latency_routing_defaults()
    demo_latency_routing_custom()
    demo_latency_aggressive()
    demo_latency_conservative()
    demo_cost_routing()
    demo_cost_routing_multiple()
    demo_cost_routing_with_cache()
    demo_fallback_routing_defaults()
    demo_fallback_routing_all_errors()
    demo_fallback_routing_multiple()
    demo_complete_configuration()
    demo_comparison_routing_modes()
    demo_latency_vs_cost()
    demo_production_setup()

    print("=" * 60)
    print("All routing configuration demos completed successfully!")
    print("=" * 60)


if __name__ == "__main__":
    main()
