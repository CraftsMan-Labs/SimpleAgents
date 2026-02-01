# ADR-006: Code DSL Alongside YAML/JSON

## Status
Accepted

## Context
While YAML/JSON provides a portable workflow definition format (ADR-001), developers often prefer type-safe, IDE-friendly code for authoring workflows. We need to decide whether to provide language-specific DSLs and how they relate to the canonical IR.

Requirements:
- **Type safety**: Catch errors at compile time where possible
- **IDE support**: Autocomplete, go-to-definition, inline docs
- **Discoverability**: Make node types and options easy to find
- **Multi-language**: Support Rust, Python, TypeScript, Go
- **Portability**: Code DSL must compile to same canonical IR as YAML
- **Versioning**: DSL changes shouldn't break IR compatibility

## Decision
Provide **builder-pattern code DSLs** for each supported language that compile to the canonical YAML/JSON IR.

Architecture:
- **Rust**: Native builder (compile to IR structs directly)
- **Python**: PyO3 bindings with builder classes
- **TypeScript**: NAPI bindings with builder classes
- **Go**: cgo bindings with builder structs
- **Compilation**: All DSLs emit identical YAML/JSON IR
- **Validation**: Compile-time + runtime validation

## Alternatives Considered

### 1. **YAML/JSON Only (No Code DSL)**
- **Pros**: Single format, no duplication, simple
- **Cons**:
  - No type safety
  - Poor IDE support
  - Higher error rate
  - Less discoverable
- **Rejected**: Developer experience too poor

### 2. **Code DSL Only (No YAML/JSON)**
- **Pros**: Type-safe, excellent IDE support
- **Cons**:
  - Not portable across languages
  - Can't store in databases or send over APIs
  - Hard to version and diff
- **Rejected**: Portability is critical

### 3. **Generate Code from YAML Schema**
- **Pros**: Single source of truth, automated
- **Cons**:
  - Generated code is often awkward
  - Poor ergonomics
  - Hard to customize
- **Rejected**: Builder pattern provides better UX

### 4. **GraphQL-Style SDL**
- **Pros**: Declarative, type-safe schema
- **Cons**:
  - Another syntax to learn
  - Limited to GraphQL semantics
  - Requires schema compiler
- **Rejected**: YAML is more familiar

### 5. **Embedded DSL in Host Language**
- **Pros**: Leverage host language features (macros, metaprogramming)
- **Cons**:
  - Different DSL per language
  - Hard to maintain parity
  - Can't compile to IR easily
- **Rejected**: Want uniform IR across languages

## Consequences

### Positive
- **Type safety**: Compile-time checking in statically-typed languages
- **IDE support**: Autocomplete, type hints, documentation
- **Discoverability**: Browse node types and options in IDE
- **Flexibility**: Choose YAML or code based on use case
- **Portability**: Code compiles to same IR as hand-written YAML
- **Refactoring**: Code can be refactored with IDE tools

### Negative
- **Maintenance burden**: Keep DSL in sync across 4 languages
- **Two ways to do things**: Some users confused about YAML vs code
- **API surface**: More API to document and support
- **Version skew**: DSL might lag behind IR changes

## Implementation Notes

### Rust DSL (Native)

```rust
use simple_agents_workflow::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    let workflow = WorkflowGraph::builder()
        .id("sentiment-analysis")
        .version("1.0.0")
        .node(
            Node::llm_call("analyze")
                .provider(Provider::OpenAI)
                .model("gpt-4")
                .prompt("Analyze sentiment: {{ input.text }}")
                .output_schema(json_schema! {
                    "type": "object",
                    "properties": {
                        "sentiment": {"type": "string"},
                        "confidence": {"type": "number"}
                    }
                })
        )
        .node(
            Node::switch("route")
                .input("$.nodes.analyze.output")
                .branch(
                    Branch::when("sentiment == 'positive'")
                        .target("celebrate")
                )
                .branch(
                    Branch::when("sentiment == 'negative'")
                        .target("investigate")
                )
                .default_branch("log")
        )
        .node(
            Node::transform("celebrate")
                .expression(r#"{"action": "send_thanks", "sentiment": $.sentiment}"#)
        )
        .node(
            Node::custom_worker("investigate")
                .language(Language::Python)
                .handler("InvestigateIssue")
                .input("$.nodes.analyze.output")
        )
        .edge(Edge::from("analyze").to("route"))
        .edge(Edge::from("route").to("celebrate").condition("sentiment == 'positive'"))
        .edge(Edge::from("route").to("investigate").condition("sentiment == 'negative'"))
        .build()?;

    // Execute
    let engine = WorkflowEngine::new()?;
    let result = engine.execute(&workflow, json!({"text": "Great product!"})).await?;
    println!("Result: {}", result);

    // Or serialize to YAML
    let yaml = workflow.to_yaml()?;
    std::fs::write("workflow.yaml", yaml)?;

    Ok(())
}
```

