#ifndef SIMPLE_AGENTS_H
#define SIMPLE_AGENTS_H

#include <stdint.h>
#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct SAClient SAClient;

typedef struct {
    const char *role;
    const char *content;
    const char *name;
    const char *tool_call_id;
} SAMessage;

typedef int32_t (*SAStreamCallback)(const char *event_json, void *user_data);
typedef int32_t (*SAWorkflowEventCallback)(const char *event_json, void *user_data);

SAClient *sa_client_new_from_env(const char *provider_name);
SAClient *sa_client_new_with_credentials(
    const char *provider_name,
    const char *api_key,
    const char *api_base
);
void sa_client_free(SAClient *client);

char *sa_complete(
    SAClient *client,
    const char *model,
    const char *prompt,
    int32_t max_tokens,
    float temperature
);

char *sa_complete_messages_json(
    SAClient *client,
    const char *model,
    const SAMessage *messages,
    size_t messages_len,
    int32_t max_tokens,
    float temperature,
    float top_p,
    const char *mode,
    const char *schema_json
);

int32_t sa_stream_messages(
    SAClient *client,
    const char *model,
    const SAMessage *messages,
    size_t messages_len,
    int32_t max_tokens,
    float temperature,
    float top_p,
    SAStreamCallback callback,
    void *user_data
);

char *sa_run_workflow_yaml(
    SAClient *client,
    const char *workflow_path,
    const char *workflow_input_json
);

char *sa_run_workflow_yaml_with_options(
    SAClient *client,
    const char *workflow_path,
    const char *workflow_input_json,
    const char *workflow_options_json
);

char *sa_run_workflow_yaml_with_events(
    SAClient *client,
    const char *workflow_path,
    const char *workflow_input_json,
    const char *workflow_options_json
);

/*
 * Stream workflow YAML: invokes callback with JSON workflow events, returns final output JSON.
 *
 * Parameters (all strings UTF-8, null-terminated except as noted):
 * - client: from sa_client_new_* ; must not be NULL.
 * - workflow_path: filesystem path to workflow YAML.
 * - workflow_input_json: JSON object (workflow input).
 * - workflow_options_json: NULL or JSON for telemetry/trace/model (YamlWorkflowRunOptions).
 * - workflow_execution_flags_json: NULL, empty string, or JSON object for YamlWorkflowExecutionFlags.
 *   When NULL/empty, Rust uses defaults: healing=false, workflow_streaming=false,
 *   node_llm_streaming=true, split_stream_deltas=false.
 *   Go bindings always pass a non-null JSON object with all four keys for discoverability.
 *   Keys (all optional; omitted keys keep defaults): "healing", "workflow_streaming",
 *   "node_llm_streaming", "split_stream_deltas" (bool). split_stream_deltas=true emits
 *   separate thinking vs output stream events (e.g. node_stream_thinking_delta).
 * - callback: called with each event JSON string; must not be NULL.
 * - user_data: passed through to callback.
 * Return: JSON string (free with sa_string_free) or NULL on error (see sa_last_error_message).
 */
char *sa_run_workflow_yaml_stream_events(
    SAClient *client,
    const char *workflow_path,
    const char *workflow_input_json,
    const char *workflow_options_json,
    const char *workflow_execution_flags_json,
    SAWorkflowEventCallback callback,
    void *user_data
);

char *sa_last_error_message(void);
void sa_string_free(char *value);

#ifdef __cplusplus
}
#endif

#endif
