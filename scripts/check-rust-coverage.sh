#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TOOL="${COVERAGE_TOOL:-auto}"
MIN_COVERAGE="${COVERAGE_MIN:-100}"

if [[ -d "${HOME}/.cargo/bin" ]]; then
  export PATH="${HOME}/.cargo/bin:${PATH}"
fi

resolve_tool() {
  case "${TOOL}" in
    auto)
      if command -v cargo-tarpaulin >/dev/null 2>&1; then
        echo "tarpaulin"
        return
      fi
      if command -v grcov >/dev/null 2>&1; then
        echo "grcov"
        return
      fi
      echo "none"
      ;;
    tarpaulin|grcov)
      echo "${TOOL}"
      ;;
    *)
      echo "unsupported"
      ;;
  esac
}

run_tarpaulin() {
  if ! command -v cargo-tarpaulin >/dev/null 2>&1; then
    echo "cargo-tarpaulin is not installed. Install with: cargo install cargo-tarpaulin"
    exit 1
  fi

  echo "==> Running tarpaulin coverage gate (minimum ${MIN_COVERAGE}%)"
  cargo tarpaulin --workspace --all-features \
    --exclude simple-agents-napi \
    --exclude simple-agents-py \
    --fail-under "${MIN_COVERAGE}" \
    --out Stdout
}

run_grcov() {
  if ! command -v grcov >/dev/null 2>&1; then
    echo "grcov is not installed. Install with: cargo install grcov"
    exit 1
  fi

  local coverage_dir="${ROOT_DIR}/target/coverage"
  local lcov_path="${coverage_dir}/lcov.info"

  rm -rf "${coverage_dir}"
  mkdir -p "${coverage_dir}"

  echo "==> Running LLVM coverage build for grcov"
  CARGO_INCREMENTAL=0 \
  RUSTFLAGS="-Cinstrument-coverage" \
  LLVM_PROFILE_FILE="${coverage_dir}/%p-%m.profraw" \
  cargo test --workspace --all-features \
    --exclude simple-agents-napi \
    --exclude simple-agents-py

  echo "==> Generating LCOV report with grcov"
  grcov "${coverage_dir}" \
    --binary-path "${ROOT_DIR}/target/debug/deps" \
    --source-dir "${ROOT_DIR}" \
    --output-type lcov \
    --branch \
    --ignore-not-existing \
    --ignore "${ROOT_DIR}/target/*" \
    --ignore "/*" \
    --output-path "${lcov_path}"

  python3 - "${lcov_path}" "${MIN_COVERAGE}" <<'PY'
import sys

lcov_path = sys.argv[1]
threshold = float(sys.argv[2])

found = 0
hit = 0

with open(lcov_path, "r", encoding="utf-8") as f:
    for raw_line in f:
        line = raw_line.strip()
        if line.startswith("DA:"):
            _, payload = line.split(":", 1)
            _, count = payload.split(",", 1)
            found += 1
            if int(count) > 0:
                hit += 1

if found == 0:
    print("No executable lines found in LCOV output")
    sys.exit(1)

coverage = (hit / found) * 100.0
print(f"grcov line coverage: {coverage:.2f}% ({hit}/{found})")
if coverage < threshold:
    print(f"Coverage gate failed: expected at least {threshold:.2f}%")
    sys.exit(1)
PY

  echo "==> grcov coverage gate passed"
}

SELECTED_TOOL="$(resolve_tool)"

case "${SELECTED_TOOL}" in
  tarpaulin)
    run_tarpaulin
    ;;
  grcov)
    run_grcov
    ;;
  none)
    echo "No Rust coverage tool found. Install one of:"
    echo "  cargo install cargo-tarpaulin"
    echo "  cargo install grcov"
    echo "Then rerun: make coverage-rust"
    exit 1
    ;;
  unsupported)
    echo "Unsupported COVERAGE_TOOL='${TOOL}'. Use 'auto', 'tarpaulin', or 'grcov'."
    exit 1
    ;;
  *)
    echo "Unexpected coverage tool selector result: ${SELECTED_TOOL}"
    exit 1
    ;;
esac
