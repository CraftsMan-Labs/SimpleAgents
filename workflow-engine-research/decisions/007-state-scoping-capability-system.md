# ADR-007: State Scoping and Capability System

## Status
Accepted

## Context
Workflows need secure, hierarchical state management that:
- **Supports scoping**: Nodes have local state, subgraphs are isolated, global state is shared
- **Enables node references**: Downstream nodes can access outputs from upstream nodes
- **Enforces security boundaries**: Capability tokens control access to models, APIs, and resources
- **Prevents leakage**: Subgraphs can't access parent workflow's scoped state (but share global state)
- **Scales efficiently**: Minimize lock contention and memory footprint for large workflows

Requirements:
- **Hierarchical lookup**: Local → Parent → ... → Root → Global
- **Capability-based access control**: Nodes declare required capabilities, engine enforces
- **Immutable node outputs**: Once written, outputs cannot be modified
- **Template resolution**: Nodes can reference prior outputs via `$.nodes.<id>.output` syntax
- **Performance**: O(1) reads for global state, O(depth) for scoped lookups

## Decision
Implement a **hierarchical state manager with capability-based access control**.

Architecture:
- **StateManager**: Manages global state, scoped state, and node outputs
- **ScopedState**: Per-node/subgraph local variables with parent references
- **CapabilityRegistry**: Validates tokens against allowlists/denylists for models and resources
- **ExecutionContext**: Provides template resolution for `$.nodes.<id>.output` references
- **Garbage collection**: Cleanup scopes after node completion to reduce memory

State visibility:
- **Global state**: Read/write by all nodes across all scopes
- **Scoped state**: Hierarchical; child can read parent, parent cannot read child
- **Node outputs**: Immutable map accessible via JSON path expressions
- **Subgraph isolation**: Subgraphs get isolated scope but share global state

## Alternatives Considered

### 1. **Flat Global State Only**
- **Pros**:
  - Simple implementation
  - No scoping complexity
  - Fast O(1) lookups
- **Cons**:
  - Name collisions between subgraphs
  - No isolation for reusable subgraphs
  - Security boundaries harder to enforce
- **Rejected**: Insufficient isolation for composable workflows

### 2. **Fully Isolated Scopes (No Global State)**
- **Pros**:
  - Strong isolation
  - No accidental state sharing
  - Easy to reason about
- **Cons**:
  - Can't share configuration across subgraphs
  - Requires explicit passing of all state
  - Verbose workflow definitions
- **Rejected**: Too restrictive for common patterns

### 3. **Thread-Local Storage**
- **Pros**:
  - Automatic scoping per execution
  - No explicit state passing
- **Cons**:
  - Not portable across async boundaries
  - Hard to debug and trace
  - Doesn't work with Tokio's work-stealing
- **Rejected**: Incompatible with async Rust execution model

### 4. **Actor Model (Message Passing)**
- **Pros**:
  - Strong isolation guarantees
  - No shared state to coordinate
  - Natural concurrency model
- **Cons**:
  - High overhead for message passing
  - Complex state synchronization
  - Hard to reference prior node outputs
- **Rejected**: Too heavyweight for workflow state management

### 5. **Context Propagation (No Mutable State)**
- **Pros**:
  - Pure functional model
  - Easy to replay and test
  - No concurrency issues
- **Cons**:
  - Requires copying entire context per node
  - Memory inefficient for large state
  - Can't model global counters/accumulators
- **Rejected**: Performance overhead too high for large workflows

## Consequences

### Positive
- **Security**: Capability tokens enforce fine-grained access control
- **Isolation**: Subgraphs are isolated from parent scopes
- **Composability**: Reusable subgraphs don't pollute parent state
- **Flexibility**: Both global and scoped state available
- **Ergonomics**: Node output references via simple JSON path syntax
- **Performance**: Read-heavy workloads benefit from RwLock concurrency

