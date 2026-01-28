 # SimpleAgents Examples

This directory contains practical examples demonstrating SimpleAgents features.

## python_client.py

**Location:** `examples/python_client.py`

Demonstrates using the Python bindings with an explicit provider, API base, and API key.

### Running

From the project root:
```bash
uv pip install -e crates/simple-agents-py
uv run python examples/python_client.py
```

### Notes

- Update `api_key` in the example with a real key before running.
- You can switch to another provider by changing the first `Client` argument.

## custom_api.rs

**Location:** `crates/simple-agents-providers/examples/custom_api.rs`

Demonstrates using SimpleAgents with custom OpenAI-compatible APIs including **response healing**.

### Features Demonstrated

1. **Custom Base URL** - Configure provider with custom API endpoint
2. **Simple Completion** - Basic request/response with custom API
3. **Streaming Response** - Real-time streaming from custom API
4. **Multi-turn Conversation** - Context-aware dialogues
5. **Response Healing** - Parse and heal malformed JSON
6. **Type Coercion** - Convert values to proper types
7. **Fuzzy Field Matching** - Handle case variations in field names
8. **Streaming Healing** - Progressive JSON parsing during streaming
9. **Streaming Structured Output** - Partial JSON emission mid-stream
10. **Metrics Collection** - Track token usage and latency

 ### Prerequisites

1. Create a `.env` file in the examples directory:
   ```bash
   cat > .env << 'EOF'
   # Add your custom API configuration
   CUSTOM_API_BASE=https://your-api-endpoint.com/v1
   CUSTOM_API_KEY=your-api-key
   CUSTOM_API_MODEL=your-model-name
   EOF
   ```

2. Or use one of the configuration examples below

### Configuration Examples

#### Azure OpenAI Service

```bash
CUSTOM_API_BASE=https://your-resource.openai.azure.com/openai/deployments/your-deployment
CUSTOM_API_KEY=your-azure-api-key
CUSTOM_API_MODEL=gpt-4
```

#### Local vLLM Server

First, start vLLM server:
```bash
python -m vllm.entrypoints.openai.api_server --model meta-llama/Llama-2-70b-chat-hf
```

Then configure:
```bash
CUSTOM_API_BASE=http://localhost:8000/v1
CUSTOM_API_KEY=dummy-key
CUSTOM_API_MODEL=meta-llama/Llama-2-70b-chat-hf
```

#### Ollama with OpenAI Compatibility

Start Ollama with CORS enabled:
```bash
OLLAMA_ORIGINS="*" ollama serve
```

Configure:
```bash
CUSTOM_API_BASE=http://localhost:11434/v1
CUSTOM_API_KEY=ollama
CUSTOM_API_MODEL=llama2
```

#### Custom Proxy Server

```bash
CUSTOM_API_BASE=https://your-proxy-server.com/v1
CUSTOM_API_KEY=your-proxy-api-key
CUSTOM_API_MODEL=gpt-3.5-turbo
```

 ### Running

```bash
cargo run --example custom_api
```

**Note:** This example is located in `crates/simple-agents-providers/examples/custom_api.rs` and requires the healing crate as a dev dependency.

### What You'll See

The example runs 9 scenarios:

1. **Simple Completion**
   - Sends a single prompt
   - Receives and displays response
   - Shows token usage

2. **Streaming Response**
   - Sends a request with streaming enabled
   - Displays text as it arrives character-by-character
   - Tracks chunk count and length

3. **Multi-turn Conversation**
   - Maintains conversation context
   - Sends follow-up questions
   - Demonstrates context awareness

4. **Response Healing - JSON Parsing**
   - Requests JSON response
   - Parses potentially malformed JSON
   - Shows confidence score and healing applied

5. **Type Coercion**
   - Requests data with mixed value types
   - Demonstrates automatic type conversion
   - Shows coercion flags and type verification

6. **Fuzzy Field Matching**
   - Requests data with inconsistent field naming
   - Demonstrates case-insensitive matching
   - Shows normalization to standard field names

7. **Streaming + Response Healing**
   - Streams JSON response with healing
   - Applies progressive parsing as chunks arrive
   - Tracks healing operations during streaming

8. **Streaming Structured Output**
   - Streams JSON array
   - Shows partial parses as they complete
   - Demonstrates progressive structured emission

9. **Streaming Graph Visualization**
   - Streams graph data (nodes + edges)
   - Live ASCII graph visualization during streaming
   - Progressive updates as graph structure emerges
   - Node type distribution and connectivity analysis

### Example Output

```
╔══════════════════════════════════════════════════════════╗
║     SimpleAgents - Custom API Endpoint Demo              ║
╚══════════════════════════════════════════════════════════╝

📋 Configuration:
  Base URL: http://localhost:8000/v1
  Model: meta-llama/Llama-2-70b-chat-hf
  API Key: dum***key (hidden for security)

✅ Provider created successfully

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
Example 1: Simple Completion
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

📤 Sending simple completion request...

📨 Response:
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
Rust is a systems programming language focused on safety, concurrency, and performance.
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

📊 Metrics:
  Tokens: 45 prompt + 20 completion = 65 total
```

### Common Issues

**Connection Refused**
- Ensure your local server is running
- Check the base URL is correct
- Verify firewall settings for localhost

**Authentication Error**
- Check that API key is correct for your provider
- Some local servers don't require auth (use "dummy-key")
- Azure requires specific key format

**Model Not Found**
- Verify the model name matches your server
- Local servers may use different model naming
- Check available models via provider's API

**Streaming Not Supported**
- Some OpenAI-compatible APIs don't support streaming
- Try with `stream(false)` for simple completion

### Advanced Usage

