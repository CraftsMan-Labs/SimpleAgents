# Go Workflow Example

Go example source:

- `bindings/go/examples/workflow_email/main.go`
- `bindings/go/examples/workflow_chat_history/main.go`

Run command:

```bash
cargo build -p simple-agents-ffi --release
CGO_CFLAGS="-I$PWD/crates/simple-agents-ffi/include" \
CGO_LDFLAGS="-L$PWD/target/release" \
LD_LIBRARY_PATH="$PWD/target/release:${LD_LIBRARY_PATH:-}" \
go run ./bindings/go/examples/workflow_email \
  examples/workflow_email/email-intake-classification.yaml \
  "Termination request, second warning already issued"
```

Interactive chat-history workflow runner (equivalent to Python `run_with_chat_history.py`):

```bash
WORKFLOW_PROVIDER=openai \
WORKFLOW_API_BASE=https://api.openai.com/v1 \
WORKFLOW_API_KEY=your_key_here \
make run-go-chat-history

# Run a different YAML file:
make run-go-chat-history WORKFLOW_YAML=examples/workflow_email/python-intern-fun-interview-system.yaml

# Pass extra runner flags:
make run-go-chat-history GO_CHAT_FLAGS="--include-events --stream --show-thinking --show-step-json"

# Optional flags for Python parity:
# --include-events
# --stream
# --show-thinking
# --show-step-json
```

## Using this in your own Go project

Create a `main.go` that mirrors the Python chat-history runner flow:

```go
package main

import (
	"context"
	"fmt"

	"simpleagents"
)

func main() {
	ctx := context.Background()
	client, err := simpleagents.NewClientFromEnv("openai")
	if err != nil {
		panic(err)
	}
	defer client.Close()

	workflowPath := "workflow_email/email-chat-draft-or-clarify.yaml"
	workflowInput := map[string]any{
		"email_text": "Can you draft a polite follow-up for order 8821?",
		"messages": []map[string]any{
			{"role": "system", "content": "You are a friendly email drafting assistant."},
			{"role": "user", "content": "Can you draft a polite follow-up for order 8821?"},
		},
	}

	out, err := client.RunWorkflowYAMLWithEvents(ctx, workflowPath, workflowInput, nil)
	if err != nil {
		panic(err)
	}

	fmt.Println("terminal node:", out.TerminalNode)
	fmt.Println("assistant:", out.TerminalOutput)
	fmt.Println("events:", len(out.Events))
}
```

This is the same model as Python `run_with_chat_history.py`: pass `email_text` + `messages` each turn, then append assistant output back into `messages` for the next turn.
