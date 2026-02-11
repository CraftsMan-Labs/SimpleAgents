# MISSION_DIRECTIVES

Reusable directive template for large codebase investigations and documentation missions.

## 1. Mission Brief

- `Primary objective`: define exact deliverables and target files.
- `Scope`: define in-scope paths and explicitly list out-of-scope paths.
- `Success criteria`: state what “complete” means before work starts.

Example:
- Deliverables: `SCRATCHPAD.md`, `PLAN.md`, `features.md`, `docs/RUST_CORE_SYSTEMS.md`
- Scope: all crates under `crates/`
- Success: every crate documented with compile-time and runtime features

## 2. Non-Negotiable Rules

- Trust `Cargo.toml` and source code over README prose.
- Distinguish:
  - compile-time features
  - runtime/API capabilities
  - interface/binding surfaces
- Do not mark complete until all checkpoints pass.

## 3. Search-and-Map Command Set

Run these as baseline commands:

```bash
pwd && ls -la
rg --files
find crates -maxdepth 2 -name Cargo.toml | sort
cat Cargo.toml
```

Feature extraction:

```bash
rg -n "^\[features\]|cfg\(feature|feature\s*=" crates/*/Cargo.toml crates/*/src/*.rs crates/*/src/*/*.rs
```

Public API surface extraction:

```bash
for d in crates/*; do
  echo "===== $(basename "$d") ====="
  rg -n "^pub mod|^pub use|^pub struct|^pub enum|^pub trait|^impl" "$d/src" "$d/Cargo.toml" 2>/dev/null | sed -n '1,240p'
done
```

Behavior capability scan:

```bash
rg -n "stream|schema|healing|routing|cache|from_env|tool|mode|retry|metrics" \
  crates/simple-agent-type/src \
  crates/simple-agents-core/src \
  crates/simple-agents-providers/src \
  crates/simple-agents-router/src \
  crates/simple-agents-healing/src \
  crates/simple-agents-cache/src \
  crates/simple-agents-cli/src \
  crates/simple-agents-ffi/src \
  crates/simple-agents-napi/src \
  crates/simple-agents-py/src
```

## 4. Thought Process Directive

1. Inventory first, summarize second.
2. Start with manifests for authoritative compile-time features.
3. Validate each finding in source.
4. Build crate-by-crate summaries with a fixed schema:
   - crate name
   - purpose
   - cargo features
   - runtime capabilities
   - extension points
5. Only then write final documentation.

## 5. Checkpoints (Hard Gates)

### Checkpoint A: Inventory Complete
- All crates in `crates/` enumerated.
- Each crate has a manifest read.

### Checkpoint B: Compile-Time Features Complete
- Every `[features]` section captured.
- Every `cfg(feature = "...")` usage mapped.

### Checkpoint C: Runtime Features Complete
- Public modules and primary APIs documented per crate.
- Key modes/strategies/commands surfaced.

### Checkpoint D: Documentation Complete
- Root summary file updated (e.g., `features.md`).
- Developer docs file updated (e.g., in `docs/`).
- Navigation links added if docs site exists.

### Checkpoint E: Consistency Pass
- Naming consistency across all docs.
- No crate omitted.
- No undocumented feature gate.

## 6. Definition of Done

Mission is complete only if:
- All in-scope crates are covered.
- Compile-time and runtime features are both documented.
- Outputs are concise, clear, and linked.
- Verification commands pass.

## 7. Verification Commands

Crate count sanity:

```bash
find crates -maxdepth 2 -name Cargo.toml | wc -l
```

Feature-gate sanity:

```bash
rg -n "^\[features\]" crates/*/Cargo.toml
rg -n "cfg\(feature" crates
```

Crate-name sanity:

```bash
rg -n "^name\s*=" crates/*/Cargo.toml
```

Output diff sanity:

```bash
git diff -- SCRATCHPAD.md PLAN.md features.md docs/RUST_CORE_SYSTEMS.md docs/.vitepress/config.mjs docs/README.md
```

## 8. Gap Prevention Checklist

- [ ] Every crate appears in final summary.
- [ ] Every Cargo feature flag appears in final summary.
- [ ] Every major runtime subsystem appears in final summary.
- [ ] Every new doc is discoverable from docs navigation.
- [ ] Preliminary notes, plan, and final docs are mutually consistent.

## 9. Reusable Output Template

Use this structure for final reporting:

1. `What was delivered`
2. `Files created/updated`
3. `Feature inventory summary`
4. `Validation and coverage checks run`
5. `Residual risk or known gaps (if any)`

