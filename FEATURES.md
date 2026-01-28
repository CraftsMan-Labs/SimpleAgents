# SimpleAgents Features

Comprehensive list of features with working examples for each. Each feature has a standalone, runnable example demonstrating its capabilities.

---

## Feature Categories

### 1. Core Provider Features

| Feature | Description | Example | Status |
|---------|-------------|---------|--------|
| **OpenAI Provider** | GPT-3.5, GPT-4 support with streaming | `examples/01_openai_basic.rs` | ⏳ Todo |
| **Anthropic Provider** | Claude-3 family support | `examples/02_anthropic_basic.rs` | ⏳ Todo |
| **OpenRouter Provider** | 100+ models via unified interface | `examples/03_openrouter_basic.rs` | ⏳ Todo |
| **Custom API Endpoints** | Any OpenAI-compatible API | `crates/simple-agents-providers/examples/custom_api.rs` | ✅ Done |

### 2. Request/Response Features

| Feature | Description | Example | Status |
|---------|-------------|---------|--------|
| **Simple Completion** | Basic request/response | `examples/04_simple_completion.rs` | ⏳ Todo |
| **Multi-Turn Conversations** | Maintain context across turns | `examples/05_multi_turn.rs` | ⏳ Todo |
| **Streaming Responses** | Server-Sent Events (SSE) streaming | `examples/06_streaming.rs` | ⏳ Todo |
| **System Messages** | Configure assistant behavior | `examples/07_system_messages.rs` | ⏳ Todo |
| **Temperature Control** | Adjust response creativity | `examples/08_temperature.rs` | ⏳ Todo |
| **Max Tokens** | Control response length | `examples/09_max_tokens.rs` | ⏳ Todo |

### 3. Response Healing Features

| Feature | Description | Example | Status |
|---------|-------------|---------|--------|
| **JSON Parsing** | Parse malformed JSON from LLMs | `examples/10_json_parsing.rs` | ⏳ Todo |
| **Markdown Stripping** | Remove code fences from responses | `examples/11_markdown_stripping.rs` | ⏳ Todo |
| **Type Coercion** | Convert string → int/float/bool | `examples/12_type_coercion.rs` | ⏳ Todo |
| **Fuzzy Field Matching** | Handle case variations in field names | `examples/13_fuzzy_fields.rs` | ⏳ Todo |
| **Schema Validation** | Validate against strict schemas | `examples/14_schema_validation.rs` | ⏳ Todo |
| **Confidence Scoring** | Transparency in healing operations | `examples/15_confidence_scoring.rs` | ⏳ Todo |
| **Default Value Injection** | Apply defaults for missing fields | `examples/16_default_values.rs` | ⏳ Todo |

### 4. Streaming Healing Features

| Feature | Description | Example | Status |
|---------|-------------|---------|--------|
| **Progressive JSON Parsing** | Parse JSON as it streams | `examples/17_progressive_parsing.rs` | ⏳ Todo |
| **Streaming with Healing** | Heal malformed JSON during streaming | `examples/18_streaming_healing.rs` | ⏳ Todo |
| **Partial Type Emission** | Emit partial structured data | `examples/19_partial_types.rs` | ⏳ Todo |
| **Streaming Arrays** | Stream JSON arrays progressively | `examples/20_streaming_arrays.rs` | ⏳ Todo |
| **Streaming Objects** | Stream nested objects progressively | `examples/21_streaming_objects.rs` | ⏳ Todo |

### 5. Caching Features

| Feature | Description | Example | Status |
|---------|-------------|---------|--------|
| **In-Memory Cache** | LRU cache with TTL support | `examples/22_cache_basic.rs` | ⏳ Todo |
| **Blake3 Hashing** | Fast cache key generation | `examples/23_cache_keys.rs` | ⏳ Todo |
| **Cache Eviction** | LRU eviction and size limits | `examples/24_cache_eviction.rs` | ⏳ Todo |
| **Cache TTL** | Time-based expiration | `examples/25_cache_ttl.rs` | ⏳ Todo |

### 6. Reliability Features

