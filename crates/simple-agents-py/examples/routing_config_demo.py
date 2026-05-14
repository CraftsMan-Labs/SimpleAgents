"""Demo of builder configuration options currently supported."""

from simple_agents_py import ClientBuilder, ProviderConfig


def demo_provider_config_objects():
    """Build using ProviderConfig entries."""
    print("=== ProviderConfig Objects ===\n")

    client = (
        ClientBuilder()
        .add_provider_config(ProviderConfig("openai", "sk-test-1", None))
        .add_provider_config(
            ProviderConfig("anthropic", "sk-ant-test-2", None)
        )
        .build()
    )

    print("Client built from ProviderConfig entries.")
    print("  - OpenAI configured")
    print("  - Anthropic configured")
    print("  - Builder currently uses first configured provider at runtime")
    print(f"\nClient type: {type(client).__name__}\n")


def demo_mixed_provider_sources():
    """Build mixing add_provider and add_provider_config."""
    print("=== Mixed Provider Sources ===\n")

    client = (
        ClientBuilder()
        .add_provider("openai", api_key="sk-openai")
        .add_provider_config(
            ProviderConfig("openrouter", "sk-or-test-3", None)
        )
        .build()
    )

    print("Client built from mixed provider setup.")
    print("  - add_provider used for OpenAI")
    print("  - add_provider_config used for OpenRouter")
    print(f"\nClient type: {type(client).__name__}\n")


def demo_healing_profiles():
    """Show healing config profiles."""
    print("=== Healing Profiles ===\n")

    strict_client = (
        ClientBuilder()
        .add_provider("openai", api_key="sk-strict")
        .with_healing_config(
            {
                "enabled": True,
                "min_confidence": 0.85,
                "fuzzy_match_threshold": 0.9,
            }
        )
        .build()
    )

    relaxed_client = (
        ClientBuilder()
        .add_provider("openai", api_key="sk-relaxed")
        .with_healing_config(
            {
                "enabled": True,
                "min_confidence": 0.6,
            }
        )
        .build()
    )

    disabled_client = (
        ClientBuilder()
        .add_provider("openai", api_key="sk-off")
        .with_healing_config({"enabled": False})
        .build()
    )

    print("Created healing profile clients:")
    print(f"  - strict: {type(strict_client).__name__}")
    print(f"  - relaxed: {type(relaxed_client).__name__}")
    print(f"  - disabled: {type(disabled_client).__name__}\n")


def main():
    """Run all supported configuration demos."""
    demo_provider_config_objects()
    demo_mixed_provider_sources()
    demo_healing_profiles()

    print("=" * 60)
    print("All builder configuration demos completed successfully!")
    print("=" * 60)


if __name__ == "__main__":
    main()
