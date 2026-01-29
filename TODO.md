# SimpleAgents Python Bindings - Missing Features TODO

This document tracks features available in the Rust core but not exposed to Python, with implementation plans and sample code.

## Priority 1: Critical - Healing Metadata & Core APIs

### 1.1 Expose Healing Metadata in Responses

**Status:** ✅ Done
**Impact:** HIGH - Users can't see what healing was applied or confidence scores
**Effort:** Medium (2-3 days)

**Current State:**
```python
# Python returns just string
result = client.complete_json(model, messages)  # returns: str
# No visibility into healing applied!
```

**Desired State:**
```python
result = client.complete_json_healed(model, messages)
print(f"Content: {result.content}")
print(f"Confidence: {result.confidence}")  # 0.0-1.0
print(f"Flags: {result.flags}")  # ["StrippedMarkdown", "FixedTrailingComma"]
print(f"Was healed: {result.was_healed}")
```

**Implementation Plan:**

1. Create new Python classes to wrap healing results:
```rust
// In simple-agents-py/src/lib.rs

#[pyclass]
struct HealedJsonResult {
    #[pyo3(get)]
    content: String,
    #[pyo3(get)]
    confidence: f32,
    #[pyo3(get)]
    was_healed: bool,
    flags: Vec<String>,
}

#[pymethods]
impl HealedJsonResult {
    #[getter]
    fn flags(&self) -> Vec<String> {
        self.flags.clone()
    }

    fn __repr__(&self) -> String {
        format!(
            "HealedJsonResult(confidence={:.2}, flags={}, content={:?}...)",
            self.confidence,
            self.flags.len(),
            &self.content.chars().take(50).collect::<String>()
        )
    }
}
```

2. Add new method to Client:
```rust
#[pymethods]
impl Client {
    // Keep existing method for backward compatibility
    fn complete_json(&self, ...) -> PyResult<String> { ... }

    // New method with metadata
    #[pyo3(signature = (model, messages, max_tokens=None, temperature=None, top_p=None))]
    fn complete_json_healed(
        &self,
        model: &str,
        messages: &Bound<'_, PyAny>,
        max_tokens: Option<u32>,
        temperature: Option<f32>,
        top_p: Option<f32>,
    ) -> PyResult<HealedJsonResult> {
        let messages = parse_messages(messages).map_err(py_err)?;
        let request = build_request_with_messages(
            model,
            messages,
            max_tokens,
            temperature,
            top_p,
            Some(ResponseFormat::JsonObject),
        )
        .map_err(py_err)?;

        let runtime = self.runtime.lock()
            .map_err(|_| PyRuntimeError::new_err("runtime lock poisoned"))?;

        // Use the healing API instead of regular complete
        let healed_response = runtime
            .block_on(self.client.complete_json(&request))
            .map_err(py_err)?;

        // Extract metadata
        let content = healed_response.response.content()
            .unwrap_or_default()
            .to_string();
        let confidence = healed_response.parsed.confidence;
        let was_healed = !healed_response.parsed.flags.is_empty();
        let flags = healed_response.parsed.flags
            .iter()
            .map(|f| f.description())
            .collect();

        Ok(HealedJsonResult {
            content,
            confidence,
            was_healed,
            flags,
        })
    }
}
```

3. Register new class in module:
```rust
#[pymodule]
fn simple_agents_py(_py: Python<'_>, module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<Client>()?;
    module.add_class::<HealedJsonResult>()?;  // Add this
    Ok(())
}
```

**Thought Process:**
- Keep backward compatibility by maintaining existing methods
- Return rich Python objects instead of strings for new methods
- Use `complete_json()` from Rust core which returns `HealedJsonResponse`
- Convert Rust `CoercionFlag` to human-readable strings for Python
- Clamp confidence to 0.0-1.0 range
- Provide `__repr__` for better debugging experience

**Testing:**
```python
def test_healing_metadata():
    client = Client("openai")

    # Simulate malformed JSON response
    result = client.complete_json_healed(
        "gpt-4o-mini",
        [{"role": "user", "content": "Return JSON with trailing comma"}]
    )

    assert 0.0 <= result.confidence <= 1.0
    assert isinstance(result.flags, list)
    if result.was_healed:
        assert len(result.flags) > 0
        print(f"Healing applied: {', '.join(result.flags)}")
```

---

