# simple-agents-healing

Response healing system for SimpleAgents - BAML-inspired JSON parsing and type coercion.

## Overview

This crate implements a sophisticated JSON healing system that handles malformed LLM outputs. Instead of failing on broken JSON, it tracks all transformations via a flag system and provides confidence scores, making LLM integrations robust in production.

## Features

### 🔧 Three-Phase Parsing

1. **Strip & Fix**: Quick string transformations
   - Remove markdown code fences (`````json ... ```)
   - Fix trailing commas (`{...,}` → `{...}`)
   - Normalize quotes (`'...'` → `"..."`)
   - Remove BOM and control characters
   - Fix unquoted keys (optional with `regex-support` feature)

2. **Standard Parse**: Fast path using `serde_json`
   - If JSON is valid after Strip & Fix, use standard parser
   - Optimal performance for slightly malformed inputs

3. **Lenient Parse**: State machine for deeply broken JSON
   - Character-by-character parsing
   - Auto-close unclosed structures
   - Handle incomplete streaming responses
   - Truncate to last valid object/array

### 📊 Confidence Scoring

Every parse result includes a confidence score (0.0-1.0):

- **1.0**: Perfect JSON, no healing needed
- **0.95-0.99**: Minor fixes (markdown, trailing commas)
- **0.85-0.94**: Quote normalization or simple fixes
- **0.70-0.84**: Type coercion or truncation
- **<0.70**: Significant healing required

### 🏷️ Transparency via Flags

All transformations are tracked with `CoercionFlag`:

- `StrippedMarkdown` - Removed code fences
- `FixedTrailingComma` - Removed trailing commas
- `FixedQuotes` - Normalized single quotes to double
- `FixedUnquotedKeys` - Added quotes to object keys
- `FixedControlCharacters` - Removed control characters
- `RemovedBom` - Removed byte order mark
- `TruncatedJson` - Truncated incomplete JSON

## Usage

### Basic Example

```rust
use simple_agents_healing::prelude::*;

let parser = JsonishParser::new();

// Parse markdown-wrapped JSON
let malformed = r#"```json
{"name": "Alice", "age": 30,}
```"#;

let result = parser.parse(malformed)?;

// Access parsed value
assert_eq!(result.value["name"], "Alice");
assert_eq!(result.value["age"], 30);

// Check confidence and flags
assert!(result.confidence > 0.85);
assert!(result.flags.contains(&CoercionFlag::StrippedMarkdown));
assert!(result.flags.contains(&CoercionFlag::FixedTrailingComma));
```

### Custom Configuration

```rust
use simple_agents_healing::prelude::*;

let config = ParserConfig {
    strip_markdown: true,
    fix_trailing_commas: true,
    fix_quotes: true,
    fix_unquoted_keys: false,
    fix_control_chars: true,
    remove_bom: true,
    min_confidence: 0.8,  // Reject results below 0.8
};

let parser = JsonishParser::with_config(config);
let result = parser.parse(input)?;
```

### Confidence Thresholds

```rust
use simple_agents_healing::prelude::*;

let strict_config = ParserConfig {
    min_confidence: 0.95,
    ..Default::default()
};

let parser = JsonishParser::with_config(strict_config);

match parser.parse(input) {
    Ok(result) => {
        println!("High confidence: {}", result.confidence);
    }
    Err(SimpleAgentsError::Healing(HealingError::LowConfidence { .. })) => {
        println!("Confidence too low, requires review");
    }
    Err(e) => {
        println!("Parse failed: {}", e);
    }
}
```

## Examples

Run the included examples:

```bash
# Basic healing demonstration
cargo run --example basic_healing
```

## Testing

```bash
# Run all tests
cargo test -p simple-agents-healing

# Run with coverage
cargo test -p simple-agents-healing --all-features

# Run clippy
cargo clippy -p simple-agents-healing -- -D warnings
```

## Architecture

### Parser Components

```
Input String
    ↓
┌─────────────────────┐
│  Strip & Fix Phase  │ ← Remove markdown, fix commas, quotes
└─────────────────────┘
    ↓
┌─────────────────────┐
│   Standard Parse    │ ← Try serde_json (fast path)
└─────────────────────┘
    ↓ (if fails)
┌─────────────────────┐
│   Lenient Parse     │ ← State machine for broken JSON
└─────────────────────┘
    ↓
CoercionResult<Value>
```

### Flag System

Every transformation is tracked:

```rust
pub enum CoercionFlag {
    StrippedMarkdown,
    FixedTrailingComma,
    FixedQuotes,
    FixedUnquotedKeys,
    FixedControlCharacters,
    RemovedBom,
    TruncatedJson,
    // More flags for coercion engine (future)
}
```

### Confidence Calculation

```rust
confidence = 1.0
             * (markdown_stripped ? 0.95 : 1.0)
             * (trailing_comma_fixed ? 0.95 : 1.0)
             * (quotes_fixed ? 0.90 : 1.0)
             * (unquoted_keys_fixed ? 0.85 : 1.0)
             * (truncated ? 0.70 : 1.0)
```

## Future Features

### Week 6 (Planned)

- **Coercion Engine**: Type coercion with schema validation
  - String → Number coercion
  - Fuzzy field matching (case-insensitive, snake_case ↔ camelCase)
  - Union resolution with best-match selection
  - Default value injection

- **Streaming Parser**: Incremental parsing
  - Partial value extraction from incomplete buffers
  - Progressive emission during streaming
  - Annotation support (`stream.not_null`, `stream.done`)

## Performance

- **Fast path**: Standard `serde_json` for valid JSON after Strip & Fix
- **Zero allocations**: In-place string transformations where possible
- **Minimal overhead**: Flags are small enums, confidence is a single f32

## Safety

- **No unsafe code**: 100% safe Rust
- **No panics**: All errors are `Result` types
- **Send + Sync**: Parser can be shared across threads

## Credits

Inspired by [BAML's Jsonish parser](https://github.com/BoundaryML/baml) and coercion system.

## License

MIT OR Apache-2.0
