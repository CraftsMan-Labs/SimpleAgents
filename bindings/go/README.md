# Go bindings

Go bindings for SimpleAgents using cgo and the C FFI.

## Build

```sh
cargo build -p simple-agents-ffi --release
```

Ensure the dynamic library is on your loader path (e.g. `LD_LIBRARY_PATH=target/release`).

## Usage

```go
package main

import (
    "fmt"
    "log"

    "simpleagents"
)

func main() {
    client, err := simpleagents.NewClientFromEnv("openai")
    if err != nil {
        log.Fatal(err)
    }
    defer client.Close()

    response, err := client.Complete("gpt-4", "Hello from Go!", 128, 0.7)
    if err != nil {
        log.Fatal(err)
    }

    fmt.Println(response)
}
```
