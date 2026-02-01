# Execution Model

## Overview

The workflow engine executes DAGs (Directed Acyclic Graphs) using a topological ordering strategy with support for loops, branches, and parallel execution. The execution model ensures deterministic behavior where possible while handling LLM non-determinism through trace recording.

## Core Execution Algorithm

### Topological Execution

```rust
pub struct WorkflowExecutor {
    graph: Arc<WorkflowGraph>,
    state: Arc<StateManager>,
    evaluator: Arc<dyn ExpressionEvaluator>,
    workers: Arc<WorkerPool>,
    agents_client: Arc<SimpleAgentsClient>,
    tracer: Arc<TraceRecorder>,
    semaphore: Arc<Semaphore>,
}

impl WorkflowExecutor {
    /// Execute workflow from entry node
    pub async fn execute(&self, input: Value) -> Result<WorkflowResult> {
        // 1. Initialize execution context
        let exec_id = ExecutionId::new();
        let ctx = ExecutionContext {
            execution_id: exec_id.clone(),
            graph_id: self.graph.id.clone(),
            input: input.clone(),
            state: self.state.clone(),
            node_outputs: Arc::new(RwLock::new(HashMap::new())),
            started_at: Utc::now(),
        };

        // 2. Record trace start
        self.tracer.record_start(&ctx).await?;

        // 3. Execute from entry node
        let result = match self.execute_node(&self.graph.entry_node, ctx.clone()).await {
            Ok(output) => {
                WorkflowResult {
                    execution_id: exec_id,
                    success: true,
                    output: Some(output.value),
                    error: None,
                    duration: Utc::now().signed_duration_since(ctx.started_at),
                }
            }
            Err(e) => {
                WorkflowResult {
                    execution_id: exec_id,
                    success: false,
                    output: None,
                    error: Some(e.to_string()),
                    duration: Utc::now().signed_duration_since(ctx.started_at),
                }
            }
        };

        // 4. Record trace completion
        self.tracer.record_completion(&ctx, &result).await?;

        Ok(result)
    }

    /// Execute single node with full lifecycle
    async fn execute_node(
        &self,
        node_id: &NodeId,
        ctx: ExecutionContext,
    ) -> Result<NodeOutput> {
        let node = self.graph.nodes.get(node_id)
            .ok_or_else(|| SimpleAgentsError::NodeNotFound(node_id.clone()))?;

        // 1. Check capabilities
        self.state.check_capabilities(&ctx, &node.required_capabilities)?;

        // 2. Validate input schema
        if let Some(schema) = &node.input_schema {
            self.validate_schema(&ctx.input, schema)?;
        }

        // 3. Acquire concurrency permit
        let _permit = self.semaphore.acquire().await
            .map_err(|_| SimpleAgentsError::ConcurrencyLimitReached)?;

        // 4. Execute based on node type
        let start = Instant::now();
        let output = self.execute_node_type(&node.node_type, &ctx).await?;
        let duration = start.elapsed();

        // 5. Validate output schema
        if let Some(schema) = &node.output_schema {
            self.validate_schema(&output.value, schema)?;
        }

        // 6. Store output for downstream references
        {
            let mut outputs = ctx.node_outputs.write().await;
            outputs.insert(node_id.clone(), output.value.clone());
        }

        // 7. Record trace event
        self.tracer.record_node_execution(
            &ctx,
            node_id,
            &ctx.input,
            &output,
            duration,
        ).await?;

        // 8. Follow outgoing edges
        self.execute_edges(node_id, output, &ctx).await
    }

    /// Execute edges from a node
    async fn execute_edges(
        &self,
        from: &NodeId,
        output: NodeOutput,
        ctx: &ExecutionContext,
    ) -> Result<NodeOutput> {
        // Find all outgoing edges
        let edges: Vec<_> = self.graph.edges
            .iter()
            .filter(|e| &e.from == from)
            .collect();

        if edges.is_empty() {
            // Terminal node
            return Ok(output);
        }

        // Evaluate edge conditions
        for edge in edges {
            // Check condition if present
            let should_traverse = if let Some(condition) = &edge.condition {
                let eval_ctx = EvaluationContext {
                    state: self.state.get_all(&ctx.node_id).await?,
                    node_outputs: ctx.node_outputs.read().await.clone(),
                };

                self.evaluator.evaluate(condition, &eval_ctx).await?
                    .as_bool()
                    .unwrap_or(false)
            } else {
                true
            };

            if should_traverse {
                // Create new context with output as input
                let next_ctx = ExecutionContext {
                    input: output.value.clone(),
                    ..ctx.clone()
                };

                return self.execute_node(&edge.to, next_ctx).await;
            }
        }

        // No edge matched
        Ok(output)
    }
}
```

