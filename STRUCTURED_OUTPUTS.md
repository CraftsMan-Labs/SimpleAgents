# Structured Outputs Guide

This guide explains how to get structured JSON outputs from LLMs using SimpleAgents.

## Provider Support

| Provider | Support | Models | Parameter | Beta Header |
|----------|---------|--------|-----------|-------------|
| **OpenAI** | ✅ Full | gpt-4o-mini, gpt-4o, gpt-4-turbo | `response_format` | Not required |
| **Anthropic** | ✅ Full | claude-sonnet-4.5, claude-opus-4.5, claude-haiku-4.5 | `output_format` | `anthropic-beta: structured-outputs-2025-11-13` |
| **OpenRouter** | ✅ Passthrough | Any OpenAI model via OpenRouter | `response_format` | Not required |
| **Other** | ❌ Use Healing System | All models | N/A | N/A |

## Two Approaches

### 1. Native Structured Outputs (Recommended for OpenAI/Anthropic)

Use provider-native structured outputs with JSON schema for guaranteed structure validation.

**Pros:**
- ✅ Guaranteed to match schema
- ✅ No parsing/healing needed
- ✅ Strict type validation
- ✅ Fastest approach

**Cons:**
- ❌ Limited to specific providers/models
- ❌ Different implementations per provider

### 2. Healing System (Universal Approach)

Use SimpleAgents' healing system for flexible JSON parsing with coercion.

**Pros:**
- ✅ Works with any LLM provider
- ✅ Handles malformed JSON
- ✅ Type coercion (e.g., `"42"` → `42`)
- ✅ Fuzzy field matching
- ✅ Streaming support with partial extraction

**Cons:**
- ❌ Requires additional parsing step
- ❌ May need confidence threshold tuning

---

## Quick Start: Native OpenAI Structured Outputs

### Method 1: JSON Object Mode

Simple mode that ensures valid JSON but doesn't validate structure:

```rust
use simple_agents_providers::openai::OpenAIProvider;
use simple_agents_providers::Provider;
use simple_agents_types::prelude::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let api_key = ApiKey::new(std::env::var("OPENAI_API_KEY")?)?;
    let provider = OpenAIProvider::new(api_key)?;

    let request = CompletionRequest::builder()
        .model("gpt-4o-mini")
        .message(Message::system("You output JSON."))
        .message(Message::user("Generate a user profile with name and age"))
        .json_mode()  // Enable JSON object mode
        .build()?;

    let provider_request = provider.transform_request(&request)?;
    let provider_response = provider.execute(provider_request).await?;
    let response = provider.transform_response(provider_response)?;

    // Parse the JSON response
    let json: serde_json::Value = serde_json::from_str(
        response.content().unwrap_or("")
    )?;

    println!("{}", serde_json::to_string_pretty(&json)?);
    Ok(())
}
```

### Method 2: JSON Schema Mode (Strict Validation)

Define a schema to guarantee the structure:

```rust
use serde_json::json;

let schema = json!({
    "type": "object",
    "properties": {
        "name": {
            "type": "string",
            "description": "User's full name"
        },
        "age": {
            "type": "integer",
            "minimum": 0,
            "maximum": 150
        },
        "email": {
            "type": "string",
            "format": "email"
        }
    },
    "required": ["name", "age", "email"],
    "additionalProperties": false
});

let request = CompletionRequest::builder()
    .model("gpt-4o-mini")
    .message(Message::user("Create a user profile"))
    .json_schema("user_profile", schema)  // Structured output with schema
    .build()?;

// ... execute request ...

// Output is guaranteed to match the schema!
```

### Method 3: Custom ResponseFormat

For advanced use cases, build your own response format:

