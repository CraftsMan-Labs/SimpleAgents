# simple-agents-cli

Command-line interface for SimpleAgents. Use it to run single completions, chat interactively, benchmark prompts, and check provider health.

## Install

From the workspace root:

```bash
cargo build -p simple-agents-cli
```

## Usage

```bash
# Single completion
cargo run -p simple-agents-cli -- complete \
  --model gpt-4 \
  --provider openai \
  --max-tokens 128 \
  "Summarize the HTTP/2 benefits"

# Interactive chat
cargo run -p simple-agents-cli -- chat --model gpt-4 --provider openai

# Benchmark a prompt
cargo run -p simple-agents-cli -- benchmark \
  --model gpt-4 \
  --provider openai \
  --runs 10 \
  "Explain Rust ownership"

# Provider health check
cargo run -p simple-agents-cli -- test-provider --model gpt-4 --provider openai

# Render workflow as Mermaid (YAML or IR JSON)
cargo run -p simple-agents-cli -- workflow mermaid examples/workflow_email/email-chat-draft-or-clarify.yaml

# Workflow trace utilities
cargo run -p simple-agents-cli -- workflow trace path/to/trace.json
cargo run -p simple-agents-cli -- workflow replay path/to/trace.json
cargo run -p simple-agents-cli -- workflow inspect path/to/trace.json classify_top_level
```

## Configuration

Provide a TOML or YAML file with provider credentials and defaults:

```toml
# simple-agents.toml

[defaults]
model = "gpt-4"
max_tokens = 256
temperature = 0.7
system = "You are concise."

[[providers]]
kind = "openai"
api_key_env = "OPENAI_API_KEY"
base_url = "https://api.openai.com/v1"

[[providers]]
kind = "anthropic"
api_key_env = "ANTHROPIC_API_KEY"
base_url = "https://api.anthropic.com/v1"

[routing]
mode = "round_robin"
```

Then run:

```bash
cargo run -p simple-agents-cli -- --config simple-agents.toml complete "Hello"
```

### Output Formats

```bash
cargo run -p simple-agents-cli -- --output json complete --model gpt-4 "Hello"
```

Supported output formats: `plain`, `json`, `markdown`.
