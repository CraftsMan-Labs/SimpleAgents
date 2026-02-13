# Domain Constraints: CLI Applications

Apply these constraints for `clap`-based tools and command-line workflows.

## Core Constraints

- UX should be deterministic and script-friendly.
- Exit codes must be meaningful and stable.
- Error output should be concise and actionable.
- Startup time and binary size may matter more than maximal abstraction.

## Recommended Patterns

- Parse args with strong types and defaults.
- Keep command handlers narrow and testable.
- Use typed errors internally; map to user-facing messages at boundary.
- Prefer synchronous code unless async is justified by I/O concurrency needs.

## Common Mistakes

- Panicking on invalid user input.
- Emitting inconsistent output formats across commands.
- Overusing async in simple linear workflows.
- Ignoring non-zero exit code semantics.
