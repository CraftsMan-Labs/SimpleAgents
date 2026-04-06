# Codebase review (read-only audit)

**Date:** 2026-04-05  
**Scope:** Security posture, code smells, redundancy, and developer experience. **No source code was modified** as part of this pass.

**What was added:** Only Markdown under this directory. See [00-documentation-record.md](./00-documentation-record.md) for a file manifest and changelog-style record.

## How to read this folder

| Document | Contents |
|----------|----------|
| [01-executive-summary.md](./01-executive-summary.md) | Top themes, quick wins, relationship to your “YAML agent” vision |
| [02-security-and-threat-model.md](./02-security-and-threat-model.md) | Threats, existing controls, gaps, recommended automated scans |
| [03-code-smells-and-architecture.md](./03-code-smells-and-architecture.md) | Large modules, duplication, panic/unwrap patterns |
| [04-api-surface-and-developer-experience.md](./04-api-surface-and-developer-experience.md) | Naming confusion (`runEmailWorkflowYaml`, `email_text` in outputs), binding parity |
| [05-redundancy-and-cleanup-candidates.md](./05-redundancy-and-cleanup-candidates.md) | Examples, skills, scripts, and API wrappers that inflate surface area |
| [06-prior-review-continuity.md](./06-prior-review-continuity.md) | What `code-review/08-baseline-truth-matrix.md` says vs. this audit |

## Clarification: “run YAML workflow email”

The repository does **not** contain a literal API named that phrase. What often confuses newcomers is:

- **`runEmailWorkflowYaml` / `run_email_workflow_yaml_*`** — convenience wrappers that inject `{"email_text": "<string>"}` as workflow input (see `crates/simple-agents-workflow/src/yaml_runner/api.rs`).
- **`examples/workflow_email/`** — a large demo area (many YAMLs + Python/Node/Go runners) that uses those patterns.

These are **domain-demo sugar**, not a separate runtime. Consolidating them behind `run_workflow_yaml(..., input)` would shrink code without removing capability.
