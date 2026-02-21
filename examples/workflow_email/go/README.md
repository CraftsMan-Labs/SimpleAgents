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
cargo build -p simple-agents-ffi --release
CGO_CFLAGS="-I$PWD/crates/simple-agents-ffi/include" \
CGO_LDFLAGS="-L$PWD/target/release" \
LD_LIBRARY_PATH="$PWD/target/release:${LD_LIBRARY_PATH:-}" \
go run ./bindings/go/examples/workflow_chat_history \
  --workflow examples/workflow_email/email-chat-draft-or-clarify.yaml

# Optional flags for Python parity:
# --include-events
# --stream
# --show-thinking
# --show-step-json
```
