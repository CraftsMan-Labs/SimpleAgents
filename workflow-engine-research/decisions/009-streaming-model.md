# ADR-009: Streaming Model

## Status
Accepted

## Context
Modern LLM applications require streaming responses for better user experience. The workflow engine must support:
- **Streaming outputs**: LLM nodes stream tokens progressively
- **Streaming edges**: Pass partial results to downstream nodes
- **Schema validation timing**: Validate complete output after streaming finishes
- **Backpressure**: Handle slow consumers without blocking producers
- **Partial results**: Allow downstream processing before stream completes
- **Mixed workflows**: Combine streaming and non-streaming nodes seamlessly

Requirements:
- **First-class streaming**: Not an afterthought; core to execution model
- **Progressive chunks**: Emit tokens/chunks as they arrive
- **Final validation**: JSON schema validation only after stream completes
- **Flexible consumption**: Some nodes consume streams incrementally, others wait for completion
- **Trace compatibility**: Record streaming chunks for replay
- **Error handling**: Gracefully handle mid-stream failures

## Decision
Implement **streaming as a first-class execution mode** with progressive chunk emission and deferred validation.

Architecture:
- **NodeOutput**: Support both `Value` (complete) and `Stream<Item = Chunk>` (progressive)
- **Streaming edges**: Explicitly marked in IR; pass chunks to downstream nodes
- **Schema validation**: Only validate final accumulated result, not partial chunks
- **Stream accumulator**: Buffer chunks until validation needed
- **Backpressure**: Use Tokio channels with bounded capacity
- **Mixed execution**: Non-streaming nodes wait for stream completion automatically

Design principles:
- **Explicit opt-in**: Nodes declare streaming capability via `stream: true` flag
- **Progressive emission**: Send chunks to client as soon as available
- **Deferred validation**: Validate schema only after stream completes
- **Transparent buffering**: Engine handles accumulation automatically
- **Graceful fallback**: Non-streaming consumers automatically buffer streams

## Alternatives Considered

### 1. **No Streaming Support (Blocking Only)**
- **Pros**:
  - Simpler implementation
  - Easier to validate and test
  - No buffering complexity
- **Cons**:
  - Poor user experience (long wait times)
  - Can't show progress
  - Incompatible with modern LLM APIs
  - Missing competitive feature
- **Rejected**: Streaming is essential for LLM applications

### 2. **Streaming Only at Workflow Output (Not Between Nodes)**
- **Pros**:
  - Simpler node-to-node protocol
  - Easier to validate intermediate results
  - Less memory overhead
- **Cons**:
  - Can't do progressive transformations
  - Limits workflow patterns (e.g., real-time summarization)
  - Forces buffering at LLM node
- **Rejected**: Want flexibility for streaming pipelines

### 3. **WebSockets/SSE Only (No Internal Streaming)**
- **Pros**:
  - Simple internal execution model
  - Streaming only at API boundary
  - Easy to implement
- **Cons**:
  - Latency from buffering
  - Can't leverage streaming for node-to-node processing
  - Limits advanced patterns
- **Rejected**: Want streaming throughout execution graph

### 4. **Actor Model (Message Passing)**
- **Pros**:
  - Natural streaming via mailboxes
  - Built-in backpressure
  - Good concurrency model
- **Cons**:
  - High overhead per message
  - Complex debugging
  - Doesn't match DAG execution model
- **Rejected**: Too heavyweight for chunk passing

### 5. **Reactive Streams (RxRust/Futures)**
- **Pros**:
  - Industry-standard streaming model
  - Rich operator library
  - Good backpressure support
- **Cons**:
  - Steep learning curve
  - Overkill for simple token streaming
  - Hard to integrate with existing async/await code
- **Rejected**: Tokio streams simpler and sufficient

## Consequences

### Positive
- **User experience**: Progressive output for long-running LLM calls
- **Latency**: Lower perceived latency (time to first token)
- **Flexibility**: Support both streaming and non-streaming nodes
- **Composability**: Stream through multi-node pipelines
- **Memory efficiency**: Don't buffer entire response in memory
- **Real-time processing**: Enable real-time transformations

### Negative
- **Complexity**: More complex execution logic
- **Validation timing**: Schema validation deferred until stream completes
- **Error handling**: Mid-stream failures harder to recover from
- **Debugging**: Harder to inspect partial states
- **Testing**: Need to test both streaming and non-streaming paths
- **Trace size**: Recording all chunks increases trace storage

