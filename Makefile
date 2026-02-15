.PHONY: help test test-rust test-python test-binding-contracts test-binding-layers clippy fmt loc-report example-providers example-full-api example-node examples \
	release-ffi release-python release-go release-node release-all \
	build-node test-node publish-node test-go-bindings \
	publish-crates publish-python publish-all \
	check-publish publish-crates-dry publish-python-dry \
	version-get version-sync version-patch version-minor version-major version-set \
	tag-release version-next-patch version-next-minor version-next-major

EXAMPLE ?= openai_basic
RUST_RELEASE_DIR ?= target/release
GO_BINDINGS_DIR ?= bindings/go
GO_CACHE_DIR ?= $(CURDIR)/.go-cache
PY_CRATE_MANIFEST ?= crates/simple-agents-py/Cargo.toml
PYTHON_PROJECT_DIR ?= crates/simple-agents-py
NAPI_CRATE ?= simple-agents-napi
NAPI_PROJECT_DIR ?= crates/simple-agents-napi
NAPI_PACKAGE_JSON ?= $(NAPI_PROJECT_DIR)/package.json
ENV_FILE ?= $(CURDIR)/.env
DOPPLER_RUN ?= doppler run --command
PUBLISH_CRATES ?= simple-agent-type simple-agents-cache simple-agents-macros \
	simple-agents-healing simple-agents-router simple-agents-providers \
	simple-agents-core simple-agents-ffi
WORKSPACE_CARGO ?= Cargo.toml
VERSION ?= 0.1.0

help:
	@echo "Testing & Quality:"
	@echo "  make test                  - Run all tests"
	@echo "  make clippy                - Run clippy on all targets"
	@echo "  make fmt                   - Check formatting"
	@echo "  make loc-report            - Print LOC report and README snippet"
	@echo "  make check-publish         - Run all pre-publish checks"
	@echo ""
	@echo "Examples:"
	@echo "  make example-providers     - Run a providers example (EXAMPLE=$(EXAMPLE))"
	@echo "  make example-full-api      - Run examples/full_api_example.rs"
	@echo "  make example-node          - Run Node example (loads $(ENV_FILE))"
	@echo "  make examples              - Run provider example + full_api_example + Node example"
	@echo ""
	@echo "Building:"
	@echo "  make release-ffi           - Build C FFI library (for Go/C/other langs)"
	@echo "  make release-python        - Build Python wheels via uv"
	@echo "  make release-go            - Build Go bindings against release FFI"
	@echo "  make release-node          - Build Node napi module (Rust cdylib)"
	@echo "  make build-node            - npm install + napi build (Node package)"
	@echo "  make release-all           - Build all language artifacts"
	@echo ""
	@echo "Testing:"
	@echo "  make test-node             - Build Node addon then run node --test"
	@echo "  make test-go-bindings      - Build FFI + run Go binding tests"
	@echo "  make test-binding-contracts - Run cross-language contract gates"
	@echo "  make test-binding-layers   - Run unit/contract/live test layers"
	@echo ""
	@echo "Publishing:"
	@echo "  make publish-crates-dry    - Dry-run publish Rust crates"
	@echo "  make publish-python-dry    - Dry-run publish Python package"
	@echo "  make publish-crates        - Publish Rust crates with Doppler env"
	@echo "  make publish-python        - Publish Python package with Doppler env"
	@echo "  make publish-node          - Publish Node package (expects NPM_TOKEN)"
	@echo "  make publish-all           - Publish Rust crates + Python + Node package"
	@echo ""
	@echo "Versioning:"
	@echo "  make version-get           - Show current version"
	@echo "  make version-sync          - Sync versions across manifests"
	@echo "  make version-patch         - Bump patch version (0.1.0 -> 0.1.1)"
	@echo "  make version-minor         - Bump minor version (0.1.0 -> 0.2.0)"
	@echo "  make version-major         - Bump major version (0.1.0 -> 1.0.0)"
	@echo "  make version-set VERSION=X - Set specific version"
	@echo "  make tag-release           - Create git tag for current version"

test: test-rust test-python

test-rust:
	cargo test --all

test-python:
	cd $(PYTHON_PROJECT_DIR) && UV_CACHE_DIR=$(CURDIR)/.uv-cache uv run --env-file $(CURDIR)/.env --reinstall --with "pytest>=8.0" pytest

test-binding-contracts:
	./scripts/run-binding-contracts.sh

test-binding-layers:
	./scripts/run-binding-tests-layered.sh

clippy:
	cargo clippy --all-targets

fmt:
	cargo fmt --all -- --check

loc-report:
	./scripts/loc-report.sh

example-providers:
	cargo run -p simple-agents-providers --example $(EXAMPLE)

example-full-api:
	cargo run --manifest-path examples/Cargo.toml --example full_api_example

