# simple-agents-macros

Procedural macros for SimpleAgents framework.

## Overview

This crate provides derive macros for automatic schema generation and partial type support for streaming LLM responses.

## Macros

### `#[derive(PartialType)]`

Generates a partial version of a struct with all fields wrapped in `Option<T>`. Useful for progressive emission during streaming responses.

#### Example

```rust
use simple_agents_macros::PartialType;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialType, Serialize, Deserialize)]
pub struct User {
    pub id: u64,
    pub name: String,
    pub email: String,
    pub age: u32,
}

// This generates a PartialUser type:
//
// #[derive(Debug, Clone, Default, Serialize, Deserialize)]
// pub struct PartialUser {
//     pub id: Option<u64>,
//     pub name: Option<String>,
//     pub email: Option<String>,
//     pub age: Option<u32>,
// }
//
// With methods:
// - PartialUser::merge(&mut self, other: PartialUser)
// - User::from_partial(partial: PartialUser) -> Result<User, String>
```

#### Streaming Usage

```rust
let mut partial = PartialUser::default();

// Chunk 1: {"id": 1, "name": "Alice"}
partial.merge(parse_chunk(chunk1));

// Chunk 2: {"email": "alice@example.com", "age": 30}
partial.merge(parse_chunk(chunk2));

// Convert to complete type
let user = User::from_partial(partial)?;
```

#### Field Attributes

- `#[partial(default)]` - Use default value if field is missing in partial
- `#[partial(skip)]` - Exclude field from partial type (always uses default)

```rust
#[derive(PartialType)]
pub struct Resume {
    pub name: String,
    pub email: String,

    #[partial(default)]
    pub skills: Vec<String>,  // Uses Vec::new() if missing

    #[partial(skip)]
    pub created_at: SystemTime,  // Always uses Default::default()
}
```

## Generated Code

For each struct annotated with `#[derive(PartialType)]`:

1. **Partial Struct**: `Partial{TypeName}` with all fields as `Option<T>`
   - Derives: `Debug`, `Clone`, `Default`, `Serialize`, `Deserialize`

2. **Conversion Method**: `from_partial()` on the original type
   - Signature: `fn from_partial(partial: PartialType) -> Result<Type, String>`
   - Returns error if required fields are missing

3. **Merge Method**: `merge()` on the partial type
   - Signature: `fn merge(&mut self, other: PartialType)`
   - Merges two partial values (newer values take precedence)

## Requirements

- Rust 1.70 or later
- Works with `serde` for JSON serialization/deserialization

## License

MIT OR Apache-2.0
