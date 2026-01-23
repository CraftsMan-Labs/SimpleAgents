# LiteLLM Architecture Analysis

**Research Date**: 2026-01-15
**Repository**: `/Users/rishub/Desktop/projects/enterprise/craftsmanlabs/litellm/`

---

## Executive Summary

LiteLLM is a Python-based unified interface for 100+ LLM providers. It normalizes different provider APIs into an OpenAI-compatible format, providing features like routing, fallbacks, retries, and caching. The codebase consists of ~24,500 lines of core Python code across two main layers: SDK (direct API calls) and Proxy (AI Gateway with auth/routing).

---

## Core Architecture

### Two-Tier System

1. **SDK Layer** (`litellm/` directory)
   - Core library for direct LLM API calls
   - Provider transformations
   - Retry and fallback logic
   - Can be used as a Python library

2. **Proxy Layer** (`litellm/proxy/` directory)
   - Full AI Gateway with HTTP server
   - Authentication and authorization
   - Rate limiting (per user, per team, per deployment)
   - Advanced routing strategies
   - Metrics and observability

---

## Provider Abstraction Pattern

### Transformation-Based Design

Each provider implements:
```python
class ProviderConfig(BaseConfig):
    def transform_request(self, model, messages, optional_params, litellm_params, headers):
        """Convert OpenAI format → Provider-specific format"""
        return transformed_data

    def transform_response(self, model, raw_response, model_response, logging_obj, ...):
        """Convert Provider-specific format → OpenAI format"""
        return ModelResponse(...)
```

### Key Providers Analyzed

#### 1. OpenAI (`llms/openai/chat/gpt_transformation.py`)
- **Format**: Mostly pass-through, minimal transformation
- **Special handling**:
  - Tool calling
  - Response format (JSON mode)
  - Streaming (SSE)
- **Authentication**: Bearer token in header

#### 2. Anthropic (`llms/anthropic/chat/transformation.py`)
- **Transformations**:
  - System message → separate `system` parameter (not in messages array)
  - Thinking blocks (extended_thinking parameter)
  - Message structure differences
- **Streaming**: Custom SSE format
- **Authentication**: `x-api-key` header

#### 3. AWS Bedrock (`llms/bedrock/chat/`)
- **Two APIs**:
  - **Converse API**: Unified interface across all Bedrock models
  - **Invoke API**: Model-specific formats
- **Complexity**: AWS signature auth (SigV4)
- **Model families**: Claude, Titan, Llama, etc. (different formats)

#### 4. Azure (`llms/azure/azure.py`)
- **Differences from OpenAI**:
  - Custom deployment URLs
  - API version in query params
  - Azure AD authentication option
- **Model mapping**: deployment name → base model

#### 5. Google Vertex AI (`llms/vertex_ai/`)
- **Authentication**: Service account JSON
- **Format**: Different message structure, role names
- **Streaming**: gRPC-based

---

## Entry Points and Main Functions

### `main.py` (7,258 lines)

**Primary Functions**:

```python
def completion(
    model: str,
    messages: List[Dict],
    temperature: Optional[float] = None,
    max_tokens: Optional[int] = None,
    stream: bool = False,
    # ... 50+ optional parameters
) -> ModelResponse:
    """Main completion function - handles all providers"""

    # 1. Parse model string ("azure/gpt-4", "anthropic/claude-3")
    # 2. Get provider from model string
    # 3. Transform request for provider
    # 4. Execute HTTP request
    # 5. Transform response to OpenAI format
    # 6. Return ModelResponse
```

**Other Functions**:
- `acompletion()`: Async version
- `embedding()` / `aembedding()`: For embeddings
- `image_generation()`: For image models
- `text_completion()`: Legacy text completion
- `transcription()`: Audio transcription

### Provider Resolution (`utils.py` - 8,930 lines)

```python
def get_llm_provider(
    model: str,
    custom_llm_provider: Optional[str] = None,
    api_base: Optional[str] = None,
    api_key: Optional[str] = None,
) -> Tuple[str, str, str, str]:
    """
    Parse model string and return (provider, model_name, api_key, api_base)

    Examples:
    - "gpt-4" → ("openai", "gpt-4", env_key, default_base)
    - "azure/my-deployment" → ("azure", "my-deployment", env_key, custom_base)
    - "anthropic/claude-3-opus" → ("anthropic", "claude-3-opus", env_key, default_base)
    """
    # Logic handles 100+ providers
    # Dynamic API key resolution from environment variables
    # Custom base URL support
```

**Supported Model Prefixes**:
- `openai/`, `azure/`, `anthropic/`, `bedrock/`, `vertex_ai/`, `cohere/`, `replicate/`, `openrouter/`, `together_ai/`, `ollama/`, etc.