```rust
use simple_agents_types::request::{ResponseFormat, JsonSchemaFormat};
use serde_json::json;

let response_format = ResponseFormat::JsonSchema {
    json_schema: JsonSchemaFormat {
        name: "my_schema".to_string(),
        schema: json!({
            "type": "object",
            "properties": {
                "result": { "type": "string" }
            }
        }),
        strict: Some(true),  // Enable strict mode
    },
};

let request = CompletionRequest::builder()
    .model("gpt-4o-mini")
    .message(Message::user("Your prompt"))
    .response_format(response_format)
    .build()?;
```

---

## JSON Schema Examples

### Simple Object

```json
{
  "type": "object",
  "properties": {
    "name": { "type": "string" },
    "age": { "type": "integer" }
  },
  "required": ["name", "age"],
  "additionalProperties": false
}
```

### Array of Objects

```json
{
  "type": "object",
  "properties": {
    "items": {
      "type": "array",
      "items": {
        "type": "object",
        "properties": {
          "id": { "type": "string" },
          "value": { "type": "number" }
        },
        "required": ["id", "value"]
      }
    }
  },
  "required": ["items"]
}
```

### Nested Objects

```json
{
  "type": "object",
  "properties": {
    "user": {
      "type": "object",
      "properties": {
        "name": { "type": "string" },
        "address": {
          "type": "object",
          "properties": {
            "street": { "type": "string" },
            "city": { "type": "string" }
          },
          "required": ["street", "city"]
        }
      },
      "required": ["name", "address"]
    }
  },
  "required": ["user"]
}
```

### Enums

```json
{
  "type": "object",
  "properties": {
    "status": {
      "type": "string",
      "enum": ["active", "inactive", "pending"]
    },
    "priority": {
      "type": "integer",
      "enum": [1, 2, 3, 4, 5]
    }
  },
  "required": ["status", "priority"]
}
```

---

## Healing System Approach

For universal LLM support or handling malformed JSON:

```rust
use simple_agents_healing::prelude::*;

// Define schema
let schema = Schema::object(vec![
    ("name".into(), Schema::String, true),
    ("age".into(), Schema::Int, true),
]);

// ... execute request ...

// Parse with healing
let parser = JsonishParser::new();
let parse_result = parser.parse(response.content().unwrap_or(""))?;

// Coerce to match schema
let engine = CoercionEngine::new();
let result = engine.coerce(&parse_result.value, &schema)?;

println!("Confidence: {:.2}", result.confidence);
println!("Output: {}", serde_json::to_string_pretty(&result.value)?);

// Check what transformations were needed
for flag in &result.flags {
    println!("Transformation: {:?}", flag);
}
```

---

## When to Use Each Approach

### Use Native OpenAI Structured Outputs when:
- Using OpenAI models (gpt-4o-mini, gpt-4o, gpt-4-turbo)
- You need guaranteed schema compliance
- Performance is critical
- You don't need to support other LLM providers

### Use Healing System when:
- Supporting multiple LLM providers (Anthropic, OpenRouter, etc.)
- Handling potentially malformed JSON
- Need streaming with partial extraction
- Want fuzzy field matching for flexibility
- Need confidence scoring

---

## Combining Both Approaches

You can use native structured outputs for OpenAI and fall back to healing for other providers:

```rust
let request = CompletionRequest::builder()
    .model("gpt-4o-mini")
    .message(Message::user("Generate data"))
    .json_schema("my_data", schema)  // Native for OpenAI
    .build()?;

// ... execute ...

// Parse response
let json_result = if provider.name() == "openai" {
    // Native structured output - parse directly
    serde_json::from_str(response.content().unwrap_or(""))?
} else {
    // Use healing system for other providers
    let parser = JsonishParser::new();
    let parse_result = parser.parse(response.content().unwrap_or(""))?;
    parse_result.value
};
```

---

## Quick Start: Anthropic Claude Structured Outputs

Anthropic uses the same API in SimpleAgents, but with different underlying implementation:

