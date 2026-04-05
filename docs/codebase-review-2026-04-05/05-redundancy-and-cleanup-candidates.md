# Redundancy and cleanup candidates

**Important:** Items here are **candidates for review**, not automatic deletions. Some duplication exists for **backward compatibility** or **language-specific tutorials**.

## A. Email-specific `run_*` wrappers (Rust `yaml_runner/api.rs` + re-exports)

**Observation:** Pairs of functions differ only by building `{"email_text": ...}` vs accepting arbitrary `Value` input.

**Cleanup angle:** Keep one implementation; express email as a one-line caller example or a deprecated thin wrapper.

**Bindings affected:** Python (`run_email_workflow_yaml*`), NAPI, FFI (`sa_run_email_workflow_yaml`), Go examples referencing `RunEmailWorkflowYAML`.

## B. Overlapping example runners (`examples/workflow_email/`)

**Observation:** Many entry scripts overlap in purpose (run one YAML, run all YAMLs, streaming, events).

**Cleanup angle:** Consolidate under:

- `examples/workflow_email/python/run.py` (flags)
- `examples/workflow_email/node/run.mjs` (flags)
- Keep `README.md` as the map; delete or merge redundant scripts after updating doc links.

## C. Triplicate “SimpleAgentsBuilder” skill trees

**Paths:**

- `skills/simpleagents-builder/`
- `.agents/skills/simpleagentsbuilder/`
- `.opencode/skills/SimpleAgentsBuilder/`

**Observation:** A quick `diff -rq` shows mostly aligned content with minor file presence differences (e.g. one tree missing a duplicate example file). This invites **documentation drift**.

**Cleanup angle:** Single source of truth with copy script in CI, or symlink policy, or one canonical folder referenced by others.

## D. Large inline test suites inside production modules

**Observation:** `yaml_runner.rs` and `runtime.rs` contain very large `#[cfg(test)]` regions.

**Cleanup angle:** Move to `tests/*.rs` integration tests or `#[cfg(test)] mod tests { mod foo; }` subfiles to shrink the “production” line count readers see. No functional change.

## E. `target/package/` artifacts in workspace (if present locally)

**Observation:** Line-count scans may pick up `target/package/...` duplicates of crates. These are **build artifacts**, not source of truth.

**Cleanup angle:** Ensure `.gitignore` keeps them out of version control (they usually are); exclude from manual review.

## F. `code-review/` historical reports

**Observation:** Only `code-review/08-baseline-truth-matrix.md` remains; it references `00-queen-consolidated-report.md`, `02-security-analysis.md`, etc., which are **not** in the tree.

**Cleanup angle:** Restore archived reports or rewrite the baseline matrix to link to **live** docs (e.g. this folder + `docs/WORKFLOW_SECURITY.md`) so newcomers are not sent to dead paths.

## What is *not* useless

- **`examples/workflow_email/*.yaml`** — Valuable as **templates** for branching, tools, subgraphs; the issue is **narrative clutter**, not the YAML itself.
- **Streaming / events APIs** — Distinct capabilities; the smell is **naming duplication** with the non-streaming paths, not the features.