---

## Router System (`router.py` - 8,334 lines)

### Routing Strategies

1. **Simple-Shuffle** (Random)
   ```python
   def simple_shuffle(deployments):
       return random.choice(deployments)
   ```

2. **Least-Busy** (Based on Active Requests)
   ```python
   def least_busy(deployments, inflight_requests):
       return min(deployments, key=lambda d: inflight_requests[d.model_name])
   ```

3. **Usage-Based Routing** (TPM/RPM Tracking)
   ```python
   def usage_based_routing(deployments, usage_tracker):
       # Pick deployment with most remaining TPM/RPM
       return max(deployments, key=lambda d: usage_tracker.remaining_budget(d))
   ```

4. **Latency-Based Routing**
   ```python
   def latency_based_routing(deployments, metrics):
       # Pick deployment with lowest p95 latency
       return min(deployments, key=lambda d: metrics.get_p95_latency(d))
   ```

5. **Cost-Based Routing**
   ```python
   def cost_based_routing(deployments, costs):
       # Pick cheapest deployment for given request
       return min(deployments, key=lambda d: costs.estimate_cost(d, request))
   ```

### Reliability Features

#### Fallbacks
```python
async def async_completion_with_fallbacks(**kwargs):
    fallbacks = [original_model] + kwargs.pop("fallbacks", [])

    for fallback in fallbacks:
        try:
            response = await litellm.acompletion(model=fallback, ...)
            if response is not None:
                return response
        except Exception as e:
            logger.warning(f"Fallback {fallback} failed: {e}")
            continue

    raise Exception("All fallback attempts failed")
```

**Configuration**:
- Max 5 fallbacks by default (`ROUTER_MAX_FALLBACKS`)
- Can be per-request or configured globally

#### Retries
```python
DEFAULT_MAX_RETRIES = 2  # From environment or hardcoded

# Exponential backoff
for attempt in range(max_retries):
    try:
        return await execute_request()
    except RetryableError:
        backoff = min(initial_backoff * (2 ** attempt), max_backoff)
        await asyncio.sleep(backoff)
```

**Retryable Errors**:
- `RateLimitError` (429)
- `ServiceUnavailableError` (503)
- `Timeout`
- Specific error codes from providers

#### Cooldown Mechanism
```python
DEFAULT_COOLDOWN_TIME_SECONDS = 5

# After failure, mark deployment as "cooling down"
deployment.cooling_down_until = time.time() + COOLDOWN_TIME_SECONDS

# Skip deployments in cooldown during routing
available = [d for d in deployments if not d.is_cooling_down()]
```

---

## HTTP Handler (`llms/custom_httpx/llm_http_handler.py`)

### Central HTTP Orchestrator

```python
class BaseLLMHTTPHandler:
    def __init__(self):
        self.client = httpx.AsyncClient(
            timeout=httpx.Timeout(timeout=600.0, connect=5.0),
            limits=httpx.Limits(max_connections=100, max_keepalive_connections=20),
        )

    async def async_completion(
        self,
        api_base: str,
        headers: dict,
        data: dict,
        model: str,
        timeout: float,
    ) -> httpx.Response:
        """Execute non-streaming completion"""
        response = await self.client.post(
            api_base,
            headers=headers,
            json=data,
            timeout=timeout,
        )
        return response

    async def async_streaming(
        self,
        api_base: str,
        headers: dict,
        data: dict,
        model: str,
        timeout: float,
    ) -> AsyncIterator[bytes]:
        """Execute streaming completion (SSE)"""
        async with self.client.stream(
            "POST",
            api_base,
            headers=headers,
            json=data,
            timeout=timeout,
        ) as response:
            async for chunk in response.aiter_bytes():
                yield chunk
```

**Features**:
- Connection pooling (max 100 connections, 20 keepalive)
- Configurable timeouts (connect vs request)
- Built-in retry for specific errors
- Streaming via SSE (Server-Sent Events)

---

## Caching System (`caching/` directory)

### Cache Backends

#### 1. In-Memory Cache (`in_memory_cache.py`)
```python
class InMemoryCache:
    def __init__(self, max_size_in_memory: int = 100):
        self.cache = OrderedDict()
        self.max_size = max_size_in_memory

    def get_cache(self, key: str) -> Optional[dict]:
        return self.cache.get(key)

    def set_cache(self, key: str, value: dict, ttl: int):
        if len(self.cache) >= self.max_size:
            self.cache.popitem(last=False)  # LRU eviction
        self.cache[key] = value
```