example-node: build-node
	@set -a; \
	if [ -f "$(ENV_FILE)" ]; then . "$(ENV_FILE)"; fi; \
	set +a; \
	node examples/node_client.js

examples: example-providers example-full-api example-node

release-ffi:
	cargo build -p simple-agents-ffi --release

release-python:
	cd $(PYTHON_PROJECT_DIR) && uv build

release-go: release-ffi
	cd $(GO_BINDINGS_DIR) && \
	CGO_CFLAGS="-I$(PWD)/crates/simple-agents-ffi/include" \
	CGO_LDFLAGS="-L$(PWD)/$(RUST_RELEASE_DIR)" \
	GOCACHE="$(GO_CACHE_DIR)" \
	LD_LIBRARY_PATH="$(PWD)/$(RUST_RELEASE_DIR):$$LD_LIBRARY_PATH" \
	go build ./...

release-node:
	cargo build -p $(NAPI_CRATE) --release

build-node:
	cd $(NAPI_PROJECT_DIR) && npm install && npm run build

release-all: release-ffi release-python release-go release-node

test-node: build-node
	@set -a; \
	if [ -f "$(ENV_FILE)" ]; then . "$(ENV_FILE)"; fi; \
	set +a; \
	cd $(NAPI_PROJECT_DIR) && npm test

test-go-bindings: release-ffi
	cd $(GO_BINDINGS_DIR) && \
	CGO_CFLAGS="-I$(PWD)/crates/simple-agents-ffi/include" \
	CGO_LDFLAGS="-L$(PWD)/$(RUST_RELEASE_DIR)" \
	GOCACHE="$(GO_CACHE_DIR)" \
	LD_LIBRARY_PATH="$(PWD)/$(RUST_RELEASE_DIR):$$LD_LIBRARY_PATH" \
	go test ./...

publish-crates:
	@set -e; for crate in $(PUBLISH_CRATES); do \
		echo "==> Publishing $$crate..."; \
		set +e; \
		out=$$($(DOPPLER_RUN) "cargo publish -p $$crate" 2>&1); \
		status=$$?; \
		set -e; \
		echo "$$out"; \
		if [ $$status -ne 0 ]; then \
			if echo "$$out" | grep -q "already exists"; then \
				echo "==> Skipping $$crate (already exists)"; \
			else \
				exit $$status; \
			fi; \
		fi; \
	done

publish-python:
	$(DOPPLER_RUN) "cd $(PYTHON_PROJECT_DIR) && rm -f dist/*.tar.gz && uv build --sdist"
	$(DOPPLER_RUN) "echo \"[publish-python] pwd=$$(pwd)\"; \
		echo \"[publish-python] python_project_dir=$(PYTHON_PROJECT_DIR)\"; \
		echo \"[publish-python] pyproject_exists=$$(test -f $(CURDIR)/$(PYTHON_PROJECT_DIR)/pyproject.toml && echo yes || echo no)\"; \
		ls -la $(CURDIR)/$(PYTHON_PROJECT_DIR) $(CURDIR)/$(PYTHON_PROJECT_DIR)/dist; \
		RAW_VERSION_LINE=$$(grep '^version = ' $(CURDIR)/$(PYTHON_PROJECT_DIR)/pyproject.toml | head -1); \
		VERSION=$$(awk -F'\"' '/^version = / {print $$2; exit}' $(CURDIR)/$(PYTHON_PROJECT_DIR)/pyproject.toml); \
		echo \"[publish-python] raw_version_line=\$$RAW_VERSION_LINE\"; \
		TOKEN_SOURCE=$$(if [ -n \"\$$V_PUBLISH_TOKEN\" ]; then echo V_PUBLISH_TOKEN; elif [ -n \"\$$UV_PUBLISH_TOKEN\" ]; then echo UV_PUBLISH_TOKEN; else echo NONE; fi); \
		TOKEN_VALUE=\$${V_PUBLISH_TOKEN:-\$$UV_PUBLISH_TOKEN}; \
		echo \"[publish-python] version=\$$VERSION\"; \
		echo \"[publish-python] token_source=\$$TOKEN_SOURCE token_len=\$${#TOKEN_VALUE}\"; \
		UV_PUBLISH_TOKEN=\$$TOKEN_VALUE uv publish $(CURDIR)/$(PYTHON_PROJECT_DIR)/dist/simple_agents_py-\$$VERSION.tar.gz"

publish-all: publish-crates publish-python publish-node

# ============================================================================
# Pre-publish checks
# ============================================================================

