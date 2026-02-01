# State Management and Hierarchical Scoping

## Overview

The workflow engine uses a hierarchical state model with capability-based access control. State is organized into:
- **Global state**: Accessible to all nodes
- **Scoped state**: Hierarchical parent-child scoping
- **Node outputs**: Immutable references to prior node results

## State Manager Architecture

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
    tokens: Vec<CapabilityToken>,
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
}
```

## Hierarchical Lookup

State resolution follows: **local → parent → ... → root → global**

```rust
impl StateManager {
    /// Get value with hierarchical lookup
    pub async fn get(&self, node_id: &NodeId, key: &str) -> Option<Value> {
        let scopes = self.scopes.read().await;

        // 1. Check local scope
        if let Some(scope) = scopes.get(node_id) {
            if let Some(val) = scope.local.get(key) {
                return Some(val.clone());
            }

            // 2. Check parent scope recursively
            if let Some(parent_id) = &scope.parent {
                drop(scopes); // Release lock before recursive call
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

    /// Get all accessible state for a node (for expression evaluation)
    pub async fn get_all(&self, node_id: &NodeId) -> Result<HashMap<String, Value>> {
        let mut state = HashMap::new();

        // Add global state
        {
            let global = self.global.read().await;
            state.extend(global.clone());
        }

        // Add scoped state (hierarchically)
        self.add_scoped_state(&mut state, node_id).await?;

        Ok(state)
    }

    /// Recursively add scoped state
    async fn add_scoped_state(&self, state: &mut HashMap<String, Value>, node_id: &NodeId) -> Result<()> {
        let scopes = self.scopes.read().await;

        if let Some(scope) = scopes.get(node_id) {
            // Add parent first (so local can override)
            if let Some(parent_id) = &scope.parent {
                drop(scopes);
                self.add_scoped_state(state, parent_id).await?;
                let scopes = self.scopes.read().await;
                if let Some(scope) = scopes.get(node_id) {
                    state.extend(scope.local.clone());
                }
            } else {
                state.extend(scope.local.clone());
            }
        }

        Ok(())
    }
}
```

## Capability-Based Access Control

### Capability Token Model

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityToken {
    /// Unique identifier
    pub id: String,

    /// Allowed LLM models
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
    /// Register a capability token
    pub async fn register(&self, token: CapabilityToken) {
        let mut tokens = self.tokens.write().await;
        tokens.insert(token.id.clone(), token);
    }

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
        // Support glob patterns
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

### Usage in Workflows

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
    required_capabilities:
      - llm_access  # Will check if gpt-4-turbo is allowed

  - id: fetch_data
    node_type:
      custom_worker:
        language: python
        handler: FetchFromAPI
    required_capabilities:
      - api_access
```

## Node Output References

Nodes can reference outputs from prior nodes using JSON path syntax:

```yaml
nodes:
  - id: analyze
    node_type:
      llm_call:
        provider: openai
        model: gpt-4
        prompt: "Analyze: {{ input.text }}"

  - id: summarize
    node_type:
      llm_call:
        provider: openai
        model: gpt-4
        prompt: |
          Based on this analysis:
          {{ $.nodes.analyze.output.analysis }}

          Create a 3-sentence summary.

  - id: classify
    node_type:
      llm_call:
        provider: anthropic
        model: claude-3-sonnet
        prompt: |
          Analysis: {{ $.nodes.analyze.output.analysis }}
          Summary: {{ $.nodes.summarize.output.summary }}

          Classify the sentiment.
```

Implementation:

```rust
pub struct ExecutionContext {
    pub execution_id: ExecutionId,
    pub graph_id: GraphId,
    pub node_id: NodeId,
    pub input: Value,
    pub state: Arc<StateManager>,

    /// Immutable map of all node outputs so far
    pub node_outputs: Arc<RwLock<HashMap<NodeId, Value>>>,

    pub started_at: DateTime<Utc>,
}

impl ExecutionContext {
    /// Resolve template with node output references
    pub async fn resolve_template(&self, template: &str) -> Result<String> {
        let mut result = template.to_string();

        // Regex: $.nodes.{node_id}.output
        let re = Regex::new(r"\{\{\s*\$\.nodes\.(\w+)\.output(?:\.(\S+))?\s*\}\}")?;

        let outputs = self.node_outputs.read().await;

        for cap in re.captures_iter(template) {
            let node_id = &cap[1];
            let path = cap.get(2).map(|m| m.as_str());

            if let Some(output) = outputs.get(node_id) {
                let value = if let Some(path) = path {
                    // JSON path traversal
                    self.json_path_get(output, path)?
                } else {
                    output.clone()
                };

                let replacement = match value {
                    Value::String(s) => s,
                    other => other.to_string(),
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

## Subgraph Scoping

Subgraphs execute with isolated scopes but share global state:

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
        evaluator: self.evaluator.clone(),
        workers: self.workers.clone(),
        agents_client: self.agents_client.clone(),
        tracer: self.tracer.clone_with_prefix(&graph_ref),
        semaphore: self.semaphore.clone(),
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

Example:

```yaml
# main.yaml
nodes:
  - id: preprocess
    node_type:
      subgraph:
        graph_ref: preprocessing-v1
        version: "^1.0.0"

  - id: analyze
    node_type:
      llm_call:
        model: gpt-4
        # Can access global state set by subgraph
        prompt: "Process this: {{ $.global.preprocessed_count }} items"
```

```yaml
# preprocessing-v1.yaml
nodes:
  - id: normalize
    node_type:
      transform:
        expression: 'input.text.toLowerCase()'

  - id: count
    node_type:
      transform:
        expression: 'input.text.split(" ").length'

  - id: store_count
    node_type:
      transform:
        expression: |
          // Set global state (accessible by parent workflow)
          $.global.preprocessed_count = $.nodes.count.output
```

## State Visibility Example

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

## Performance Considerations

### Memory Footprint

- **Global state**: O(G) where G = global variables
- **Scoped state**: O(N × S) where N = nodes, S = avg local variables
- **Node outputs**: O(N × O) where O = avg output size

Optimization: Garbage collect scopes after node completion

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

### Lock Contention

Use read-heavy workloads with `RwLock`:
- Reads: Concurrent (no blocking)
- Writes: Exclusive (blocking)

For high-concurrency, consider sharding:

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
}
```