## Node Type Execution

### LLM Call Node

```rust
async fn execute_llm_call(
    &self,
    provider: &str,
    model: &str,
    ctx: &ExecutionContext,
) -> Result<NodeOutput> {
    // 1. Build CompletionRequest from context
    let messages = ctx.get_messages()?;

    let request = CompletionRequest::builder()
        .model(model)
        .messages(messages)
        .temperature(ctx.node.config.get("temperature").and_then(|v| v.as_f64()).map(|f| f as f32))
        .max_tokens(ctx.node.config.get("max_tokens").and_then(|v| v.as_u64()).map(|u| u as u32))
        .build()?;

    // 2. Execute via existing SimpleAgentsClient
    // This automatically uses routing, caching, healing, streaming
    let response = self.agents_client.complete(&request).await?;

    // 3. Extract output
    let output_value = if let Some(content) = response.content() {
        // Try to parse as JSON first
        serde_json::from_str(content).unwrap_or_else(|_| json!({"content": content}))
    } else {
        json!(null)
    };

    Ok(NodeOutput {
        value: output_value,
        streaming: false,
        metadata: Some(NodeMetadata {
            provider: response.provider.clone(),
            model: Some(response.model.clone()),
            tokens: Some(response.usage.total_tokens),
            latency: None,
        }),
    })
}
```

### Switch Node (Branching)

```rust
async fn execute_switch(
    &self,
    branches: &[SwitchBranch],
    default: &Option<NodeId>,
    ctx: &ExecutionContext,
) -> Result<NodeOutput> {
    let eval_ctx = EvaluationContext {
        state: self.state.get_all(&ctx.node_id).await?,
        node_outputs: ctx.node_outputs.read().await.clone(),
    };

    // Evaluate branches in order
    for branch in branches {
        let result = self.evaluator.evaluate(&branch.condition, &eval_ctx).await?;

        if result.as_bool().unwrap_or(false) {
            // Record decision
            self.tracer.record_decision(&ctx, "switch", &branch.target).await?;

            // Execute target branch
            return self.execute_node(&branch.target, ctx.clone()).await;
        }
    }

    // No branch matched
    if let Some(default_node) = default {
        self.tracer.record_decision(&ctx, "switch", default_node).await?;
        return self.execute_node(default_node, ctx.clone()).await;
    }

    Err(SimpleAgentsError::NoMatchingBranch)
}
```

### Loop Node

```rust
async fn execute_loop(
    &self,
    condition: &Expression,
    body: &NodeId,
    max_iterations: Option<usize>,
    ctx: &ExecutionContext,
) -> Result<NodeOutput> {
    let mut iteration = 0;
    let max = max_iterations.unwrap_or(1000); // Safety limit
    let mut results = vec![];

    loop {
        // Evaluate condition
        let eval_ctx = EvaluationContext {
            state: self.state.get_all(&ctx.node_id).await?,
            node_outputs: ctx.node_outputs.read().await.clone(),
        };

        let should_continue = self.evaluator
            .evaluate(condition, &eval_ctx)
            .await?
            .as_bool()
            .unwrap_or(false);

        if !should_continue || iteration >= max {
            break;
        }

        // Record iteration
        self.tracer.record_loop_iteration(&ctx, iteration).await?;

        // Execute body
        let output = self.execute_node(body, ctx.clone()).await?;
        results.push(output.value.clone());

        // Update state for next iteration
        self.state.set(&ctx.node_id, "iteration", json!(iteration)).await?;
        self.state.set(&ctx.node_id, "last_result", output.value.clone()).await?;

        iteration += 1;
    }

    Ok(NodeOutput {
        value: json!({
            "iterations": iteration,
            "results": results,
        }),
        streaming: false,
        metadata: None,
    })
}
```

### Parallel Node (Fan-out)