check-publish:
	@echo "==> Running pre-publish checks..."
	@echo ""
	@echo "==> Running tests..."
	@$(MAKE) test
	@echo ""
	@echo "==> Running clippy..."
	@$(MAKE) clippy
	@echo ""
	@echo "==> Checking formatting..."
	@$(MAKE) fmt
	@echo ""
	@echo "==> Verifying crate metadata..."
	@for crate in $(PUBLISH_CRATES); do \
		echo "Checking $$crate..."; \
		cargo package --list -p $$crate --allow-dirty > /dev/null || exit 1; \
	done
	@echo ""
	@echo "==> Dry-run publishing crates..."
	@$(MAKE) publish-crates-dry
	@echo ""
	@echo "==> All pre-publish checks passed! ✓"

publish-crates-dry:
	@echo "==> Dry-run publishing Rust crates..."
	@set -e; for crate in $(PUBLISH_CRATES); do \
		echo ""; \
		echo "Dry-run: $$crate"; \
		cargo package -p $$crate --allow-dirty --list > /dev/null || exit 1; \
		echo "  ✓ $$crate packages successfully"; \
	done
	@echo ""
	@echo "==> Dry-run completed successfully! ✓"
	@echo "Note: This validates packaging only. Dependencies will be resolved during actual publish."