### Negative
- **Complexity**: Hierarchical lookup adds cognitive overhead
- **Lock contention**: RwLock can bottleneck under high write load
- **Memory overhead**: O(N × S) for N nodes with S local variables each
- **Debugging**: Scoping rules can be confusing when troubleshooting
- **Capability maintenance**: Keeping tokens in sync across workflow changes

## Implementation Notes

### StateManager Architecture

```rust
pub struct StateManager {
    /// Global state (read/write by all nodes)
    global: Arc<RwLock<HashMap<String, Value>>>,

    /// Scoped state (hierarchical per node)
    scopes: Arc<RwLock<HashMap<NodeId, ScopedState>>>,

    /// Capability registry for access control
    capabilities: Arc<CapabilityRegistry>,
}

pub struct ScopedState {
    /// Local variables for this node/subgraph
    local: HashMap<String, Value>,

    /// Parent scope reference (for hierarchical lookup)
    parent: Option<NodeId>,

    /// Capability tokens granted to this scope
    tokens: Vec<String>,
}

impl StateManager {
    /// Create new state manager with global state
    pub fn new() -> Self {
        Self {
            global: Arc::new(RwLock::new(HashMap::new())),
            scopes: Arc::new(RwLock::new(HashMap::new())),
            capabilities: Arc::new(CapabilityRegistry::new()),
        }
    }

    /// Create isolated child scope (for subgraphs)
    pub fn new_isolated(parent: Arc<StateManager>) -> Self {
        Self {
            global: parent.global.clone(), // Share global
            scopes: Arc::new(RwLock::new(HashMap::new())), // Isolated scopes
            capabilities: parent.capabilities.clone(),
        }
    }

    /// Get value with hierarchical lookup: local → parent → ... → global
    pub async fn get(&self, node_id: &NodeId, key: &str) -> Option<Value> {
        let scopes = self.scopes.read().await;

        // 1. Check local scope
        if let Some(scope) = scopes.get(node_id) {
            if let Some(val) = scope.local.get(key) {
                return Some(val.clone());
            }

            // 2. Check parent scope recursively
            if let Some(parent_id) = &scope.parent {
                drop(scopes);
                return self.get(parent_id, key).await;
            }
        }

        drop(scopes);

        // 3. Check global state
        let global = self.global.read().await;
        global.get(key).cloned()
    }

    /// Set value in local scope
    pub async fn set(&self, node_id: &NodeId, key: String, value: Value) -> Result<()> {
        let mut scopes = self.scopes.write().await;

        let scope = scopes.entry(node_id.clone())
            .or_insert_with(|| ScopedState {
                local: HashMap::new(),
                parent: None,
                tokens: vec![],
            });

        scope.local.insert(key, value);
        Ok(())
    }

    /// Set global state
    pub async fn set_global(&self, key: String, value: Value) {
        let mut global = self.global.write().await;
        global.insert(key, value);
    }
}
```

### Capability-Based Access Control

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityToken {
    /// Unique identifier
    pub id: String,

    /// Allowed LLM models (glob patterns supported)
    pub allowed_models: Option<Vec<String>>,

    /// Allowed external resources (URLs, APIs)
    pub allowed_resources: Option<Vec<String>>,

    /// Denied models/resources (blocklist)
    pub denied: Option<Vec<String>>,

    /// Expiration time
    pub expires_at: Option<DateTime<Utc>>,
}

pub struct CapabilityRegistry {
    tokens: RwLock<HashMap<String, CapabilityToken>>,
}

impl CapabilityRegistry {
    /// Check if capabilities allow an operation
    pub async fn check(&self, scope_tokens: &[String], required: &[String]) -> Result<()> {
        let tokens = self.tokens.read().await;

        for req in required {
            let mut granted = false;

            for token_id in scope_tokens {
                if let Some(token) = tokens.get(token_id) {
                    // Check expiration
                    if let Some(exp) = token.expires_at {
                        if Utc::now() > exp {
                            continue;
                        }
                    }

                    // Check if requirement is allowed
                    if self.is_allowed(token, req) {
                        granted = true;
                        break;
                    }
                }
            }

            if !granted {
                return Err(SimpleAgentsError::CapabilityDenied(req.clone()));
            }
        }

        Ok(())
    }

