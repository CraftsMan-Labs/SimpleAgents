#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

if [[ -f "${ROOT_DIR}/.env" ]]; then
	set -a
	# shellcheck disable=SC1091
	. "${ROOT_DIR}/.env"
	set +a
fi

echo "==> Node unit tests"
npm --prefix "${ROOT_DIR}/crates/simple-agents-napi" ci
npm --prefix "${ROOT_DIR}/crates/simple-agents-napi" run build:debug
npm --prefix "${ROOT_DIR}/crates/simple-agents-napi" run test:unit

echo "==> Node contract tests"
npm --prefix "${ROOT_DIR}/crates/simple-agents-napi" run test:contract

echo "==> Python unit tests"
UV_CACHE_DIR="${ROOT_DIR}/.uv-cache" uv run \
  --directory "${ROOT_DIR}/crates/simple-agents-py" \
  --with "pytest>=8.0" \
  pytest tests/test_client.py tests/test_direct_healing.py tests/test_healing.py tests/test_streaming_parser.py \
         tests/test_workflow_payload.py tests/test_workflow_stream_dispatch.py

echo "==> Python contract tests"
UV_CACHE_DIR="${ROOT_DIR}/.uv-cache" uv run \
  --directory "${ROOT_DIR}/crates/simple-agents-py" \
  --with "pytest>=8.0" \
  pytest tests/test_contract_fixtures.py tests/test_error_mapping_consistency.py

if [[ -n "${CUSTOM_API_KEY:-}" && -n "${CUSTOM_API_MODEL:-}" && -n "${PROVIDER:-}" ]]; then
  echo "==> Live credentials detected; running live tests"

  npm --prefix "${ROOT_DIR}/crates/simple-agents-napi" run test:live

  UV_CACHE_DIR="${ROOT_DIR}/.uv-cache" uv run \
    --directory "${ROOT_DIR}/crates/simple-agents-py" \
    --with "pytest>=8.0" \
    pytest tests/test_integration_openai.py tests/test_streaming.py
else
  echo "==> Live credentials not set; skipping live layer"
fi

echo "==> Layered binding tests complete"