## Implementation Notes

### NodeOutput with Streaming

```rust
pub enum NodeOutput {
    /// Complete output (non-streaming)
    Complete {
        value: Value,
        metadata: Option<NodeMetadata>,
    },

    /// Streaming output
    Streaming {
        stream: Pin<Box<dyn Stream<Item = Result<Chunk>> + Send>>,
        metadata: Option<NodeMetadata>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chunk {
    /// Chunk index (0-based)
    pub index: usize,

    /// Partial data
    pub data: ChunkData,

    /// Whether this is the final chunk
    pub is_final: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ChunkData {
    /// Text chunk (for LLM responses)
    Text(String),

    /// JSON chunk (partial object)
    Json(Value),

    /// Binary chunk
    Binary(Vec<u8>),
}
```

### Streaming LLM Node

```rust
async fn execute_llm_call_stream(
    &self,
    provider: &str,
    model: &str,
    stream: bool,
    ctx: &ExecutionContext,
) -> Result<NodeOutput> {
    let request = CompletionRequest::builder()
        .model(model)
        .messages(ctx.get_messages()?)
        .stream(stream)
        .build()?;

    if stream {
        // Use existing SimpleAgentsClient streaming support
        let stream = self.agents_client.stream(&request).await?;

        // Convert CompletionChunk to our Chunk format
        let chunk_stream = stream.enumerate().map(|(index, result)| {
            result.map(|completion_chunk| {
                let content = completion_chunk.choices.first()
                    .and_then(|c| c.delta.content.clone())
                    .unwrap_or_default();

                Chunk {
                    index,
                    data: ChunkData::Text(content),
                    is_final: completion_chunk.choices.first()
                        .map(|c| c.finish_reason.is_some())
                        .unwrap_or(false),
                }
            })
        });

        Ok(NodeOutput::Streaming {
            stream: Box::pin(chunk_stream),
            metadata: Some(NodeMetadata {
                provider: Some(provider.to_string()),
                model: Some(model.to_string()),
                tokens: None, // Will be filled on stream completion
                latency_ms: None,
            }),
        })
    } else {
        // Non-streaming path
        let response = self.agents_client.complete(&request).await?;

        let output_value = if let Some(content) = response.content() {
            serde_json::from_str(content).unwrap_or_else(|_| json!({"content": content}))
        } else {
            json!(null)
        };

        Ok(NodeOutput::Complete {
            value: output_value,
            metadata: Some(NodeMetadata {
                provider: Some(response.provider.clone()),
                model: Some(response.model.clone()),
                tokens: Some(response.usage.total_tokens),
                latency_ms: None,
            }),
        })
    }
}
```

### Streaming Edge Execution

```rust
async fn execute_streaming_edge(
    &self,
    from: &NodeId,
    to: &NodeId,
    stream: impl Stream<Item = Result<Chunk>>,
    ctx: &ExecutionContext,
) -> Result<NodeOutput> {
    let to_node = self.graph.nodes.get(to)
        .ok_or_else(|| SimpleAgentsError::NodeNotFound(to.clone()))?;

    // Check if downstream node accepts streaming
    let supports_streaming = to_node.config.get("supports_streaming")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    if supports_streaming {
        // Pass stream directly to downstream node
        self.execute_node_with_stream(to, stream, ctx).await
    } else {
        // Buffer stream and pass complete result
        let accumulated = self.accumulate_stream(stream).await?;

        let next_ctx = ExecutionContext {
            input: accumulated,
            ..ctx.clone()
        };

        self.execute_node(to, next_ctx).await
    }
}

/// Accumulate stream into complete value
async fn accumulate_stream(
    &self,
    stream: impl Stream<Item = Result<Chunk>>,
) -> Result<Value> {
    let mut accumulated = String::new();
    let mut chunks = Vec::new();

    pin_mut!(stream);

    while let Some(chunk_result) = stream.next().await {
        let chunk = chunk_result?;
        chunks.push(chunk.clone());

        match chunk.data {
            ChunkData::Text(text) => {
                accumulated.push_str(&text);
            }
            ChunkData::Json(json) => {
                // Merge JSON chunks (implementation-specific)
                accumulated = serde_json::to_string(&json)?;
            }
            ChunkData::Binary(_) => {
                // Handle binary accumulation
            }
        }

        // Emit chunk to tracer
        self.tracer.record_stream_chunk(&chunk).await?;

        if chunk.is_final {
            break;
        }
    }

    // Parse final accumulated result
    let final_value = serde_json::from_str(&accumulated)
        .unwrap_or_else(|_| json!({"content": accumulated}));

    Ok(final_value)
}
```

