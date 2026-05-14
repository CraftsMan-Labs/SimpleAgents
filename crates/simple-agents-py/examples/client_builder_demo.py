"""Demo of ClientBuilder with currently supported configuration."""

from simple_agents_py import ClientBuilder


def demo_basic_builder():
    """Basic builder usage with single provider."""
    print("=== Basic Builder Usage ===\n")

    client = (
        ClientBuilder().add_provider("openai", api_key="sk-test-key").build()
    )

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
    """Full configuration example using supported methods."""
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
        .with_healing_config(healing_config)
        .build()
    )

    print("Client with full configuration:")
    print("  Providers:")
    print("    - OpenAI")
    print("    - Anthropic")
    print("  Build behavior: uses first configured provider")
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
        ClientBuilder().add_provider("openai", api_key="sk-test").build()
    )

    print("Old Client class:")
    print("  - Single provider only")
    print("  - Simple configuration")
    print()
    print("New ClientBuilder:")
    print("  - Multiple providers supported")
    print("  - Configurable healing")
    print("  - Custom API base URLs")
    print()
    print("Both provide the unified complete functionality")
    print(f"Client type: {type(new_client).__name__}\n")


def main():
    """Run all demos."""
    demo_basic_builder()
    demo_multi_provider()
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
