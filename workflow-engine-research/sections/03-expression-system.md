# Expression System

## Recommended Repo
- https://github.com/google/cel-go

## Why This Repo
- CEL is a widely used, portable expression language suitable for policy and routing.
- Reference implementation clarifies evaluation behavior and type system.

## Pros
- Portable, well-defined expression language with type checking.
- Good fit for policy, routing, and branch selection.

## Cons
- Native implementation is Go; Rust integration requires embedding or porting.
- Limited for complex custom scripting without extensions.

## What We Want To Build From This
- CEL-compatible expression syntax for conditions and routing.
- Pluggable evaluator layer to support CEL plus custom evaluators.
- Test harness patterns for expression validation.

## Why
- CEL provides a stable, portable base for multi-language authoring.

## Sources
- https://github.com/google/cel-spec
- https://cel.dev/

## Notes
- Use CEL as the baseline expression language; map into a Rust evaluator or embed via bindings.