### Schema Validation for Streaming

```rust
async fn validate_streaming_output(
    &self,
    stream: impl Stream<Item = Result<Chunk>>,
    schema: &JsonSchema,
) -> Result<Value> {
    // Accumulate stream
    let complete_value = self.accumulate_stream(stream).await?;

    // Validate only after stream completes
    self.validate_schema(&complete_value, schema)?;

    Ok(complete_value)
}

impl WorkflowExecutor {
    async fn execute_node(
        &self,
        node_id: &NodeId,
        ctx: ExecutionContext,
    ) -> Result<NodeOutput> {
        let node = self.graph.nodes.get(node_id)
            .ok_or_else(|| SimpleAgentsError::NodeNotFound(node_id.clone()))?;

        // Execute node
        let output = self.execute_node_type(&node.node_type, &ctx).await?;

        // Validate output schema
        if let Some(schema) = &node.output_schema {
            match output {
                NodeOutput::Complete { value, metadata } => {
                    // Validate immediately
                    self.validate_schema(&value, schema)?;
                    Ok(NodeOutput::Complete { value, metadata })
                }
                NodeOutput::Streaming { stream, metadata } => {
                    // Validate after accumulation
                    let complete_value = self.validate_streaming_output(stream, schema).await?;
                    Ok(NodeOutput::Complete {
                        value: complete_value,
                        metadata,
                    })
                }
            }
        } else {
            Ok(output)
        }
    }
}
```

### Backpressure with Bounded Channels

```rust
pub struct StreamBuffer {
    tx: mpsc::Sender<Chunk>,
    rx: mpsc::Receiver<Chunk>,
    capacity: usize,
}

impl StreamBuffer {
    pub fn new(capacity: usize) -> Self {
        let (tx, rx) = mpsc::channel(capacity);
        Self { tx, rx, capacity }
    }

    /// Send chunk with backpressure
    pub async fn send(&self, chunk: Chunk) -> Result<()> {
        self.tx.send(chunk).await
            .map_err(|_| SimpleAgentsError::StreamClosed)
    }

    /// Receive chunk
    pub async fn recv(&mut self) -> Option<Chunk> {
        self.rx.recv().await
    }

    /// Convert to stream
    pub fn into_stream(self) -> impl Stream<Item = Result<Chunk>> {
        ReceiverStream::new(self.rx).map(Ok)
    }
}

impl WorkflowExecutor {
    async fn execute_streaming_pipeline(
        &self,
        nodes: &[NodeId],
        ctx: &ExecutionContext,
    ) -> Result<NodeOutput> {
        // Create bounded buffer between nodes (backpressure)
        let buffer = StreamBuffer::new(100); // 100 chunks buffered

        // Spawn producer
        let producer = tokio::spawn({
            let executor = self.clone();
            let buffer = buffer.clone();
            let ctx = ctx.clone();
            let node_id = nodes[0].clone();

            async move {
                let output = executor.execute_node(&node_id, ctx).await?;

                match output {
                    NodeOutput::Streaming { stream, .. } => {
                        pin_mut!(stream);
                        while let Some(chunk_result) = stream.next().await {
                            let chunk = chunk_result?;
                            buffer.send(chunk).await?;
                        }
                    }
                    NodeOutput::Complete { value, .. } => {
                        // Convert complete value to single chunk
                        buffer.send(Chunk {
                            index: 0,
                            data: ChunkData::Json(value),
                            is_final: true,
                        }).await?;
                    }
                }

                Ok::<_, SimpleAgentsError>(())
            }
        });

        // Consumer processes stream
        let stream = buffer.into_stream();
        Ok(NodeOutput::Streaming {
            stream: Box::pin(stream),
            metadata: None,
        })
    }
}
```

### Workflow Example with Streaming

