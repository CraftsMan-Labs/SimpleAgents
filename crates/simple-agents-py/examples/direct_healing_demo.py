#!/usr/bin/env python3
"""
Example script demonstrating direct JSON healing APIs.

This script shows how to use heal_json and coerce_to_schema without making LLM API calls.
"""

# Import the module (this would be installed via maturin)
try:
    import simple_agents_py
except ImportError:
    print("Note: simple_agents_py module not installed.")
    print("Run: maturin develop --release")
    print("\nDemonstrating direct healing APIs:")
    print("```python")
    print("import simple_agents_py")
    print()
    print("# Parse malformed JSON")
    print('result = simple_agents_py.heal_json(\'```json\\n{"key": "value"}\\n```\')')
    print("print(result.value)  # {'key': 'value'}")
    print("print(result.confidence)  # 0.95")
    print("print(result.flags)  # ['Stripped markdown code fences']")
    print()
    print("# Coerce to schema")
    print("result = simple_agents_py.coerce_to_schema(")
    print("    {'age': '30'},")
    print("    {'type': 'object', 'properties': {'age': {'type': 'integer'}}}")
    print(")")
    print("print(result.value)  # {'age': 30}")
    print("```")
    exit(0)

print("=== Direct JSON Healing Demo ===\n")

# Example 1: Parse malformed JSON
print("1. Parsing JSON with markdown and trailing comma:")
malformed_json = '```json\n{"name": "Alice", "age": 30,}\n```'
result = simple_agents_py.heal_json(malformed_json)

print(f"   Input: {malformed_json!r}")
print(f"   Output: {result.value}")
print(f"   Confidence: {result.confidence:.2f}")
print(f"   Healed: {result.was_healed}")
print(f"   Flags: {result.flags}\n")

# Example 2: Coerce data to schema
print("2. Coercing string types to integers:")
data = {"age": "25", "score": "98.5"}
schema = {
    "type": "object",
    "properties": {
        "age": {"type": "integer"},
        "score": {"type": "number"},
    },
}
result = simple_agents_py.coerce_to_schema(data, schema)

print(f"   Input: {data}")
print(f"   Output: {result.value}")
print(f"   Confidence: {result.confidence:.2f}")
print(f"   Coerced: {result.was_coerced}")
print(f"   Flags: {result.flags}\n")

# Example 3: Perfect JSON - no healing needed
print("3. Perfect JSON (no healing):")
perfect_json = '{"name": "Bob", "age": 35}'
result = simple_agents_py.heal_json(perfect_json)

print(f"   Input: {perfect_json!r}")
print(f"   Output: {result.value}")
print(f"   Confidence: {result.confidence:.2f}")
print(f"   Healed: {result.was_healed}")
print(f"   Flags: {result.flags}\n")
