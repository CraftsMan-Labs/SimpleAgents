# ADR-010: Testing Strategy and Golden Traces

## Status
Accepted

## Context
Workflow engines are complex systems requiring comprehensive testing at multiple levels:
- **Unit tests**: Individual node execution logic
- **Integration tests**: Multi-node workflows end-to-end
- **Contract tests**: Worker protocol compliance across languages
- **Golden traces**: Regression detection via trace comparison
- **Performance tests**: Throughput, latency, and resource usage
- **Determinism tests**: Replay verification

Requirements:
- **Local runner**: Execute workflows without external dependencies
- **Fixture management**: Reusable test inputs and expected outputs
- **Trace comparison**: Detect regressions by comparing execution traces
- **Mock LLM responses**: Predictable testing without live API calls
- **Multi-language validation**: Ensure workers behave consistently
- **Fast feedback**: Tests run in <1 minute for CI/CD

## Decision
Implement a **comprehensive testing strategy** with golden trace validation as the cornerstone.

Testing pyramid:
- **Unit tests**: Test individual node types with mocked dependencies
- **Golden trace tests**: Compare execution traces against known-good baselines
- **Contract tests**: Validate worker RPC protocol compliance
- **Integration tests**: Test real workflows with mock LLM responses
- **Performance tests**: Benchmark throughput and latency
- **Fuzz tests**: Validate error handling with random inputs

Tools:
- **insta**: Snapshot testing for golden traces (Rust)
- **pytest**: Python worker contract tests
- **cargo test**: Rust unit and integration tests
- **criterion**: Performance benchmarking (Rust)
- **proptest**: Property-based testing for fuzzing

## Alternatives Considered

### 1. **Manual Testing Only**
- **Pros**:
  - No test infrastructure needed
  - Quick to start
  - Flexible
- **Cons**:
  - Not scalable
  - Regression-prone
  - No CI/CD integration
  - Slow feedback
- **Rejected**: Inadequate for production system

### 2. **Integration Tests Only (No Unit Tests)**
- **Pros**:
  - Tests real behavior
  - Fewer mocks
  - Catches integration issues
- **Cons**:
  - Slow feedback (minutes per test)
  - Hard to isolate failures
  - Brittle (many dependencies)
  - Poor debugging experience
- **Rejected**: Need fast unit tests for TDD

### 3. **Property-Based Testing Only**
- **Pros**:
  - Finds edge cases
  - High coverage
  - Minimal test code
- **Cons**:
  - Hard to write good properties
  - Slow (generates many test cases)
  - Doesn't catch regressions in specific scenarios
  - Poor for workflow logic testing
- **Rejected**: Use as supplement, not primary strategy

### 4. **Record/Replay Without Golden Traces**
- **Pros**:
  - Simple implementation
  - Automatic test generation
- **Cons**:
  - Can't detect correctness, only consistency
  - Baseline might be wrong
  - No explicit assertions
  - Hard to understand test intent
- **Rejected**: Need explicit expected outputs

### 5. **E2E Tests Only (Live APIs)**
- **Pros**:
  - Tests real integrations
  - No mocking complexity
  - Catches real-world issues
- **Cons**:
  - Expensive (API costs)
  - Slow (network latency)
  - Non-deterministic (LLM variability)
  - Can't run in CI
  - Flaky
- **Rejected**: Use for smoke tests only, not primary strategy

## Consequences

### Positive
- **Regression detection**: Golden traces catch unintended changes
- **Fast feedback**: Unit tests run in milliseconds
- **Determinism**: Mock responses ensure predictable tests
- **Multi-language validation**: Contract tests ensure worker parity
- **Debugging**: Trace diffs show exactly what changed
- **Documentation**: Tests serve as executable examples

### Negative
- **Maintenance**: Golden traces need updates when behavior changes intentionally
- **Snapshot drift**: Risk of approving incorrect baselines
- **Test complexity**: Multiple test types increase maintenance burden
- **Mock accuracy**: Mocks may not match real API behavior
- **Storage overhead**: Golden traces can be large

## Implementation Notes

### Local Runner