    fn is_allowed(&self, token: &CapabilityToken, requirement: &str) -> bool {
        // Check denied list first
        if let Some(denied) = &token.denied {
            if denied.iter().any(|d| self.matches(d, requirement)) {
                return false;
            }
        }

        // Check allowed models
        if let Some(allowed_models) = &token.allowed_models {
            if allowed_models.iter().any(|m| self.matches(m, requirement)) {
                return true;
            }
        }

        // Check allowed resources
        if let Some(allowed_resources) = &token.allowed_resources {
            if allowed_resources.iter().any(|r| self.matches(r, requirement)) {
                return true;
            }
        }

        false
    }

    fn matches(&self, pattern: &str, value: &str) -> bool {
        // Support glob patterns: gpt-4* matches gpt-4-turbo, gpt-4-32k, etc.
        if pattern.contains('*') {
            let regex = pattern.replace("*", ".*");
            regex::Regex::new(&format!("^{}$", regex))
                .map(|re| re.is_match(value))
                .unwrap_or(false)
        } else {
            pattern == value
        }
    }
}
```

### Node Output References

```rust
pub struct ExecutionContext {
    pub execution_id: ExecutionId,
    pub node_id: NodeId,
    pub input: Value,
    pub state: Arc<StateManager>,

    /// Immutable map of all node outputs so far
    pub node_outputs: Arc<RwLock<HashMap<NodeId, Value>>>,
}

impl ExecutionContext {
    /// Resolve template with node output references
    /// Example: "{{ $.nodes.analyze.output.sentiment }}"
    pub async fn resolve_template(&self, template: &str) -> Result<String> {
        let mut result = template.to_string();

        // Regex: $.nodes.{node_id}.output[.path]
        let re = Regex::new(r"\{\{\s*\$\.nodes\.(\w+)\.output(?:\.(\S+))?\s*\}\}")?;

        let outputs = self.node_outputs.read().await;

        for cap in re.captures_iter(template) {
            let node_id = &cap[1];
            let path = cap.get(2).map(|m| m.as_str());

            if let Some(output) = outputs.get(node_id) {
                let value = if let Some(path) = path {
                    // JSON path traversal: sentiment → output["sentiment"]
                    self.json_path_get(output, path)?
                } else {
                    output.clone()
                };

                let replacement = match value {
                    Value::String(s) => s,
                    other => serde_json::to_string(&other)?,
                };

                result = result.replace(&cap[0], &replacement);
            }
        }

        Ok(result)
    }

    fn json_path_get(&self, value: &Value, path: &str) -> Result<Value> {
        let parts: Vec<&str> = path.split('.').collect();
        let mut current = value;

        for part in parts {
            current = current.get(part)
                .ok_or_else(|| SimpleAgentsError::PathNotFound(path.to_string()))?;
        }

        Ok(current.clone())
    }
}
```

### Workflow Example with Capabilities

```yaml
# workflow.yaml
capabilities:
  - id: llm_access
    allowed_models:
      - "gpt-4*"  # All GPT-4 variants
      - "claude-3-sonnet*"
    denied:
      - "gpt-4-32k"  # Too expensive

  - id: api_access
    allowed_resources:
      - "https://api.example.com/*"
    denied:
      - "https://api.example.com/admin/*"

