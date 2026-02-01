# Canonical IR and DSL

## Recommended Repo
- https://github.com/argoproj/argo-workflows

## Why This Repo
- YAML-first workflow definitions with DAG support map well to a canonical IR.
- Mature tooling and ecosystem for workflow authoring patterns.

## Pros
- Proven YAML schema for DAG workflows and parameters.
- Strong ecosystem patterns for workflow templating and reuse.

## Cons
- Kubernetes-centric design assumptions.
- YAML schema complexity can become heavy for pure library use.

## What We Want To Build From This
- A YAML/JSON canonical IR structure inspired by Argo’s DAG and template patterns.
- Clear separation between workflow metadata, graph structure, and node specs.
- Authoring ergonomics for non-Rust users while still compiling to the same IR.

## Why
- It gives a battle-tested shape for workflow definitions and a user-friendly DSL style.

## Sources
- https://github.com/meirwah/awesome-workflow-engines
- https://www.bytebase.com/blog/top-open-source-workflow-orchestration-tools/

## Notes
- Use Argo as reference for DSL ergonomics and schema structure.
