# SimpleAgents

SimpleAgents is a Rust-first workspace for building LLM applications with a unified client, provider adapters, routing, caching, healing/coercion, workflow execution, and language bindings.

[![GitHub Stars](https://img.shields.io/github/stars/CraftsMan-Labs/SimpleAgents?style=flat-square)](https://github.com/CraftsMan-Labs/SimpleAgents/stargazers)
[![GitHub Forks](https://img.shields.io/github/forks/CraftsMan-Labs/SimpleAgents?style=flat-square)](https://github.com/CraftsMan-Labs/SimpleAgents/network/members)
[![GitHub Issues](https://img.shields.io/github/issues/CraftsMan-Labs/SimpleAgents?style=flat-square)](https://github.com/CraftsMan-Labs/SimpleAgents/issues)
[![License](https://img.shields.io/badge/license-Apache--2.0%20%2F%20MIT-blue?style=flat-square)](LICENSE)

[![PyPI Version](https://img.shields.io/pypi/v/simple-agents-py?style=flat-square&logo=python)](https://pypi.org/project/simple-agents-py/)
[![PyPI Downloads](https://static.pepy.tech/badge/simple-agents-py/month)](https://pepy.tech/project/simple-agents-py)
[![npm Version](https://img.shields.io/npm/v/simple-agents-node?style=flat-square&logo=npm)](https://www.npmjs.com/package/simple-agents-node)
[![npm Downloads](https://img.shields.io/npm/dm/simple-agents-node?style=flat-square)](https://www.npmjs.com/package/simple-agents-node)

[![simple-agent-type](https://img.shields.io/crates/v/simple-agent-type?style=flat-square&logo=rust)](https://crates.io/crates/simple-agent-type)
[![simple-agent-type downloads](https://img.shields.io/crates/d/simple-agent-type?style=flat-square)](https://crates.io/crates/simple-agent-type)
[![simple-agents-core](https://img.shields.io/crates/v/simple-agents-core?style=flat-square&logo=rust)](https://crates.io/crates/simple-agents-core)
[![simple-agents-core downloads](https://img.shields.io/crates/d/simple-agents-core?style=flat-square)](https://crates.io/crates/simple-agents-core)

## Links

- Docs: https://docs.simpleagents.craftsmanlabs.net/
- Playground: https://yamslam.craftsmanlabs.net/playground
- Public skills: `skills/`

## Package Registry Stats

| Package | Registry | Version | Downloads |
|---|---|---|---|
| `simple-agents-py` | [PyPI](https://pypi.org/project/simple-agents-py/) | ![PyPI Version](https://img.shields.io/pypi/v/simple-agents-py?style=flat-square&logo=python) | ![PyPI Downloads](https://static.pepy.tech/badge/simple-agents-py/month) |
| `simple-agents-node` | [npm](https://www.npmjs.com/package/simple-agents-node) | ![npm Version](https://img.shields.io/npm/v/simple-agents-node?style=flat-square&logo=npm) | ![npm Downloads](https://img.shields.io/npm/dm/simple-agents-node?style=flat-square) |
| `simple-agent-type` | [crates.io](https://crates.io/crates/simple-agent-type) | ![crates simple-agent-type](https://img.shields.io/crates/v/simple-agent-type?style=flat-square&logo=rust) | ![downloads simple-agent-type](https://img.shields.io/crates/d/simple-agent-type?style=flat-square) |
| `simple-agents-core` | [crates.io](https://crates.io/crates/simple-agents-core) | ![crates simple-agents-core](https://img.shields.io/crates/v/simple-agents-core?style=flat-square&logo=rust) | ![downloads simple-agents-core](https://img.shields.io/crates/d/simple-agents-core?style=flat-square) |
| `simple-agents-healing` | [crates.io](https://crates.io/crates/simple-agents-healing) | ![crates simple-agents-healing](https://img.shields.io/crates/v/simple-agents-healing?style=flat-square&logo=rust) | ![downloads simple-agents-healing](https://img.shields.io/crates/d/simple-agents-healing?style=flat-square) |

## Overview

- Rust source-of-truth architecture: core behavior is implemented in Rust crates first.
- Unified client flow: request building, provider execution, routing, optional caching, and optional healing.
- Multiple integration surfaces: Rust crates, CLI, Node package, Python package, and WASM package.
- Workflow support: YAML workflow execution, runtime validation, tracing/timings, replay, and inspection tooling.

## Key Capabilities

- Provider-agnostic core with concrete providers for OpenAI, Anthropic, and OpenRouter.
- Routing and resilience: round-robin, latency-based, cost-based, fallback, and circuit-breaker helpers.
- Structured output handling: healed JSON mode and schema-coercion mode.
- Optional response caching with in-memory TTL/eviction implementation.
- Streaming support in core and bindings (binding-specific constraints apply).
- Workflow system with YAML authoring, canonical IR validation, and observability outputs.
- Cross-language capability contract fixtures and parity checks.

## Workspace Layout

- `crates/simple-agent-type` - canonical request/response types, contracts, and traits.
- `crates/simple-agents-core` - unified client orchestration.
- `crates/simple-agents-providers` - provider adapters and utilities.
- `crates/simple-agents-healing` - JSON-ish parsing and schema coercion.
- `crates/simple-agents-workflow` - YAML workflow engine, IR, validation, tracing.
- `crates/simple-agents-napi` - Node.js binding (`Client.run`, `.stream`, `.resume`).
- `crates/simple-agents-py` - Python binding (`Client.run`, `.stream`, `.resume`, `Message`, `Role`, `ContentPart`).
- `bindings/wasm` - WASM binding (`Client.runYamlString`).
- `examples/` - runnable Rust/Python/Node workflow examples.
- `docs/` - project documentation.

## Quick Start

### 1) Build and test workspace

```bash
cargo build --all
cargo test --all
```

Want the simplest YAML setup path? Start with `docs/WORKFLOW_QUICKSTART.md`.

### 3) Run a Rust example (requires provider API key)

```bash
cargo run --manifest-path examples/Cargo.toml --example full_api_example
```

### 4) Use Makefile targets

```bash
make test-rust
make clippy
make fmt
```

## Example Pointers

- Rust quick start: `docs/QUICKSTART.md`
- Rust usage patterns: `docs/USAGE.md`
- Cross-language snippets: `docs/EXAMPLES.md`
- Example programs:
  - `examples/full_api_example.rs`
  - `examples/python_client.py`
  - `examples/node_client.js`
  - `examples/workflow_email/run_with_python_package.py`
  - `examples/workflow_email/run_with_node_package.js`

## Testing and Quality

Core checks:

```bash
make test
make test-rust
make test-python
make clippy
make fmt
```

Bindings and parity checks:

```bash
make build-node
make test-node
make test-binding-contracts
make test-binding-layers
```

Rust coverage gate:

```bash
make coverage-rust
```

## Bindings Status

Current language surfaces in this repository:

- Rust crates (source-of-truth implementation)
- Node.js/TypeScript (`crates/simple-agents-napi`)
- Python (`crates/simple-agents-py`)
- Browser/WASM (`bindings/wasm`)

Cross-language capability baseline and parity details: `docs/CAPABILITY_MATRIX.md`.

## Documentation

- Docs home: `docs/index.md`
- Docs map: `docs/DOCS_MAP.md`
- Quick start: `docs/QUICKSTART.md`
- Workflow quickstart: `docs/WORKFLOW_QUICKSTART.md`
- Usage: `docs/USAGE.md`
- Architecture: `docs/ARCHITECTURE.md`
- Rust core systems: `docs/RUST_CORE_SYSTEMS.md`
- API surface map: `docs/API.md`
- Examples guide: `docs/EXAMPLES.md`
- Workflow YAML system: `docs/YAML_WORKFLOW_SYSTEM.md`
- Binding guides:
  - Python: `docs/BINDINGS_PYTHON.md`
  - Node: `docs/BINDINGS_NODE.md`
- Troubleshooting: `docs/TROUBLESHOOTING.md`
- Development guide: `docs/DEVELOPMENT.md`

## Contributing

- Start with `CONTRIBUTING.md` and `docs/DEVELOPMENT.md`.
- Follow task-tracking expectations in `TODO.md` (and `SUBAGENT_TODO.md` for larger parallel workstreams).
- Run relevant test/lint/format/parity commands before opening a PR.

## License

- Repository license file: `LICENSE` (Apache License 2.0 text).
- Package metadata in workspace includes `MIT OR Apache-2.0` for crates/bindings where declared.

For redistribution/compliance-sensitive usage, verify root license files and per-package metadata.

## Star History

[![Star History Chart](https://api.star-history.com/svg?repos=CraftsMan-Labs/SimpleAgents&type=Date)](https://star-history.com/#CraftsMan-Labs/SimpleAgents&Date)
