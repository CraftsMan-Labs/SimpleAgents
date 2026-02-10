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

SAClient *sa_client_new_from_env(const char *provider_name);
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

char *sa_last_error_message(void);
void sa_string_free(char *value);

#ifdef __cplusplus
}
#endif

#endif