```rust
/// Local workflow runner for testing
pub struct LocalRunner {
    /// In-memory graph registry
    graphs: HashMap<GraphId, WorkflowGraph>,

    /// Mock LLM responses
    llm_mocks: HashMap<String, Value>,

    /// Mock worker handlers
    worker_mocks: HashMap<String, Box<dyn Fn(Value) -> Result<Value>>>,

    /// Trace storage
    traces: Vec<ExecutionTrace>,
}

impl LocalRunner {
    pub fn new() -> Self {
        Self {
            graphs: HashMap::new(),
            llm_mocks: HashMap::new(),
            worker_mocks: HashMap::new(),
            traces: Vec::new(),
        }
    }

    /// Load workflow from file or builder
    pub fn load_workflow(&mut self, graph: WorkflowGraph) -> &mut Self {
        self.graphs.insert(graph.id.clone(), graph);
        self
    }

    /// Register mock LLM response
    pub fn mock_llm(&mut self, prompt_hash: String, response: Value) -> &mut Self {
        self.llm_mocks.insert(prompt_hash, response);
        self
    }

    /// Register mock worker handler
    pub fn mock_worker<F>(&mut self, handler: String, func: F) -> &mut Self
    where
        F: Fn(Value) -> Result<Value> + 'static,
    {
        self.worker_mocks.insert(handler, Box::new(func));
        self
    }

    /// Execute workflow with mocks
    pub async fn execute(&mut self, graph_id: &GraphId, input: Value) -> Result<WorkflowResult> {
        let graph = self.graphs.get(graph_id)
            .ok_or_else(|| SimpleAgentsError::GraphNotFound(graph_id.clone()))?;

        // Create executor with mocked clients
        let executor = WorkflowExecutor {
            graph: Arc::new(graph.clone()),
            state: Arc::new(StateManager::new()),
            evaluator: Arc::new(CelEvaluator::new()),
            workers: Arc::new(self.create_mock_worker_pool()),
            agents_client: Arc::new(self.create_mock_llm_client()),
            tracer: Arc::new(TraceRecorder::new_in_memory()),
            semaphore: Arc::new(Semaphore::new(10)),
        };

        let result = executor.execute(input).await?;

        // Save trace
        if let Some(trace) = executor.tracer.get_trace().await? {
            self.traces.push(trace);
        }

        Ok(result)
    }

    fn create_mock_llm_client(&self) -> MockLlmClient {
        MockLlmClient::new(self.llm_mocks.clone())
    }

    fn create_mock_worker_pool(&self) -> MockWorkerPool {
        MockWorkerPool::new(self.worker_mocks.clone())
    }
}
```

### Golden Trace Testing with Insta

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use insta::assert_json_snapshot;

    #[tokio::test]
    async fn test_sentiment_analysis_workflow() {
        let mut runner = LocalRunner::new();

        // Load workflow
        let workflow = WorkflowGraph::builder()
            .id("sentiment-analysis")
            .version("1.0.0")
            .node(
                Node::llm_call("analyze")
                    .provider(Provider::OpenAI)
                    .model("gpt-4")
                    .prompt("Analyze sentiment: {{ input.text }}")
            )
            .node(
                Node::switch("route")
                    .input("$.nodes.analyze.output")
                    .branch_when("sentiment == 'positive'", "celebrate")
                    .branch_when("sentiment == 'negative'", "investigate")
            )
            .build()
            .unwrap();

        runner.load_workflow(workflow);

        // Mock LLM response
        runner.mock_llm(
            "analyze_sentiment".to_string(),
            json!({
                "sentiment": "positive",
                "confidence": 0.95
            })
        );

        // Execute
        let result = runner.execute(
            &"sentiment-analysis".into(),
            json!({"text": "Great product!"})
        ).await.unwrap();

        // Assert output
        assert_eq!(result.success, true);
        assert_json_snapshot!(result.output);

        // Assert trace (golden trace)
        let trace = runner.traces.last().unwrap();
        assert_json_snapshot!(trace);
    }

    #[tokio::test]
    async fn test_loop_workflow() {
        let mut runner = LocalRunner::new();

        let workflow = WorkflowGraph::builder()
            .node(
                Node::loop_node("paginate")
                    .condition("$.state.page < 3")
                    .body("fetch_page")
                    .max_iterations(10)
            )
            .build()
            .unwrap();

        runner.load_workflow(workflow);

        // Mock worker response
        runner.mock_worker("fetch_page".to_string(), |input| {
            let page = input.get("page").and_then(|v| v.as_u64()).unwrap_or(0);
            Ok(json!({
                "data": format!("Page {}", page),
                "page": page + 1
            }))
        });

        let result = runner.execute(
            &"loop-workflow".into(),
            json!({"page": 0})
        ).await.unwrap();

        // Golden trace snapshot
        let trace = runner.traces.last().unwrap();
        assert_json_snapshot!(trace, {
            ".execution_id" => "[execution_id]",
            ".started_at" => "[timestamp]",
            ".completed_at" => "[timestamp]",
            ".events[].timestamp" => "[timestamp]",
        });
    }
}
```

### Fixture Management

```rust
/// Test fixture loader
pub struct FixtureLoader {
    base_path: PathBuf,
}

