#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

count_lines() {
  local glob="$1"
  shift
  local files

  files="$(cd "$ROOT_DIR" && rg --files -g "$glob" "$@" 2>/dev/null || true)"
  if [[ -z "$files" ]]; then
    echo 0
    return
  fi

  (cd "$ROOT_DIR" && printf '%s\n' "$files" | xargs wc -l | awk 'END{print $1 + 0}')
}

types_src="$(count_lines '*.rs' crates/simple-agent-type/src)"
providers_src="$(count_lines '*.rs' crates/simple-agents-providers/src)"
healing_src="$(count_lines '*.rs' crates/simple-agents-healing/src)"
core_src="$(count_lines '*.rs' crates/simple-agents-core/src)"
workflow_src="$(count_lines '*.rs' crates/simple-agents-workflow/src)"
py_src="$(count_lines '*.rs' crates/simple-agents-py/src)"
napi_src="$(count_lines '*.rs' crates/simple-agents-napi/src)"

all_rs_src="$(count_lines '*.rs' crates/**/src)"
all_rs_repo="$(count_lines '*.rs' crates)"
wasm_total="$(count_lines '*.rs' bindings/wasm)"
py_total="$(count_lines '*.py' crates/simple-agents-py examples)"
node_total="$(count_lines '*.js' crates/simple-agents-napi examples)"
ts_total="$(count_lines '*.ts' crates/simple-agents-napi examples)"

src_k="$(((all_rs_src + 999) / 1000))"

echo "SimpleAgents LOC report"
echo "Repository: $ROOT_DIR"
echo
echo "Rust crate LOC (src only)"
echo "- simple-agent-type:       $types_src"
echo "- simple-agents-providers: $providers_src"
echo "- simple-agents-healing:   $healing_src"
echo "- simple-agents-core:      $core_src"
echo "- simple-agents-workflow:  $workflow_src"
echo "- simple-agents-py:        $py_src"
echo "- simple-agents-napi:      $napi_src"
echo
echo "Additional totals"
echo "- All Rust src/*.rs in crates/: $all_rs_src"
echo "- All Rust *.rs in crates/:     $all_rs_repo"
echo "- WASM Rust (*.rs):             $wasm_total"
echo "- Python (*.py):                $py_total"
echo "- Node JS (*.js):               $node_total"
echo "- Node TS (*.ts):               $ts_total"
echo
echo "README snippet (copy/paste)"
echo "- 🚀 **${src_k},000+ lines** of production Rust source code"