#### 2. Redis Cache (`redis_cache.py` - 53,980 lines!)
- Supports Redis Cluster
- TTL management
- Cache invalidation patterns
- Semantic caching (vector-based similarity)

#### 3. Disk Cache (`disk_cache.py`)
- SQLite-based persistent cache
- File system storage
- Good for development/testing

#### 4. S3 Cache (`s3_cache.py`)
- AWS S3 as cache backend
- For multi-region deployments
- Shared cache across instances

### Cache Handler (`caching_handler.py`)

```python
async def cache_handler(
    request: CompletionRequest,
    cache: Cache,
    ttl: int,
) -> Optional[CompletionResponse]:
    # Generate cache key
    key = generate_cache_key(request)

    # Check cache
    cached_response = await cache.get(key)
    if cached_response:
        return CompletionResponse.from_dict(cached_response)

    # Execute request (cache miss)
    response = await execute_completion(request)

    # Store in cache
    await cache.set(key, response.to_dict(), ttl)

    return response
```

**Cache Key Generation**:
```python
def generate_cache_key(request: CompletionRequest) -> str:
    # Hash deterministic fields only
    hashable = {
        "model": request.model,
        "messages": request.messages,
        "temperature": request.temperature,
        "max_tokens": request.max_tokens,
        # Exclude: user, logprobs, stream, etc.
    }
    return hashlib.md5(json.dumps(hashable, sort_keys=True).encode()).hexdigest()
```

---

## Configuration Management

### Model Configuration (`model_prices_and_context_window.json`)

**1.2MB JSON file** with pricing and capabilities for all models:

```json
{
  "gpt-4": {
    "litellm_provider": "openai",
    "mode": "chat",
    "input_cost_per_token": 0.00003,
    "output_cost_per_token": 0.00006,
    "max_tokens": 8192,
    "max_input_tokens": 8192,
    "max_output_tokens": 4096,
    "supports_vision": false,
    "supports_function_calling": true,
    "supports_parallel_function_calling": true,
    "supports_assistant_prefill": false
  },
  "claude-3-opus-20240229": {
    "litellm_provider": "anthropic",
    "mode": "chat",
    "input_cost_per_token": 0.000015,
    "output_cost_per_token": 0.000075,
    "max_tokens": 200000,
    "max_input_tokens": 200000,
    "max_output_tokens": 4096,
    "supports_vision": true,
    "supports_function_calling": true,
    "supports_parallel_function_calling": false,
    "supports_assistant_prefill": true
  }
}
```

### API Key Resolution

**Priority Order**:
1. Explicit parameter (`api_key="sk-..."`)
2. Environment variable (`OPENAI_API_KEY`, `ANTHROPIC_API_KEY`)
3. Secret manager (AWS Secrets Manager, Azure Key Vault, Google Secret Manager)
4. Config file

**Dynamic Environment Variables**:
```python
# Support for dynamic keys
model = "openai/gpt-4"
api_key = "os.environ/CUSTOM_OPENAI_KEY"  # Special syntax
# LiteLLM will fetch from os.environ["CUSTOM_OPENAI_KEY"]
```

---

## Exception Hierarchy (`exceptions.py`)

OpenAI-compatible exceptions:

```python
# Base exception
class LiteLLMException(Exception):
    pass

# HTTP status-based exceptions (all inherit from OpenAI's exceptions)
class AuthenticationError(LiteLLMException):  # 401
    pass

class NotFoundError(LiteLLMException):  # 404
    pass

class BadRequestError(LiteLLMException):  # 400
    pass

class RateLimitError(LiteLLMException):  # 429
    def __init__(self, message, retry_after=None):
        self.retry_after = retry_after  # Seconds to wait
        super().__init__(message)

class ServiceUnavailableError(LiteLLMException):  # 503
    pass

class Timeout(LiteLLMException):
    pass

class ContentPolicyViolationError(LiteLLMException):
    pass
```

**Error Mapping**:
Each provider maps its errors to these standardized exceptions.

---

## Constants and Defaults (`constants.py`)

```python
# Retry
DEFAULT_MAX_RETRIES = 2
DEFAULT_ALLOWED_FAILS = 3
DEFAULT_COOLDOWN_TIME_SECONDS = 5

# Tokens
DEFAULT_MAX_TOKENS = 4096

# Router
ROUTER_MAX_FALLBACKS = 5

# Timeouts
DEFAULT_TIMEOUT = 600  # seconds

# Connection pooling (aiohttp)
DEFAULT_MAX_PARALLEL_REQUESTS = 1000
DEFAULT_MAX_CONNECTIONS = 100
DEFAULT_MAX_KEEPALIVE_CONNECTIONS = 20

# SSL/TLS
DEFAULT_SSL_CIPHERS = [
    "ECDHE-RSA-AES128-GCM-SHA256",
    "ECDHE-RSA-AES256-GCM-SHA384",
    # ... more
]
```