| Feature | Description | Example | Status |
|---------|-------------|---------|--------|
| **Retry Logic** | Exponential backoff with jitter | `examples/26_retry_logic.rs` | ⏳ Todo |
| **Rate Limiting** | Token bucket algorithm | `examples/27_rate_limiting.rs` | ⏳ Todo |
| **Connection Pooling** | HTTP/2 multiplexing | `examples/28_connection_pooling.rs` | ⏳ Todo |
| **Timeout Configuration** | Request timeout handling | `examples/29_timeouts.rs` | ⏳ Todo |
| **Error Handling** | Comprehensive error types | `examples/30_error_handling.rs` | ⏳ Todo |

### 7. Security Features

| Feature | Description | Example | Status |
|---------|-------------|---------|--------|
| **API Key Security** | Never logged, constant-time comparison | `examples/31_api_key_security.rs` | ⏳ Todo |
| **Input Validation** | Automatic request validation | `examples/32_input_validation.rs` | ⏳ Todo |
| **Redaction** | Automatic sensitive data redaction | `examples/33_redaction.rs` | ⏳ Todo |

### 8. Observability Features

| Feature | Description | Example | Status |
|---------|-------------|---------|--------|
| **Request Metrics** | Track token usage and latency | `examples/34_metrics.rs` | ⏳ Todo |
| **Request Timers** | Measure request duration | `examples/35_request_timers.rs` | ⏳ Todo |
| **Usage Tracking** | Monitor API usage | `examples/36_usage_tracking.rs` | ⏳ Todo |

### 9. Advanced Use Cases

| Feature | Description | Example | Status |
|---------|-------------|---------|--------|
| **Function Calling** | Structured function calls | `examples/37_function_calling.rs` | ⏳ Todo |
| **Tool Use** | Anthropic-style tool use | `examples/38_tool_use.rs` | ⏳ Todo |
| **Vision Models** | Image input support | `examples/39_vision.rs` | ⏳ Todo |
| **Embeddings** | Generate text embeddings | `examples/40_embeddings.rs` | ⏳ Todo |

---

## Example Structure

Each example follows this pattern:

```rust
//! Feature Name
//!
//! Description of what this feature does and why it's useful.
//!
//! # Use Cases
//!
//! - Use case 1
//! - Use case 2
//!
//! # Prerequisites
//!
//! 1. Environment setup
//! 2. Dependencies
//!
//! # Run
//!
//! ```bash
//! cargo run --example feature_name
//! ```

use simple_agents_healing::prelude::*;
use simple_agents_providers::Provider;
use simple_agent_type::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    dotenv::dotenv().ok();

    println!("╔════════════════════════════════════════════════════════╗");
    println!("║     SimpleAgents - Feature Name Demo                  ║");
    println!("╚════════════════════════════════════════════════════════╝\n");

    // Feature demonstration

    println!("\n╔══════════════════════════════════════════════════════════╗");
    println!("║                    Demo Complete!                         ║");
    println!("╚══════════════════════════════════════════════════════════╝");

    Ok(())
}
```

---

## Documentation Integration

Examples are integrated into the documentation using code includes:

### In Markdown Files

```markdown
## Feature Name

Description...

### Example

<!-- INCLUDE: examples/feature_name.rs -->

### Output

\`\`\`
Expected output...
\`\`\`
```

### In Docstrings

```rust
/// # Examples
///
/// ```rust
#[doc = include_str!("../examples/feature_name.rs")]
/// ```
```

---

## Testing Examples

All examples must:

1. **Compile**: `cargo build --examples`
2. **Run Successfully**: `cargo run --example feature_name`
3. **Be Self-Contained**: No external dependencies beyond crates
4. **Have Clear Output**: Show what feature is doing
5. **Be Documented**: Explain use cases and prerequisites

Test all examples:

```bash
# Build all examples
cargo build --examples

# Run specific example
cargo run --example feature_name

# Test example compilation as part of doc tests
cargo test --doc
```

---

## Progress Tracking

**Total Features**: 40
**Completed**: 1 (2.5%)
**In Progress**: 0
**Todo**: 39 (97.5%)

Last Updated: 2026-01-24