### 1.2 Direct JSON Healing API (No LLM Call)

**Status:** ✅ Done
**Impact:** HIGH - Enable testing/debugging of healing without API calls
**Effort:** Small (1 day)

**Current State:**
```python
# Can't parse malformed JSON without making LLM API call
```

**Desired State:**
```python
from simple_agents_py import heal_json, coerce_to_schema

# Parse malformed JSON
malformed = '```json\n{"name": "Alice", "age": 30,}\n```'
result = heal_json(malformed)
print(result.value)  # {"name": "Alice", "age": 30}
print(result.confidence)  # 0.85
print(result.flags)  # ["StrippedMarkdown", "FixedTrailingComma"]

# Coerce to schema
data = {"age": "25", "score": "98.5"}
schema = {
    "type": "object",
    "properties": {
        "age": {"type": "integer"},
        "score": {"type": "number"}
    }
}
result = coerce_to_schema(data, schema)
print(result.value)  # {"age": 25, "score": 98.5}
print(result.flags)  # ["TypeCoercion(string->int)", ...]
```

**Implementation Plan:**

1. Create standalone Python classes:
```rust
#[pyclass]
struct ParseResult {
    #[pyo3(get)]
    value: Py<PyDict>,  // Parsed JSON as Python dict
    #[pyo3(get)]
    confidence: f32,
    flags: Vec<String>,
}

#[pymethods]
impl ParseResult {
    #[getter]
    fn flags(&self) -> Vec<String> {
        self.flags.clone()
    }
}

#[pyclass]
struct CoercionResult {
    #[pyo3(get)]
    value: Py<PyDict>,
    #[pyo3(get)]
    confidence: f32,
    flags: Vec<String>,
}

#[pymethods]
impl CoercionResult {
    #[getter]
    fn flags(&self) -> Vec<String> {
        self.flags.clone()
    }
}
```

2. Add module-level functions:
```rust
use pyo3::types::PyDict;
use pythonize::pythonize;

/// Parse malformed JSON-ish text into proper JSON
#[pyfunction]
#[pyo3(signature = (text, config=None))]
fn heal_json(
    py: Python<'_>,
    text: &str,
    config: Option<&Bound<'_, PyDict>>,
) -> PyResult<ParseResult> {
    // Build parser config from Python dict if provided
    let parser_config = if let Some(cfg) = config {
        // Parse config options like:
        // {"strip_markdown": true, "min_confidence": 0.5, ...}
        parse_parser_config(cfg)?
    } else {
        ParserConfig::default()
    };

    let parser = JsonishParser::with_config(parser_config);
    let result = parser.parse(text).map_err(|e| {
        PyRuntimeError::new_err(format!("Parsing failed: {}", e))
    })?;

    // Convert serde_json::Value to Python dict
    let py_value = pythonize(py, &result.value)
        .map_err(|e| PyRuntimeError::new_err(format!("Conversion failed: {}", e)))?;

    let py_dict = py_value.downcast_bound::<PyDict>(py)?.clone().unbind();

    Ok(ParseResult {
        value: py_dict,
        confidence: result.confidence,
        flags: result.flags.iter().map(|f| f.description()).collect(),
    })
}

/// Coerce data to match a schema
#[pyfunction]
#[pyo3(signature = (data, schema, config=None))]
fn coerce_to_schema(
    py: Python<'_>,
    data: &Bound<'_, PyAny>,
    schema: &Bound<'_, PyDict>,
    config: Option<&Bound<'_, PyDict>>,
) -> PyResult<CoercionResult> {
    // Convert Python data to serde_json::Value
    let value: serde_json::Value = pythonize::depythonize(data)
        .map_err(|e| PyRuntimeError::new_err(format!("Invalid data: {}", e)))?;

    // Convert Python schema dict to Schema
    let schema_value: serde_json::Value = pythonize::depythonize(schema)
        .map_err(|e| PyRuntimeError::new_err(format!("Invalid schema: {}", e)))?;
    let schema = Schema::from_json_schema(&schema_value)
        .map_err(|e| PyRuntimeError::new_err(format!("Schema conversion failed: {}", e)))?;

    // Build coercion config
    let coercion_config = if let Some(cfg) = config {
        parse_coercion_config(cfg)?
    } else {
        CoercionConfig::default()
    };

    let engine = CoercionEngine::with_config(coercion_config);
    let result = engine.coerce(&value, &schema)
        .map_err(|e| PyRuntimeError::new_err(format!("Coercion failed: {}", e)))?;

    // Convert result back to Python dict
    let py_value = pythonize(py, &result.value)
        .map_err(|e| PyRuntimeError::new_err(format!("Conversion failed: {}", e)))?;
    let py_dict = py_value.downcast_bound::<PyDict>(py)?.clone().unbind();

    Ok(CoercionResult {
        value: py_dict,
        confidence: result.confidence,
        flags: result.flags.iter().map(|f| f.description()).collect(),
    })
}
```

