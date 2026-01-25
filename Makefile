.PHONY: help test clippy fmt example-providers example-full-api examples

EXAMPLE ?= openai_basic

help:
	@echo "make test                  - Run all tests"
	@echo "make clippy                - Run clippy on all targets"
	@echo "make fmt                   - Check formatting"
	@echo "make example-providers     - Run a providers example (EXAMPLE=$(EXAMPLE))"
	@echo "make example-full-api      - Run examples/full_api_example.rs"
	@echo "make examples              - Run provider example + full_api_example"

test:
	cargo test --all

clippy:
	cargo clippy --all-targets

fmt:
	cargo fmt --all -- --check

example-providers:
	cargo run -p simple-agents-providers --example $(EXAMPLE)

example-full-api:
	cargo run --manifest-path examples/Cargo.toml --example full_api_example

examples: example-providers example-full-api
