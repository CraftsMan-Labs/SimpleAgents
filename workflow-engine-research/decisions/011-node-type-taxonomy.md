# ADR-011: Node Type Taxonomy

## Status
Accepted

## Context
Workflow engines need a comprehensive set of node types to support diverse use cases. The taxonomy must:
- **Cover common patterns**: LLM calls, branching, loops, parallel execution, data transformation
- **Enable composition**: Complex workflows from simple building blocks
- **Remain extensible**: Support custom nodes via worker protocol
- **Balance complexity**: Not too many types (overwhelming), not too few (limiting)
- **Support streaming**: First-class streaming for real-time responses
- **Provide error handling**: Retry, compensation, and failure recovery

Requirements:
- **Core execution**: LLM calls, custom workers, transformations
- **Control flow**: Branching (switch), loops, conditionals
- **Concurrency**: Parallel execution, map/reduce
- **Integration**: Subgraphs, caching, human approval
- **Reliability**: Retry, timeout, circuit breakers
- **Data flow**: Filter, batch, merge

We evaluated 30+ node types and narrowed to **15 essential types** that cover 95% of use cases while remaining learnable.

## Decision
Support **15 node types** organized into 5 categories:

**1. Execution Nodes** (3):
- **LlmCall**: Call LLM provider (OpenAI, Anthropic, etc.)
- **CustomWorker**: Execute language-specific handler (Python, Go, TypeScript, Rust)
- **Transform**: JSON transformation via CEL expressions

**2. Control Flow Nodes** (3):
- **Switch**: Conditional branching (if/else if/else)
- **Loop**: Iterate while condition is true
- **Filter**: Guard/predicate (skip downstream if condition fails)

**3. Concurrency Nodes** (4):
- **Parallel**: Fan-out (execute multiple branches concurrently)
- **Merge**: Fan-in (combine multiple inputs with policy: all/first/quorum)
- **Map**: Apply node to each item in collection (parallelizable)
- **Reduce**: Aggregate collection to single value (sequential)

**4. Composition Nodes** (2):
- **Subgraph**: Invoke another workflow graph
- **Batch**: Collect N items or wait timeout before processing

**5. Reliability & Integration Nodes** (3):
- **Cache**: Read/write cache for memoization
- **Retry**: Retry with backoff and optional compensation
- **HumanApproval**: Pause for human input with timeout and fallback

All 15 types support:
- Input/output schema validation (JSON Schema)
- Timeout configuration
- Capability requirements
- Streaming (where applicable)
- Metadata tracking

## Alternatives Considered

### 1. **Minimal Set (5 Node Types Only)**
- **Types**: LlmCall, CustomWorker, Switch, Parallel, Subgraph
- **Pros**:
  - Simple to learn
  - Easy to implement
  - Clear mental model