nodes:
  - id: analyze
    node_type:
      llm_call:
        provider: openai
        model: gpt-4-turbo
        prompt: "Analyze: {{ input.text }}"

    required_capabilities:
      - llm_access  # Will check if gpt-4-turbo is allowed

  - id: summarize
    node_type:
      llm_call:
        provider: openai
        model: gpt-4
        prompt: |
          Based on this analysis:
          {{ $.nodes.analyze.output.analysis }}

          Create a 3-sentence summary.

    required_capabilities:
      - llm_access

  - id: fetch_data
    node_type:
      custom_worker:
        language: python
        handler: FetchFromAPI

    required_capabilities:
      - api_access
```

### Subgraph Scoping Example

```rust
async fn execute_subgraph(
    &self,
    graph_ref: &GraphId,
    version: &Option<String>,
    ctx: &ExecutionContext,
) -> Result<NodeOutput> {
    // Load subgraph
    let subgraph = self.resolve_subgraph(graph_ref, version).await?;

    // Create isolated state manager
    let sub_state = StateManager::new_isolated(ctx.state.clone());

    // Copy input to subgraph's local scope
    sub_state.set(&subgraph.entry_node, "input".to_string(), ctx.input.clone()).await?;

    // Create sub-executor
    let sub_executor = WorkflowExecutor {
        graph: Arc::new(subgraph.clone()),
        state: Arc::new(sub_state),
        // ... other fields
    };

    // Execute subgraph
    let result = sub_executor.execute(ctx.input.clone()).await?;

    Ok(NodeOutput {
        value: result.output.unwrap_or(json!(null)),
        streaming: false,
        metadata: None,
    })
}
```

### State Visibility Example

```
Global State: { "user_id": "123", "session": "abc" }

Main Workflow (root scope)
  Local: { "workflow_start": "2026-01-31T10:00:00Z" }

  ├─ Node: analyze
  │    Local: { "model": "gpt-4", "temperature": 0.7 }
  │    Visible: global + root + analyze
  │
  └─ Subgraph: preprocessing
       Local: { "subgraph_start": "2026-01-31T10:00:05Z" }

       └─ Node: normalize
            Local: { "normalized": true }
            Visible: global + preprocessing + normalize
            NOT visible: root.workflow_start, analyze.model
```

### Performance Optimization: Garbage Collection

```rust
impl StateManager {
    /// Remove scope after node completes (if not referenced)
    pub async fn cleanup_scope(&self, node_id: &NodeId) -> Result<()> {
        let mut scopes = self.scopes.write().await;

        // Check if any other scope has this as parent
        let is_referenced = scopes.values()
            .any(|s| s.parent.as_ref() == Some(node_id));

        if !is_referenced {
            scopes.remove(node_id);
        }

        Ok(())
    }
}
```

### Performance Optimization: Sharded State

For high-concurrency workloads, shard global state to reduce lock contention:

```rust
pub struct ShardedStateManager {
    shards: Vec<Arc<RwLock<HashMap<String, Value>>>>,
    num_shards: usize,
}

impl ShardedStateManager {
    fn shard_index(&self, key: &str) -> usize {
        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        (hasher.finish() as usize) % self.num_shards
    }

    pub async fn get(&self, key: &str) -> Option<Value> {
        let shard = &self.shards[self.shard_index(key)];
        let guard = shard.read().await;
        guard.get(key).cloned()
    }

    pub async fn set(&self, key: String, value: Value) {
        let shard = &self.shards[self.shard_index(&key)];
        let mut guard = shard.write().await;
        guard.insert(key, value);
    }
}
```

## Related Decisions
- ADR-001: Canonical IR Format (YAML/JSON)
- ADR-002: CEL Expression Language
- ADR-008: Trace Recording and Replayability
- ADR-011: Node Type Taxonomy

## Future Enhancements
- **Distributed state**: Redis/DynamoDB backend for multi-instance workflows
- **State snapshots**: Checkpoint state for long-running workflows
- **Capability delegation**: Subgraphs can grant subset of parent capabilities
- **State encryption**: Encrypt sensitive state at rest and in transit
- **Audit logging**: Track all state mutations for compliance