```rust
use simple_agents_providers::anthropic::AnthropicProvider;
use simple_agents_providers::Provider;
use simple_agents_types::prelude::*;
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let api_key = ApiKey::new(std::env::var("ANTHROPIC_API_KEY")?)?;
    let provider = AnthropicProvider::new(api_key)?;

    let schema = json!({
        "type": "object",
        "properties": {
            "name": { "type": "string" },
            "age": { "type": "integer" }
        },
        "required": ["name", "age"]
    });

    let request = CompletionRequest::builder()
        .model("claude-sonnet-4.5")
        .message(Message::user("Create a user profile"))
        .json_schema("user_profile", schema)  // Same API!
        .build()?;

    // ... execute request ...
    // Output is guaranteed to match the schema!
    Ok(())
}
```

**Note:** Anthropic automatically adds the required `anthropic-beta` header when structured outputs are used.

---

## Running Examples

```bash
# OpenAI structured output example
export OPENAI_API_KEY="sk-..."
cargo run --example openai_structured_output

# Anthropic structured output example
export ANTHROPIC_API_KEY="sk-ant-..."
cargo run --example anthropic_structured_output

# Healing system example (works with any provider)
cargo run --example streaming_with_healing
```

---

## Provider-Specific Details

### OpenAI

- **API Parameter**: `response_format`
- **Modes**: `text`, `json_object`, `json_schema`
- **Models**: gpt-4o-mini, gpt-4o, gpt-4-turbo
- **Beta Header**: Not required
- **Streaming**: Supported

### Anthropic

- **API Parameter**: `output_format` (converted automatically)
- **Modes**: `json_schema` only (no plain json_object mode)
- **Models**: claude-sonnet-4.5, claude-opus-4.5, claude-haiku-4.5
- **Beta Header**: `anthropic-beta: structured-outputs-2025-11-13` (added automatically)
- **Streaming**: Supported
- **Public Beta**: Available since November 14, 2025

### OpenRouter

- **API Parameter**: `response_format` (passthrough)
- **Modes**: Depends on underlying model
- **Models**: Any OpenAI model accessed via OpenRouter
- **Note**: Only works if the underlying model supports structured outputs

---

## JSON Schema Resources

- [OpenAI Structured Outputs Docs](https://platform.openai.com/docs/guides/structured-outputs)
- [Anthropic Structured Outputs Docs](https://platform.claude.com/docs/en/build-with-claude/structured-outputs)
- [OpenRouter Structured Outputs Docs](https://openrouter.ai/docs/guides/features/structured-outputs)
- [JSON Schema Specification](https://json-schema.org/)
- [JSON Schema Validator](https://www.jsonschemavalidator.net/)

---

## Troubleshooting

### "Model does not support structured outputs"

Only these models support native structured outputs:
- `gpt-4o-mini` (recommended)
- `gpt-4o`
- `gpt-4-turbo`

### "Schema validation failed"

Ensure your schema is valid JSON Schema format:
- All required properties are listed
- Types match expected values
- No extra properties if `additionalProperties: false`

### "Response is not valid JSON"

If using `json_mode` (not `json_schema`), the model might still produce invalid JSON. Consider:
1. Improve your system prompt
2. Use `json_schema` instead for strict validation
3. Fall back to the healing system

---

## Advanced Usage

### Strict Mode

Strict mode enforces exact schema compliance:

```rust
use simple_agents_types::request::{ResponseFormat, JsonSchemaFormat};

let response_format = ResponseFormat::JsonSchema {
    json_schema: JsonSchemaFormat {
        name: "strict_schema".to_string(),
        schema: your_schema,
        strict: Some(true),  // Enable strict mode
    },
};
```

### Dynamic Schema Generation

Generate schemas from Rust types using schemars:

```rust
use schemars::{schema_for, JsonSchema};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, JsonSchema)]
struct User {
    name: String,
    age: u32,
}

let schema = schema_for!(User);
let schema_json = serde_json::to_value(&schema)?;

let request = CompletionRequest::builder()
    .model("gpt-4o-mini")
    .json_schema("user", schema_json)
    .build()?;
```

Add to `Cargo.toml`:
```toml
[dependencies]
schemars = "0.8"
```