### Python DSL

```python
from simple_agents.workflow import (
    WorkflowGraph,
    Node,
    Edge,
    Provider,
    Language,
)

# Build workflow
workflow = (
    WorkflowGraph()
    .id("sentiment-analysis")
    .version("1.0.0")
    .node(
        Node.llm_call("analyze")
        .provider(Provider.OPENAI)
        .model("gpt-4")
        .prompt("Analyze sentiment: {{ input.text }}")
        .output_schema({
            "type": "object",
            "properties": {
                "sentiment": {"type": "string"},
                "confidence": {"type": "number"},
            },
        })
    )
    .node(
        Node.switch("route")
        .input("$.nodes.analyze.output")
        .branch_when("sentiment == 'positive'", target="celebrate")
        .branch_when("sentiment == 'negative'", target="investigate")
        .default_branch("log")
    )
    .node(
        Node.transform("celebrate")
        .expression('{"action": "send_thanks", "sentiment": $.sentiment}')
    )
    .node(
        Node.custom_worker("investigate")
        .language(Language.PYTHON)
        .handler("InvestigateIssue")
        .input("$.nodes.analyze.output")
    )
    .edge(Edge.from_("analyze").to("route"))
    .build()
)

# Execute
from simple_agents.workflow import WorkflowEngine

engine = WorkflowEngine()
result = await engine.execute(workflow, {"text": "Great product!"})
print(f"Result: {result}")

# Or serialize to YAML
yaml_str = workflow.to_yaml()
with open("workflow.yaml", "w") as f:
    f.write(yaml_str)
```

### TypeScript DSL

```typescript
import {
  WorkflowGraph,
  Node,
  Edge,
  Provider,
  Language,
  WorkflowEngine,
} from '@simple-agents/workflow';

// Build workflow (with TypeScript type safety)
const workflow = new WorkflowGraph()
  .id('sentiment-analysis')
  .version('1.0.0')
  .node(
    Node.llmCall('analyze')
      .provider(Provider.OpenAI)
      .model('gpt-4')
      .prompt('Analyze sentiment: {{ input.text }}')
      .outputSchema({
        type: 'object',
        properties: {
          sentiment: { type: 'string' },
          confidence: { type: 'number' },
        },
      })
  )
  .node(
    Node.switch('route')
      .input('$.nodes.analyze.output')
      .branch({
        condition: "sentiment == 'positive'",
        target: 'celebrate',
      })
      .branch({
        condition: "sentiment == 'negative'",
        target: 'investigate',
      })
      .defaultBranch('log')
  )
  .node(
    Node.transform('celebrate')
      .expression('{"action": "send_thanks", "sentiment": $.sentiment}')
  )
  .node(
    Node.customWorker('investigate')
      .language(Language.Python)
      .handler('InvestigateIssue')
      .input('$.nodes.analyze.output')
  )
  .edge(Edge.from('analyze').to('route'))
  .build();

// Execute
const engine = new WorkflowEngine();
const result = await engine.execute(workflow, { text: 'Great product!' });
console.log('Result:', result);

// Or serialize to YAML
const yaml = workflow.toYaml();
await fs.writeFile('workflow.yaml', yaml);
```

### Go DSL

```go
package main

import (
    "context"
    "fmt"
    workflow "github.com/simple-agents/workflow-go"
)

func main() {
    // Build workflow
    wf := workflow.NewGraph().
        ID("sentiment-analysis").
        Version("1.0.0").
        Node(
            workflow.LLMCall("analyze").
                Provider(workflow.ProviderOpenAI).
                Model("gpt-4").
                Prompt("Analyze sentiment: {{ input.text }}").
                OutputSchema(map[string]interface{}{
                    "type": "object",
                    "properties": map[string]interface{}{
                        "sentiment": map[string]string{"type": "string"},
                        "confidence": map[string]string{"type": "number"},
                    },
                }),
        ).
        Node(
            workflow.Switch("route").
                Input("$.nodes.analyze.output").
                Branch("sentiment == 'positive'", "celebrate").
                Branch("sentiment == 'negative'", "investigate").
                DefaultBranch("log"),
        ).
        Node(
            workflow.Transform("celebrate").
                Expression(`{"action": "send_thanks", "sentiment": $.sentiment}`),
        ).
        Node(
            workflow.CustomWorker("investigate").
                Language(workflow.LanguagePython).
                Handler("InvestigateIssue").
                Input("$.nodes.analyze.output"),
        ).
        Edge(workflow.NewEdge("analyze", "route")).
        Build()

    if err := wf.Validate(); err != nil {
        panic(err)
    }

    // Execute
    engine := workflow.NewEngine()
    ctx := context.Background()
    result, err := engine.Execute(ctx, wf, map[string]interface{}{
        "text": "Great product!",
    })
    if err != nil {
        panic(err)
    }
    fmt.Printf("Result: %v\n", result)

    // Or serialize to YAML
    yaml, err := wf.ToYAML()
    if err != nil {
        panic(err)
    }
    os.WriteFile("workflow.yaml", []byte(yaml), 0644)
}
```