publish-python-dry:
	@echo "==> Dry-run publishing Python package..."
	@cd $(PYTHON_PROJECT_DIR) && rm -f dist/*.tar.gz && uv build --sdist
	@echo "==> Build successful! ✓"
	@echo ""
	@echo "To publish for real, run: make publish-python"

publish-node: version-sync build-node
	@set -e; \
	$(DOPPLER_RUN) "set -e; \
		cd $(NAPI_PROJECT_DIR); \
		tmp_npmrc=\$$(mktemp); \
		trap 'rm -f \"\$$tmp_npmrc\"' EXIT; \
		if [ -z \"\$$NPM_TOKEN\" ]; then \
			echo 'NPM_TOKEN is missing in Doppler context'; \
			exit 1; \
		fi; \
		printf 'registry=https://registry.npmjs.org/\n//registry.npmjs.org/:_authToken=%s\n' \"\$$NPM_TOKEN\" > \"\$$tmp_npmrc\"; \
		NPM_CONFIG_USERCONFIG=\"\$$tmp_npmrc\" npm whoami; \
		NPM_CONFIG_USERCONFIG=\"\$$tmp_npmrc\" npm publish --access public"

# ============================================================================
# Version management
# ============================================================================

version-get:
	@grep '^version = ' $(WORKSPACE_CARGO) | head -1 | sed 's/version = "\(.*\)"/\1/'

version-sync:
	@./scripts/sync-versions.sh
	@version=$$($(MAKE) --no-print-directory version-get); \
	if [ -f "$(NAPI_PACKAGE_JSON)" ]; then \
		node -e "const fs=require('fs'); const p=process.argv[1]; const v=process.argv[2]; const j=JSON.parse(fs.readFileSync(p,'utf8')); j.version=v; fs.writeFileSync(p, JSON.stringify(j, null, 2)+'\n');" "$(NAPI_PACKAGE_JSON)" "$$version"; \
		echo "✓ Node package version updated ($(NAPI_PACKAGE_JSON) -> $$version)"; \
	fi

version-next-patch:
	@current=$$($(MAKE) --no-print-directory version-get); \
	IFS='.' read -r major minor patch <<< "$$current"; \
	patch=$$((patch + 1)); \
	echo "$$major.$$minor.$$patch"

version-next-minor:
	@current=$$($(MAKE) --no-print-directory version-get); \
	IFS='.' read -r major minor patch <<< "$$current"; \
	minor=$$((minor + 1)); \
	echo "$$major.$$minor.0"

version-next-major:
	@current=$$($(MAKE) --no-print-directory version-get); \
	IFS='.' read -r major minor patch <<< "$$current"; \
	major=$$((major + 1)); \
	echo "$$major.0.0"

version-patch:
	@current=$$($(MAKE) --no-print-directory version-get); \
	new=$$($(MAKE) --no-print-directory version-next-patch); \
	echo "Bumping version: $$current -> $$new"; \
	sed -i.bak 's/^version = ".*"/version = "'$$new'"/' $(WORKSPACE_CARGO); \
	rm -f $(WORKSPACE_CARGO).bak; \
	sed -i.bak 's/^version = ".*"/version = "'$$new'"/' $(PY_CRATE_MANIFEST); \
	rm -f $(PY_CRATE_MANIFEST).bak; \
	sed -i.bak 's/^version = ".*"/version = "'$$new'"/' crates/simple-agents-py/pyproject.toml; \
	rm -f crates/simple-agents-py/pyproject.toml.bak; \
	sed -i.bak 's/^version = ".*"/version = "'$$new'"/' examples/pyproject.toml; \
	rm -f examples/pyproject.toml.bak; \
	$(MAKE) --no-print-directory version-sync; \
	echo "Version bumped to $$new"; \
	echo ""; \
	echo "Next steps:"; \
	echo "  1. Review changes: git diff"; \
	echo "  2. Run checks: make check-publish"; \
	echo "  3. Commit: git commit -am 'chore(release): bump version to $$new'"; \
	echo "  4. Tag: make tag-release"; \
	echo "  5. Push: git push origin main --tags"

version-minor:
	@current=$$($(MAKE) --no-print-directory version-get); \
	new=$$($(MAKE) --no-print-directory version-next-minor); \
	echo "Bumping version: $$current -> $$new"; \
	sed -i.bak 's/^version = ".*"/version = "'$$new'"/' $(WORKSPACE_CARGO); \
	rm -f $(WORKSPACE_CARGO).bak; \
	sed -i.bak 's/^version = ".*"/version = "'$$new'"/' $(PY_CRATE_MANIFEST); \
	rm -f $(PY_CRATE_MANIFEST).bak; \
	sed -i.bak 's/^version = ".*"/version = "'$$new'"/' crates/simple-agents-py/pyproject.toml; \
	rm -f crates/simple-agents-py/pyproject.toml.bak; \
	sed -i.bak 's/^version = ".*"/version = "'$$new'"/' examples/pyproject.toml; \
	rm -f examples/pyproject.toml.bak; \
	$(MAKE) --no-print-directory version-sync; \
	echo "Version bumped to $$new"; \
	echo ""; \
	echo "Next steps:"; \
	echo "  1. Review changes: git diff"; \
	echo "  2. Run checks: make check-publish"; \
	echo "  3. Commit: git commit -am 'chore(release): bump version to $$new'"; \
	echo "  4. Tag: make tag-release"; \
	echo "  5. Push: git push origin main --tags"

version-major:
	@current=$$($(MAKE) --no-print-directory version-get); \
	new=$$($(MAKE) --no-print-directory version-next-major); \
	echo "Bumping version: $$current -> $$new"; \
	sed -i.bak 's/^version = ".*"/version = "'$$new'"/' $(WORKSPACE_CARGO); \
	rm -f $(WORKSPACE_CARGO).bak; \
	sed -i.bak 's/^version = ".*"/version = "'$$new'"/' $(PY_CRATE_MANIFEST); \
	rm -f $(PY_CRATE_MANIFEST).bak; \
	sed -i.bak 's/^version = ".*"/version = "'$$new'"/' crates/simple-agents-py/pyproject.toml; \
	rm -f crates/simple-agents-py/pyproject.toml.bak; \
	sed -i.bak 's/^version = ".*"/version = "'$$new'"/' examples/pyproject.toml; \
	rm -f examples/pyproject.toml.bak; \
	$(MAKE) --no-print-directory version-sync; \
	echo "Version bumped to $$new"; \
	echo ""; \
	echo "Next steps:"; \
	echo "  1. Review changes: git diff"; \
	echo "  2. Run checks: make check-publish"; \
	echo "  3. Commit: git commit -am 'chore(release): bump version to $$new'"; \
	echo "  4. Tag: make tag-release"; \
	echo "  5. Push: git push origin main --tags"

version-set:
	@if [ -z "$(VERSION)" ]; then \
		echo "Error: VERSION not specified"; \
		echo "Usage: make version-set VERSION=0.2.0"; \
		exit 1; \
	fi; \
	current=$$($(MAKE) --no-print-directory version-get); \
	echo "Setting version: $$current -> $(VERSION)"; \
	sed -i.bak 's/^version = ".*"/version = "$(VERSION)"/' $(WORKSPACE_CARGO); \
	rm -f $(WORKSPACE_CARGO).bak; \
	sed -i.bak 's/^version = ".*"/version = "$(VERSION)"/' $(PY_CRATE_MANIFEST); \
	rm -f $(PY_CRATE_MANIFEST).bak; \
	sed -i.bak 's/^version = ".*"/version = "$(VERSION)"/' crates/simple-agents-py/pyproject.toml; \
	rm -f crates/simple-agents-py/pyproject.toml.bak; \
	sed -i.bak 's/^version = ".*"/version = "$(VERSION)"/' examples/pyproject.toml; \
	rm -f examples/pyproject.toml.bak; \
	$(MAKE) --no-print-directory version-sync; \
	echo "Version set to $(VERSION)"; \
	echo ""; \
	echo "Next steps:"; \
	echo "  1. Review changes: git diff"; \
	echo "  2. Run checks: make check-publish"; \
	echo "  3. Commit: git commit -am 'chore(release): bump version to $(VERSION)'"; \
	echo "  4. Tag: make tag-release"; \
	echo "  5. Push: git push origin main --tags"

tag-release:
	@version=$$($(MAKE) --no-print-directory version-get); \
	echo "Creating release tag v$$version..."; \
	git tag -a "v$$version" -m "Release version $$version"; \
	echo "Tag v$$version created"; \
	echo ""; \
	echo "Push with: git push origin main --tags"
