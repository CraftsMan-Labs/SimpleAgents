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

phase_1="$(count_lines '*.rs' crates/simple-agent-type/src)"
phase_2="$(count_lines '*.rs' crates/simple-agents-providers/src)"
phase_3="$(count_lines '*.rs' crates/simple-agents-healing/src)"
phase_4="$(count_lines '*.rs' crates/simple-agents-router/src)"
phase_5="$(count_lines '*.rs' crates/simple-agents-core/src)"
phase_6="$(count_lines '*.rs' crates/simple-agents-cli/src)"
ffi_src="$(count_lines '*.rs' crates/simple-agents-ffi/src)"
py_src="$(count_lines '*.rs' crates/simple-agents-py/src)"
napi_src="$(count_lines '*.rs' crates/simple-agents-napi/src)"
phase_7="$((ffi_src + py_src + napi_src))"

phase_total="$((phase_1 + phase_2 + phase_3 + phase_4 + phase_5 + phase_6 + phase_7))"
all_rs_src="$(count_lines '*.rs' crates/**/src)"
all_rs_repo="$(count_lines '*.rs' crates)"
go_total="$(count_lines '*.go' bindings/go)"
py_total="$(count_lines '*.py' crates/simple-agents-py examples)"
node_total="$(count_lines '*.js' crates/simple-agents-napi examples)"
ts_total="$(count_lines '*.ts' crates/simple-agents-napi examples)"
src_k="$(((all_rs_src + 999) / 1000))"

echo "SimpleAgents LOC report"
echo "Repository: $ROOT_DIR"
echo
echo "Rust LOC (src only)"
echo "- Phase 1 (simple-agent-type): $phase_1"
echo "- Phase 2 (providers):         $phase_2"
echo "- Phase 3 (healing):           $phase_3"
echo "- Phase 4 (router):            $phase_4"
echo "- Phase 5 (core):              $phase_5"
echo "- Phase 6 (cli):               $phase_6"
echo "- Phase 7 (ffi+py+napi):       $phase_7"
echo "- Phase total:                 $phase_total"
echo
echo "Additional totals"
echo "- All Rust src/*.rs in crates/: $all_rs_src"
echo "- All Rust *.rs in crates/:     $all_rs_repo"
echo "- Go (*.go):                    $go_total"
echo "- Python (*.py):                $py_total"
echo "- Node JS (*.js):               $node_total"
echo "- Node TS (*.ts):               $ts_total"
echo
echo "README snippet (copy/paste)"
echo "- 🚀 **${src_k},000+ lines** of production Rust source code"
echo "| **Phase 1** | Foundation (types, traits) | ✅ Complete | $phase_1 |"
echo "| **Phase 2** | Provider Integration | ✅ Complete | $phase_2 |"
echo "| **Phase 3** | Response Healing | ✅ Complete | $phase_3 |"
echo "| **Phase 4** | Router & Strategies | ✅ Complete | $phase_4 |"
echo "| **Phase 5** | Unified Client API | ✅ Complete | $phase_5 |"
echo "| **Phase 6** | CLI & Tools | ✅ Complete | $phase_6 |"
echo "| **Phase 7** | Language Bindings | ✅ Complete | $phase_7 |"
echo "| | **TOTAL** | **✅ 100%** | **$phase_total** |"