**Custom Headers:**
```rust
let provider = OpenAIProvider::with_base_url(api_key, base_url)?;
// The provider automatically adds standard headers
```

**Timeout Configuration:**
```rust
// Default is 30 seconds
// Can be adjusted in Provider trait implementation
```

**Multiple Providers:**
```rust
let azure = OpenAIProvider::with_base_url(azure_key, azure_url)?;
let local = OpenAIProvider::with_base_url(local_key, local_url)?;

// Use different providers based on use case
let provider = if use_local {
    local
} else {
    azure
};
```

### Learn More

- [OpenAI Provider API](../docs/API.md#openai-provider)
- [Streaming Documentation](../docs/USAGE.md#streaming)
- [Custom Server Setup](../docs/ARCHITECTURE.md#provider-system)
- [vLLM Documentation](https://docs.vllm.ai/)
- [Ollama Documentation](https://ollama.com/docs)

## full_api_example.rs

A comprehensive example showing real API integration with coercion healing.

### Features Demonstrated

1. **Real API Calls** - Connects to OpenAI GPT-3.5 using your API key
2. **JSON Healing** - Parses and heals malformed JSON from LLM responses
3. **Type Coercion** - Converts string values to proper types (int, float, bool)
4. **Schema Validation** - Validates responses against strict schemas
5. **Fuzzy Field Matching** - Handles case variations and typos in field names
6. **Streaming Healing** - Progressive JSON parsing with healing during streaming
7. **Streaming Structured Output** - Partial JSON emission mid-stream
8. **Streaming Graph Visualization** - Live graph visualization as data streams
9. **Metrics Collection** - Records request metrics with RequestTimer
10. **Confidence Scoring** - Shows confidence levels for each parse/coercion

### Prerequisites

1. Copy `.env.example` to `.env`:
   ```bash
   cp .env.example .env
   ```

2. Add your OpenAI API key to `.env`:
   ```
   OPENAI_API_KEY=sk-your-actual-api-key-here
   ```

### Running the Example

From the project root:
```bash
cargo run --example full_api_example
```

  ### What You'll See

 The example runs 7 scenarios:

 1. **Basic JSON Healing**
    - Requests simple JSON from GPT-3.5
    - Demonstrates parsing LLM output (often wrapped in markdown)
    - Shows confidence score and any healing applied

 2. **Type Coercion**
    - Requests data with numeric values as strings
    - Shows automatic type conversion (string → int, string → float, string → bool)
    - Displays coercion flags and type verification

 3. **Schema Validation**
    - Requests complex structured data with inconsistent naming
    - Validates against strict schema (camelCase, snake_case, mixed case)
    - Shows fuzzy field matching and default value injection

 4. **Fuzzy Field Matching**
    - Requests data in ALL CAPS
    - Demonstrates case-insensitive matching
    - Shows normalization to lowercase field names

 5. **Streaming + Response Healing**
    - Streams JSON response with healing
    - Applies progressive parsing as chunks arrive
    - Tracks healing operations during streaming

 6. **Streaming Structured Output**
    - Streams JSON array
    - Shows partial parses as they complete
    - Demonstrates progressive structured emission

 7. **Streaming Graph Visualization**
    - Streams graph data (nodes + edges)
    - Live ASCII graph visualization during streaming
    - Progressive updates as graph structure emerges
    - Node type distribution and connectivity analysis

### Example Output

```
╔══════════════════════════════════════════════════════════╗
║   SimpleAgents - Full API + Coercion Healing Demo        ║
╚══════════════════════════════════════════════════════════╝

✅ API key loaded successfully

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
Example 1: Basic JSON Healing
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

📤 Requesting simple JSON response from GPT-3.5...

📨 Raw response from LLM:
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
```json
{
  "name": "Alice",
  "age": 30,
  "city": "San Francisco"
}
```
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

✅ Parse Result:
  Confidence: 0.95
  Parsed value:
  {
    "name": "Alice",
    "age": 30,
    "city": "San Francisco"
  }

  🔧 Healing applied:
    - Stripped markdown code fences

📊 Tokens used:
  Prompt: 38
  Completion: 25
  Total: 63
```

### Key Concepts

**Confidence Scores:**
- `1.0` - Perfect JSON, no healing needed
- `0.95-0.99` - Minor fixes (markdown, trailing commas)
- `0.85-0.94` - Quote normalization or simple fixes
- `0.70-0.84` - Type coercion or truncation
- `<0.70` - Significant healing required

**Coercion Flags:**
- `StrippedMarkdown` - Removed code fences
- `FixedTrailingComma` - Removed trailing commas
- `FixedQuotes` - Normalized single quotes to double
- `StringToNumberCoerced` - Converted string to int/float
- `FuzzyFieldMatch` - Matched field with different casing
- `UsedDefaultValue` - Applied default for missing field

### Troubleshooting

**Error: OPENAI_API_KEY not set**
```bash
# Make sure .env file exists
ls -la .env

# Add your API key
echo "OPENAI_API_KEY=sk-your-key" > .env
```

**Error: Invalid API key**
- Ensure your API key is valid and active
- Check that you have credits remaining on OpenAI

**Error: Rate limit exceeded**
- Wait a minute before running again
- The example makes 4 sequential requests

### Customizing the Example

You can modify the prompts or schemas to test different scenarios:

```rust
// Change the model
.model("gpt-4")  // instead of gpt-3.5-turbo

// Adjust temperature
.temperature(0.9)  // More creative

// Add more fields to schema
Field::optional("metadata", Schema::Object(ObjectSchema { ... })),
```

### Learn More

- [API Documentation](../docs/API.md)
- [Usage Guide](../docs/USAGE.md)
- [Response Healing](../docs/USAGE.md#response-healing)
- [Schema Types](../docs/API.md#schema-types)
