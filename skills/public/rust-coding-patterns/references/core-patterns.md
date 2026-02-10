# Core Rust Patterns

## Table of Contents

1. Ownership and Borrowing Decisions
2. Mutability and Interior Mutability
3. Type-Driven Design and Invariants
4. Error Handling Strategy
5. Concurrency Model Selection
6. Dispatch and Abstraction Choices
7. Performance and Allocation Discipline
8. Ecosystem Integration and Tooling
9. Anti-Pattern Remediation

## 1. Ownership and Borrowing Decisions

Core question: who owns data and for how long?

Default API rules:
- Inputs: borrow (`&str`, `&[T]`, `&T`) unless ownership transfer is required.
- Outputs: return owned values when data must outlive local scope.
- Small `Copy` values: pass by value.

Smart pointer guide:
- `Box<T>`: single owner + heap allocation.
- `Rc<T>`: shared ownership, single-threaded.
- `Arc<T>`: shared ownership, multi-threaded.
- `Weak<T>`: break cycles in graph-like relationships.

## 2. Mutability and Interior Mutability

Choose the least powerful mutability primitive that works:
- `&mut T`: preferred for exclusive mutation.
- `Cell<T>`: interior mutability for small `Copy` data.
- `RefCell<T>`: interior mutability with runtime borrow checks.
- `Mutex<T>` / `RwLock<T>`: thread-safe interior mutability.
- Atomics: primitive lock-free counters/flags.

Rule: keep mutation scopes short and explicit.

## 3. Type-Driven Design and Invariants

Encode constraints in types:
- Newtype for semantic meaning (`UserId`, `Email`, `Currency`).
- Private fields + validated constructors for always-valid state.
- Typestate for compile-time transition constraints.
- Builder when valid construction requires multiple steps.

Use runtime checks only when validation depends on external state.

## 4. Error Handling Strategy

Failure semantics:
- Recoverable failure: `Result<T, E>`.
- Expected absence: `Option<T>`.
- Invariant violation/bug: `panic!`/`assert!`.

Crate-level guidance:
- Library/public API: typed error enum with `thiserror`.
- Application/service boundary: `anyhow` + context.

Hard rules:
- No bare `unwrap()`/`expect()` in production paths without explicit invariant rationale.
- Include context near boundaries (network, disk, DB, serialization).

## 5. Concurrency Model Selection

Decision order:
1. Is work CPU-bound? Use threads/rayon.
2. Is work I/O-bound? Use Tokio async.
3. Is work mixed? Use async orchestration and isolate blocking with `spawn_blocking`.

Sharing defaults:
- No sharing needed: channels.
- Shared immutable data: `Arc<T>`.
- Shared mutable data: `Arc<Mutex<T>>` or `Arc<RwLock<T>>`.

## 6. Dispatch and Abstraction Choices

- Prefer static dispatch (generics/`impl Trait`) for hot paths.
- Use dynamic dispatch (`dyn Trait`) for runtime polymorphism or heterogeneous collections.
- Use enums for closed sets of variants.
- Minimize trait bounds and keep them where needed.

## 7. Performance and Allocation Discipline

Optimization priority:
1. Algorithmic complexity.
2. Data structure fit to access patterns.
3. Allocation elimination and reuse.
4. Cache locality.
5. Parallelism/SIMD.

Practical defaults:
- Benchmark only in `--release`.
- Pre-allocate with `with_capacity` when sizes are known.
- Avoid hidden clones in loops.
- Avoid collect-then-reiterate when chained iterators work.

## 8. Ecosystem Integration and Tooling

Quality baseline:
- `cargo fmt`
- `cargo clippy --all-targets --all-features -- -D warnings`
- Unit + integration tests

Dependency policy:
- Prefer maintained crates and minimal feature sets.
- Avoid broad dependency additions for narrow use cases.
- Keep versions explicit and workspace-aligned.

## 9. Anti-Pattern Remediation

Common smell -> root cause -> fix:
- Many `.clone()` calls -> unclear ownership -> redesign ownership flow.
- Many `.unwrap()` in app logic -> missing error strategy -> propagate `Result` and add context.
- `Arc<Mutex<T>>` everywhere -> over-shared state -> push toward channels and ownership partitioning.
- Lifetimes repeatedly fight code shape -> wrong data flow -> redesign model boundaries.
