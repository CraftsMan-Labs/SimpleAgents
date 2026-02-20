#!/usr/bin/env bash
set -euo pipefail

workspace_version="$({
python - <<'PY'
from pathlib import Path
import tomllib

data = tomllib.loads(Path("Cargo.toml").read_text(encoding="utf-8"))
print(data["workspace"]["package"]["version"])
PY
})"

readme_version="$({
python - <<'PY'
from pathlib import Path
import re
import sys

text = Path("README.md").read_text(encoding="utf-8")
match = re.search(r"\*\*Current Version\*\*:\s*([0-9]+\.[0-9]+\.[0-9]+)", text)
if not match:
    print("missing")
    sys.exit(0)
print(match.group(1))
PY
})"

if [[ "$readme_version" == "missing" ]]; then
  echo "README.md does not contain a '**Current Version**: x.y.z' marker."
  exit 1
fi

if [[ "$workspace_version" != "$readme_version" ]]; then
  echo "Version drift detected: workspace=$workspace_version README=$readme_version"
  exit 1
fi

echo "README version check passed: $readme_version"