impl FixtureLoader {
    pub fn new(base_path: impl Into<PathBuf>) -> Self {
        Self {
            base_path: base_path.into(),
        }
    }

    /// Load workflow fixture
    pub fn load_workflow(&self, name: &str) -> Result<WorkflowGraph> {
        let path = self.base_path.join("workflows").join(format!("{}.yaml", name));
        let yaml = std::fs::read_to_string(path)?;
        serde_yaml::from_str(&yaml).map_err(Into::into)
    }

    /// Load input fixture
    pub fn load_input(&self, name: &str) -> Result<Value> {
        let path = self.base_path.join("inputs").join(format!("{}.json", name));
        let json = std::fs::read_to_string(path)?;
        serde_json::from_str(&json).map_err(Into::into)
    }

    /// Load expected output
    pub fn load_output(&self, name: &str) -> Result<Value> {
        let path = self.base_path.join("outputs").join(format!("{}.json", name));
        let json = std::fs::read_to_string(path)?;
        serde_json::from_str(&json).map_err(Into::into)
    }

    /// Load golden trace
    pub fn load_trace(&self, name: &str) -> Result<ExecutionTrace> {
        let path = self.base_path.join("traces").join(format!("{}.json", name));
        let json = std::fs::read_to_string(path)?;
        serde_json::from_str(&json).map_err(Into::into)
    }
}

#[cfg(test)]
mod fixture_tests {
    use super::*;

    #[tokio::test]
    async fn test_with_fixtures() {
        let fixtures = FixtureLoader::new("tests/fixtures");
        let mut runner = LocalRunner::new();

        // Load workflow fixture
        let workflow = fixtures.load_workflow("sentiment_analysis").unwrap();
        runner.load_workflow(workflow);

        // Load input fixture
        let input = fixtures.load_input("positive_review").unwrap();

        // Execute
        let result = runner.execute(&"sentiment-analysis".into(), input).await.unwrap();

        // Compare with expected output
        let expected = fixtures.load_output("positive_review_result").unwrap();
        assert_eq!(result.output.unwrap(), expected);

        // Compare with golden trace
        let expected_trace = fixtures.load_trace("positive_review_trace").unwrap();
        let actual_trace = runner.traces.last().unwrap();

        // Compare ignoring timestamps
        assert_traces_equal(actual_trace, &expected_trace);
    }
}

fn assert_traces_equal(actual: &ExecutionTrace, expected: &ExecutionTrace) {
    assert_eq!(actual.graph_id, expected.graph_id);
    assert_eq!(actual.status, expected.status);
    assert_eq!(actual.input, expected.input);
    assert_eq!(actual.output, expected.output);

    // Compare events (ignoring timestamps)
    assert_eq!(actual.events.len(), expected.events.len());

    for (actual_event, expected_event) in actual.events.iter().zip(&expected.events) {
        match (actual_event, expected_event) {
            (
                TraceEvent::NodeCompleted { node_id: a_id, output: a_out, .. },
                TraceEvent::NodeCompleted { node_id: e_id, output: e_out, .. }
            ) => {
                assert_eq!(a_id, e_id);
                assert_eq!(a_out, e_out);
            }
            (
                TraceEvent::Decision { branch_taken: a_branch, .. },
                TraceEvent::Decision { branch_taken: e_branch, .. }
            ) => {
                assert_eq!(a_branch, e_branch);
            }
            _ => {
                // Compare discriminants
                assert_eq!(
                    std::mem::discriminant(actual_event),
                    std::mem::discriminant(expected_event)
                );
            }
        }
    }
}
```

### Contract Tests for Workers

```rust
/// Worker contract test suite
#[async_trait]
pub trait WorkerContractTest {
    /// Test basic execution
    async fn test_execute_simple(&self) -> Result<()>;

