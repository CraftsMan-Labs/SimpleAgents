# ADR-001: Canonical IR Format (YAML/JSON)

## Status
Accepted

## Context
We need a language-agnostic format for defining workflows that can be:
- Written by hand or generated programmatically
- Versioned and stored in version control
- Validated statically before execution
- Portable across different runtimes and languages

## Decision
Use **YAML as the primary format** with JSON as an alternative, both mapping to the same Rust types via Serde.

The canonical IR will include:
- `WorkflowGraph`: Top-level container
- `NodeDefinition`: Individual node specifications
- `EdgeDefinition`: Control flow connections
- `NodeType`: Enum of all 15 node types
- JSON Schema for input/output validation

All types implement `Serialize` and `Deserialize` for automatic YAML/JSON conversion.

## Alternatives Considered

### 1. **Pure Code DSL Only**
- **Pros**: Type-safe, IDE autocomplete, refactoring support
- **Cons**: Not portable across languages, harder to visualize, can't be stored in databases
- **Rejected**: Limits language flexibility; we want both code DSL AND declarative format

### 2. **Custom Domain-Specific Language (DSL)**
- **Pros**: Could be optimized for workflow semantics
- **Cons**: Requires parser, tooling, learning curve
- **Rejected**: YAML/JSON already have excellent tooling and are familiar

### 3. **Protocol Buffers**
- **Pros**: Efficient binary format, strong typing, versioning
- **Cons**: Not human-readable, harder to edit by hand, requires compilation
- **Rejected**: Human-readability is important for workflows

### 4. **XML**
- **Pros**: Schema validation (XSD), widely used
- **Cons**: Verbose, less readable than YAML
- **Rejected**: YAML is more concise and modern

## Consequences

### Positive
- **Human-readable**: Easy to write and review workflows
- **Tooling**: Syntax highlighting, validation in editors
- **Portability**: Can be stored in git, databases, or sent over APIs
- **Dual format**: YAML for humans, JSON for APIs (same Rust types)
- **Validation**: JSON Schema for input/output contracts

### Negative
- **No compile-time checking**: Errors only caught at runtime (mitigated by validation step)
- **Indentation-sensitive**: YAML can be tricky with indentation
- **No IDE autocomplete**: Unlike code DSL (mitigated by providing code DSL wrapper)

## Implementation Notes

### Rust Types
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowGraph {
    pub id: GraphId,
    pub version: Version,
    pub nodes: HashMap<NodeId, NodeDefinition>,
    pub edges: Vec<EdgeDefinition>,
    // ...
}
```

### YAML Example
```yaml
id: my-workflow
version: 1.0.0
nodes:
  - id: node1
    node_type:
      llm_call:
        provider: openai
        model: gpt-4
```

### JSON Example
```json
{
  "id": "my-workflow",
  "version": "1.0.0",
  "nodes": [{
    "id": "node1",
    "node_type": {
      "llm_call": {
        "provider": "openai",
        "model": "gpt-4"
      }
    }
  }]
}
```

### Validation
- Validate YAML/JSON against JSON Schema before execution
- Serde will catch type mismatches during deserialization
- Custom validation logic for semantic checks (e.g., all referenced nodes exist)

## Related Decisions
- ADR-002: Execution Engine Design
- ADR-006: Code DSL alongside YAML/JSON
