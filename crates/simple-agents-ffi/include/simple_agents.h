#ifndef SIMPLE_AGENTS_H
#define SIMPLE_AGENTS_H

#include <stdint.h>
#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct SAClient SAClient;

/* Callback invoked for each streaming chunk (JSON string). Return 0 to continue, non-zero to cancel. */
typedef int32_t (*SAStreamCallback)(const char *event_json, void *user_data);

/*
 * Create a new client.
 *
 * Parameters:
 * - api_key: OpenAI-compatible API key (required, must not be NULL).
 * - model:   Reserved for future use; pass NULL.
 * - base_url: Override base URL (e.g. "https://api.openai.com/v1"). Pass NULL for the default.
 *
 * Returns a live client pointer, or NULL on error (check sa_last_error_message).
 * Must be freed with sa_client_free.
 */
SAClient *sa_client_new(
    const char *api_key,
    const char *model,
    const char *base_url
);

/*
 * Free a client created by sa_client_new.
 * Safe to call with NULL.
 */
void sa_client_free(SAClient *client);

/*
 * Execute a completion request.
 *
 * Parameters:
 * - client:       Live pointer from sa_client_new; must not be NULL.
 * - request_json: JSON string matching the CompletionRequest schema.
 *
 * Returns a JSON string with the CompletionResponse, or NULL on error.
 * Caller must free with sa_string_free.
 */
char *sa_complete(SAClient *client, const char *request_json);

/*
 * Stream a completion request.
 *
 * Parameters:
 * - client:       Live pointer from sa_client_new; must not be NULL.
 * - request_json: JSON string with "stream": true.
 * - callback:     Called for each chunk; return 0 to continue, non-zero to cancel.
 * - user_data:    Passed through to callback unchanged.
 *
 * Returns 0 on success, -1 on error (check sa_last_error_message).
 */
int32_t sa_stream(
    SAClient *client,
    const char *request_json,
    SAStreamCallback callback,
    void *user_data
);

/*
 * Run a workflow YAML file synchronously.
 *
 * Parameters:
 * - client:     Live pointer from sa_client_new; must not be NULL.
 * - yaml_path:  Filesystem path to the workflow YAML file.
 * - input_json: JSON object for the workflow input.
 *
 * Returns a JSON string with the YamlWorkflowRunOutput, or NULL on error.
 * Caller must free with sa_string_free.
 */
char *sa_run_workflow(
    SAClient *client,
    const char *yaml_path,
    const char *input_json
);

/*
 * Get the last error message for the current thread.
 * Returns NULL if no error.
 * Caller must free with sa_string_free.
 */
char *sa_last_error_message(void);

/*
 * Free a string returned by sa_complete, sa_run_workflow, or sa_last_error_message.
 * Safe to call with NULL.
 */
void sa_string_free(char *value);

#ifdef __cplusplus
}
#endif

#endif
