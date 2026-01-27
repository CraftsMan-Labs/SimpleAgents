.PHONY: help test clippy fmt example-providers example-full-api examples \
	release-ffi release-python release-go release-node release-all \
	publish-crates publish-python publish-all

EXAMPLE ?= openai_basic
RUST_RELEASE_DIR ?= target/release
GO_BINDINGS_DIR ?= bindings/go
PY_CRATE_MANIFEST ?= crates/simple-agents-py/Cargo.toml
NAPI_CRATE ?= simple-agents-napi
DOPPLER_RUN ?= doppler run --command
PUBLISH_CRATES ?= simple-agents-types simple-agents-providers simple-agents-cache simple-agents-core simple-agents-ffi

help:
	@echo "make test                  - Run all tests"
	@echo "make clippy                - Run clippy on all targets"
	@echo "make fmt                   - Check formatting"
	@echo "make example-providers     - Run a providers example (EXAMPLE=$(EXAMPLE))"
	@echo "make example-full-api      - Run examples/full_api_example.rs"
	@echo "make examples              - Run provider example + full_api_example"
	@echo "make release-ffi           - Build C FFI library (for Go/C/other langs)"
	@echo "make release-python        - Build Python wheels via maturin"
	@echo "make release-go            - Build Go bindings against release FFI"
	@echo "make release-node          - Build Node napi module (Rust cdylib)"
	@echo "make release-all           - Build all language artifacts"
	@echo "make publish-crates        - Publish Rust crates with Doppler env"
	@echo "make publish-python        - Publish Python package with Doppler env"
	@echo "make publish-all           - Publish Rust crates + Python package"

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

release-ffi:
	cargo build -p simple-agents-ffi --release

release-python:
	maturin build -m $(PY_CRATE_MANIFEST) --release

release-go: release-ffi
	CGO_CFLAGS="-I$(PWD)/crates/simple-agents-ffi/include" \
	CGO_LDFLAGS="-L$(PWD)/$(RUST_RELEASE_DIR)" \
	go build ./$(GO_BINDINGS_DIR)

release-node:
	cargo build -p $(NAPI_CRATE) --release

release-all: release-ffi release-python release-go release-node

publish-crates:
	@set -e; for crate in $(PUBLISH_CRATES); do \
		$(DOPPLER_RUN) "cargo publish -p $$crate"; \
	done

publish-python:
	$(DOPPLER_RUN) "maturin publish -m $(PY_CRATE_MANIFEST)"

publish-all: publish-crates publish-python
