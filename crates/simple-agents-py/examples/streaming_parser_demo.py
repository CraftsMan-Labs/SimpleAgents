"""Demo of StreamingParser for incremental JSON parsing."""

from simple_agents_py import StreamingParser


def demo_basic_streaming():
    """Basic streaming parsing example."""
    print("=== Basic Streaming Parsing ===\n")

    parser = StreamingParser()

    # Simulate streaming LLM response chunks
    chunks = [
        '{"name": "Alice", ',
        '"age": 30, ',
        '"email": "alice@example.com"}',
    ]

    for i, chunk in enumerate(chunks, 1):
        print(f"Feeding chunk {i}: {repr(chunk)}")
        parser.feed(chunk)
        print(f"Buffer size: {parser.buffer_len()} bytes\n")

    result = parser.finalize()

    print("Final result:")
    print(f"  Value: {result.value}")
    print(f"  Confidence: {result.confidence:.2f}")
    print(f"  Was healed: {result.was_healed}")
    print(f"  Flags: {result.flags}\n")


def demo_markdown_handling():
    """Demonstrate handling of markdown code fences."""
    print("=== Markdown Code Fence Handling ===\n")

    parser = StreamingParser()

    # LLM often wraps JSON in markdown code blocks
    chunks = [
        "Here's the JSON response:\n",
        "```json\n",
        '{"name": "Bob", ',
        '"age": 25}',
        "\n```\n",
        "Hope this helps!",
    ]

    print("Simulating LLM response with markdown formatting:")
    for i, chunk in enumerate(chunks, 1):
        print(f"Chunk {i}: {repr(chunk)}")
        parser.feed(chunk)

    result = parser.finalize()

    print("\nFinal result:")
    print(f"  Value: {result.value}")
    print(f"  Confidence: {result.confidence:.2f}")
    print(f"  Was healed: {result.was_healed}")
    print(f"  Flags: {result.flags}\n")


def demo_malformed_json():
    """Demonstrate healing of malformed JSON."""
    print("=== Malformed JSON Healing ===\n")

    parser = StreamingParser()

    # Malformed JSON with trailing comma and unquoted key
    chunks = [
        '{name: "Charlie",',  # unquoted key
        ' "age": 30,',  # trailing comma
        ' "active": true}',
    ]

    print("Simulating malformed LLM response:")
    for i, chunk in enumerate(chunks, 1):
        print(f"Chunk {i}: {repr(chunk)}")
        parser.feed(chunk)

    result = parser.finalize()

    print("\nFinal result (healed):")
    print(f"  Value: {result.value}")
    print(f"  Confidence: {result.confidence:.2f}")
    print(f"  Was healed: {result.was_healed}")
    print(f"  Flags: {result.flags}\n")


def demo_array_streaming():
    """Demonstrate streaming array parsing."""
    print("=== Array Streaming ===\n")

    parser = StreamingParser()

    # Streaming array of objects
    chunks = [
        "[",
        '{"id": 1, "name": "Item 1"}, ',
        '{"id": 2, "name": "Item 2"}, ',
        '{"id": 3, "name": "Item 3"}',
        "]",
    ]

    print("Streaming array of items:")
    for i, chunk in enumerate(chunks, 1):
        print(f"Chunk {i}: {repr(chunk)}")
        parser.feed(chunk)

    result = parser.finalize()

    print("\nFinal result:")
    print(f"  Array length: {len(result.value)}")
    for item in result.value:
        print(f"    {item}")
    print(f"  Confidence: {result.confidence:.2f}\n")


def demo_buffer_management():
    """Demonstrate buffer management operations."""
    print("=== Buffer Management ===\n")

    parser = StreamingParser()

    print("Initial state:")
    print(f"  Is empty: {parser.is_empty()}")
    print(f"  Buffer length: {parser.buffer_len()}\n")

    parser.feed('{"partial": "data"}')

    print("After feeding data:")
    print(f"  Is empty: {parser.is_empty()}")
    print(f"  Buffer length: {parser.buffer_len()}")
    print(f"  Repr: {repr(parser)}\n")

    parser.clear()

    print("After clear:")
    print(f"  Is empty: {parser.is_empty()}")
    print(f"  Buffer length: {parser.buffer_len()}")
    print(f"  Repr: {repr(parser)}\n")


def demo_nested_objects():
    """Demonstrate streaming with nested objects."""
    print("=== Nested Object Streaming ===\n")

    parser = StreamingParser()

    chunks = [
        '{"user": {',
        '  "profile": {',
        '    "name": "Dave",',
        '    "age": 35',
        "  },",
        '  "settings": {',
        '    "theme": "dark",',
        '    "notifications": true',
        "  }",
        "}}",
    ]

    print("Streaming nested object structure:")
    for i, chunk in enumerate(chunks, 1):
        print(f"Chunk {i}: {repr(chunk)}")
        parser.feed(chunk)

    result = parser.finalize()

    print("\nFinal result:")
    print(f"  User name: {result.value['user']['profile']['name']}")
    print(f"  User age: {result.value['user']['profile']['age']}")
    print(f"  Theme: {result.value['user']['settings']['theme']}")
    print(f"  Confidence: {result.confidence:.2f}\n")


def main():
    """Run all demos."""
    demo_basic_streaming()
    demo_markdown_handling()
    demo_malformed_json()
    demo_array_streaming()
    demo_buffer_management()
    demo_nested_objects()

    print("=" * 50)
    print("All demos completed successfully!")
    print("=" * 50)


if __name__ == "__main__":
    main()