3. Register in module:
```rust
#[pymodule]
fn simple_agents_py(_py: Python<'_>, module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<Client>()?;
    module.add_class::<ParseResult>()?;
    module.add_class::<CoercionResult>()?;
    module.add_function(wrap_pyfunction!(heal_json, module)?)?;
    module.add_function(wrap_pyfunction!(coerce_to_schema, module)?)?;
    Ok(())
}
```

**Thought Process:**
- Expose healing/coercion as standalone functions, not just in client
- Useful for testing, debugging, and processing non-LLM JSON
- Config dicts are optional - defaults work for most cases
- Return Python dicts for easy manipulation
- Keep same confidence/flags pattern as other APIs

**Testing:**
```python
def test_heal_json():
    # Test markdown stripping
    result = heal_json('```json\n{"key": "value"}\n```')
    assert result.value == {"key": "value"}
    assert result.confidence > 0.9
    assert "StrippedMarkdown" in result.flags

    # Test with custom config
    result = heal_json(
        '{"key": "value",}',
        config={"min_confidence": 0.95}
    )
    assert result.confidence >= 0.95

def test_coerce_to_schema():
    result = coerce_to_schema(
        {"age": "30"},
        {"type": "object", "properties": {"age": {"type": "integer"}}}
    )
    assert result.value == {"age": 30}
    assert any("TypeCoercion" in f for f in result.flags)
```

---

### 1.3 Streaming Parser API

**Status:** ✅ Done
**Impact:** MEDIUM - Useful for progressive parsing
**Effort:** Small (1 day)

**Desired State:**
```python
from simple_agents_py import StreamingParser

parser = StreamingParser()
parser.feed('{"name": "Alice", ')
parser.feed('"age": 30}')

result = parser.finalize()
print(result.value)  # {"name": "Alice", "age": 30}
```

**Implementation Plan:**

```rust
#[pyclass]
struct StreamingParser {
    parser: simple_agents_healing::streaming::StreamingParser,
}

#[pymethods]
impl StreamingParser {
    #[new]
    fn new() -> Self {
        Self {
            parser: simple_agents_healing::streaming::StreamingParser::new(),
        }
    }

    fn feed(&mut self, chunk: &str) -> PyResult<()> {
        self.parser.feed(chunk);
        Ok(())
    }

    fn finalize(&mut self, py: Python<'_>) -> PyResult<ParseResult> {
        let result = self.parser.finalize()
            .map_err(|e| PyRuntimeError::new_err(format!("Parse failed: {}", e)))?;

        let py_value = pythonize(py, &result.value)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        let py_dict = py_value.downcast_bound::<PyDict>(py)?.clone().unbind();

        Ok(ParseResult {
            value: py_dict,
            confidence: result.confidence,
            flags: result.flags.iter().map(|f| f.description()).collect(),
        })
    }
}
```

**Thought Process:**
- Mutable state in Python class wraps Rust StreamingParser
- Feed chunks incrementally as they arrive
- Finalize when complete to get result
- Useful for real-time processing of streaming responses

---

## Priority 2: High - Client Builder & Multi-Provider

### 2.1 Client Builder with Multi-Provider Support

**Status:** ✅ Done
**Impact:** HIGH - Enable routing, caching, middleware
**Effort:** Large (5-7 days)

**Current State:**
```python
# Single provider only
client = Client("openai", api_key="sk-...")
```

**Desired State:**
```python
from simple_agents_py import ClientBuilder

client = (
    ClientBuilder()
    .add_provider("openai", api_key="sk-...")
    .add_provider("anthropic", api_key="sk-ant-...")
    .with_routing("round_robin")
    .with_cache(ttl_seconds=300)
    .with_healing_config({
        "fuzzy_match_threshold": 0.8,
        "min_confidence": 0.7
    })
    .build()
)

# Automatically routes between providers
response = client.complete("gpt-4o-mini", "Hello!")
```

