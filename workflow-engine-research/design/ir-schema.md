# Canonical IR Schema

## Overview

The Canonical Intermediate Representation (IR) is a language-agnostic format for defining workflows. It's designed to be:
- **Serializable**: YAML/JSON for portability
- **Type-safe**: JSON Schema for validation
- **Versioned**: Explicit versioning for compatibility
- **Composable**: Subgraphs for reusability

## Core Types

### WorkflowGraph

The top-level workflow definition.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowGraph {
    /// Unique identifier
    pub id: GraphId,

    /// Semantic version for compatibility
    pub version: Version,

    /// Human-readable metadata
    pub metadata: GraphMetadata,

    /// Entry point node ID
    pub entry_node: NodeId,

    /// All node definitions
    pub nodes: HashMap<NodeId, NodeDefinition>,

    /// Edge definitions (control flow)
    pub edges: Vec<EdgeDefinition>,

    /// Graph-level defaults
    pub defaults: GraphDefaults,

    /// Capability tokens for access control
    pub capabilities: Vec<CapabilityToken>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphMetadata {
    pub name: String,
    pub description: Option<String>,
    pub author: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphDefaults {
    /// Default retry policy
    pub retry: Option<RetryPolicy>,

    /// Default timeout for nodes
    pub timeout: Option<Duration>,

    /// Max concurrent nodes
    pub max_concurrency: Option<usize>,

    /// Default capability tokens
    pub capabilities: Vec<String>,
}
```

**YAML Example**:

```yaml
id: sentiment-analysis-v1
version: 1.0.0
metadata:
  name: "Sentiment Analysis Workflow"
  description: "Analyzes sentiment from user reviews"
  author: "team@example.com"
  tags: ["nlp", "sentiment", "reviews"]

entry_node: fetch_reviews

defaults:
  timeout: 30s
  max_concurrency: 10
  retry:
    max_attempts: 3
    backoff:
      type: exponential
      initial: 1s
      max: 30s

capabilities:
  - id: llm_access
    allowed_models: ["gpt-4", "claude-3-sonnet"]
  - id: api_access
    allowed_resources: ["https://api.example.com/*"]

nodes:
  # ... (defined below)

edges:
  # ... (defined below)
```

---

### NodeDefinition

Defines a single node in the workflow.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeDefinition {
    /// Unique identifier within this graph
    pub id: NodeId,

    /// Human-readable name
    pub name: String,

    /// Node type and configuration
    pub node_type: NodeType,

    /// JSON Schema for input validation
    pub input_schema: Option<JsonSchema>,

    /// JSON Schema for output validation
    pub output_schema: Option<JsonSchema>,

    /// Node-level retry override
    pub retry: Option<RetryPolicy>,

    /// Node-level timeout override
    pub timeout: Option<Duration>,

    /// Required capability tokens
    pub required_capabilities: Vec<String>,

    /// Node-specific configuration
    pub config: serde_json::Value,
}
```

**YAML Example**:

```yaml
nodes:
  - id: analyze_sentiment
    name: "Analyze Sentiment with GPT-4"
    node_type:
      llm_call:
        provider: openai
        model: gpt-4
        prompt: |
          Analyze the sentiment of this review:
          {{ $.nodes.fetch_reviews.output.text }}

          Respond with JSON: {"sentiment": "positive|negative|neutral", "confidence": 0.0-1.0}

    input_schema:
      type: object
      required: ["text"]
      properties:
        text:
          type: string

    output_schema:
      type: object
      required: ["sentiment", "confidence"]
      properties:
        sentiment:
          type: string
          enum: ["positive", "negative", "neutral"]
        confidence:
          type: number
          minimum: 0.0
          maximum: 1.0

    timeout: 60s
    retry:
      max_attempts: 3
      backoff:
        type: exponential
        initial: 2s

    required_capabilities:
      - llm_access
```

---

### NodeType (All 15 Types)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum NodeType {
    /// LLM provider call
    LlmCall {
        provider: String,
        model: String,
        #[serde(default)]
        temperature: Option<f32>,
        #[serde(default)]
        max_tokens: Option<u32>,
        #[serde(default)]
        stream: bool,
    },

    /// Conditional branching
    Switch {
        /// Branches evaluated in order
        branches: Vec<SwitchBranch>,
        /// Optional default branch if no conditions match
        default: Option<NodeId>,
    },

    /// Loop with condition
    Loop {
        /// Condition evaluated before each iteration
        condition: Expression,
        /// Node to execute in loop body
        body: NodeId,
        /// Max iterations (safety)
        max_iterations: Option<usize>,
    },

    /// Map over collection
    Map {
        /// Node to apply to each item
        node_ref: NodeId,
        /// Max parallelism
        max_parallel: Option<usize>,
    },

    /// Reduce collection
    Reduce {
        /// Node to execute for each item
        node_ref: NodeId,
        /// Initial accumulator value
        initial: serde_json::Value,
    },

    /// Subgraph invocation
    Subgraph {
        /// Graph ID to invoke
        graph_ref: GraphId,
        /// Version constraint (semver)
        version: Option<String>,
    },

    /// Parallel fan-out
    Parallel {
        /// Nodes to execute concurrently
        nodes: Vec<NodeId>,
    },

    /// Merge/join with policy
    Merge {
        /// Merge policy
        policy: MergePolicy,
    },

    /// Filter/guard
    Filter {
        /// Predicate expression
        predicate: Expression,
        /// What to do if predicate fails
        on_false: FilterAction,
    },

    /// Batch/window
    Batch {
        /// Batch size
        size: usize,
        /// Optional time window
        timeout: Option<Duration>,
    },

    /// Transform/enrichment
    Transform {
        /// Transformation expression
        expression: Expression,
    },

    /// Cache operation
    Cache {
        /// Read or write
        operation: CacheOperation,
        /// Cache key template
        key: String,
        /// TTL for writes
        ttl: Option<Duration>,
    },

    /// Retry/compensate
    Retry {
        /// Max attempts
        max_attempts: usize,
        /// Backoff strategy
        backoff: BackoffStrategy,
        /// Compensation node on final failure
        compensation: Option<NodeId>,
    },

    /// Human-in-the-loop
    HumanApproval {
        /// Approval timeout
        timeout: Duration,
        /// Fallback node on timeout
        fallback: Option<NodeId>,
        /// Approval UI template
        template: String,
    },

    /// Custom worker invocation
    CustomWorker {
        /// Target language
        language: Language,
        /// Handler function/class name
        handler: String,
    },
}
```

---

## Node Type Examples

### 1. LLM Call Node

```yaml
nodes:
  - id: generate_summary
    name: "Generate Summary"
    node_type:
      llm_call:
        provider: anthropic
        model: claude-3-sonnet-20240229
        temperature: 0.7
        max_tokens: 500
        stream: false

    config:
      prompt: |
        Summarize the following text in 3 bullet points:

        {{ input.text }}

      system: "You are a helpful summarization assistant."
```

**Generated CompletionRequest**:

```rust
CompletionRequest {
    model: "claude-3-sonnet-20240229",
    messages: vec![
        Message::system("You are a helpful summarization assistant."),
        Message::user("Summarize the following text...\n\n[user input]"),
    ],
    temperature: Some(0.7),
    max_tokens: Some(500),
    stream: Some(false),
    ..Default::default()
}
```

---

### 2. Switch Node (Conditional Branching)

```yaml
nodes:
  - id: route_by_sentiment
    name: "Route by Sentiment"
    node_type:
      switch:
        branches:
          - condition: '$.nodes.analyze.output.sentiment == "positive"'
            target: handle_positive
          - condition: '$.nodes.analyze.output.sentiment == "negative"'
            target: handle_negative
        default: handle_neutral

  - id: handle_positive
    node_type:
      transform:
        expression: '{"action": "celebrate", "input": $.input}'

  - id: handle_negative
    node_type:
      transform:
        expression: '{"action": "investigate", "input": $.input}'

  - id: handle_neutral
    node_type:
      transform:
        expression: '{"action": "monitor", "input": $.input}'
```

**Execution**:

```rust
impl WorkflowExecutor {
    async fn execute_switch(&self, branches: &[SwitchBranch], default: &Option<NodeId>, ctx: &ExecutionContext) -> Result<NodeOutput> {
        for branch in branches {
            let result = self.evaluator.evaluate(&branch.condition, ctx).await?;

            if result.as_bool().unwrap_or(false) {
                return self.execute_node(&branch.target, ctx).await;
            }
        }

        // No branch matched, use default
        if let Some(default_node) = default {
            return self.execute_node(default_node, ctx).await;
        }

        Err(SimpleAgentsError::NoMatchingBranch)
    }
}
```

---

### 3. Loop Node

```yaml
nodes:
  - id: paginate_results
    name: "Paginate API Results"
    node_type:
      loop:
        condition: '$.state.has_more == true'
        body: fetch_page
        max_iterations: 100

  - id: fetch_page
    node_type:
      custom_worker:
        language: python
        handler: FetchPage

edges:
  - from: fetch_page
    to: paginate_results  # Loop back
```

**Execution**:

```rust
async fn execute_loop(&self, condition: &Expression, body: &NodeId, max_iter: Option<usize>, ctx: &ExecutionContext) -> Result<NodeOutput> {
    let mut iteration = 0;
    let max = max_iter.unwrap_or(usize::MAX);
    let mut results = vec![];

    loop {
        // Check condition
        let should_continue = self.evaluator
            .evaluate(condition, ctx)
            .await?
            .as_bool()
            .unwrap_or(false);

        if !should_continue || iteration >= max {
            break;
        }

        // Execute body
        let output = self.execute_node(body, ctx).await?;
        results.push(output.value.clone());

        iteration += 1;
    }

    Ok(NodeOutput {
        value: json!({
            "iterations": iteration,
            "results": results,
        }),
        streaming: false,
    })
}
```

---

### 4. Parallel + Merge Nodes

```yaml
nodes:
  - id: parallel_analysis
    name: "Run Multiple Analyses"
    node_type:
      parallel:
        nodes:
          - sentiment_analysis
          - entity_extraction
          - topic_classification

  - id: sentiment_analysis
    node_type:
      llm_call:
        provider: openai
        model: gpt-4

  - id: entity_extraction
    node_type:
      llm_call:
        provider: anthropic
        model: claude-3-haiku-20240307

  - id: topic_classification
    node_type:
      custom_worker:
        language: python
        handler: ClassifyTopic

  - id: merge_results
    name: "Combine All Analyses"
    node_type:
      merge:
        policy:
          type: all  # Wait for all results
          timeout: 60s

edges:
  - from: parallel_analysis
    to: sentiment_analysis
  - from: parallel_analysis
    to: entity_extraction
  - from: parallel_analysis
    to: topic_classification
  - from: sentiment_analysis
    to: merge_results
  - from: entity_extraction
    to: merge_results
  - from: topic_classification
    to: merge_results
```

**Execution**:

```rust
async fn execute_parallel(&self, node_ids: &[NodeId], ctx: &ExecutionContext) -> Result<NodeOutput> {
    let tasks = node_ids.iter().map(|id| {
        let executor = self.clone();
        let ctx = ctx.clone();
        let id = id.clone();

        tokio::spawn(async move {
            executor.execute_node(&id, ctx).await
        })
    });

    let results = futures::future::try_join_all(tasks).await?;

    Ok(NodeOutput {
        value: json!(results.into_iter().map(|r| r.value).collect::<Vec<_>>()),
        streaming: false,
    })
}

async fn execute_merge(&self, policy: &MergePolicy, ctx: &ExecutionContext) -> Result<NodeOutput> {
    match policy {
        MergePolicy::All { timeout } => {
            // Wait for all inputs
            let results = self.collect_all_inputs(ctx, *timeout).await?;
            Ok(NodeOutput {
                value: json!(results),
                streaming: false,
            })
        }
        MergePolicy::First => {
            // Return first result
            let result = self.collect_first_input(ctx).await?;
            Ok(result)
        }
        MergePolicy::Quorum { count, timeout } => {
            // Wait for N results
            let results = self.collect_quorum_inputs(ctx, *count, *timeout).await?;
            Ok(NodeOutput {
                value: json!(results),
                streaming: false,
            })
        }
    }
}
```

---

### 5. Map/Reduce Nodes

```yaml
nodes:
  - id: process_items
    name: "Process Each Item"
    node_type:
      map:
        node_ref: analyze_item
        max_parallel: 5

  - id: analyze_item
    node_type:
      llm_call:
        provider: openai
        model: gpt-4-turbo
        prompt: "Analyze this item: {{ item }}"

  - id: aggregate_results
    name: "Aggregate All Results"
    node_type:
      reduce:
        node_ref: combine
        initial: {"total": 0, "items": []}

  - id: combine
    node_type:
      transform:
        expression: |
          {
            "total": accumulator.total + 1,
            "items": accumulator.items + [current]
          }
```

**Execution**:

```rust
async fn execute_map(&self, node_ref: &NodeId, max_parallel: Option<usize>, ctx: &ExecutionContext) -> Result<NodeOutput> {
    let items = ctx.input.as_array().ok_or(SimpleAgentsError::InvalidInput)?;
    let semaphore = Arc::new(Semaphore::new(max_parallel.unwrap_or(items.len())));

    let tasks = items.iter().map(|item| {
        let executor = self.clone();
        let ctx = ctx.clone_with_input(item.clone());
        let node_ref = node_ref.clone();
        let permit = semaphore.clone();

        tokio::spawn(async move {
            let _permit = permit.acquire().await.unwrap();
            executor.execute_node(&node_ref, ctx).await
        })
    });

    let results = futures::future::try_join_all(tasks).await?;

    Ok(NodeOutput {
        value: json!(results.into_iter().map(|r| r.value).collect::<Vec<_>>()),
        streaming: false,
    })
}

async fn execute_reduce(&self, node_ref: &NodeId, initial: &Value, ctx: &ExecutionContext) -> Result<NodeOutput> {
    let items = ctx.input.as_array().ok_or(SimpleAgentsError::InvalidInput)?;
    let mut accumulator = initial.clone();

    for item in items {
        let ctx = ctx.clone_with_values(json!({
            "accumulator": accumulator,
            "current": item,
        }));

        let result = self.execute_node(node_ref, ctx).await?;
        accumulator = result.value;
    }

    Ok(NodeOutput {
        value: accumulator,
        streaming: false,
    })
}
```

---

### 6. Subgraph Node

```yaml
# main-workflow.yaml
nodes:
  - id: preprocess
    node_type:
      subgraph:
        graph_ref: data-preprocessing-v1
        version: "^1.0.0"  # Semver constraint

  - id: analyze
    node_type:
      llm_call:
        provider: openai
        model: gpt-4

edges:
  - from: preprocess
    to: analyze
```

```yaml
# data-preprocessing-v1.yaml
id: data-preprocessing-v1
version: 1.2.0
entry_node: validate

nodes:
  - id: validate
    node_type:
      transform:
        expression: 'input.text != null'

  - id: normalize
    node_type:
      transform:
        expression: 'input.text.toLowerCase()'

edges:
  - from: validate
    to: normalize
```

---

### 7. Filter Node

```yaml
nodes:
  - id: check_confidence
    name: "Filter Low Confidence Results"
    node_type:
      filter:
        predicate: '$.nodes.analyze.output.confidence > 0.8'
        on_false: skip  # Skip downstream nodes

  - id: process_result
    node_type:
      transform:
        expression: '{"high_confidence": true, "result": $.input}'

edges:
  - from: check_confidence
    to: process_result  # Only executes if predicate is true
```

---

### 8. Batch Node

```yaml
nodes:
  - id: batch_requests
    name: "Batch API Requests"
    node_type:
      batch:
        size: 10
        timeout: 5s  # Or wait 5 seconds

  - id: process_batch
    node_type:
      custom_worker:
        language: python
        handler: ProcessBatch

edges:
  - from: batch_requests
    to: process_batch
```

---

### 9. Cache Node

```yaml
nodes:
  - id: check_cache
    name: "Check Cache"
    node_type:
      cache:
        operation: read
        key: "analysis:{{ input.id }}"

  - id: compute
    node_type:
      llm_call:
        provider: openai
        model: gpt-4

  - id: write_cache
    node_type:
      cache:
        operation: write
        key: "analysis:{{ input.id }}"
        ttl: 3600s  # 1 hour

edges:
  - from: check_cache
    to: compute
    condition: '$.nodes.check_cache.output == null'  # Only if cache miss
  - from: compute
    to: write_cache
```

---

### 10. Retry Node

```yaml
nodes:
  - id: retry_api_call
    name: "Retry External API"
    node_type:
      retry:
        max_attempts: 5
        backoff:
          type: exponential
          initial: 1s
          max: 60s
          multiplier: 2.0
          jitter: 0.3
        compensation: log_failure

  - id: log_failure
    node_type:
      custom_worker:
        language: python
        handler: LogFailure
```

---

### 11. Human Approval Node

```yaml
nodes:
  - id: require_approval
    name: "Require Human Approval"
    node_type:
      human_approval:
        timeout: 24h
        fallback: auto_reject
        template: |
          Please review this analysis:

          Sentiment: {{ $.nodes.analyze.output.sentiment }}
          Confidence: {{ $.nodes.analyze.output.confidence }}

          Approve or Reject?

  - id: auto_reject
    node_type:
      transform:
        expression: '{"approved": false, "reason": "timeout"}'

edges:
  - from: require_approval
    to: process_approved
    condition: '$.nodes.require_approval.output.approved == true'
```

---

### 12. Custom Worker Node

```yaml
nodes:
  - id: validate_data
    name: "Custom Python Validation"
    node_type:
      custom_worker:
        language: python
        handler: ValidateData

  - id: process_with_go
    name: "High-Performance Processing"
    node_type:
      custom_worker:
        language: go
        handler: ProcessData

  - id: format_output
    name: "Format with TypeScript"
    node_type:
      custom_worker:
        language: typescript
        handler: FormatOutput
```

**Python Handler**:

```python
# workers/python/handlers.py
class ValidateData:
    async def execute(self, input: dict, context: dict) -> dict:
        # Custom validation logic
        if "required_field" not in input:
            raise ValueError("Missing required_field")

        return {
            "valid": True,
            "normalized": input["required_field"].lower()
        }
```

**Go Handler**:

```go
// workers/go/handlers.go
type ProcessData struct{}

func (p *ProcessData) Execute(input map[string]interface{}, context map[string]interface{}) (map[string]interface{}, error) {
    // High-performance processing
    result := make(map[string]interface{})
    result["processed"] = true
    return result, nil
}
```

---

## EdgeDefinition

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeDefinition {
    /// Source node ID
    pub from: NodeId,

    /// Target node ID
    pub to: NodeId,

    /// Optional condition (CEL expression)
    pub condition: Option<Expression>,

    /// Whether this edge supports streaming
    pub streaming: bool,
}
```

**YAML Example**:

```yaml
edges:
  - from: analyze
    to: positive_handler
    condition: '$.nodes.analyze.output.sentiment == "positive"'
    streaming: false

  - from: analyze
    to: negative_handler
    condition: '$.nodes.analyze.output.sentiment == "negative"'
    streaming: false

  - from: stream_results
    to: consumer
    streaming: true  # Progressive chunks
```

---

## Complete Example Workflow

```yaml
id: customer-feedback-analysis
version: 1.0.0

metadata:
  name: "Customer Feedback Analysis Pipeline"
  description: "Analyzes customer feedback with sentiment analysis and routing"
  author: "team@example.com"
  tags: ["customer", "feedback", "nlp"]

entry_node: fetch_feedback

defaults:
  timeout: 30s
  max_concurrency: 10
  retry:
    max_attempts: 3
    backoff:
      type: exponential
      initial: 1s

capabilities:
  - id: llm_access
    allowed_models: ["gpt-4", "claude-3-sonnet"]

nodes:
  # Fetch feedback from API
  - id: fetch_feedback
    name: "Fetch Customer Feedback"
    node_type:
      custom_worker:
        language: python
        handler: FetchFeedback

  # Batch feedback for processing
  - id: batch_feedback
    node_type:
      batch:
        size: 10
        timeout: 5s

  # Map: Analyze each feedback item
  - id: analyze_each
    node_type:
      map:
        node_ref: analyze_sentiment
        max_parallel: 5

  # LLM analysis
  - id: analyze_sentiment
    name: "Analyze Sentiment"
    node_type:
      llm_call:
        provider: openai
        model: gpt-4
        temperature: 0.7

    config:
      prompt: |
        Analyze the sentiment of this customer feedback:

        {{ item.text }}

        Respond with JSON:
        {
          "sentiment": "positive|negative|neutral",
          "confidence": 0.0-1.0,
          "key_topics": ["topic1", "topic2"],
          "urgency": "low|medium|high"
        }

    output_schema:
      type: object
      required: ["sentiment", "confidence", "urgency"]
      properties:
        sentiment:
          type: string
        confidence:
          type: number
        urgency:
          type: string

  # Filter: Only high-confidence results
  - id: filter_confident
    node_type:
      filter:
        predicate: '$.nodes.analyze_each.output[*].confidence > 0.8'
        on_false: skip

  # Switch: Route by urgency
  - id: route_by_urgency
    node_type:
      switch:
        branches:
          - condition: '$.nodes.analyze_each.output[*].urgency == "high"'
            target: handle_urgent
          - condition: '$.nodes.analyze_each.output[*].urgency == "medium"'
            target: handle_medium
        default: handle_low

  # Urgent handler
  - id: handle_urgent
    node_type:
      parallel:
        nodes:
          - notify_team
          - create_ticket
          - send_acknowledgment

  - id: notify_team
    node_type:
      custom_worker:
        language: python
        handler: NotifySlack

  - id: create_ticket
    node_type:
      custom_worker:
        language: go
        handler: CreateJiraTicket

  - id: send_acknowledgment
    node_type:
      custom_worker:
        language: typescript
        handler: SendEmail

  # Medium handler
  - id: handle_medium
    node_type:
      transform:
        expression: '{"action": "queue", "priority": "medium"}'

  # Low handler
  - id: handle_low
    node_type:
      transform:
        expression: '{"action": "log", "priority": "low"}'

edges:
  - from: fetch_feedback
    to: batch_feedback

  - from: batch_feedback
    to: analyze_each

  - from: analyze_each
    to: filter_confident

  - from: filter_confident
    to: route_by_urgency

  - from: route_by_urgency
    to: handle_urgent
    condition: 'routing_decision == "urgent"'

  - from: route_by_urgency
    to: handle_medium
    condition: 'routing_decision == "medium"'

  - from: route_by_urgency
    to: handle_low
    condition: 'routing_decision == "low"'
```

## JSON Schema for IR

Complete JSON Schema for validation:

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "title": "WorkflowGraph",
  "type": "object",
  "required": ["id", "version", "entry_node", "nodes", "edges"],
  "properties": {
    "id": {
      "type": "string",
      "pattern": "^[a-z0-9-]+$"
    },
    "version": {
      "type": "string",
      "pattern": "^\\d+\\.\\d+\\.\\d+$"
    },
    "metadata": {
      "type": "object",
      "properties": {
        "name": {"type": "string"},
        "description": {"type": "string"},
        "author": {"type": "string"},
        "tags": {
          "type": "array",
          "items": {"type": "string"}
        }
      }
    },
    "entry_node": {"type": "string"},
    "nodes": {
      "type": "array",
      "items": {"$ref": "#/definitions/NodeDefinition"}
    },
    "edges": {
      "type": "array",
      "items": {"$ref": "#/definitions/EdgeDefinition"}
    }
  },
  "definitions": {
    "NodeDefinition": {
      "type": "object",
      "required": ["id", "node_type"],
      "properties": {
        "id": {"type": "string"},
        "name": {"type": "string"},
        "node_type": {"$ref": "#/definitions/NodeType"},
        "input_schema": {"type": "object"},
        "output_schema": {"type": "object"},
        "timeout": {"type": "string"},
        "required_capabilities": {
          "type": "array",
          "items": {"type": "string"}
        }
      }
    },
    "NodeType": {
      "oneOf": [
        {
          "type": "object",
          "required": ["llm_call"],
          "properties": {
            "llm_call": {
              "type": "object",
              "required": ["provider", "model"],
              "properties": {
                "provider": {"type": "string"},
                "model": {"type": "string"},
                "temperature": {"type": "number"},
                "max_tokens": {"type": "integer"}
              }
            }
          }
        }
        // ... other node types
      ]
    },
    "EdgeDefinition": {
      "type": "object",
      "required": ["from", "to"],
      "properties": {
        "from": {"type": "string"},
        "to": {"type": "string"},
        "condition": {"type": "string"},
        "streaming": {"type": "boolean"}
      }
    }
  }
}
```
