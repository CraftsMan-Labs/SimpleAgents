#!/usr/bin/env bash
# Ensure workspace.package.version matches internal [workspace.dependencies] pins
# and that Cargo.lock is consistent with manifests.
set -euo pipefail

ROOT="$(git rev-parse --show-toplevel 2>/dev/null || true)"
if [[ -z "$ROOT" ]]; then
  ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
fi
cd "$ROOT"
if [[ ! -f Cargo.toml ]]; then
  echo "Error: Cargo.toml not found (expected at repo root: $ROOT)."
  exit 1
fi

python3 <<'PY'
from __future__ import annotations

import sys
import tomllib
from pathlib import Path

root = Path.cwd()
cargo_toml = root / "Cargo.toml"
data = tomllib.loads(cargo_toml.read_text(encoding="utf-8"))

try:
    ws_ver = data["workspace"]["package"]["version"]
except KeyError as exc:
    print("Error: missing [workspace.package] version in root Cargo.toml", file=sys.stderr)
    raise SystemExit(1) from exc

deps = data.get("workspace", {}).get("dependencies", {})
if not isinstance(deps, dict):
    print("Error: [workspace.dependencies] must be a table", file=sys.stderr)
    raise SystemExit(1)

internal_prefixes = ("simple-agents-",)
for key, spec in deps.items():
    if key != "simple-agent-type" and not key.startswith(internal_prefixes):
        continue
    if isinstance(spec, str):
        print(
            f"Error: internal workspace dependency {key!r} uses a bare version string {spec!r}; "
            "use {{ path = \"...\" }} or pin version to match [workspace.package].version.",
            file=sys.stderr,
        )
        raise SystemExit(1)
    if not isinstance(spec, dict):
        continue
    if "version" in spec:
        pinned = spec["version"]
        if pinned != ws_ver:
            print(
                f"Error: [workspace.dependencies].{key} has version {pinned!r} but "
                f"[workspace.package].version is {ws_ver!r}. "
                "Run make version-sync or align pins (prefer path-only entries for in-tree crates).",
                file=sys.stderr,
            )
            raise SystemExit(1)

print(f"OK: internal workspace dependency pins match workspace version {ws_ver!r}.")
PY

echo "Running cargo metadata --locked..."
cargo metadata --locked --format-version 1 > /dev/null
echo "OK: Cargo.lock matches manifests."
