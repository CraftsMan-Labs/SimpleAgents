#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

echo "==> Running FFI contract tests"
cargo test -p simple-agents-ffi --test ffi_contract

echo "==> Running Go binding contract tests"
make -C "${ROOT_DIR}" test-go-bindings

echo "==> Running Node binding contract tests"
npm --prefix "${ROOT_DIR}/crates/simple-agents-napi" ci
npm --prefix "${ROOT_DIR}/crates/simple-agents-napi" test

echo "==> Running Python binding contract tests"
UV_CACHE_DIR="${ROOT_DIR}/.uv-cache" uv run \
  --directory "${ROOT_DIR}/crates/simple-agents-py" \
  --with "pytest>=8.0" \
  pytest tests/test_contract_fixtures.py tests/test_error_mapping_consistency.py

echo "==> Binding contract runner complete"
