# Development Guide

This guide is for developers who want to contribute to SimpleAgents or understand its internals.

## Project Structure

SimpleAgents is a Rust workspace with multiple crates:

```
SimpleAgents/
├── crates/
│   ├── simple-agent-type/        # Core types and traits
│   ├── simple-agents-core/        # Client orchestration
│   ├── simple-agents-providers/   # Provider implementations
│   ├── simple-agents-router/      # Routing strategies
│   ├── simple-agents-cache/       # Cache implementations
│   ├── simple-agents-healing/     # JSON healing + schema coercion
│   ├── simple-agents-macros/      # Proc macros (PartialType)
│   ├── simple-agents-cli/         # CLI
│   ├── simple-agents-napi/        # Node bindings
│   └── simple-agents-py/          # Python bindings
├── docs/                          # Documentation site
├── Cargo.toml                     # Workspace manifest
└── README.md
```

## Crate Responsibilities (Quick Map)

- `simple-agent-type`: contracts + shared types.
- `simple-agents-core`: client orchestration (routing, cache, healing, middleware).
- `simple-agents-providers`: provider integrations and utilities.
- `simple-agents-router`: routing strategies and resilience helpers.
- `simple-agents-healing`: JSON-ish parsing and schema coercion.
- `simple-agents-cache`: cache implementations.
- `simple-agents-cli`: CLI interface.
- `simple-agents-napi` / `simple-agents-py`: language bindings.
- `simple-agents-macros`: proc macros for partial streaming types.

## Building

### Prerequisites

- Rust 1.75 or later
- Cargo (comes with Rust)

### Build All Crates

```bash
cargo build --all
```

### Build with Release Optimizations

```bash
cargo build --all --release
```

### Build Specific Crate

```bash
cargo build -p simple-agents-core
cargo build -p simple-agents-providers
cargo build -p simple-agents-router
```

### Check Without Building

```bash
cargo check --all
```

## Testing

### Run All Tests

```bash
cargo test --all
```

### Run Tests for Specific Crate

```bash
cargo test -p simple-agents-core
cargo test -p simple-agents-providers
cargo test -p simple-agents-healing
```

### Run Cross-Language Contract Gates

```bash
./scripts/run-binding-contracts.sh
```

### Run Layered Binding Tests (unit/contract/live)

```bash
make test-binding-layers
```

This runs unit + contract checks for Node/Python by default, and runs live suites only when credentials are provided.

### Run Workflow Runtime Benchmarks

```bash
cargo bench -p simple-agents-workflow --bench runtime_benchmarks
```

### Run Tests with Output

```bash
cargo test -- --nocapture
```

## Contributing

See the repository-level [Contributing Guide](https://github.com/CraftsMan-Labs/SimpleAgents/blob/main/CONTRIBUTING.md) for checklist discipline, required task-board updates, and skill-usage expectations.

### Getting Started

1. Fork the repository.
2. Create a feature branch: `git checkout -b feature/my-feature`.
3. Make your changes.
4. Run tests: `cargo test --all`.
5. Run clippy: `cargo clippy --all`.
6. Run fmt: `cargo fmt --all`.
7. Commit and open a PR.

### Commit Message Format

Use conventional commits:

```
feat: add routing strategy
fix: handle schema coercion edge case
docs: update usage guide
```

### Pull Request Checklist

- [ ] Tests pass (`cargo test --all`)
- [ ] No clippy warnings (`cargo clippy --all`)
- [ ] Code formatted (`cargo fmt --all`)
- [ ] Documentation updated

## Code Style

### Formatting

Use `rustfmt` with default settings:

```bash
cargo fmt --all
```

### Linting

Use `clippy` with strict settings:

```bash
cargo clippy --all -- -D warnings
```

### Documentation

- Public APIs should include doc comments.
- Document panics, errors, and safety invariants.
- Provide usage examples for complex APIs.

## Security Considerations

- Never log API keys; use `ApiKey` for validation and exposure control.
- Validate requests with `CompletionRequest::build()`.
- Use constant-time comparisons for secrets.

## Debugging

### Enable Logging

```rust
use tracing_subscriber;

tracing_subscriber::fmt::init();
```

### Run with Logs

```bash
RUST_LOG=debug cargo test
RUST_LOG=simple_agents=trace cargo run
```

## Release Process

1. Update version in all `Cargo.toml` files.
2. Update CHANGELOG.md.
3. Run full test suite: `cargo test --all`.
4. Build release: `cargo build --all --release`.
5. Tag release: `git tag v0.x.0`.
6. Push tags: `git push --tags`.
7. Publish crates as needed.
