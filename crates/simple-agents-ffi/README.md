# simple-agents-ffi

C-compatible bindings for SimpleAgents.

## Build

```sh
cargo build -p simple-agents-ffi --release
```

The header is at `crates/simple-agents-ffi/include/simple_agents.h`.

## Usage

```c
#include "simple_agents.h"
#include <stdio.h>

int main() {
    SAClient *client = sa_client_new_from_env("openai");
    if (!client) {
        char *err = sa_last_error_message();
        fprintf(stderr, "Error: %s\n", err ? err : "unknown");
        sa_string_free(err);
        return 1;
    }

    char *response = sa_complete(client, "gpt-4", "Hello!", 128, 0.7f);
    if (!response) {
        char *err = sa_last_error_message();
        fprintf(stderr, "Error: %s\n", err ? err : "unknown");
        sa_string_free(err);
        sa_client_free(client);
        return 1;
    }

    printf("%s\n", response);
    sa_string_free(response);
    sa_client_free(client);
    return 0;
}
```

## Notes

- `sa_client_new_from_env` expects provider env vars (e.g. `OPENAI_API_KEY`).
- Pass `max_tokens <= 0` to omit max tokens.
- Pass `temperature < 0.0` to omit temperature.