**Implementation Plan:**

1. Create ClientBuilder class:
```rust
#[pyclass]
struct ClientBuilder {
    providers: Vec<Arc<dyn Provider>>,
    routing_mode: Option<String>,
    cache_ttl: Option<u64>,
    healing_config: Option<HealingSettings>,
}

#[pymethods]
impl ClientBuilder {
    #[new]
    fn new() -> Self {
        Self {
            providers: Vec::new(),
            routing_mode: None,
            cache_ttl: None,
            healing_config: None,
        }
    }

    #[pyo3(signature = (provider, api_key=None, api_base=None))]
    fn add_provider(
        mut slf: PyRefMut<'_, Self>,
        provider: &str,
        api_key: Option<String>,
        api_base: Option<String>,
    ) -> PyResult<PyRefMut<'_, Self>> {
        let provider = provider_from_params(
            provider,
            api_key.as_deref(),
            api_base.as_deref(),
        )
        .map_err(py_err)?;

        slf.providers.push(provider);
        Ok(slf)
    }

    fn with_routing(mut slf: PyRefMut<'_, Self>, mode: &str) -> PyResult<PyRefMut<'_, Self>> {
        slf.routing_mode = Some(mode.to_string());
        Ok(slf)
    }

    fn with_cache(mut slf: PyRefMut<'_, Self>, ttl_seconds: u64) -> PyResult<PyRefMut<'_, Self>> {
        slf.cache_ttl = Some(ttl_seconds);
        Ok(slf)
    }

    fn with_healing_config(
        mut slf: PyRefMut<'_, Self>,
        config: &Bound<'_, PyDict>,
    ) -> PyResult<PyRefMut<'_, Self>> {
        // Parse config dict into HealingSettings
        let mut healing = HealingSettings::default();

        if let Some(threshold) = config.get_item("fuzzy_match_threshold")? {
            let val: f64 = threshold.extract()?;
            healing.coercion_config.fuzzy_match_threshold = val;
        }

        if let Some(min_conf) = config.get_item("min_confidence")? {
            let val: f32 = min_conf.extract()?;
            healing.parser_config.min_confidence = val;
        }

        slf.healing_config = Some(healing);
        Ok(slf)
    }

    fn build(slf: PyRefMut<'_, Self>) -> PyResult<Client> {
        if slf.providers.is_empty() {
            return Err(PyRuntimeError::new_err("At least one provider required"));
        }

        let mut builder = SimpleAgentsClientBuilder::new();

        // Add providers
        for provider in slf.providers.iter() {
            builder = builder.with_provider(provider.clone());
        }

        // Set routing mode
        if let Some(mode) = &slf.routing_mode {
            let routing_mode = match mode.as_str() {
                "direct" => RoutingMode::Direct,
                "round_robin" => RoutingMode::RoundRobin,
                "latency" => RoutingMode::Latency(LatencyRouterConfig::default()),
                "cost" => RoutingMode::Cost(CostRouterConfig::default()),
                "fallback" => RoutingMode::Fallback(FallbackRouterConfig::default()),
                _ => return Err(PyRuntimeError::new_err(format!("Unknown routing mode: {}", mode))),
            };
            builder = builder.with_routing_mode(routing_mode);
        }

        // Set cache
        if let Some(ttl) = slf.cache_ttl {
            let cache = Arc::new(InMemoryCache::new());
            builder = builder
                .with_cache(cache)
                .with_cache_ttl(Duration::from_secs(ttl));
        }

        // Set healing config
        if let Some(healing) = &slf.healing_config {
            builder = builder.with_healing_settings(healing.clone());
        }

        let client = builder.build().map_err(py_err)?;
        let runtime = Runtime::new()
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;

        Ok(Client {
            runtime: Mutex::new(runtime),
            client,
        })
    }
}
```

**Thought Process:**
- Builder pattern enables fluent chaining in Python
- Each method returns `PyRefMut<Self>` for chaining
- Defaults to single provider if only one added
- Cache is opt-in via `with_cache()`
- Healing config uses dict for flexibility
- Routing modes as strings for simplicity

