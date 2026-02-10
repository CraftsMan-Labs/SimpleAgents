# Core JS/TS Patterns

## 1. Type Boundaries

- Prefer strict TS config for production libraries/services.
- Validate untrusted data at boundaries (`zod`, `valibot`, `io-ts`, etc.).
- Avoid leaking `any`; isolate unavoidable `unknown` and narrow quickly.

## 2. API and Module Design

- Keep modules cohesive and single-purpose.
- Favor pure functions in core logic.
- Use interfaces/types to describe contracts; avoid inheritance-heavy trees.

## 3. Error Strategy

- Use typed/domain error classes or discriminated unions.
- Preserve cause/context when rethrowing.
- Convert errors at boundaries (HTTP, queue, UI).

## 4. Performance

- Measure before optimization.
- Reduce allocations/churn in hot loops.
- Use streaming/iterative processing for large payloads.
- Watch serialization/deserialization costs.

## 5. Tooling Baseline

- Format: `prettier`.
- Lint: `eslint` with strict rules.
- Typecheck: `tsc --noEmit`.
- Tests: unit + integration (Jest/Vitest/Playwright as relevant).

## 6. Anti-Patterns

- Implicit `any` spread.
- Runtime assumptions without validation.
- Catch-all errors without typed handling.
- Shared mutable singleton state across modules.