```yaml
nodes:
  # LLM node with streaming enabled
  - id: generate_story
    name: "Generate Story"
    node_type:
      llm_call:
        provider: openai
        model: gpt-4-turbo
        stream: true  # Enable streaming
        prompt: "Write a creative story about {{ input.topic }}"

    config:
      supports_streaming: true

  # Transform node consumes stream progressively
  - id: format_chunks
    name: "Format Chunks"
    node_type:
      transform:
        expression: |
          {
            "formatted": chunk.data,
            "index": chunk.index
          }

    config:
      supports_streaming: true  # Processes chunks as they arrive

  # Non-streaming node waits for completion
  - id: analyze_complete
    name: "Analyze Complete Story"
    node_type:
      llm_call:
        provider: anthropic
        model: claude-3-sonnet
        stream: false  # Wait for full input
        prompt: "Analyze this story: {{ input.content }}"

    config:
      supports_streaming: false  # Buffers upstream stream

edges:
  - from: generate_story
    to: format_chunks
    streaming: true  # Streaming edge

  - from: format_chunks
    to: analyze_complete
    streaming: false  # Buffered edge
```

### Client-Side Streaming

```rust
// Rust client
let workflow = WorkflowGraph::builder()
    .node(Node::llm_call("generate").stream(true))
    .build()?;

let engine = WorkflowEngine::new()?;

// Stream results to client
let mut stream = engine.execute_stream(&workflow, input).await?;

while let Some(chunk) = stream.next().await {
    let chunk = chunk?;
    println!("Received chunk {}: {:?}", chunk.index, chunk.data);
}
```

```python
# Python client
workflow = (
    WorkflowGraph()
    .node(Node.llm_call("generate").stream(True))
    .build()
)

engine = WorkflowEngine()

# Stream results
async for chunk in engine.execute_stream(workflow, input):
    print(f"Received chunk {chunk.index}: {chunk.data}")
```

```typescript
// TypeScript client
const workflow = new WorkflowGraph()
  .node(Node.llmCall('generate').stream(true))
  .build();

const engine = new WorkflowEngine();

// Stream results
const stream = await engine.executeStream(workflow, input);

for await (const chunk of stream) {
  console.log(`Received chunk ${chunk.index}:`, chunk.data);
}
```

### Streaming Trace Recording

```rust
impl TraceRecorder {
    /// Record streaming chunk
    pub async fn record_stream_chunk(&self, chunk: &Chunk) -> Result<()> {
        let event = TraceEvent::StreamChunk {
            node_id: self.current_node.clone(),
            timestamp: Utc::now(),
            chunk_index: chunk.index,
            chunk_data: match &chunk.data {
                ChunkData::Text(s) => json!({"text": s}),
                ChunkData::Json(v) => v.clone(),
                ChunkData::Binary(b) => json!({"binary_len": b.len()}),
            },
        };

        let mut events = self.events.write().await;
        events.push(event);

        Ok(())
    }
}

/// Replay streaming execution
impl ReplayEngine {
    async fn replay_stream(&mut self, node_id: &NodeId) -> Result<impl Stream<Item = Result<Chunk>>> {
        let chunks: Vec<Chunk> = self.trace.events
            .iter()
            .filter_map(|e| match e {
                TraceEvent::StreamChunk { node_id: id, chunk_index, chunk_data, .. }
                    if id == node_id =>
                {
                    Some(Chunk {
                        index: *chunk_index,
                        data: ChunkData::Json(chunk_data.clone()),
                        is_final: false, // Determined from subsequent events
                    })
                }
                _ => None,
            })
            .collect();

        Ok(futures::stream::iter(chunks).map(Ok))
    }
}
```

## Related Decisions
- ADR-001: Canonical IR Format (YAML/JSON)
- ADR-008: Trace Recording and Replayability
- ADR-011: Node Type Taxonomy

## Future Enhancements
- **Partial schema validation**: Validate chunks against incremental schemas
- **Stream transformation**: Built-in operators (map, filter, reduce) for streams
- **Multi-cast streams**: Fan out stream to multiple consumers
- **Stream compression**: Compress chunks on the wire
- **Stream encryption**: Encrypt sensitive streaming data
- **Adaptive buffering**: Dynamic buffer size based on consumer speed
- **Stream metrics**: Track throughput, latency per chunk, backpressure events
