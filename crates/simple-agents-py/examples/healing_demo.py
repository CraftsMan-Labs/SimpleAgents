#!/usr/bin/env python3
"""
Example script demonstrating healing metadata in Python bindings.

This script demonstrates the new HealedJsonResult class and the unified complete method.
"""

# Import the module (this would be installed via maturin)
try:
    import simple_agents_py
    from simple_agents_py import HealedJsonResult
except ImportError:
    print("Note: simple_agents_py module not installed.")
    print("Run: maturin develop --release")
    print("\nDemonstrating HealedJsonResult class structure:")
    print("```python")
    print('result = client.complete(model, messages, response_format="json", heal=True)')
    print('print(f"Content: {result.content}")')
    print('print(f"Confidence: {result.confidence}")')
    print('print(f"Was healed: {result.was_healed}")')
    print('print(f"Flags: {result.flags}")')
    print("```")
    exit(0)

# Example usage (requires API key)
client = simple_agents_py.Client("openai")

messages = [
    {
        "role": "user",
        "content": 'Return JSON with trailing comma: {"name": "Alice", "age": 30,}',
    }
]

# Use the new healing metadata API
result = client.complete("gpt-4o-mini", messages, response_format="json", heal=True)
if not isinstance(result, HealedJsonResult):
    raise TypeError(f"Expected HealedJsonResult, got {type(result).__name__}")

print(f"Content: {result.content}")
print(f"Confidence: {result.confidence}")
print(f"Was healed: {result.was_healed}")
print(f"Flags: {result.flags}")

# Check if healing was applied
if result.was_healed:
    print(f"\nHealing applied: {len(result.flags)} transformations")
    for flag in result.flags:
        print(f"  - {flag}")
else:
    print("\nNo healing applied - response was perfect!")
