# Chat History Command Snippets

## Node

```bash
make run-node-chat-history

# Optional: run with bun runtime
make run-node-chat-history JS_RUNTIME=bun

# Optional: pick a workflow YAML
make run-node-chat-history WORKFLOW_YAML=examples/workflow_email/python-intern-fun-interview-system.yaml

# Optional parity flags: --include-events --stream --show-thinking --show-step-json
make run-node-chat-history NODE_CHAT_FLAGS="--include-events --stream --show-thinking --show-step-json"
```

## Go

```bash
WORKFLOW_PROVIDER=openai \
WORKFLOW_API_BASE=https://api.openai.com/v1 \
WORKFLOW_API_KEY=your_key_here \
make run-go-chat-history

# Optional: pick a workflow YAML
make run-go-chat-history WORKFLOW_YAML=examples/workflow_email/python-intern-fun-interview-system.yaml

# Optional parity flags: --include-events --stream --show-thinking --show-step-json
make run-go-chat-history GO_CHAT_FLAGS="--include-events --stream --show-thinking --show-step-json"
```

## Cross-language parity

```bash
make run-python-chat-history \
  WORKFLOW_YAML=examples/workflow_email/email-chat-draft-or-clarify.yaml \
  PY_CHAT_FLAGS="--include-events --stream --show-thinking --show-step-json"

make run-go-chat-history \
  WORKFLOW_YAML=examples/workflow_email/email-chat-draft-or-clarify.yaml \
  GO_CHAT_FLAGS="--include-events --stream --show-thinking --show-step-json"
```