**Testing:**
```python
def test_multi_provider():
    client = (
        ClientBuilder()
        .add_provider("openai", api_key="sk-1")
        .add_provider("anthropic", api_key="sk-2")
        .with_routing("round_robin")
        .build()
    )

    # First call goes to openai
    r1 = client.complete("gpt-4o-mini", "Hello")
    # Second call goes to anthropic
    r2 = client.complete("claude-3-haiku-20240307", "Hi")
```

---

### 2.2 Routing Configuration

**Status:** ✅ Done
**Impact:** MEDIUM - Fine-grained routing control
**Effort:** Medium (2-3 days)

**Desired State:**
```python
# Latency-based routing with config
client = (
    ClientBuilder()
    .add_provider("openai", api_key="sk-1")
    .add_provider("anthropic", api_key="sk-2")
    .with_latency_routing({
        "smoothing_factor": 0.8,
        "window_size": 10
    })
    .build()
)

# Cost-based routing
client = (
    ClientBuilder()
    .add_provider("openai", api_key="sk-1")
    .add_provider("anthropic", api_key="sk-2")
    .with_cost_routing({
        "provider_costs": {
            "openai": {"input": 0.00001, "output": 0.00003},
            "anthropic": {"input": 0.000008, "output": 0.000024}
        }
    })
    .build()
)
```

**Implementation Plan:**

```rust
#[pymethods]
impl ClientBuilder {
    fn with_latency_routing(
        mut slf: PyRefMut<'_, Self>,
        config: &Bound<'_, PyDict>,
    ) -> PyResult<PyRefMut<'_, Self>> {
        let mut latency_config = LatencyRouterConfig::default();

        if let Some(factor) = config.get_item("smoothing_factor")? {
            latency_config.smoothing_factor = factor.extract()?;
        }

        if let Some(window) = config.get_item("window_size")? {
            latency_config.window_size = window.extract()?;
        }

        slf.routing_mode = Some(RoutingMode::Latency(latency_config));
        Ok(slf)
    }

    fn with_cost_routing(
        mut slf: PyRefMut<'_, Self>,
        config: &Bound<'_, PyDict>,
    ) -> PyResult<PyRefMut<'_, Self>> {
        let costs_dict = config.get_item("provider_costs")?
            .ok_or_else(|| PyRuntimeError::new_err("provider_costs required"))?;

        let mut provider_costs = Vec::new();
        for (provider_name, costs) in costs_dict.downcast::<PyDict>()?.iter() {
            let name: String = provider_name.extract()?;
            let costs_dict = costs.downcast::<PyDict>()?;

            let input_cost: f64 = costs_dict.get_item("input")?.unwrap().extract()?;
            let output_cost: f64 = costs_dict.get_item("output")?.unwrap().extract()?;

            provider_costs.push(ProviderCost {
                provider_name: name,
                cost_per_input_token: input_cost,
                cost_per_output_token: output_cost,
            });
        }

        slf.routing_mode = Some(RoutingMode::Cost(CostRouterConfig {
            provider_costs,
        }));
        Ok(slf)
    }
}
```

**Thought Process:**
- Separate methods for each routing type with specific config
- Latency routing adapts based on historical performance
- Cost routing minimizes spend per token
- Dict-based config for flexibility

---

## Priority 3: Medium - Streaming & Structured Outputs

### 3.1 Response Streaming

**Status:** ✅ Done
**Impact:** MEDIUM - Real-time UX
**Effort:** Large (5-7 days)

**Desired State:**
```python
# Stream tokens as they arrive
for chunk in client.stream("gpt-4o-mini", "Write a story"):
    print(chunk.content, end="", flush=True)
print()

# Async streaming
async for chunk in client.stream_async("gpt-4o-mini", "Hello"):
    print(chunk.content, end="")
```

**Implementation Plan:**

1. Add streaming to Rust core client (if not already available)
2. Expose via Python generator:

