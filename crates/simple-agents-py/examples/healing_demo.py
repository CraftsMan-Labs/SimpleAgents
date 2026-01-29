#!/usr/bin/env python3
"""
Example script demonstrating healing metadata in Python bindings.

This script demonstrates the new HealedJsonResult class and complete_json_healed method.
"""

# Import the module (this would be installed via maturin)
try:
    import simple_agents_py
except ImportError:
    print("Note: simple_agents_py module not installed.")
    print("Run: maturin develop --release")
    print("\nDemonstrating HealedJsonResult class structure:")
    print("```python")
    print("result = client.complete_json_healed(model, messages)")
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
result = client.complete_json_healed("gpt-4o-mini", messages)

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
