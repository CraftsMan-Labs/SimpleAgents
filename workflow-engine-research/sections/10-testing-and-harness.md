# Testing and Harness

## Recommended Repo
- https://github.com/mitsuhiko/insta

## Why This Repo
- Snapshot testing framework suitable for golden traces and deterministic comparisons.
- Simple integration into Rust test suites.

## Pros
- Very fast to add golden trace assertions.
- Great for checking large structured outputs.

## Cons
- Snapshot drift can hide real regressions if not reviewed carefully.
- Needs clear update policy for snapshots.

## What We Want To Build From This
- Golden trace capture and comparison for workflows.
- Deterministic replay verification outputs.

## Why
- We need a fast test harness for complex workflow outputs.

## Sources
- https://insta.rs/
- https://blog.logrocket.com/using-insta-rust-snapshot-testing/

## Notes
- Use snapshots for golden trace verification and replay testing outputs.