    /// Test error handling
    async fn test_execute_error(&self) -> Result<()>;

    /// Test timeout handling
    async fn test_execute_timeout(&self) -> Result<()>;

    /// Test concurrent execution
    async fn test_concurrent_execution(&self) -> Result<()>;

    /// Test health check
    async fn test_health_check(&self) -> Result<()>;
}

/// Run contract tests against a worker
pub async fn verify_worker_contract(
    worker: &dyn WorkerClient,
    test_suite: &dyn WorkerContractTest,
) -> Result<()> {
    test_suite.test_execute_simple().await?;
    test_suite.test_execute_error().await?;
    test_suite.test_execute_timeout().await?;
    test_suite.test_concurrent_execution().await?;
    test_suite.test_health_check().await?;

    Ok(())
}

#[cfg(test)]
mod worker_contract_tests {
    use super::*;

    struct PythonWorkerContractTest {
        worker: PythonWorkerClient,
    }

    #[async_trait]
    impl WorkerContractTest for PythonWorkerContractTest {
        async fn test_execute_simple(&self) -> Result<()> {
            let request = WorkerRequest {
                handler: "EchoHandler".to_string(),
                input: json!({"message": "hello"}),
                context: HashMap::new(),
            };

            let response = self.worker.execute(request).await?;

            assert_eq!(response.output, json!({"message": "hello"}));
            assert!(response.error.is_none());

            Ok(())
        }

        async fn test_execute_error(&self) -> Result<()> {
            let request = WorkerRequest {
                handler: "ErrorHandler".to_string(),
                input: json!({}),
                context: HashMap::new(),
            };

            let response = self.worker.execute(request).await?;

            assert!(response.error.is_some());
            assert!(response.output.is_null());

            Ok(())
        }

        async fn test_execute_timeout(&self) -> Result<()> {
            let request = WorkerRequest {
                handler: "SlowHandler".to_string(),
                input: json!({"delay_ms": 5000}),
                context: HashMap::new(),
            };

            let result = tokio::time::timeout(
                Duration::from_secs(1),
                self.worker.execute(request)
            ).await;

            assert!(result.is_err()); // Timeout

            Ok(())
        }

        async fn test_concurrent_execution(&self) -> Result<()> {
            let requests: Vec<_> = (0..10).map(|i| {
                WorkerRequest {
                    handler: "EchoHandler".to_string(),
                    input: json!({"id": i}),
                    context: HashMap::new(),
                }
            }).collect();

            let tasks: Vec<_> = requests.into_iter().map(|req| {
                let worker = self.worker.clone();
                tokio::spawn(async move {
                    worker.execute(req).await
                })
            }).collect();

            let results = futures::future::join_all(tasks).await;

            for result in results {
                assert!(result.is_ok());
            }

            Ok(())
        }

        async fn test_health_check(&self) -> Result<()> {
            let healthy = self.worker.health_check().await?;
            assert!(healthy);

            Ok(())
        }
    }

    #[tokio::test]
    async fn test_python_worker_contract() {
        let worker = PythonWorkerClient::connect("localhost:50051").await.unwrap();
        let test_suite = PythonWorkerContractTest { worker };

        verify_worker_contract(&test_suite.worker, &test_suite).await.unwrap();
    }
}
```

### Python Worker Contract Tests

```python
# tests/test_worker_contract.py
import pytest
from simple_agents.workflow.worker import WorkerServer, WorkerRequest, WorkerResponse

class EchoHandler:
    async def execute(self, input: dict, context: dict) -> dict:
        return input

class ErrorHandler:
    async def execute(self, input: dict, context: dict) -> dict:
        raise ValueError("Intentional error")

class SlowHandler:
    async def execute(self, input: dict, context: dict) -> dict:
        import asyncio
        delay = input.get("delay_ms", 1000) / 1000
        await asyncio.sleep(delay)
        return {"completed": True}

@pytest.fixture
async def worker_server():
    server = WorkerServer(port=50051)
    server.register_handler("EchoHandler", EchoHandler())
    server.register_handler("ErrorHandler", ErrorHandler())
    server.register_handler("SlowHandler", SlowHandler())

    await server.start()
    yield server
    await server.stop()

