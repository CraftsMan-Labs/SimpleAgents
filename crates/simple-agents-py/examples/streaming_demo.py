"""Demo of streaming functionality."""

import sys
import os

# Add the target directory to path
sys.path.insert(
    0, os.path.join(os.path.dirname(__file__), "..", "..", "target", "debug")
)

from simple_agents_py import Client


def demo_basic_streaming():
    """Demonstrate basic streaming."""
    print("=" * 60)
    print("Demo 1: Basic Streaming")
    print("=" * 60)

    client = Client("openai")
    messages = [{"role": "user", "content": "Write a haiku about programming"}]

    print("\nStreaming response:")
    full_content = []
    for chunk in client.stream("gpt-4o-mini", messages):
        print(chunk.content, end="", flush=True)
        full_content.append(chunk.content)
        if chunk.finish_reason:
            print(f"\n\nFinish reason: {chunk.finish_reason}")
            print(f"Model: {chunk.model}")

    print(f"\n\nTotal chunks received: {len(full_content)}")
    print()


def demo_streaming_with_params():
    """Demonstrate streaming with parameters."""
    print("=" * 60)
    print("Demo 2: Streaming with Parameters")
    print("=" * 60)

    client = Client("openai")
    messages = [
        {"role": "system", "content": "You are a helpful assistant."},
        {"role": "user", "content": "Count from 1 to 5"},
    ]

    print("\nStreaming with temperature=0.9:")
    for chunk in client.stream("gpt-4o-mini", messages, temperature=0.9, max_tokens=50):
        print(chunk.content, end="", flush=True)

    print("\n")


def demo_streaming_chunks():
    """Demonstrate accessing chunk metadata."""
    print("=" * 60)
    print("Demo 3: Accessing Chunk Metadata")
    print("=" * 60)

    client = Client("openai")
    messages = [{"role": "user", "content": "Say 'Hello World'"}]

    print("\nIterating through chunks:")
    chunk_count = 0
    for i, chunk in enumerate(client.stream("gpt-4o-mini", messages, max_tokens=10)):
        chunk_count += 1
        print(f"\nChunk {i}:")
        print(f"  Content: {repr(chunk.content[:50])}")  # First 50 chars
        print(f"  Index: {chunk.index}")
        print(f"  Model: {chunk.model}")
        if chunk.finish_reason:
            print(f"  Finish Reason: {chunk.finish_reason}")

    print(f"\nTotal chunks: {chunk_count}")
    print()


def demo_streaming_error_handling():
    """Demonstrate error handling in streaming."""
    print("=" * 60)
    print("Demo 4: Error Handling")
    print("=" * 60)

    client = Client("openai")

    # Try with empty messages (should fail)
    print("\nTrying to stream with empty messages:")
    try:
        list(client.stream("gpt-4o-mini", []))
    except Exception as e:
        print(f"  Caught expected error: {type(e).__name__}: {e}")

    print()


def main():
    """Run all demos."""
    print("\n" + "=" * 60)
    print("SimpleAgents Python Bindings - Streaming Demos")
    print("=" * 60 + "\n")

    try:
        demo_basic_streaming()
        demo_streaming_with_params()
        demo_streaming_chunks()
        demo_streaming_error_handling()

        print("=" * 60)
        print("All demos completed successfully!")
        print("=" * 60)

    except Exception as e:
        print(f"\nError running demos: {e}")
        import traceback

        traceback.print_exc()


if __name__ == "__main__":
    main()