---

## Type System (`types/` directory)

Extensive Pydantic models for validation:

```python
class ModelResponse(BaseModel):
    id: str
    choices: List[Choices]
    created: int
    model: str
    usage: Optional[Usage] = None
    system_fingerprint: Optional[str] = None

class Choices(BaseModel):
    index: int
    message: Message
    finish_reason: Optional[str] = None

class Message(BaseModel):
    role: str
    content: Optional[str] = None
    tool_calls: Optional[List[ToolCall]] = None
    function_call: Optional[FunctionCall] = None

class Usage(BaseModel):
    prompt_tokens: int
    completion_tokens: int
    total_tokens: int
```

---

## Key Insights for Rust Rebuild

### 1. **Modular Provider System**
- Each provider is self-contained
- Transformation pattern is consistent
- Easy to add new providers (just implement the Config class)

### 2. **Async-First Design**
- Heavy use of `async/await` throughout
- Connection pooling critical for performance
- Streaming requires special handling (SSE parsing)

### 3. **HTTP-Centric**
- Most complexity is HTTP client management
- Connection pooling, timeouts, retries
- SSL/TLS configuration important

### 4. **Configuration-Driven**
- JSON file for model metadata (pricing, context windows)
- Environment variables for secrets
- Flexible fallback to defaults

### 5. **Router Complexity**
- Multiple routing strategies needed
- State management (inflight requests, usage tracking, latency metrics)
- Cooldown mechanism prevents thundering herd

### 6. **Error Handling**
- Comprehensive error mapping to OpenAI exceptions
- Retry logic needs to be error-specific
- Rate limit handling with backoff

### 7. **Caching Strategies**
- Multiple backends for different use cases
- Cache key generation is critical (deterministic hashing)
- TTL management

### 8. **Type Safety**
- Pydantic for runtime validation
- Rust can do this at compile-time with strong typing

---

## Recommendations for SimpleAgents (Rust)

### Keep
- Provider abstraction pattern (trait-based in Rust)
- Transformation pipeline (request → provider → response)
- Retry and fallback mechanisms
- Configuration-driven design
- Error hierarchy (use `thiserror` crate)

### Simplify
- Start with 3 providers for MVP (OpenAI, Anthropic, OpenRouter)
- Single routing strategy initially (round-robin)
- In-memory cache only at first
- Fewer configuration options (focus on essentials)

### Improve with Rust
- Compile-time type safety (no Pydantic needed)
- Zero-cost abstractions (trait objects, generics)
- Better concurrency with Tokio
- Memory safety guarantees
- Smaller binary size

### Avoid
- Too many providers upfront (focus on quality over quantity)
- Complex proxy features for MVP
- Multiple cache backends initially
- Advanced routing strategies before MVP

---

## Code Statistics

- **Total lines**: ~24,500 in core files
- **Main entry point**: `main.py` (7,258 lines)
- **Utilities**: `utils.py` (8,930 lines)
- **Router**: `router.py` (8,334 lines)
- **Providers**: 115 subdirectories in `llms/`
- **Dependencies**: 50+ Python packages

---

## Critical Files for Reference

1. `/Users/rishub/Desktop/projects/enterprise/craftsmanlabs/litellm/litellm/main.py`
   - Main completion functions
   - Entry point logic

2. `/Users/rishub/Desktop/projects/enterprise/craftsmanlabs/litellm/litellm/llms/custom_httpx/llm_http_handler.py`
   - HTTP client orchestration
   - Streaming implementation

3. `/Users/rishub/Desktop/projects/enterprise/craftsmanlabs/litellm/litellm/llms/base_llm/chat/transformation.py`
   - Base transformation pattern
   - Template for provider implementations

4. `/Users/rishub/Desktop/projects/enterprise/craftsmanlabs/litellm/litellm/router.py`
   - Routing logic
   - Fallback and retry mechanisms

5. `/Users/rishub/Desktop/projects/enterprise/craftsmanlabs/litellm/model_prices_and_context_window.json`
   - Model metadata
   - Pricing and capabilities

6. `/Users/rishub/Desktop/projects/enterprise/craftsmanlabs/litellm/litellm/exceptions.py`
   - Error hierarchy
   - Exception mapping

---

## Conclusion

LiteLLM's architecture provides a solid foundation for understanding how to build a unified LLM interface. The key takeaway is the **transformation pattern** - each provider implements request/response transformations while the core orchestrates HTTP calls, retries, and routing. Rust can improve upon this with stronger typing, better performance, and memory safety guarantees.