```rust
#[pymethods]
impl Client {
    fn stream<'py>(
        &self,
        py: Python<'py>,
        model: &str,
        prompt: &str,
    ) -> PyResult<Bound<'py, PyIterator>> {
        // Create Python generator/iterator
        let stream = self.create_stream(model, prompt)?;

        // Convert Rust stream to Python iterator
        let py_iter = PyStreamIterator::new(stream);
        Ok(py_iter.into_py(py))
    }
}

#[pyclass]
struct PyStreamIterator {
    runtime: Runtime,
    stream: Option<Pin<Box<dyn Stream<Item = Result<CompletionChunk>>>>>,
}

#[pymethods]
impl PyStreamIterator {
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __next__(mut slf: PyRefMut<'_, Self>) -> PyResult<Option<StreamChunk>> {
        let stream = slf.stream.as_mut()
            .ok_or_else(|| PyRuntimeError::new_err("Stream exhausted"))?;

        match slf.runtime.block_on(stream.next()) {
            Some(Ok(chunk)) => Ok(Some(StreamChunk::from(chunk))),
            Some(Err(e)) => Err(PyRuntimeError::new_err(e.to_string())),
            None => {
                slf.stream = None;
                Ok(None)
            }
        }
    }
}

#[pyclass]
struct StreamChunk {
    #[pyo3(get)]
    content: String,
    #[pyo3(get)]
    finish_reason: Option<String>,
}
```

**Thought Process:**
- Implement Python iterator protocol (`__iter__`, `__next__`)
- Block in `__next__` to wait for next chunk
- Return `None` when stream ends
- Separate async version using `pyo3-asyncio` for true async support
- Challenge: Bridging Rust async with Python sync/async

---

### 3.2 Structured Streaming with Partial Updates

**Status:** ✅ Done
**Impact:** MEDIUM - Progressive UI updates
**Effort:** Large (5-7 days)

**Desired State:**
```python
schema = {
    "type": "object",
    "properties": {
        "name": {"type": "string"},
        "items": {
            "type": "array",
            "items": {"type": "string"}
        }
    }
}

for event in client.stream_structured(model, messages, schema):
    if event.is_partial:
        print(f"Partial: {event.partial_value}")
    else:
        print(f"Complete: {event.value}")
```

**Implementation Plan:**

```rust
#[pyclass]
struct StructuredEvent {
    #[pyo3(get)]
    is_partial: bool,
    #[pyo3(get)]
    is_complete: bool,
    value: Py<PyDict>,
    partial_value: Option<Py<PyDict>>,
}

#[pymethods]
impl StructuredEvent {
    #[getter]
    fn value(&self, py: Python<'_>) -> PyObject {
        self.value.clone_ref(py).into()
    }

    #[getter]
    fn partial_value(&self, py: Python<'_>) -> Option<PyObject> {
        self.partial_value.as_ref().map(|v| v.clone_ref(py).into())
    }
}

#[pymethods]
impl Client {
    fn stream_structured<'py>(
        &self,
        py: Python<'py>,
        model: &str,
        messages: &Bound<'_, PyAny>,
        schema: &Bound<'_, PyDict>,
    ) -> PyResult<Bound<'py, PyIterator>> {
        // Use StructuredStream from providers
        // Emit partial updates as they become parseable
        todo!("Implement structured streaming")
    }
}
```

**Thought Process:**
- Leverage existing `StructuredStream` from Rust
- Emit events for both partial and complete parses
- Partial updates useful for UI (show progress)
- Complete event at end with full validated object

---

## Priority 4: Medium - Middleware & Observability

### 4.1 Middleware Hooks

**Status:** ✅ Done
**Impact:** MEDIUM - Logging, metrics, custom logic
**Effort:** Medium (3-4 days)

**Desired State:**
```python
class LoggingMiddleware:
    def before_request(self, request):
        print(f"Sending: {request.model}")

    def after_response(self, request, response, latency_ms):
        print(f"Received: {response.usage.total_tokens} tokens in {latency_ms}ms")

    def on_error(self, request, error, latency_ms):
        print(f"Error: {error}")

client = (
    ClientBuilder()
    .add_provider("openai")
    .add_middleware(LoggingMiddleware())
    .build()
)
```

**Implementation Plan:**

