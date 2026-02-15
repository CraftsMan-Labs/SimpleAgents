# Cross-Language Capability Matrix

This matrix is the contract baseline for Rust, FFI, Go, Node, and Python surfaces.

## Capability Matrix

| Capability | Rust core | C FFI | Go | Node | Python |
|---|---|---|---|---|---|
| Prompt completion | Yes | Yes | Yes | Yes | Yes |
| Message completion | Yes | Yes | Yes | Yes | Yes |
| Structured tool-call results | Yes | Yes | Yes | Yes | Yes |
| Healing modes (`healed_json`, `schema`) | Yes | Yes | Yes | Yes | Yes |
| Streaming events | Yes | Yes | Yes | Yes | Yes |
| Streaming terminal event (`done`) | Yes | Yes | Yes | Yes | Yes |
| Canonical finish reasons (`stop`, `length`, `content_filter`, `tool_calls`) | Yes | Yes | Yes | Yes | Yes |

## Required CI Gates

The following checks are required for parity and DX stability:

1. `capability-contract-gates` in `.github/workflows/bindings-ci.yml`
   - Runs `scripts/run-binding-contracts.sh`.
   - Verifies shared fixture contract checks in Rust/FFI, Go, Node, and Python.
2. Binding-specific workflows must still pass independently:
   - `go-bindings`
   - `node-bindings`
   - `python-bindings`

## Shared Fixtures

- `parity-fixtures/binding_contract.json` is the source of truth for:
  - shared request/response/healing/streaming/tool-call fixture sections
  - binding-specific symbol expectations (FFI/Go/Node/Python)

If any binding changes API shape, update this fixture first, then update binding tests and docs together.