```rust
async fn execute_parallel(
    &self,
    node_ids: &[NodeId],
    ctx: &ExecutionContext,
) -> Result<NodeOutput> {
    // Spawn concurrent tasks
    let tasks: Vec<_> = node_ids
        .iter()
        .map(|id| {
            let executor = Arc::new(self.clone());
            let ctx = ctx.clone();
            let id = id.clone();

            tokio::spawn(async move {
                executor.execute_node(&id, ctx).await
            })
        })
        .collect();

    // Wait for all to complete
    let results = futures::future::try_join_all(tasks)
        .await
        .map_err(|e| SimpleAgentsError::Internal(format!("Task join error: {}", e)))?;

    // Collect outputs
    let outputs: Vec<_> = results.into_iter()
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .map(|output| output.value)
        .collect();

    Ok(NodeOutput {
        value: json!(outputs),
        streaming: false,
        metadata: None,
    })
}
```

### Merge Node (Fan-in)

```rust
pub enum MergePolicy {
    /// Wait for all inputs
    All { timeout: Option<Duration> },
    /// Return first result
    First,
    /// Wait for quorum
    Quorum { count: usize, timeout: Option<Duration> },
}

async fn execute_merge(
    &self,
    policy: &MergePolicy,
    ctx: &ExecutionContext,
) -> Result<NodeOutput> {
    match policy {
        MergePolicy::All { timeout } => {
            // Collect all incoming edge results
            let results = self.collect_all_inputs(ctx, *timeout).await?;

            Ok(NodeOutput {
                value: json!(results),
                streaming: false,
                metadata: None,
            })
        }

        MergePolicy::First => {
            // Return first result that arrives
            let result = self.collect_first_input(ctx).await?;

            Ok(result)
        }

        MergePolicy::Quorum { count, timeout } => {
            // Wait for N results
            let results = self.collect_quorum_inputs(ctx, *count, *timeout).await?;

            Ok(NodeOutput {
                value: json!(results),
                streaming: false,
                metadata: None,
            })
        }
    }
}

async fn collect_all_inputs(
    &self,
    ctx: &ExecutionContext,
    timeout: Option<Duration>,
) -> Result<Vec<Value>> {
    let incoming_edges: Vec<_> = self.graph.edges
        .iter()
        .filter(|e| &e.to == &ctx.node_id)
        .collect();

    let expected_count = incoming_edges.len();
    let mut results = vec![];

    let deadline = timeout.map(|d| Instant::now() + d);

    while results.len() < expected_count {
        // Check timeout
        if let Some(deadline) = deadline {
            if Instant::now() > deadline {
                return Err(SimpleAgentsError::MergeTimeout);
            }
        }

        // Check for new results (simplified - in reality use channels)
        let outputs = ctx.node_outputs.read().await;
        for edge in &incoming_edges {
            if let Some(output) = outputs.get(&edge.from) {
                if !results.contains(output) {
                    results.push(output.clone());
                }
            }
        }

        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    Ok(results)
}
```

### Map Node

```rust
async fn execute_map(
    &self,
    node_ref: &NodeId,
    max_parallel: Option<usize>,
    ctx: &ExecutionContext,
) -> Result<NodeOutput> {
    // Input must be array
    let items = ctx.input.as_array()
        .ok_or(SimpleAgentsError::InvalidInput("Map node requires array input".into()))?;

    let semaphore = Arc::new(Semaphore::new(max_parallel.unwrap_or(items.len())));

    // Map over items with concurrency control
    let tasks = items.iter().enumerate().map(|(index, item)| {
        let executor = Arc::new(self.clone());
        let permit = semaphore.clone();
        let node_ref = node_ref.clone();

        // Create context with single item
        let item_ctx = ExecutionContext {
            input: item.clone(),
            ..ctx.clone()
        };

        tokio::spawn(async move {
            let _permit = permit.acquire().await.unwrap();

            let result = executor.execute_node(&node_ref, item_ctx).await;

            (index, result)
        })
    });

    // Collect results in order
    let mut results: Vec<Option<NodeOutput>> = vec![None; items.len()];

    for task in tasks {
        let (index, result) = task.await
            .map_err(|e| SimpleAgentsError::Internal(format!("Task error: {}", e)))?;

        results[index] = Some(result?);
    }

    let outputs: Vec<_> = results.into_iter()
        .map(|r| r.unwrap().value)
        .collect();

    Ok(NodeOutput {
        value: json!(outputs),
        streaming: false,
        metadata: None,
    })
}
```

### Reduce Node