```rust
// Define Python trait for middleware
#[pyclass]
trait PyMiddleware {
    fn before_request(&self, request: &PyRequest) -> PyResult<()>;
    fn after_response(&self, request: &PyRequest, response: &PyResponse, latency_ms: u64) -> PyResult<()>;
    fn on_error(&self, request: &PyRequest, error: &str, latency_ms: u64) -> PyResult<()>;
}

// Wrap Python middleware to implement Rust Middleware trait
struct PyMiddlewareAdapter {
    py_middleware: PyObject,
}

#[async_trait]
impl Middleware for PyMiddlewareAdapter {
    async fn before_request(&self, request: &CompletionRequest) -> Result<()> {
        Python::with_gil(|py| {
            let py_request = PyRequest::from(request);
            self.py_middleware.call_method1(py, "before_request", (py_request,))
                .map_err(|e| SimpleAgentsError::Config(e.to_string()))?;
            Ok(())
        })
    }

    async fn after_response(
        &self,
        request: &CompletionRequest,
        response: &CompletionResponse,
        latency: Duration,
    ) -> Result<()> {
        Python::with_gil(|py| {
            let py_request = PyRequest::from(request);
            let py_response = PyResponse::from(response);
            let latency_ms = latency.as_millis() as u64;

            self.py_middleware.call_method1(
                py,
                "after_response",
                (py_request, py_response, latency_ms),
            )
            .map_err(|e| SimpleAgentsError::Config(e.to_string()))?;
            Ok(())
        })
    }

    // Similar for on_error, on_cache_hit
}

#[pymethods]
impl ClientBuilder {
    fn add_middleware(
        mut slf: PyRefMut<'_, Self>,
        middleware: PyObject,
    ) -> PyResult<PyRefMut<'_, Self>> {
        let adapter = Arc::new(PyMiddlewareAdapter {
            py_middleware: middleware,
        });
        slf.middleware.push(adapter);
        Ok(slf)
    }
}
```

**Thought Process:**
- Python defines middleware as duck-typed classes
- Rust wraps Python objects and calls into Python from Rust
- Need `Python::with_gil` to call Python from Rust async code
- Convert Rust types to Python types for callbacks
- Middleware methods are optional (check `hasattr` in Python)

---

### 4.2 Response Metadata Access

**Status:** ✅ Done
**Impact:** MEDIUM - Token usage, latency tracking
**Effort:** Small (1-2 days)

**Desired State:**
```python
response = client.complete_with_metadata(model, prompt)
print(f"Tokens: {response.usage.total_tokens}")
print(f"Provider: {response.provider}")
print(f"Latency: {response.latency_ms}ms")
print(f"Content: {response.content}")
```

**Implementation Plan:**

```rust
#[pyclass]
struct ResponseWithMetadata {
    #[pyo3(get)]
    content: String,
    #[pyo3(get)]
    provider: Option<String>,
    usage: Py<PyDict>,
    #[pyo3(get)]
    model: String,
    #[pyo3(get)]
    finish_reason: String,
}

#[pymethods]
impl ResponseWithMetadata {
    #[getter]
    fn usage(&self, py: Python<'_>) -> PyObject {
        self.usage.clone_ref(py).into()
    }
}

#[pymethods]
impl Client {
    fn complete_with_metadata(
        &self,
        model: &str,
        prompt: &str,
    ) -> PyResult<ResponseWithMetadata> {
        let response = self.complete_internal(model, prompt)?;

        Ok(ResponseWithMetadata {
            content: response.content().unwrap_or_default().to_string(),
            provider: response.provider.clone(),
            usage: convert_usage_to_py_dict(&response.usage)?,
            model: response.model.clone(),
            finish_reason: response.choices.first()
                .map(|c| format!("{:?}", c.finish_reason))
                .unwrap_or_default(),
        })
    }
}
```

**Thought Process:**
- Wrap full `CompletionResponse` with all metadata
- Usage as Python dict with `{prompt_tokens, completion_tokens, total_tokens}`
- Provider name useful for debugging multi-provider setups
- Finish reason indicates why generation stopped

---

## Priority 5: Low - Advanced Features

### 5.1 Tool/Function Calling

**Status:** ✅ Done
**Impact:** LOW - Advanced LLM features
**Effort:** Large (7+ days)

**Desired State:**
```python
def get_weather(location: str) -> str:
    return f"Weather in {location}: Sunny, 72°F"

tools = [
    {
        "type": "function",
        "function": {
            "name": "get_weather",
            "description": "Get weather for a location",
            "parameters": {
                "type": "object",
                "properties": {
                    "location": {"type": "string"}
                },
                "required": ["location"]
            }
        }
    }
]

response = client.complete_with_tools(
    model="gpt-4o",
    messages=[{"role": "user", "content": "What's the weather in SF?"}],
    tools=tools
)

if response.tool_calls:
    for call in response.tool_calls:
        result = get_weather(**call.arguments)
        # Continue conversation with tool result
```

