# Go Workflow Example

Go example source:

- `bindings/go/examples/workflow_email/main.go`

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
