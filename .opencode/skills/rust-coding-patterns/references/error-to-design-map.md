# Error-to-Design Map

Map common compiler/runtime failures to design questions before applying tactical fixes.

## Table of Contents

1. Ownership Errors
2. Borrowing and Mutability Errors
3. Trait and Type Errors
4. Concurrency and Async Errors
5. Error-Handling Smells

## 1. Ownership Errors

### `E0382` use of moved value

Do not default to cloning.

Ask:
- Should ownership stay with caller?
- Should callee borrow instead?
- Is shared ownership actually needed (`Rc`/`Arc`)?

### `E0515` cannot return reference to local value

Ask:
- Should return type be owned?
- Is the source data lifetime correct for borrowing?

### `E0507` cannot move out of borrowed content

Ask:
- Is consumption attempted through `&T`?
- Should API take ownership instead?

## 2. Borrowing and Mutability Errors

### `E0499` multiple mutable borrows

Ask:
- Can mutation be sequenced into smaller scopes?
- Should data be split into independent sub-structs?

### `E0502` cannot borrow mutable because immutable borrow exists

Ask:
- Can immutable borrow end sooner?
- Can derived value be copied/cloned once and borrow dropped?

### Runtime `RefCell` panic (already borrowed)

Ask:
- Is interior mutability needed here?
- Should control flow enforce borrowing at compile-time with `&mut`?

## 3. Trait and Type Errors

### `E0277` trait bound not satisfied

Ask:
- Is abstraction boundary correct?
- Should this be static dispatch, dynamic dispatch, or concrete type?

### `E0038` trait not object safe

Ask:
- Is `dyn Trait` required?
- Can generic static dispatch replace object safety requirements?

### `E0308` mismatched types

Ask:
- Is conversion explicit and intentional?
- Is newtype or enum needed to represent domain distinctions?

## 4. Concurrency and Async Errors

### `future is not Send` / `E0277` with `Send`

Ask:
- Does task need to cross threads (`tokio::spawn`)?
- Can non-Send values be dropped before `.await`?
- Should work run in local task set instead?

### Lock guard held across `.await`

Symptom:
- deadlocks, stalled throughput, or borrow checker issues around guards.

Ask:
- Can lock scope be reduced to pre/post await sections?
- Can data be copied out before awaiting?

### Blocking in async context

Ask:
- Is this CPU-bound/blocking I/O path that belongs in `spawn_blocking`?
- Can async-native client/library replace blocking API?

## 5. Error-Handling Smells

### Frequent `unwrap()`/`expect()`

Ask:
- Is failure truly impossible?
- If not, who should handle failure and with what context?

### Lossy string errors

Ask:
- Does caller need typed variants for recovery?
- Should error enum be introduced at boundary?

### Retry everywhere

Ask:
- Which errors are transient?
- What are attempt limits, backoff policy, and timeout budgets?
