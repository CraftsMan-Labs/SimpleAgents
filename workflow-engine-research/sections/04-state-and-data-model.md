# State and Data Model

## Recommended Repo
- https://github.com/Stranger6667/jsonschema

## Why This Repo
- Rust-native JSON Schema validator for schema-validated node I/O.
- Strong fit for a Rust-first core with JSON/YAML IR.

## Pros
- Mature JSON Schema support in Rust.
- Enables schema validation for node inputs/outputs.

## Cons
- JSON Schema can be verbose and complex to author.
- Runtime validation adds overhead if used on every node edge.

## What We Want To Build From This
- Schema validation of final payloads before edge transitions.
- Shared schema library used across node types and language workers.

## Why
- Schema validation is required for correctness and safe data flow across nodes.

## Sources
- https://docs.rs/jsonschema/latest/jsonschema/
- https://rustrepo.com/repo/stranger6667-jsonschema-rs

## Notes
- Use JSON Schema for validation of final payloads before edge transitions.
