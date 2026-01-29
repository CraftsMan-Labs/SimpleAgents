"""Demo of structured streaming functionality."""

import sys
import os

# Add the target directory to path
sys.path.insert(
    0, os.path.join(os.path.dirname(__file__), "..", "..", "target", "debug")
)

from simple_agents_py import Client


def demo_basic_structured_streaming():
    """Demonstrate basic structured streaming."""
    print("=" * 60)
    print("Demo 1: Basic Structured Streaming")
    print("=" * 60)

    client = Client("openai")

    schema = {
        "type": "object",
        "properties": {"name": {"type": "string"}, "age": {"type": "number"}},
        "required": ["name", "age"],
    }

    messages = [
        {"role": "user", "content": "Extract name and age from: John is 30 years old"}
    ]

    print("\nStreaming structured output:")
    print(f"Schema: {schema}\n")

    for event in client.stream_structured(
        "gpt-4o-mini", messages, schema, max_tokens=50
    ):
        if event.is_partial:
            print(f"Partial: {event.partial_value}")
        elif event.is_complete:
            print(f"Complete: {event.value}")
            print(f"Confidence: {event.confidence:.2%}")
            print(f"Healing applied: {event.was_healed}")

    print()


def demo_structured_streaming_with_arrays():
    """Demonstrate structured streaming with arrays."""
    print("=" * 60)
    print("Demo 2: Structured Streaming with Arrays")
    print("=" * 60)

    client = Client("openai")

    schema = {
        "type": "object",
        "properties": {"items": {"type": "array", "items": {"type": "string"}}},
        "required": ["items"],
    }

    messages = [{"role": "user", "content": "List three programming languages"}]

    print("\nStreaming array output:")
    print(f"Schema: {schema}\n")

    for event in client.stream_structured(
        "gpt-4o-mini", messages, schema, max_tokens=50
    ):
        if event.is_complete:
            print(f"Final result: {event.value}")
            print(f"Confidence: {event.confidence:.2%}")

    print()


def demo_structured_streaming_nested():
    """Demonstrate structured streaming with nested objects."""
    print("=" * 60)
    print("Demo 3: Structured Streaming with Nested Objects")
    print("=" * 60)

    client = Client("openai")

    schema = {
        "type": "object",
        "properties": {
            "person": {
                "type": "object",
                "properties": {
                    "name": {"type": "string"},
                    "age": {"type": "number"},
                    "city": {"type": "string"},
                },
            }
        },
        "required": ["person"],
    }

    messages = [
        {
            "role": "user",
            "content": "Create a person named Alice, age 25, living in Paris",
        }
    ]

    print("\nStreaming nested object output:")
    print(f"Schema: {schema}\n")

    for event in client.stream_structured(
        "gpt-4o-mini", messages, schema, max_tokens=50
    ):
        if event.is_complete:
            print(f"Final result: {event.value}")
            print(f"Confidence: {event.confidence:.2%}")

    print()


def demo_structured_streaming_with_params():
    """Demonstrate structured streaming with parameters."""
    print("=" * 60)
    print("Demo 4: Structured Streaming with Parameters")
    print("=" * 60)

    client = Client("openai")

    schema = {
        "type": "object",
        "properties": {
            "sentiment": {"type": "string"},
            "confidence": {"type": "number"},
        },
        "required": ["sentiment", "confidence"],
    }

    messages = [{"role": "user", "content": "Analyze sentiment of: This is great!"}]

    print("\nStreaming with temperature=0.7, top_p=0.9:")
    print(f"Schema: {schema}\n")

    for event in client.stream_structured(
        "gpt-4o-mini", messages, schema, temperature=0.7, top_p=0.9, max_tokens=50
    ):
        if event.is_complete:
            print(f"Final result: {event.value}")
            print(f"Confidence: {event.confidence:.2%}")

    print()


def demo_structured_streaming_event_details():
    """Demonstrate accessing structured event details."""
    print("=" * 60)
    print("Demo 5: Accessing Structured Event Details")
    print("=" * 60)

    client = Client("openai")

    schema = {
        "type": "object",
        "properties": {"message": {"type": "string"}},
        "required": ["message"],
    }

    messages = [{"role": "user", "content": "Say hello"}]

    print("\nIterating through events:")

    event_count = 0
    for event in client.stream_structured(
        "gpt-4o-mini", messages, schema, max_tokens=30
    ):
        event_count += 1
        print(f"\nEvent {event_count}:")
        print(f"  Is partial: {event.is_partial}")
        print(f"  Is complete: {event.is_complete}")
        print(f"  Value: {event.value}")
        print(f"  Partial value: {event.partial_value}")
        print(f"  Confidence: {event.confidence}")
        print(f"  Was healed: {event.was_healed}")

    print(f"\nTotal events: {event_count}")
    print()


def demo_structured_streaming_error_handling():
    """Demonstrate error handling in structured streaming."""
    print("=" * 60)
    print("Demo 6: Error Handling")
    print("=" * 60)

    client = Client("openai")

    schema = {"type": "object", "properties": {"name": {"type": "string"}}}

    # Try with empty messages (should fail)
    print("\nTrying to stream with empty messages:")
    try:
        list(client.stream_structured("gpt-4o-mini", [], schema))
    except Exception as e:
        print(f"  Caught expected error: {type(e).__name__}: {e}")

    # Try with invalid schema (should fail)
    print("\nTrying to stream with invalid schema:")
    try:
        list(
            client.stream_structured(
                "gpt-4o-mini", [{"role": "user", "content": "Test"}], "invalid"
            )
        )
    except Exception as e:
        print(f"  Caught expected error: {type(e).__name__}: {e}")

    print()


def main():
    """Run all demos."""
    print("\n" + "=" * 60)
    print("SimpleAgents Python Bindings - Structured Streaming Demos")
    print("=" * 60 + "\n")

    try:
        demo_basic_structured_streaming()
        demo_structured_streaming_with_arrays()
        demo_structured_streaming_nested()
        demo_structured_streaming_with_params()
        demo_structured_streaming_event_details()
        demo_structured_streaming_error_handling()

        print("=" * 60)
        print("All demos completed successfully!")
        print("=" * 60)

    except Exception as e:
        print(f"\nError running demos: {e}")
        import traceback

        traceback.print_exc()


if __name__ == "__main__":
    main()
