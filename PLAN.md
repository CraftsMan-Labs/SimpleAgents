# PLAN

Date: 2026-02-11
Goal: Read Rust core systems in `crates/`, document all crate features in `features.md` (project root), and add concise developer documentation in `docs/`.

## Task List

1. Inventory and baseline
- Confirm workspace members and crate list.
- Identify source files and existing crate READMEs.

2. Feature extraction (Cargo-level)
- Read each crate `Cargo.toml`.
- Capture `[features]`, default features, and optional deps tied to features.
- Record crates with no explicit Cargo features.

3. Capability extraction (code-level)
- Review crate `src/` public modules and key APIs.
- Identify major capabilities exposed by each crate (routing modes, provider support, healing modes, bindings, etc.).

4. Preliminary-to-final synthesis
- Normalize findings into a consistent schema:
  - Crate
  - Purpose
  - Cargo features
  - Runtime capabilities
  - Key modules/APIs

5. Author root feature reference
- Create/update `features.md` in project root with complete crate-by-crate feature inventory.

6. Author developer docs in `docs/`
- Create a dedicated Rust-core systems developer doc (concise, navigable).
- Include architecture map, crate responsibilities, key extension points, and where features are implemented.

7. Sanity pass
- Verify new docs reflect actual code and manifest names.
- Check for naming consistency and clear language.

8. Report completion
- Summarize what was documented and where.

## Simple execution strategy
- Start from manifests (`Cargo.toml`) for authoritative compile-time features.
- Validate behavior by scanning public APIs in `src/lib.rs` + key modules.
- Use existing crate READMEs only as supportive context, not primary source of truth.
