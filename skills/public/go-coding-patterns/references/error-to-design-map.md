# Error-to-Design Map (Go)

## Concurrency Failures

- Goroutine leaks -> missing ownership/shutdown design; introduce context + errgroup.
- Deadlocks -> circular waits or unbounded blocking ops; redesign flow and buffering.
- Data races -> shared mutable state without clear ownership; synchronize or redesign ownership.

## Error-Handling Smells

- Repeated ignored errors (`_ = ...`) -> missing failure model; force explicit handling.
- String comparison on errors -> weak taxonomy; use wrapped/sentinel/custom types.

## API Design Smells

- Large interfaces for testing convenience -> wrong abstraction direction; define minimal consumer interfaces.
- Widespread `interface{}`/`any` in business logic -> weak domain model; add typed boundaries.