### Compilation to Canonical IR

All DSLs compile to the same canonical IR:

```yaml
id: sentiment-analysis
version: 1.0.0
entry_node: analyze

nodes:
  - id: analyze
    node_type:
      llm_call:
        provider: openai
        model: gpt-4
        prompt: "Analyze sentiment: {{ input.text }}"
    output_schema:
      type: object
      properties:
        sentiment:
          type: string
        confidence:
          type: number

  - id: route
    node_type:
      switch:
        input: "$.nodes.analyze.output"
        branches:
          - condition: "sentiment == 'positive'"
            target: celebrate
          - condition: "sentiment == 'negative'"
            target: investigate
        default: log

  - id: celebrate
    node_type:
      transform:
        expression: '{"action": "send_thanks", "sentiment": $.sentiment}'

  - id: investigate
    node_type:
      custom_worker:
        language: python
        handler: InvestigateIssue
    input: "$.nodes.analyze.output"

edges:
  - from: analyze
    to: route
  - from: route
    to: celebrate
    condition: "sentiment == 'positive'"
  - from: route
    to: investigate
    condition: "sentiment == 'negative'"
```

### Builder Pattern Implementation

```rust
pub struct WorkflowGraphBuilder {
    id: Option<String>,
    version: Option<Version>,
    nodes: Vec<NodeDefinition>,
    edges: Vec<EdgeDefinition>,
    metadata: HashMap<String, Value>,
}

impl WorkflowGraphBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    pub fn version(mut self, version: impl Into<Version>) -> Self {
        self.version = Some(version.into());
        self
    }

    pub fn node(mut self, node: NodeDefinition) -> Self {
        self.nodes.push(node);
        self
    }

    pub fn edge(mut self, edge: EdgeDefinition) -> Self {
        self.edges.push(edge);
        self
    }

    pub fn build(self) -> Result<WorkflowGraph> {
        let graph = WorkflowGraph {
            id: self.id.ok_or(Error::MissingField("id"))?,
            version: self.version.ok_or(Error::MissingField("version"))?,
            nodes: self.nodes.into_iter()
                .map(|n| (n.id.clone(), n))
                .collect(),
            edges: self.edges,
            metadata: self.metadata,
            entry_node: self.determine_entry_node()?,
        };

        // Validate graph
        graph.validate()?;

        Ok(graph)
    }
}
```

### Type-Safe Node Builders

```rust
pub struct LLMCallNodeBuilder {
    id: NodeId,
    provider: Option<Provider>,
    model: Option<String>,
    prompt: Option<String>,
    output_schema: Option<JsonSchema>,
    // ...
}

impl LLMCallNodeBuilder {
    pub fn provider(mut self, provider: Provider) -> Self {
        self.provider = Some(provider);
        self
    }

    pub fn model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    pub fn prompt(mut self, prompt: impl Into<String>) -> Self {
        self.prompt = Some(prompt.into());
        self
    }

    pub fn output_schema(mut self, schema: JsonSchema) -> Self {
        self.output_schema = Some(schema);
        self
    }

    pub fn build(self) -> Result<NodeDefinition> {
        Ok(NodeDefinition {
            id: self.id,
            node_type: NodeType::LlmCall(LlmCallNode {
                provider: self.provider.ok_or(Error::MissingField("provider"))?,
                model: self.model.ok_or(Error::MissingField("model"))?,
                prompt: self.prompt.ok_or(Error::MissingField("prompt"))?,
                output_schema: self.output_schema,
                // ...
            }),
            // ...
        })
    }
}

// Convenience constructor
impl Node {
    pub fn llm_call(id: impl Into<NodeId>) -> LLMCallNodeBuilder {
        LLMCallNodeBuilder {
            id: id.into(),
            provider: None,
            model: None,
            prompt: None,
            output_schema: None,
        }
    }
}
```

### Language Bindings via FFI

**Python (PyO3):**

