#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

check_link() {
  local path="$1"
  local expected_rel="$2"
  local full_path="${ROOT_DIR}/${path}"

  if [ ! -L "${full_path}" ]; then
    echo "Error: ${path} must be a symlink"
    return 1
  fi

  local actual_target
  actual_target="$(readlink "${full_path}")"
  if [ "${actual_target}" != "${expected_rel}" ]; then
    echo "Error: ${path} points to '${actual_target}', expected '${expected_rel}'"
    return 1
  fi

  if [ ! -e "${full_path}" ]; then
    echo "Error: ${path} is a broken symlink"
    return 1
  fi
}

check_link ".agents/skills/simpleagentsbuilder/examples/minimal-chat.yaml" "../../../../skills/simpleagents-builder/examples/minimal-chat.yaml"
check_link ".agents/skills/simpleagentsbuilder/examples/email-classification.yaml" "../../../../skills/simpleagents-builder/examples/email-classification.yaml"
check_link ".agents/skills/simpleagentsbuilder/references/patterns.md" "../../../../skills/simpleagents-builder/references/patterns.md"
check_link ".agents/skills/simpleagentsbuilder/references/checklist.md" "../../../../skills/simpleagents-builder/references/checklist.md"
check_link ".opencode/skills/SimpleAgentsBuilder/examples/minimal-chat.yaml" "../../../../skills/simpleagents-builder/examples/minimal-chat.yaml"
check_link ".opencode/skills/SimpleAgentsBuilder/examples/email-classification.yaml" "../../../../skills/simpleagents-builder/examples/email-classification.yaml"
check_link ".opencode/skills/SimpleAgentsBuilder/references/patterns.md" "../../../../skills/simpleagents-builder/references/patterns.md"
check_link ".opencode/skills/SimpleAgentsBuilder/references/checklist.md" "../../../../skills/simpleagents-builder/references/checklist.md"

echo "OK: SimpleAgentsBuilder duplicate assets are canonical symlinks."