**Implementation Plan:**
- Add tool definitions to `CompletionRequest`
- Parse tool calls from response
- Return structured tool call objects
- Support multi-turn tool calling loop

---

### 5.2 Advanced Schema Features

**Status:** ✅ Done
**Impact:** LOW - Power users
**Effort:** Medium (3-4 days)

**Desired State:**
```python
from simple_agents_py import SchemaBuilder

schema = (
    SchemaBuilder()
    .field("name", type="string", required=True)
    .field("age", type="integer", default=0)
    .field("email", type="string", aliases=["emailAddress", "mail"])
    .field("tags", type="array", items="string")
    .build()
)

result = coerce_to_schema(data, schema)
```

**Implementation Plan:**
- Builder pattern for Schema construction
- Support aliases, defaults, descriptions
- Stream annotations for progressive parsing

---

### 5.3 Caching with Custom Backends

**Status:** ✅ Done
**Impact:** LOW - Enterprise use cases
**Effort:** Medium (3-4 days)

**Desired State:**
```python
class RedisCache:
    def __init__(self, redis_url):
        self.redis = redis.from_url(redis_url)

    async def get(self, key: str) -> Optional[bytes]:
        return self.redis.get(key)

    async def set(self, key: str, value: bytes, ttl_seconds: int):
        self.redis.setex(key, ttl_seconds, value)

client = (
    ClientBuilder()
    .add_provider("openai")
    .with_custom_cache(RedisCache("redis://localhost"))
    .build()
)
```

**Implementation Plan:**
- Define Python Cache protocol
- Implement Rust Cache trait that calls into Python
- Support async cache operations
- Handle serialization/deserialization

---

## Implementation Priorities

**Phase 1 (2 weeks):**
1. Healing metadata in responses (1.1)
2. Direct healing APIs (1.2)
3. Client builder basics (2.1)

**Phase 2 (2 weeks):**
4. Response metadata (4.2)
5. Streaming parser (1.3)
6. Routing configuration (2.2)

**Phase 3 (3 weeks):**
7. Response streaming (3.1)
8. Middleware hooks (4.1)
9. Structured streaming (3.2)

**Phase 4 (Optional):**
10. Advanced features (tool calling, custom caching, etc.)

---

## Testing Strategy

Each feature should include:
1. Unit tests in Rust (test Rust→Python conversion)
2. Integration tests in Python (test Python API)
3. Example scripts demonstrating usage
4. Documentation updates

**Test Structure:**
```
crates/simple-agents-py/
├── tests/
│   ├── test_healing.py       # Healing features
│   ├── test_builder.py       # Client builder
│   ├── test_streaming.py     # Streaming APIs
│   └── test_middleware.py    # Middleware system
└── examples/
    ├── healing_demo.py
    ├── multi_provider.py
    └── streaming_example.py
```

---

## Documentation TODO

Each feature needs:
1. API reference in docstrings
2. Usage examples
3. Migration guide from current API
4. Type stubs (.pyi files) for IDE support

**Type Stubs Example:**
```python
# simple_agents_py.pyi
from typing import Optional, List, Dict, Any, Iterator

class HealedJsonResult:
    content: str
    confidence: float
    was_healed: bool
    flags: List[str]

class Client:
    def complete_json_healed(
        self,
        model: str,
        messages: List[Dict[str, str]],
        max_tokens: Optional[int] = None,
        temperature: Optional[float] = None,
        top_p: Optional[float] = None,
    ) -> HealedJsonResult: ...

def heal_json(
    text: str,
    config: Optional[Dict[str, Any]] = None,
) -> ParseResult: ...
```

---

## Dependencies to Add

```toml
# Cargo.toml additions
[dependencies]
pythonize = "0.21"  # Rust ↔ Python JSON conversion
pyo3-asyncio = "0.21"  # Async support
futures-util = "0.3"  # Stream utilities
```

---

## Breaking Changes & Migration

To avoid breaking existing users:
1. Keep existing methods (complete, complete_json, etc.)
2. Add new methods with "_healed", "_with_metadata" suffixes
3. Deprecation warnings for old API (if needed)
4. Major version bump when removing old methods

**Migration Guide Example:**
```python
# Old API (still works)
result = client.complete_json(model, messages)  # returns str

# New API (recommended)
result = client.complete_json_healed(model, messages)
print(f"Confidence: {result.confidence}")
print(f"Content: {result.content}")
```