```rust
// crates/simple-agents-workflow-python/src/lib.rs
use pyo3::prelude::*;

#[pyclass]
pub struct PyWorkflowGraph {
    inner: WorkflowGraph,
}

#[pymethods]
impl PyWorkflowGraph {
    #[new]
    fn new() -> Self {
        Self {
            inner: WorkflowGraph::default(),
        }
    }

    fn id(&mut self, id: String) -> PyResult<Self> {
        self.inner.id = id.into();
        Ok(self)
    }

    fn to_yaml(&self) -> PyResult<String> {
        serde_yaml::to_string(&self.inner)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()))
    }

    fn execute(&self, py: Python, input: PyObject) -> PyResult<PyObject> {
        // Convert Python object to Rust Value
        let input_value: Value = pythonize::depythonize(input.as_ref(py))?;

        // Execute workflow (async bridge)
        pyo3_asyncio::tokio::future_into_py(py, async move {
            let engine = WorkflowEngine::new()?;
            let result = engine.execute(&self.inner, input_value).await?;

            // Convert back to Python
            Ok(pythonize::pythonize(&result)?)
        })
    }
}
```

**TypeScript (NAPI):**

```rust
// crates/simple-agents-workflow-node/src/lib.rs
use napi::bindgen_prelude::*;
use napi_derive::napi;

#[napi]
pub struct WorkflowGraph {
    inner: simple_agents_workflow::WorkflowGraph,
}

#[napi]
impl WorkflowGraph {
    #[napi(constructor)]
    pub fn new() -> Self {
        Self {
            inner: simple_agents_workflow::WorkflowGraph::default(),
        }
    }

    #[napi]
    pub fn id(&mut self, id: String) -> Result<&Self> {
        self.inner.id = id.into();
        Ok(self)
    }

    #[napi]
    pub fn to_yaml(&self) -> Result<String> {
        serde_yaml::to_string(&self.inner)
            .map_err(|e| Error::from_reason(e.to_string()))
    }

    #[napi]
    pub async fn execute(&self, input: JsObject) -> Result<JsObject> {
        // Convert JS object to Rust Value
        let input_value: Value = serde_json::from_str(&input.to_string()?)?;

        // Execute
        let engine = simple_agents_workflow::WorkflowEngine::new()
            .map_err(|e| Error::from_reason(e.to_string()))?;
        let result = engine.execute(&self.inner, input_value).await
            .map_err(|e| Error::from_reason(e.to_string()))?;

        // Convert back to JS
        let js_result = JsObject::from_str(&serde_json::to_string(&result)?)?;
        Ok(js_result)
    }
}
```

### Validation at Build Time

```rust
impl WorkflowGraph {
    pub fn validate(&self) -> Result<()> {
        // Check all edge references exist
        for edge in &self.edges {
            if !self.nodes.contains_key(&edge.from) {
                return Err(Error::InvalidEdge {
                    edge: edge.clone(),
                    reason: format!("Source node '{}' not found", edge.from),
                });
            }
            if !self.nodes.contains_key(&edge.to) {
                return Err(Error::InvalidEdge {
                    edge: edge.clone(),
                    reason: format!("Target node '{}' not found", edge.to),
                });
            }
        }

        // Check for cycles (unless explicitly allowed)
        self.detect_cycles()?;

        // Validate node configurations
        for node in self.nodes.values() {
            node.validate()?;
        }

        // Validate expressions (CEL syntax)
        for node in self.nodes.values() {
            if let Some(condition) = &node.condition {
                cel::validate_expression(condition)?;
            }
        }

        Ok(())
    }
}
```

## Documentation Strategy

- **API docs**: Generated from code (rustdoc, pydoc, typedoc, godoc)
- **Examples**: Side-by-side YAML and code for each language
- **Migration guide**: How to convert YAML to code and vice versa
- **Best practices**: When to use YAML vs code

## Testing Strategy

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_code_dsl_equals_yaml() {
        // Build workflow with code DSL
        let workflow_code = WorkflowGraph::builder()
            .id("test")
            .version("1.0.0")
            .node(Node::llm_call("node1").provider(Provider::OpenAI).model("gpt-4"))
            .build()
            .unwrap();

        // Load equivalent YAML
        let workflow_yaml: WorkflowGraph = serde_yaml::from_str(r#"
            id: test
            version: 1.0.0
            nodes:
              - id: node1
                node_type:
                  llm_call:
                    provider: openai
                    model: gpt-4
        "#).unwrap();

        // Should produce identical IR
        assert_eq!(workflow_code, workflow_yaml);
    }
}
```

## Related Decisions
- ADR-001: Canonical IR Format (YAML/JSON)
- ADR-011: Node Type Taxonomy

## Future Enhancements
- Visual workflow editor that emits code DSL
- Workflow templates library
- DSL macros for common patterns
- Schema inference from usage
