# ADR-008: Trace Recording and Replayability

## Status
Accepted

## Context
Workflows must be debuggable, auditable, and recoverable. This requires:
- **Deterministic replay**: Re-execute workflows from any node using recorded traces
- **Debugging**: Inspect inputs, outputs, and decisions at each node
- **Auditing**: Track which models were called, what data was processed, and what decisions were made
- **Recovery**: Resume workflows after failures without re-executing completed nodes
- **LLM caching**: Handle non-determinism by recording and replaying LLM responses

Challenges:
- **LLM non-determinism**: Same prompt can yield different responses (mitigate with caching)
- **External API variability**: APIs may return different data over time
- **Trace storage growth**: Long workflows or high throughput generate large traces
- **Partial replay**: Resume from arbitrary checkpoint, not just start
- **Streaming outputs**: Capture progressive chunks for replay

## Decision
Implement a **comprehensive trace recording system** with deterministic replay capabilities.

Architecture:
- **TraceRecorder**: Records execution events (node starts, completions, decisions, errors)
- **Trace format**: JSON-based event log with inputs, outputs, metadata, and timing
- **Checkpointing**: Save state at node boundaries for resume-from-node capability
- **LLM response caching**: Cache responses by prompt hash for deterministic replay
- **Replay engine**: Re-execute workflow using recorded events instead of live execution
- **Streaming support**: Record streaming chunks for faithful replay

Design principles:
- **Event-sourcing**: All state changes recorded as immutable events
- **Self-contained traces**: Include all data needed for replay (no external dependencies)
- **Versioned format**: Trace schema versioned for forward/backward compatibility
- **Storage-agnostic**: Support multiple backends (filesystem, S3, database)

## Alternatives Considered

### 1. **No Trace Recording (Stateless Execution)**
- **Pros**:
  - Simple implementation
  - No storage overhead
  - No replay complexity
- **Cons**:
  - Can't debug past executions
  - No audit trail
  - Must re-execute from start on failure
  - Can't analyze workflow performance
- **Rejected**: Debugging and auditability are critical requirements

### 2. **Checkpoint-Only (No Event History)**
- **Pros**:
  - Smaller storage footprint
  - Fast resume from checkpoints
  - Simple implementation
- **Cons**:
  - Can't inspect intermediate steps
  - No audit trail of decisions
  - Can't analyze performance bottlenecks
  - Limited debugging capability
- **Rejected**: Insufficient for debugging and auditing needs

### 3. **Database Transaction Log**
- **Pros**:
  - ACID guarantees
  - Built-in indexing and querying
  - Proven durability
- **Cons**:
  - Tight coupling to database
  - Schema migrations required
  - Higher operational complexity
  - Scaling challenges for high throughput
- **Rejected**: Want storage-agnostic solution

### 4. **Temporal-Style Event Sourcing (Full Determinism)**
- **Pros**:
  - Proven production model
  - Strict determinism guarantees
  - Comprehensive replay support
- **Cons**:
  - LLM calls are inherently non-deterministic
  - Requires strict workflow versioning
  - Heavy runtime overhead
  - Complex to implement correctly
- **Rejected**: Too strict for LLM workflows; we use response caching instead

### 5. **Log-Only (No Structured Traces)**
- **Pros**:
  - Simple logging infrastructure
  - No schema to maintain
  - Flexible format
- **Cons**:
  - Hard to parse and query
  - No replay capability
  - Can't validate completeness
  - Poor developer experience
- **Rejected**: Need structured data for replay

## Consequences

### Positive
- **Debuggability**: Inspect any past execution step-by-step
- **Auditability**: Complete record of model usage, API calls, and data processed
- **Recoverability**: Resume from any node after failure
- **Testing**: Golden traces for regression testing
- **Optimization**: Identify performance bottlenecks from trace data
- **LLM caching**: Deterministic replay using cached responses