```rust
async fn execute_reduce(
    &self,
    node_ref: &NodeId,
    initial: &Value,
    ctx: &ExecutionContext,
) -> Result<NodeOutput> {
    let items = ctx.input.as_array()
        .ok_or(SimpleAgentsError::InvalidInput("Reduce requires array input".into()))?;

    let mut accumulator = initial.clone();

    for (index, item) in items.iter().enumerate() {
        // Create context with accumulator and current item
        let reduce_ctx = ExecutionContext {
            input: json!({
                "accumulator": accumulator,
                "current": item,
                "index": index,
            }),
            ..ctx.clone()
        };

        // Execute reducer node
        let output = self.execute_node(node_ref, reduce_ctx).await?;

        // Update accumulator
        accumulator = output.value;

        // Record progress
        self.tracer.record_reduce_step(&ctx, index, &accumulator).await?;
    }

    Ok(NodeOutput {
        value: accumulator,
        streaming: false,
        metadata: None,
    })
}
```

### Subgraph Node

```rust
async fn execute_subgraph(
    &self,
    graph_ref: &GraphId,
    version: &Option<String>,
    ctx: &ExecutionContext,
) -> Result<NodeOutput> {
    // 1. Resolve subgraph (version matching)
    let subgraph = self.resolve_subgraph(graph_ref, version).await?;

    // 2. Create isolated executor
    let sub_executor = WorkflowExecutor {
        graph: Arc::new(subgraph),
        state: Arc::new(StateManager::new_isolated(ctx.state.clone())), // Child scope
        evaluator: self.evaluator.clone(),
        workers: self.workers.clone(),
        agents_client: self.agents_client.clone(),
        tracer: self.tracer.clone_with_prefix(graph_ref),
        semaphore: self.semaphore.clone(),
    };

    // 3. Execute subgraph
    let result = sub_executor.execute(ctx.input.clone()).await?;

    Ok(NodeOutput {
        value: result.output.unwrap_or(json!(null)),
        streaming: false,
        metadata: None,
    })
}

async fn resolve_subgraph(
    &self,
    graph_ref: &GraphId,
    version: &Option<String>,
) -> Result<WorkflowGraph> {
    // Load from registry/storage
    let available_graphs = self.graph_registry.list(graph_ref).await?;

    if let Some(version_req) = version {
        // Match semantic version
        let req = VersionReq::parse(version_req)?;

        for graph in available_graphs {
            if req.matches(&graph.version) {
                return Ok(graph);
            }
        }

        Err(SimpleAgentsError::SubgraphNotFound(graph_ref.clone(), version_req.clone()))
    } else {
        // Return latest
        available_graphs.into_iter()
            .max_by_key(|g| g.version.clone())
            .ok_or_else(|| SimpleAgentsError::SubgraphNotFound(graph_ref.clone(), "latest".into()))
    }
}
```

### Custom Worker Node

```rust
async fn execute_custom_worker(
    &self,
    language: &Language,
    handler: &str,
    ctx: &ExecutionContext,
) -> Result<NodeOutput> {
    // 1. Select worker from pool
    let worker = self.workers.select_worker(language).await?;

    // 2. Execute via gRPC
    let output = worker.execute(handler, ctx.input.clone()).await?;

    Ok(NodeOutput {
        value: output.value,
        streaming: output.streaming,
        metadata: Some(NodeMetadata {
            provider: Some(format!("worker:{}", language)),
            model: None,
            tokens: None,
            latency: Some(output.latency),
        }),
    })
}
```

## Streaming Execution

### Streaming LLM Nodes

```rust
async fn execute_llm_call_stream(
    &self,
    provider: &str,
    model: &str,
    ctx: &ExecutionContext,
) -> Result<impl Stream<Item = Result<CompletionChunk>>> {
    let request = CompletionRequest::builder()
        .model(model)
        .messages(ctx.get_messages()?)
        .stream(true)
        .build()?;

    // Use existing streaming support
    let stream = self.agents_client.stream(&request).await?;

    Ok(stream)
}
```

### Streaming Edges

```rust
async fn execute_streaming_edge(
    &self,
    from: &NodeId,
    to: &NodeId,
    stream: impl Stream<Item = Result<CompletionChunk>>,
    ctx: &ExecutionContext,
) -> Result<NodeOutput> {
    // Progressive accumulation
    let mut accumulated = String::new();
    let mut chunks = vec![];

    pin_mut!(stream);

    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;

        if let Some(delta) = chunk.choices.first() {
            if let Some(content) = &delta.delta.content {
                accumulated.push_str(content);
                chunks.push(chunk.clone());

                // Emit to downstream (if downstream supports streaming)
                self.emit_chunk(to, chunk, ctx).await?;
            }
        }
    }

    // Final validation
    let final_value = serde_json::from_str(&accumulated)
        .unwrap_or_else(|_| json!({"content": accumulated}));

    Ok(NodeOutput {
        value: final_value,
        streaming: true,
        metadata: None,
    })
}
```

