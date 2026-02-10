# Core Go Patterns

## 1. Package and API Design

- Keep packages cohesive with clear responsibility.
- Favor small interfaces defined by consumers.
- Return concrete types from constructors when possible.

## 2. Error Strategy

- Return errors, do not panic for expected failures.
- Wrap errors with context (`fmt.Errorf("...: %w", err)`).
- Use `errors.Is` / `errors.As` for branching.
- Keep sentinel errors limited and well-documented.

## 3. Data and State

- Prefer explicit structs over map-heavy loosely typed state.
- Keep invariants close to constructors/methods.
- Avoid exposing mutable internal fields.

## 4. Performance

- Measure first (`pprof`, benchmarks).
- Minimize allocations in hot loops.
- Reuse buffers where appropriate.
- Watch lock contention and channel bottlenecks.

## 5. Tooling Baseline

- `gofmt` / `go fmt`
- `go vet`
- `golangci-lint` (where adopted)
- `go test ./...`
- `go test -race ./...` for concurrency-sensitive changes

## 6. Anti-Patterns

- Ignoring returned errors.
- Context not propagated across call stack.
- Unbounded goroutine spawning under load.
- Overly broad interfaces (“interface pollution”).
