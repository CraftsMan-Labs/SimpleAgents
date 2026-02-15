#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

echo "==> Building FFI release library"
cargo build -p simple-agents-ffi --release

echo "==> Go unit tests"
pushd "${ROOT_DIR}/bindings/go" >/dev/null
CGO_CFLAGS="-I${ROOT_DIR}/crates/simple-agents-ffi/include" \
CGO_LDFLAGS="-L${ROOT_DIR}/target/release" \
LD_LIBRARY_PATH="${ROOT_DIR}/target/release:${LD_LIBRARY_PATH:-}" \
go test ./... -run 'TestValidate|TestCompleteMessagesUninitializedClient|TestCompleteWithContextUninitializedClient|TestStreamMessagesUninitializedClient' \
  -count=1

echo "==> Go contract tests"
CGO_CFLAGS="-I${ROOT_DIR}/crates/simple-agents-ffi/include" \
CGO_LDFLAGS="-L${ROOT_DIR}/target/release" \
LD_LIBRARY_PATH="${ROOT_DIR}/target/release:${LD_LIBRARY_PATH:-}" \
go test ./... -run 'TestGoBindingsFollowSharedContractFixture|TestValidateCompleteOptionsGoldenCases' \
  -count=1

echo "==> Node unit tests"
npm --prefix "${ROOT_DIR}/crates/simple-agents-napi" ci
npm --prefix "${ROOT_DIR}/crates/simple-agents-napi" run test:unit

echo "==> Node contract tests"
npm --prefix "${ROOT_DIR}/crates/simple-agents-napi" run test:contract

echo "==> Python unit tests"
UV_CACHE_DIR="${ROOT_DIR}/.uv-cache" uv run \
  --directory "${ROOT_DIR}/crates/simple-agents-py" \
  --with "pytest>=8.0" \
  pytest tests/test_client_builder.py tests/test_client.py tests/test_direct_healing.py tests/test_healing.py tests/test_routing_config.py tests/test_streaming_parser.py

echo "==> Python contract tests"
UV_CACHE_DIR="${ROOT_DIR}/.uv-cache" uv run \
  --directory "${ROOT_DIR}/crates/simple-agents-py" \
  --with "pytest>=8.0" \
  pytest tests/test_contract_fixtures.py tests/test_error_mapping_consistency.py

if [[ -n "${CUSTOM_API_KEY:-}" && -n "${CUSTOM_API_MODEL:-}" && -n "${PROVIDER:-}" ]]; then
  echo "==> Live credentials detected; running live tests"

  CGO_CFLAGS="-I${ROOT_DIR}/crates/simple-agents-ffi/include" \
  CGO_LDFLAGS="-L${ROOT_DIR}/target/release" \
  LD_LIBRARY_PATH="${ROOT_DIR}/target/release:${LD_LIBRARY_PATH:-}" \
  go test ./... -run 'TestLive' -count=1
  popd >/dev/null

  npm --prefix "${ROOT_DIR}/crates/simple-agents-napi" run test:live

  UV_CACHE_DIR="${ROOT_DIR}/.uv-cache" uv run \
    --directory "${ROOT_DIR}/crates/simple-agents-py" \
    --with "pytest>=8.0" \
    pytest tests/test_integration_openai.py tests/test_streaming.py tests/test_structured_streaming.py
else
  popd >/dev/null
  echo "==> Live credentials not set; skipping live layer"
fi

echo "==> Layered binding tests complete"