### Negative
- **Storage overhead**: Traces can be large (mitigated by compression and retention policies)
- **Performance impact**: Recording adds latency (typically <1ms per event)
- **Privacy concerns**: Traces contain sensitive data (mitigated by encryption and redaction)
- **Non-determinism**: External APIs may return different data on replay
- **Trace versioning**: Schema changes require migration or compatibility layers

## Implementation Notes

### Trace Format

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionTrace {
    /// Unique execution identifier
    pub execution_id: ExecutionId,

    /// Workflow graph ID and version
    pub graph_id: GraphId,
    pub graph_version: Version,

    /// Execution metadata
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub status: ExecutionStatus,

    /// Workflow input
    pub input: Value,

    /// Final output
    pub output: Option<Value>,

    /// Error if failed
    pub error: Option<String>,

    /// Ordered list of execution events
    pub events: Vec<TraceEvent>,

    /// Trace format version
    pub trace_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TraceEvent {
    /// Node execution started
    NodeStarted {
        node_id: NodeId,
        timestamp: DateTime<Utc>,
        input: Value,
    },

    /// Node execution completed
    NodeCompleted {
        node_id: NodeId,
        timestamp: DateTime<Utc>,
        output: Value,
        duration_ms: u64,
        metadata: NodeMetadata,
    },

    /// Node execution failed
    NodeFailed {
        node_id: NodeId,
        timestamp: DateTime<Utc>,
        error: String,
        retry_count: usize,
    },

    /// Decision point (switch, filter)
    Decision {
        node_id: NodeId,
        timestamp: DateTime<Utc>,
        decision_type: String,  // "switch", "filter", etc.
        branch_taken: String,
        condition_result: bool,
    },

    /// Loop iteration
    LoopIteration {
        node_id: NodeId,
        timestamp: DateTime<Utc>,
        iteration: usize,
        condition_result: bool,
    },

    /// Streaming chunk
    StreamChunk {
        node_id: NodeId,
        timestamp: DateTime<Utc>,
        chunk_index: usize,
        chunk_data: Value,
    },

    /// State mutation
    StateChange {
        node_id: NodeId,
        timestamp: DateTime<Utc>,
        scope: StateScope,  // "global", "local", "node_output"
        key: String,
        value: Value,
    },

    /// Checkpoint saved
    Checkpoint {
        node_id: NodeId,
        timestamp: DateTime<Utc>,
        state_snapshot: HashMap<String, Value>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExecutionStatus {
    Running,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeMetadata {
    /// LLM provider (if applicable)
    pub provider: Option<String>,

    /// Model name (if applicable)
    pub model: Option<String>,

    /// Token usage (if applicable)
    pub tokens: Option<u64>,

    /// Latency (if applicable)
    pub latency_ms: Option<u64>,

    /// Custom metadata
    pub custom: HashMap<String, Value>,
}
```

### TraceRecorder Implementation

```rust
pub struct TraceRecorder {
    /// Current execution ID
    execution_id: ExecutionId,

    /// Events buffer
    events: Arc<RwLock<Vec<TraceEvent>>>,

    /// Storage backend
    storage: Arc<dyn TraceStorage>,

    /// LLM response cache
    llm_cache: Arc<LlmResponseCache>,
}

impl TraceRecorder {
    pub fn new(execution_id: ExecutionId, storage: Arc<dyn TraceStorage>) -> Self {
        Self {
            execution_id,
            events: Arc::new(RwLock::new(Vec::new())),
            storage,
            llm_cache: Arc::new(LlmResponseCache::new()),
        }
    }

    /// Record execution start
    pub async fn record_start(&self, ctx: &ExecutionContext) -> Result<()> {
        let event = TraceEvent::NodeStarted {
            node_id: ctx.node_id.clone(),
            timestamp: Utc::now(),
            input: ctx.input.clone(),
        };

        let mut events = self.events.write().await;
        events.push(event);

        Ok(())
    }

    /// Record node completion
    pub async fn record_node_execution(
        &self,
        ctx: &ExecutionContext,
        node_id: &NodeId,
        input: &Value,
        output: &NodeOutput,
        duration: Duration,
    ) -> Result<()> {
        let event = TraceEvent::NodeCompleted {
            node_id: node_id.clone(),
            timestamp: Utc::now(),
            output: output.value.clone(),
            duration_ms: duration.as_millis() as u64,
            metadata: output.metadata.clone().unwrap_or_default(),
        };

        let mut events = self.events.write().await;
        events.push(event);

        // Cache LLM responses for replay
        if let Some(metadata) = &output.metadata {
            if metadata.provider.is_some() {
                self.llm_cache.store(input, &output.value).await?;
            }
        }

        Ok(())
    }

    /// Record decision point
    pub async fn record_decision(
        &self,
        ctx: &ExecutionContext,
        decision_type: &str,
        branch_taken: &str,
    ) -> Result<()> {
        let event = TraceEvent::Decision {
            node_id: ctx.node_id.clone(),
            timestamp: Utc::now(),
            decision_type: decision_type.to_string(),
            branch_taken: branch_taken.to_string(),
            condition_result: true,
        };

        let mut events = self.events.write().await;
        events.push(event);

        Ok(())
    }

    /// Record checkpoint
    pub async fn record_checkpoint(
        &self,
        node_id: &NodeId,
        state: HashMap<String, Value>,
    ) -> Result<()> {
        let event = TraceEvent::Checkpoint {
            node_id: node_id.clone(),
            timestamp: Utc::now(),
            state_snapshot: state,
        };

        let mut events = self.events.write().await;
        events.push(event);

        Ok(())
    }

    /// Flush trace to storage
    pub async fn flush(&self, trace: ExecutionTrace) -> Result<()> {
        self.storage.store(&self.execution_id, &trace).await
    }
}
```

### LLM Response Cache

```rust
pub struct LlmResponseCache {
    cache: Arc<RwLock<HashMap<String, Value>>>,
}

impl LlmResponseCache {
    pub fn new() -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Store LLM response by prompt hash
    pub async fn store(&self, prompt: &Value, response: &Value) -> Result<()> {
        let key = self.hash_prompt(prompt);
        let mut cache = self.cache.write().await;
        cache.insert(key, response.clone());
        Ok(())
    }

    /// Retrieve cached response
    pub async fn get(&self, prompt: &Value) -> Option<Value> {
        let key = self.hash_prompt(prompt);
        let cache = self.cache.read().await;
        cache.get(&key).cloned()
    }

    fn hash_prompt(&self, prompt: &Value) -> String {
        use sha2::{Sha256, Digest};
        let json = serde_json::to_string(prompt).unwrap();
        let mut hasher = Sha256::new();
        hasher.update(json.as_bytes());
        format!("{:x}", hasher.finalize())
    }
}
```

### Replay Engine

```rust
pub struct ReplayEngine {
    trace: ExecutionTrace,
    current_event: usize,
    llm_cache: Arc<LlmResponseCache>,
}

impl ReplayEngine {
    pub fn new(trace: ExecutionTrace) -> Self {
        Self {
            trace,
            current_event: 0,
            llm_cache: Arc::new(LlmResponseCache::new()),
        }
    }

    /// Replay from start
    pub async fn replay(&mut self) -> Result<WorkflowResult> {
        self.replay_from_node(&self.trace.graph_id.clone()).await
    }

    /// Replay from specific node
    pub async fn replay_from_node(&mut self, from_node: &NodeId) -> Result<WorkflowResult> {
        // 1. Restore state up to from_node
        let state = self.restore_state_to_node(from_node).await?;

        // 2. Find starting event index
        self.current_event = self.find_event_index(from_node)?;

        // 3. Execute using recorded events
        self.execute_with_events(state).await
    }

    async fn restore_state_to_node(&self, target_node: &NodeId) -> Result<HashMap<String, Value>> {
        let mut state = HashMap::new();

        for event in &self.trace.events {
            match event {
                TraceEvent::Checkpoint { node_id, state_snapshot, .. } => {
                    if node_id == target_node {
                        return Ok(state_snapshot.clone());
                    }
                }
                TraceEvent::StateChange { key, value, .. } => {
                    state.insert(key.clone(), value.clone());
                }
                TraceEvent::NodeCompleted { node_id, .. } => {
                    if node_id == target_node {
                        break;
                    }
                }
                _ => {}
            }
        }

        Ok(state)
    }

    async fn execute_with_events(&mut self, initial_state: HashMap<String, Value>) -> Result<WorkflowResult> {
        // Use recorded events to drive execution
        while self.current_event < self.trace.events.len() {
            let event = &self.trace.events[self.current_event];

            match event {
                TraceEvent::NodeCompleted { output, .. } => {
                    // Use recorded output instead of re-executing
                    // Continue to next node
                }
                TraceEvent::Decision { branch_taken, .. } => {
                    // Follow recorded decision
                }
                _ => {}
            }

            self.current_event += 1;
        }

        Ok(WorkflowResult {
            execution_id: self.trace.execution_id.clone(),
            success: self.trace.status == ExecutionStatus::Completed,
            output: self.trace.output.clone(),
            error: self.trace.error.clone(),
            duration: self.trace.completed_at
                .and_then(|end| end.signed_duration_since(self.trace.started_at).to_std().ok())
                .unwrap_or_default(),
        })
    }

    fn find_event_index(&self, node_id: &NodeId) -> Result<usize> {
        self.trace.events.iter()
            .position(|e| match e {
                TraceEvent::NodeStarted { node_id: id, .. } => id == node_id,
                _ => false,
            })
            .ok_or_else(|| SimpleAgentsError::NodeNotFoundInTrace(node_id.clone()))
    }
}
```

### Storage Interface

```rust
#[async_trait]
pub trait TraceStorage: Send + Sync {
    /// Store execution trace
    async fn store(&self, execution_id: &ExecutionId, trace: &ExecutionTrace) -> Result<()>;

    /// Retrieve execution trace
    async fn get(&self, execution_id: &ExecutionId) -> Result<ExecutionTrace>;

    /// List traces by graph ID
    async fn list_by_graph(&self, graph_id: &GraphId, limit: usize) -> Result<Vec<ExecutionTrace>>;

    /// Delete old traces (retention policy)
    async fn cleanup(&self, older_than: DateTime<Utc>) -> Result<usize>;
}

/// Filesystem-based storage
pub struct FileTraceStorage {
    base_path: PathBuf,
}

impl FileTraceStorage {
    pub fn new(base_path: PathBuf) -> Self {
        Self { base_path }
    }

    fn trace_path(&self, execution_id: &ExecutionId) -> PathBuf {
        self.base_path.join(format!("{}.json", execution_id))
    }
}

#[async_trait]
impl TraceStorage for FileTraceStorage {
    async fn store(&self, execution_id: &ExecutionId, trace: &ExecutionTrace) -> Result<()> {
        let path = self.trace_path(execution_id);
        let json = serde_json::to_string_pretty(trace)?;

        tokio::fs::create_dir_all(&self.base_path).await?;
        tokio::fs::write(path, json).await?;

        Ok(())
    }

    async fn get(&self, execution_id: &ExecutionId) -> Result<ExecutionTrace> {
        let path = self.trace_path(execution_id);
        let json = tokio::fs::read_to_string(path).await?;
        let trace = serde_json::from_str(&json)?;

        Ok(trace)
    }

    async fn list_by_graph(&self, graph_id: &GraphId, limit: usize) -> Result<Vec<ExecutionTrace>> {
        let mut traces = Vec::new();

        let mut entries = tokio::fs::read_dir(&self.base_path).await?;

        while let Some(entry) = entries.next_entry().await? {
            if traces.len() >= limit {
                break;
            }

            if let Ok(trace) = self.get(&entry.file_name().to_string_lossy().to_string()).await {
                if &trace.graph_id == graph_id {
                    traces.push(trace);
                }
            }
        }

        Ok(traces)
    }

    async fn cleanup(&self, older_than: DateTime<Utc>) -> Result<usize> {
        let mut deleted = 0;
        let mut entries = tokio::fs::read_dir(&self.base_path).await?;

        while let Some(entry) = entries.next_entry().await? {
            if let Ok(metadata) = entry.metadata().await {
                if let Ok(modified) = metadata.modified() {
                    let modified_dt: DateTime<Utc> = modified.into();
                    if modified_dt < older_than {
                        tokio::fs::remove_file(entry.path()).await?;
                        deleted += 1;
                    }
                }
            }
        }

        Ok(deleted)
    }
}
```

### Checkpoint Strategy

```rust
pub struct CheckpointStrategy {
    /// Checkpoint every N nodes
    pub interval: Option<usize>,

    /// Checkpoint before expensive operations
    pub before_llm: bool,

    /// Checkpoint before branches
    pub before_branch: bool,

    /// Checkpoint after loops
    pub after_loop: bool,
}

impl WorkflowExecutor {
    async fn should_checkpoint(&self, node: &NodeDefinition, event_count: usize) -> bool {
        let strategy = &self.checkpoint_strategy;

        // Interval-based
        if let Some(interval) = strategy.interval {
            if event_count % interval == 0 {
                return true;
            }
        }

        // Node type-based
        match &node.node_type {
            NodeType::LlmCall { .. } if strategy.before_llm => true,
            NodeType::Switch { .. } if strategy.before_branch => true,
            NodeType::Loop { .. } if strategy.after_loop => true,
            _ => false,
        }
    }
}
```

### Example Trace

```json
{
  "execution_id": "exec_abc123",
  "graph_id": "sentiment-analysis",
  "graph_version": "1.0.0",
  "started_at": "2026-01-31T10:00:00Z",
  "completed_at": "2026-01-31T10:00:15Z",
  "status": "completed",
  "input": {"text": "Great product!"},
  "output": {"sentiment": "positive", "confidence": 0.95},
  "trace_version": "1.0",
  "events": [
    {
      "type": "node_started",
      "node_id": "analyze",
      "timestamp": "2026-01-31T10:00:01Z",
      "input": {"text": "Great product!"}
    },
    {
      "type": "node_completed",
      "node_id": "analyze",
      "timestamp": "2026-01-31T10:00:05Z",
      "output": {"sentiment": "positive", "confidence": 0.95},
      "duration_ms": 4000,
      "metadata": {
        "provider": "openai",
        "model": "gpt-4",
        "tokens": 150
      }
    },
    {
      "type": "decision",
      "node_id": "route",
      "timestamp": "2026-01-31T10:00:06Z",
      "decision_type": "switch",
      "branch_taken": "positive_handler",
      "condition_result": true
    },
    {
      "type": "node_completed",
      "node_id": "celebrate",
      "timestamp": "2026-01-31T10:00:15Z",
      "output": {"action": "send_thanks"},
      "duration_ms": 50,
      "metadata": {}
    }
  ]
}
```

## Related Decisions
- ADR-001: Canonical IR Format (YAML/JSON)
- ADR-007: State Scoping and Capability System
- ADR-010: Testing Strategy and Golden Traces

## Future Enhancements
- **Distributed tracing**: OpenTelemetry integration
- **Trace compression**: Gzip or snappy for large traces
- **Incremental replay**: Resume from mid-node (streaming checkpoints)
- **Trace visualization**: Web UI for trace inspection
- **Privacy controls**: Automatic PII redaction in traces
- **Trace analytics**: Query traces for performance analysis