## Error Handling

### Retry Logic

```rust
async fn execute_with_retry(
    &self,
    node_id: &NodeId,
    ctx: ExecutionContext,
    retry_policy: &RetryPolicy,
) -> Result<NodeOutput> {
    let mut attempt = 0;
    let mut last_error = None;

    while attempt < retry_policy.max_attempts {
        match self.execute_node(node_id, ctx.clone()).await {
            Ok(output) => return Ok(output),
            Err(e) => {
                if !e.is_retryable() || attempt + 1 >= retry_policy.max_attempts {
                    last_error = Some(e);
                    break;
                }

                // Backoff
                let delay = retry_policy.backoff.calculate_delay(attempt);
                tokio::time::sleep(delay).await;

                self.tracer.record_retry(&ctx, node_id, attempt, &e).await?;

                attempt += 1;
            }
        }
    }

    Err(last_error.unwrap())
}
```

### Compensation

```rust
async fn execute_with_compensation(
    &self,
    node_id: &NodeId,
    compensation_id: &Option<NodeId>,
    ctx: ExecutionContext,
) -> Result<NodeOutput> {
    match self.execute_node(node_id, ctx.clone()).await {
        Ok(output) => Ok(output),
        Err(e) => {
            if let Some(comp_id) = compensation_id {
                // Execute compensation
                self.tracer.record_compensation(&ctx, node_id, comp_id).await?;

                let comp_ctx = ExecutionContext {
                    input: json!({
                        "error": e.to_string(),
                        "original_input": ctx.input,
                    }),
                    ..ctx
                };

                self.execute_node(comp_id, comp_ctx).await?;
            }

            Err(e)
        }
    }
}
```

## Performance Characteristics

### Time Complexity

- **Linear DAG**: O(N) where N = number of nodes
- **Branching (Switch)**: O(B) evaluation + O(N) execution where B = number of branches
- **Parallel**: O(N/P) where P = parallelism factor
- **Map**: O(N × M) where N = items, M = node execution time
- **Reduce**: O(N) sequential

### Space Complexity

- **State storage**: O(N × S) where N = nodes, S = average output size
- **Trace recording**: O(N × T) where T = trace data per node
- **Concurrent tasks**: O(P) where P = max parallelism

### Latency

- **Node overhead**: <1ms (validation, scheduling)
- **LLM call**: 500ms - 5s (provider latency)
- **Worker RPC**: 2-5ms (gRPC overhead)
- **Expression eval**: <0.1ms (CEL, cached)

## Determinism and Replayability

### Trace Format

```json
{
  "execution_id": "exec_abc123",
  "graph_id": "workflow-v1",
  "version": "1.0.0",
  "started_at": "2026-01-31T10:00:00Z",
  "completed_at": "2026-01-31T10:00:15Z",
  "input": {"prompt": "analyze this"},
  "output": {"result": "..."},
  "events": [
    {
      "type": "node_execution",
      "node_id": "analyze",
      "timestamp": "2026-01-31T10:00:01Z",
      "input": {"prompt": "analyze this"},
      "output": {"sentiment": "positive"},
      "duration_ms": 850,
      "metadata": {
        "provider": "openai",
        "model": "gpt-4",
        "tokens": 150
      }
    },
    {
      "type": "decision",
      "node_id": "route",
      "timestamp": "2026-01-31T10:00:02Z",
      "decision_type": "switch",
      "branch_taken": "positive_handler"
    }
  ]
}
```

### Replay from Trace

```rust
pub async fn replay_from_trace(&self, trace: &ExecutionTrace) -> Result<WorkflowResult> {
    // Use recorded decisions and outputs
    let replay_ctx = ReplayContext {
        trace,
        current_event: 0,
    };

    self.execute_with_replay(replay_ctx).await
}

pub async fn resume_from_node(&self, trace: &ExecutionTrace, from_node: &NodeId) -> Result<WorkflowResult> {
    // Restore state up to from_node
    self.restore_state_from_trace(trace, from_node).await?;

    // Continue execution from that node
    let ctx = self.build_context_from_trace(trace, from_node).await?;

    self.execute_node(from_node, ctx).await
}
```