@pytest.mark.asyncio
async def test_execute_simple(worker_server):
    """Contract test: basic execution"""
    request = WorkerRequest(
        handler="EchoHandler",
        input={"message": "hello"},
        context={}
    )

    response = await worker_server.execute(request)

    assert response.output == {"message": "hello"}
    assert response.error is None

@pytest.mark.asyncio
async def test_execute_error(worker_server):
    """Contract test: error handling"""
    request = WorkerRequest(
        handler="ErrorHandler",
        input={},
        context={}
    )

    response = await worker_server.execute(request)

    assert response.error is not None
    assert "Intentional error" in response.error

@pytest.mark.asyncio
async def test_concurrent_execution(worker_server):
    """Contract test: concurrent execution"""
    import asyncio

    async def execute_request(i):
        request = WorkerRequest(
            handler="EchoHandler",
            input={"id": i},
            context={}
        )
        return await worker_server.execute(request)

    tasks = [execute_request(i) for i in range(10)]
    results = await asyncio.gather(*tasks)

    assert len(results) == 10
    assert all(r.error is None for r in results)

@pytest.mark.asyncio
async def test_health_check(worker_server):
    """Contract test: health check"""
    healthy = await worker_server.health_check()
    assert healthy is True
```

### Performance Benchmarks

```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};

fn benchmark_node_execution(c: &mut Criterion) {
    let mut group = c.benchmark_group("node_execution");

    // Benchmark transform node
    group.bench_function("transform", |b| {
        b.to_async(tokio::runtime::Runtime::new().unwrap())
            .iter(|| async {
                let executor = create_test_executor();
                let ctx = create_test_context();

                executor.execute_transform(
                    black_box(&Expression::new("input.value + 1")),
                    black_box(&ctx)
                ).await.unwrap()
            })
    });

    // Benchmark LLM node (mocked)
    group.bench_function("llm_call_mocked", |b| {
        b.to_async(tokio::runtime::Runtime::new().unwrap())
            .iter(|| async {
                let executor = create_test_executor();
                let ctx = create_test_context();

                executor.execute_llm_call(
                    black_box("openai"),
                    black_box("gpt-4"),
                    black_box(&ctx)
                ).await.unwrap()
            })
    });

    group.finish();
}

fn benchmark_workflow_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("workflow_throughput");

    for size in [10, 100, 1000].iter() {
        group.bench_with_input(
            BenchmarkId::new("parallel_nodes", size),
            size,
            |b, &size| {
                b.to_async(tokio::runtime::Runtime::new().unwrap())
                    .iter(|| async move {
                        let workflow = create_parallel_workflow(size);
                        let executor = create_test_executor();

                        executor.execute(workflow, json!({})).await.unwrap()
                    })
            },
        );
    }

    group.finish();
}

criterion_group!(benches, benchmark_node_execution, benchmark_workflow_throughput);
criterion_main!(benches);
```

### Test Organization

```
tests/
├── fixtures/
│   ├── workflows/
│   │   ├── sentiment_analysis.yaml
│   │   ├── loop_workflow.yaml
│   │   └── parallel_workflow.yaml
│   ├── inputs/
│   │   ├── positive_review.json
│   │   ├── negative_review.json
│   │   └── neutral_review.json
│   ├── outputs/
│   │   ├── positive_review_result.json
│   │   ├── negative_review_result.json
│   │   └── neutral_review_result.json
│   └── traces/
│       ├── positive_review_trace.json
│       ├── negative_review_trace.json
│       └── neutral_review_trace.json
├── snapshots/
│   └── tests__*.snap (generated by insta)
├── unit/
│   ├── test_nodes.rs
│   ├── test_expressions.rs
│   └── test_state.rs
├── integration/
│   ├── test_workflows.rs
│   └── test_replay.rs
├── contract/
│   ├── test_python_worker.rs
│   ├── test_go_worker.rs
│   └── test_ts_worker.rs
└── benchmarks/
    └── workflow_benchmarks.rs
```

## Related Decisions
- ADR-008: Trace Recording and Replayability
- ADR-003: gRPC Worker Protocol
- ADR-004: Long-Lived Worker Pools

## Future Enhancements
- **Visual trace diffing**: Web UI to compare traces
- **Automatic fixture generation**: Generate fixtures from executions
- **Mutation testing**: Verify test quality
- **Chaos testing**: Inject failures to test resilience
- **Load testing**: Distributed load generation
- **Coverage reports**: Track code coverage by test type
