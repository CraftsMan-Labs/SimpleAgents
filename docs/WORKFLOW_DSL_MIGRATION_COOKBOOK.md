# Workflow DSL Migration Cookbook

Use this cookbook when converting workflow definitions between YAML and code-first canonical IR.

## Migration Rules

1. Keep node ids stable. Renaming ids changes graph wiring and replay semantics.
2. Preserve edge fields by node type (`next`, `on_true`, `on_false`, `body`, `branches`, `sources`).
3. Keep `version: v0` unless an explicit IR version migration is planned.
4. Validate that every edge target references a real node id.
5. Verify merge nodes keep the same `sources` list after migration.

## YAML -> Canonical IR (Rust)

Start from YAML:

```yaml
name: map-reduce-review
version: v0
nodes:
  - id: start
    type: start
    next: map_docs
  - id: map_docs
    type: map
    tool: summarize_doc
    items_path: $.state.docs
    next: reduce_docs
  - id: reduce_docs
    type: reduce
    source: map_docs
    operation: count
    next: end
  - id: end
    type: end
```

Equivalent canonical IR in Rust:

```rust
use simple_agents_workflow::ir::{Node, NodeKind, ReduceOperation, WorkflowDefinition};

let workflow = WorkflowDefinition {
    version: "v0".to_string(),
    name: "map-reduce-review".to_string(),
    nodes: vec![
        Node {
            id: "start".to_string(),
            kind: NodeKind::Start {
                next: "map_docs".to_string(),
            },
        },
        Node {
            id: "map_docs".to_string(),
            kind: NodeKind::Map {
                tool: "summarize_doc".to_string(),
                items_path: "$.state.docs".to_string(),
                next: "reduce_docs".to_string(),
                max_in_flight: Some(4),
            },
        },
        Node {
            id: "reduce_docs".to_string(),
            kind: NodeKind::Reduce {
                source: "map_docs".to_string(),
                operation: ReduceOperation::Count,
                next: "end".to_string(),
            },
        },
        Node {
            id: "end".to_string(),
            kind: NodeKind::End,
        },
    ],
};
```

## Code -> YAML Guidance

When exporting code-authored IR to YAML:

- Emit the same node ids in a deterministic order.
- Emit only valid fields for each node type.
- Convert enums directly to snake_case string values (`quorum`, `count`, `sum`).
- Keep optional edges (`next`) omitted only when `None` in canonical IR.

## Cross-Language Parity Check

Golden fixture and parity tests:

- Fixture: `parity-fixtures/workflow_dsl_ir_golden.json`
- Rust: `cargo test -p simple-agents-workflow --test workflow_dsl_ir_fixtures`
- Python: `uv run --directory crates/simple-agents-py --with "pytest>=8.0" pytest tests/test_contract_fixtures.py`
- Node: `npm --prefix crates/simple-agents-napi run test:contract`
- Go: `go test ./... -run 'TestGoBindingsFollowSharedContractFixture|TestWorkflowDSLFixturePreservesCanonicalIRWires' -count=1`
