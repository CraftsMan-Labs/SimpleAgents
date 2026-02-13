# Core Python Patterns

## 1. Typing Strategy

- Type public APIs and boundary layers first.
- Use `Protocol` for behavior contracts; avoid deep inheritance where composition works.
- Prefer precise models (`TypedDict`, `dataclass`, pydantic models where appropriate).

## 2. Data Modeling

- Use immutable/value-style objects for domain invariants when possible.
- Keep constructors validated; avoid partially initialized objects.

## 3. Error Strategy

- Raise domain-specific exceptions near domain logic.
- Map internal exceptions to boundary-safe errors (HTTP/CLI/queue handlers).
- Do not use broad `except Exception` without strict re-raise/logging rationale.

## 4. Architecture Boundaries

- Keep pure business logic separated from I/O adapters.
- Keep functions short and side-effect scope explicit.
- Inject dependencies for testability.

## 5. Performance Discipline

- Measure first (`cProfile`, `py-spy`, tracing).
- Reduce allocations in hot loops.
- Use iterators/generators for streaming workflows.
- Vectorize numeric paths or move CPU hotspots to compiled paths when needed.

## 6. Tooling Baseline

- Format: `ruff format` or `black`.
- Lint: `ruff check`.
- Type check: `pyright` or `mypy`.
- Tests: `pytest` with unit + integration coverage.

## 7. Anti-Patterns

- Dynamic typing everywhere in large codebases.
- God modules with mixed concerns.
- Silent exception suppression.
- Mutable default args.