- **Cons**:
  - Forces custom workers for common patterns (loops, map/reduce)
  - Poor ergonomics for common use cases
  - More complexity in worker code
  - Harder to optimize (engine doesn't know intent)
- **Rejected**: Too restrictive; users need built-in loop/map/reduce

### 2. **Comprehensive Set (30+ Node Types)**
- **Types**: All possible variations (ForEach, While, DoWhile, MapParallel, MapSequential, etc.)
- **Pros**:
  - Every use case has dedicated node
  - Maximum expressiveness
  - No ambiguity
- **Cons**:
  - Overwhelming for users
  - High maintenance burden
  - Overlapping functionality
  - Harder to document
- **Rejected**: Diminishing returns; 15 types cover 95% of use cases

### 3. **Temporal-Style Activities (Generic Execute Node)**
- **Types**: Only one "Activity" node; all logic in workers
- **Pros**:
  - Ultimate flexibility
  - Simple core engine
  - Easy to extend
- **Cons**:
  - No declarative control flow
  - Hard to visualize workflows
  - Can't optimize or analyze workflow structure
  - Poor portability (logic in code, not IR)
- **Rejected**: Want declarative workflows, not imperative code

### 4. **AWS Step Functions Model**
- **Types**: Task, Choice, Parallel, Map, Pass, Wait, Succeed, Fail
- **Pros**:
  - Proven model
  - Good balance of types
  - Well-documented
- **Cons**:
  - No built-in LLM node (everything is Task)
  - Missing loop construct (uses recursion)
  - No streaming support
  - Limited transformation (Pass node too simple)
- **Rejected**: Good inspiration but missing LLM-specific features

### 5. **Apache Airflow Model (Operators)**
- **Types**: 100s of specialized operators
- **Pros**:
  - Rich ecosystem
  - Every integration has operator
- **Cons**:
  - Too many types to learn
  - High coupling to specific tools
  - Not portable across environments
  - Maintenance nightmare
- **Rejected**: Want small, composable set of primitives

## Consequences

### Positive
- **Coverage**: 15 types cover 95% of workflows
- **Learnability**: Small enough to master in a day
- **Composability**: Complex workflows from simple building blocks
- **Optimization**: Engine can optimize known patterns (map, parallel, cache)
- **Visualization**: Clear semantics for visual editors
- **Portability**: Declarative workflows work across environments

### Negative
- **Custom nodes still needed**: Some use cases require CustomWorker
- **Potential overlap**: Multiple ways to achieve same result (e.g., Switch vs Filter)
- **Learning curve**: 15 types is not trivial
- **API surface**: More types means more API to document and test
- **Version skew**: Adding new types requires IR schema updates

## Implementation Notes

### Complete Node Type Taxonomy

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum NodeType {
    // ========== EXECUTION NODES ==========

    /// Call LLM provider
    LlmCall {
        provider: String,  // "openai", "anthropic", "google", etc.
        model: String,
        #[serde(default)]
        temperature: Option<f32>,
        #[serde(default)]
        max_tokens: Option<u32>,
        #[serde(default)]
        stream: bool,
        #[serde(default)]
        prompt: Option<String>,  // Template with {{ }} placeholders
    },

    /// Execute custom language-specific handler
    CustomWorker {
        language: Language,  // Python, Go, TypeScript, Rust
        handler: String,     // Function/class name
    },

    /// Transform data via CEL expression
    Transform {
        expression: Expression,  // CEL expression for transformation
    },

    // ========== CONTROL FLOW NODES ==========

    /// Conditional branching (if/else if/else)
    Switch {
        branches: Vec<SwitchBranch>,
        default: Option<NodeId>,
    },

    /// Loop while condition is true
    Loop {
        condition: Expression,
        body: NodeId,
        max_iterations: Option<usize>,  // Safety limit
    },

    /// Filter/guard node
    Filter {
        predicate: Expression,
        on_false: FilterAction,  // Skip, Error, DefaultValue
    },

    // ========== CONCURRENCY NODES ==========

    /// Parallel fan-out
    Parallel {
        nodes: Vec<NodeId>,
    },

    /// Merge/join with policy
    Merge {
        policy: MergePolicy,  // All, First, Quorum
    },

    /// Map over collection
    Map {
        node_ref: NodeId,
        max_parallel: Option<usize>,  // Concurrency limit
    },

    /// Reduce collection
    Reduce {
        node_ref: NodeId,
        initial: Value,  // Initial accumulator value
    },

    // ========== COMPOSITION NODES ==========

    /// Invoke subgraph
    Subgraph {
        graph_ref: GraphId,
        version: Option<String>,  // Semver constraint (e.g., "^1.0.0")
    },

    /// Batch/window
    Batch {
        size: usize,
        timeout: Option<Duration>,  // Or wait this long
    },

    // ========== RELIABILITY & INTEGRATION NODES ==========

    /// Cache read/write
    Cache {
        operation: CacheOperation,  // Read, Write, ReadOrCompute
        key: String,                // Template for cache key
        ttl: Option<Duration>,      // TTL for writes
    },

    /// Retry with backoff
    Retry {
        max_attempts: usize,
        backoff: BackoffStrategy,
        compensation: Option<NodeId>,  // On final failure
    },

    /// Human-in-the-loop approval
    HumanApproval {
        timeout: Duration,
        fallback: Option<NodeId>,  // On timeout
        template: String,           // Approval UI template
    },
}

// ========== SUPPORTING TYPES ==========

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwitchBranch {
    pub condition: Expression,
    pub target: NodeId,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Language {
    Python,
    Go,
    TypeScript,
    Rust,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FilterAction {
    Skip,           // Skip downstream nodes
    Error,          // Raise error
    DefaultValue(Value),  // Return default value
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MergePolicy {
    All { timeout: Option<Duration> },
    First,
    Quorum { count: usize, timeout: Option<Duration> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CacheOperation {
    Read,
    Write,
    ReadOrCompute,  // Cache-aside pattern
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackoffStrategy {
    Constant { delay: Duration },
    Linear { initial: Duration, increment: Duration },
    Exponential { initial: Duration, multiplier: f64, max: Duration },
}
```

### Node Type Examples

#### 1. LlmCall - LLM Invocation

```yaml
nodes:
  - id: generate_summary
    name: "Generate Summary"
    node_type:
      llm_call:
        provider: openai
        model: gpt-4-turbo
        temperature: 0.7
        max_tokens: 500
        stream: true
        prompt: |
          Summarize this article in 3 bullet points:

          {{ input.article }}

    output_schema:
      type: object
      properties:
        summary:
          type: array
          items:
            type: string
```

**Use cases**: Text generation, classification, extraction, translation, summarization

---

#### 2. CustomWorker - External Function

```yaml
nodes:
  - id: validate_email
    node_type:
      custom_worker:
        language: python
        handler: ValidateEmail

  - id: process_payment
    node_type:
      custom_worker:
        language: go
        handler: ProcessPayment
```

**Use cases**: Business logic, external API calls, database operations, custom validation

---

#### 3. Transform - Data Transformation

```yaml
nodes:
  - id: extract_fields
    node_type:
      transform:
        expression: |
          {
            "name": input.user.name,
            "email": input.user.email,
            "timestamp": timestamp()
          }

  - id: calculate_total
    node_type:
      transform:
        expression: "input.items.sum(i, i.price * i.quantity)"
```

**Use cases**: Data mapping, enrichment, filtering, normalization, calculations

---

#### 4. Switch - Conditional Routing

```yaml
nodes:
  - id: route_by_priority
    node_type:
      switch:
        branches:
          - condition: 'input.priority == "urgent"'
            target: handle_urgent
          - condition: 'input.priority == "high"'
            target: handle_high
          - condition: 'input.priority == "medium"'
            target: handle_medium
        default: handle_low
```

**Use cases**: Routing, decision trees, A/B testing, feature flags

---

#### 5. Loop - Iteration

```yaml
nodes:
  - id: paginate
    node_type:
      loop:
        condition: '$.state.has_more && $.state.page < 100'
        body: fetch_page
        max_iterations: 100

  - id: retry_until_success
    node_type:
      loop:
        condition: '!$.nodes.check_status.output.success'
        body: attempt_operation
        max_iterations: 5
```

**Use cases**: Pagination, polling, retry loops, batch processing

---

#### 6. Filter - Conditional Execution

```yaml
nodes:
  - id: require_confidence
    node_type:
      filter:
        predicate: '$.nodes.analyze.output.confidence > 0.8'
        on_false: skip

  - id: validate_input
    node_type:
      filter:
        predicate: 'input.email.contains("@")'
        on_false: error
```

**Use cases**: Validation, guards, quality checks, short-circuit evaluation

---

#### 7. Parallel - Concurrent Execution

```yaml
nodes:
  - id: analyze_all
    node_type:
      parallel:
        nodes:
          - sentiment_analysis
          - entity_extraction
          - topic_classification
          - keyword_extraction
```

**Use cases**: Parallel API calls, concurrent LLM analysis, fan-out processing

---

#### 8. Merge - Synchronization

```yaml
nodes:
  - id: wait_for_all
    node_type:
      merge:
        policy:
          all:
            timeout: 60s

  - id: first_to_respond
    node_type:
      merge:
        policy: first

  - id: wait_for_quorum
    node_type:
      merge:
        policy:
          quorum:
            count: 3
            timeout: 30s
```

**Use cases**: Fan-in, consensus, race conditions, redundant calls

---

#### 9. Map - Collection Processing

```yaml
nodes:
  - id: process_reviews
    node_type:
      map:
        node_ref: analyze_review
        max_parallel: 5  # Process 5 at a time

  - id: translate_paragraphs
    node_type:
      map:
        node_ref: translate_text
        max_parallel: 10
```

**Use cases**: Batch processing, parallel transformations, data pipelines

---

#### 10. Reduce - Aggregation

```yaml
nodes:
  - id: aggregate_scores
    node_type:
      reduce:
        node_ref: combine_score
        initial: {"total": 0, "count": 0}

  - id: concatenate_summaries
    node_type:
      reduce:
        node_ref: merge_summary
        initial: ""
```

**Use cases**: Aggregation, accumulation, summarization, reporting

---

#### 11. Subgraph - Workflow Composition

```yaml
nodes:
  - id: preprocess
    node_type:
      subgraph:
        graph_ref: data-preprocessing-v1
        version: "^1.0.0"

  - id: validate
    node_type:
      subgraph:
        graph_ref: input-validation
        version: "^2.0.0"
```

**Use cases**: Reusable workflows, modular design, versioning, isolation

---

#### 12. Batch - Windowing

```yaml
nodes:
  - id: batch_requests
    node_type:
      batch:
        size: 10
        timeout: 5s  # Or wait 5 seconds

  - id: daily_rollup
    node_type:
      batch:
        size: 1000
        timeout: 24h
```

**Use cases**: API rate limiting, batching for efficiency, windowing

---

#### 13. Cache - Memoization

```yaml
nodes:
  - id: check_cache
    node_type:
      cache:
        operation: read
        key: "analysis:{{ input.id }}"

  - id: write_cache
    node_type:
      cache:
        operation: write
        key: "analysis:{{ input.id }}"
        ttl: 3600s

  - id: cached_computation
    node_type:
      cache:
        operation: read_or_compute
        key: "result:{{ input.query }}"
        ttl: 1800s
```

**Use cases**: Performance optimization, cost reduction, deduplication

---

#### 14. Retry - Fault Tolerance

```yaml
nodes:
  - id: reliable_api_call
    node_type:
      retry:
        max_attempts: 5
        backoff:
          exponential:
            initial: 1s
            multiplier: 2.0
            max: 60s
        compensation: log_failure

  - id: transient_failure_handler
    node_type:
      retry:
        max_attempts: 3
        backoff:
          constant:
            delay: 2s
```

**Use cases**: External API reliability, transient errors, fault tolerance

---

#### 15. HumanApproval - Human-in-the-Loop

```yaml
nodes:
  - id: require_review
    node_type:
      human_approval:
        timeout: 24h
        fallback: auto_reject
        template: |
          Please review this analysis:

          Sentiment: {{ $.nodes.analyze.output.sentiment }}
          Confidence: {{ $.nodes.analyze.output.confidence }}

          Approve or Reject?

  - id: manual_override
    node_type:
      human_approval:
        timeout: 1h
        fallback: use_default
```

**Use cases**: Approval workflows, content moderation, compliance, overrides

---

### Extensibility Model

New node types can be added in two ways:

**1. CustomWorker (Recommended for Most Cases)**

```yaml
nodes:
  - id: my_custom_logic
    node_type:
      custom_worker:
        language: python
        handler: MyCustomHandler
```

**2. Core Engine Extension (For Widely-Used Patterns)**

```rust
// Add to NodeType enum
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NodeType {
    // ... existing types

    /// Debounce/throttle
    Debounce {
        delay: Duration,
        strategy: DebounceStrategy,  // Leading, Trailing, Both
    },
}
```

Criteria for core node type:
- Used in >20% of workflows
- Has clear execution semantics
- Benefits from engine optimization
- Can't be easily implemented with CustomWorker

---

### Complete Workflow Example

```yaml
id: customer-support-pipeline
version: 1.0.0

nodes:
  # 1. Fetch tickets (CustomWorker)
  - id: fetch_tickets
    node_type:
      custom_worker:
        language: python
        handler: FetchZendeskTickets

  # 2. Batch tickets
  - id: batch_tickets
    node_type:
      batch:
        size: 10
        timeout: 5s

  # 3. Map: Analyze each ticket
  - id: analyze_each
    node_type:
      map:
        node_ref: analyze_ticket
        max_parallel: 5

  # 4. LLM analysis
  - id: analyze_ticket
    node_type:
      llm_call:
        provider: openai
        model: gpt-4
        prompt: |
          Analyze this support ticket:
          {{ item.description }}

          Classify urgency, sentiment, and topic.

  # 5. Filter high-confidence results
  - id: filter_confident
    node_type:
      filter:
        predicate: '$.nodes.analyze_each.output[*].confidence > 0.8'
        on_false: skip

  # 6. Switch by urgency
  - id: route_by_urgency
    node_type:
      switch:
        branches:
          - condition: 'urgency == "critical"'
            target: handle_critical
        default: handle_normal

  # 7. Parallel handling for critical
  - id: handle_critical
    node_type:
      parallel:
        nodes:
          - notify_oncall
          - create_incident
          - send_sms

  # 8. Human approval for escalation
  - id: require_approval
    node_type:
      human_approval:
        timeout: 2h
        fallback: auto_approve
        template: "Approve escalation?"

  # 9. Retry API call
  - id: update_ticket
    node_type:
      retry:
        max_attempts: 3
        backoff:
          exponential:
            initial: 1s
            multiplier: 2.0
            max: 30s

  # 10. Cache results
  - id: cache_analysis
    node_type:
      cache:
        operation: write
        key: "ticket:{{ input.id }}"
        ttl: 3600s

  # 11. Transform for output
  - id: format_response
    node_type:
      transform:
        expression: |
          {
            "processed": $.nodes.analyze_each.output.size(),
            "critical": $.nodes.handle_critical.output.size(),
            "timestamp": timestamp()
          }
```

---

## Related Decisions
- ADR-001: Canonical IR Format (YAML/JSON)
- ADR-002: CEL Expression Language
- ADR-006: Code DSL alongside YAML/JSON
- ADR-009: Streaming Model

## Future Enhancements
- **Router/Selector**: Choose provider/model based on policy (cost, latency, availability)
- **EventTrigger**: Start workflows from cron/webhook/queue
- **Debounce/Throttle**: Rate limiting patterns
- **CircuitBreaker**: Automatic failure detection and recovery
- **Saga**: Distributed transaction coordination
- **WaitForEvent**: Pause until external event arrives
