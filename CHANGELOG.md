# Changelog

All notable changes to this project are documented in this file.

The format follows Keep a Changelog, and versions are tracked in repository tags.

## [Unreleased]

### Added
- _No entries yet._

### Changed
- _No entries yet._

### Fixed
- _No entries yet._

## [0.3.7] - 2026-04-07

### Removed

- Go/FFI bindings (breaking change).

### Changed

- WASM bindings require the Rust WASM backend; the JavaScript fallback was removed.

## [0.3.6] - 2026-04-07

### Added

- Default workflow stream handlers and typed workflow event types in bindings.
- Template interpolation for custom worker JSON payloads in the workflow runtime.

### Changed

- Pinned WASM-related dependency updates.

## [0.3.5] - 2026-04-06

### Added

- PEP 561 type stubs shipped inside the `simple-agents-py` package.

### Changed

- Synced the WASM Rust lockfile for the `simple-agents-wasm` crate.

### Fixed

- Tag release workflow matches Keep a Changelog section headings when building release notes.

## [0.3.4] - 2026-04-06

### Added
- Multimodal `ContentPart` support for audio and video, plus MIME-related helpers in `simple-agent-type`.
- Multimodal message content surfaced through Python, NAPI, WASM, and Go bindings.
- Criterion benchmark harness for the workflow runtime (`runtime_benchmarks`), with a concurrency guard for bench runs.

### Changed
- _No entries yet._

### Fixed
- _No entries yet._

## [0.2.33] - 2026-04-03

### Added
- Bootstrapped release changelog tracking and CI automation.

### Changed
- Tag-triggered GitHub releases now use the matching `CHANGELOG.md` version section as release notes.

### Fixed
- _No entries yet._
