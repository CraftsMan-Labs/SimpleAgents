// Package simpleagents provides Go bindings for the SimpleAgents library.
//
// The unified API surface is:
//
//	client, _ := simpleagents.NewClient(apiKey, "")
//	defer client.Close()
//
//	// Direct LLM completion
//	result, _ := client.Complete(ctx, requestJSON)
//
//	// Streaming LLM completion
//	_ = client.StreamComplete(ctx, requestJSON, func(chunkJSON string) error { ... })
//
//	// Workflow (blocking)
//	output, _ := client.Run(ctx, "workflow.yaml", inputJSON)
//
//	// Workflow (streaming events)
//	output, _ := client.Stream(ctx, "workflow.yaml", inputJSON, func(eventJSON string) error { ... })
//
//	// Resume from checkpoint
//	output, _ := client.Resume(ctx, checkpointJSON)
//
// Tools are included in the workflow YAML definition and are executed by the
// Rust engine. Runtime tool executors are not yet supported from Go.
package simpleagents

/*
#cgo CFLAGS: -I${SRCDIR}/../../crates/simple-agents-ffi/include
#cgo LDFLAGS: -lsimple_agents_ffi

#include <stdlib.h>
#include <stdint.h>
#include "simple_agents.h"

// Bridge callbacks: Go callbacks can't be passed as C function pointers directly.
// We use an integer "handle" as user_data and call back into Go via the exported bridge.
extern int32_t saGoStreamCallbackBridge(char *event_json, size_t user_handle);

static int32_t sa_stream_callback_trampoline(const char *event_json, void *user_data) {
    return saGoStreamCallbackBridge((char *)event_json, (size_t)user_data);
}

static int32_t sa_stream_go(SAClient *client, const char *request_json, size_t user_handle) {
    return sa_stream(client, request_json, sa_stream_callback_trampoline, (void *)user_handle);
}

static int32_t sa_stream_workflow_go(
    SAClient *client,
    const char *yaml_path,
    const char *input_json,
    size_t user_handle
) {
    return sa_stream_workflow(client, yaml_path, input_json, sa_stream_callback_trampoline, (void *)user_handle);
}
*/
import "C"

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"runtime/cgo"
	"unsafe"
)

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

// MessageRole is the role of a conversation message.
type MessageRole string

const (
	MessageRoleSystem    MessageRole = "system"
	MessageRoleUser      MessageRole = "user"
	MessageRoleAssistant MessageRole = "assistant"
	MessageRoleTool      MessageRole = "tool"
)

// Message is a single conversation turn.
type Message struct {
	Role       MessageRole `json:"role"`
	Content    string      `json:"content"`
	Name       string      `json:"name,omitempty"`
	ToolCallID string      `json:"tool_call_id,omitempty"`
}

// WorkflowInput is a convenience wrapper for the messages envelope that YAML
// workflows expect.
type WorkflowInput struct {
	Messages []Message              `json:"messages"`
	Extra    map[string]interface{} `json:"-"`
}

// MarshalJSON merges Extra fields with Messages into a single JSON object.
func (w WorkflowInput) MarshalJSON() ([]byte, error) {
	m := make(map[string]interface{}, len(w.Extra)+1)
	for k, v := range w.Extra {
		m[k] = v
	}
	m["messages"] = w.Messages
	return json.Marshal(m)
}

// RunOpts holds optional per-run settings.
type RunOpts struct {
	// WorkflowOptionsJSON is an optional JSON object matching YamlWorkflowRunOptions.
	// Pass nil to use defaults.
	WorkflowOptionsJSON []byte
}

// ---------------------------------------------------------------------------
// Client
// ---------------------------------------------------------------------------

// Client wraps the Rust SimpleAgents FFI client.
type Client struct {
	ptr *C.SAClient
}

// NewClient creates a new client from an explicit API key.
// Pass baseURL as empty string to use the OpenAI default.
func NewClient(apiKey, baseURL string) (*Client, error) {
	cKey := C.CString(apiKey)
	defer C.free(unsafe.Pointer(cKey))

	var cBase *C.char
	if baseURL != "" {
		cBase = C.CString(baseURL)
		defer C.free(unsafe.Pointer(cBase))
	}

	ptr := C.sa_client_new(cKey, nil, cBase)
	if ptr == nil {
		return nil, fmt.Errorf("sa_client_new failed: %s", lastError())
	}
	return &Client{ptr: ptr}, nil
}

// Close frees the underlying Rust client. Must be called when done.
func (c *Client) Close() {
	if c.ptr != nil {
		C.sa_client_free(c.ptr)
		c.ptr = nil
	}
}

// ---------------------------------------------------------------------------
// Direct LLM calls
// ---------------------------------------------------------------------------

// Complete sends a completion request (JSON in, JSON out).
// requestJSON must be a JSON object matching the CompletionRequest schema.
func (c *Client) Complete(_ context.Context, requestJSON []byte) ([]byte, error) {
	cReq := C.CString(string(requestJSON))
	defer C.free(unsafe.Pointer(cReq))

	result := C.sa_complete(c.ptr, cReq)
	if result == nil {
		return nil, fmt.Errorf("sa_complete failed: %s", lastError())
	}
	defer C.sa_string_free(result)
	return []byte(C.GoString(result)), nil
}

// StreamComplete streams a completion, calling onChunk for each chunk JSON.
// requestJSON must have "stream": true.
func (c *Client) StreamComplete(_ context.Context, requestJSON []byte, onChunk func(chunkJSON string) error) error {
	handle := cgo.NewHandle(onChunk)
	defer handle.Delete()

	cReq := C.CString(string(requestJSON))
	defer C.free(unsafe.Pointer(cReq))

	status := C.sa_stream_go(c.ptr, cReq, C.size_t(handle))
	if status != 0 {
		return fmt.Errorf("sa_stream failed: %s", lastError())
	}
	return nil
}

// ---------------------------------------------------------------------------
// Workflow calls
// ---------------------------------------------------------------------------

// Run executes a YAML workflow file and returns the output JSON.
// inputJSON is a JSON object (at minimum {"messages": [...]}).
func (c *Client) Run(_ context.Context, workflowPath string, inputJSON []byte) ([]byte, error) {
	cPath := C.CString(workflowPath)
	defer C.free(unsafe.Pointer(cPath))
	cInput := C.CString(string(inputJSON))
	defer C.free(unsafe.Pointer(cInput))

	result := C.sa_run_workflow(c.ptr, cPath, cInput)
	if result == nil {
		return nil, fmt.Errorf("sa_run_workflow failed: %s", lastError())
	}
	defer C.sa_string_free(result)
	return []byte(C.GoString(result)), nil
}

// RunWithMessages is a convenience wrapper around Run that accepts typed messages.
func (c *Client) RunWithMessages(ctx context.Context, workflowPath string, messages []Message, opts *RunOpts) ([]byte, error) {
	input := WorkflowInput{Messages: messages}
	inputJSON, err := json.Marshal(input)
	if err != nil {
		return nil, fmt.Errorf("marshal input: %w", err)
	}
	return c.Run(ctx, workflowPath, inputJSON)
}

// Stream executes a YAML workflow file, calling onEvent for each event JSON.
// Tools are configured in the YAML — onEvent is for observability, not tool dispatch.
func (c *Client) Stream(_ context.Context, workflowPath string, inputJSON []byte, onEvent func(eventJSON string) error) error {
	handle := cgo.NewHandle(onEvent)
	defer handle.Delete()

	cPath := C.CString(workflowPath)
	defer C.free(unsafe.Pointer(cPath))
	cInput := C.CString(string(inputJSON))
	defer C.free(unsafe.Pointer(cInput))

	status := C.sa_stream_workflow_go(c.ptr, cPath, cInput, C.size_t(handle))
	if status != 0 {
		return fmt.Errorf("sa_stream_workflow failed: %s", lastError())
	}
	return nil
}

// StreamWithMessages is a convenience wrapper around Stream that accepts typed messages.
func (c *Client) StreamWithMessages(ctx context.Context, workflowPath string, messages []Message, onEvent func(eventJSON string) error, opts *RunOpts) error {
	input := WorkflowInput{Messages: messages}
	inputJSON, err := json.Marshal(input)
	if err != nil {
		return fmt.Errorf("marshal input: %w", err)
	}
	return c.Stream(ctx, workflowPath, inputJSON, onEvent)
}

// Resume restarts a workflow from a serialized checkpoint JSON.
// checkpointJSON is the JSON-encoded WorkflowCheckpoint from a failed run.
func (c *Client) Resume(_ context.Context, checkpointJSON []byte) ([]byte, error) {
	cCheckpoint := C.CString(string(checkpointJSON))
	defer C.free(unsafe.Pointer(cCheckpoint))

	result := C.sa_resume(c.ptr, cCheckpoint)
	if result == nil {
		return nil, fmt.Errorf("sa_resume failed: %s", lastError())
	}
	defer C.sa_string_free(result)
	return []byte(C.GoString(result)), nil
}

// ---------------------------------------------------------------------------
// Callback bridge (exported to C)
// ---------------------------------------------------------------------------

//export saGoStreamCallbackBridge
func saGoStreamCallbackBridge(eventJSON *C.char, userHandle C.size_t) C.int32_t {
	h := cgo.Handle(userHandle)
	fn, ok := h.Value().(func(string) error)
	if !ok {
		return -1
	}
	if err := fn(C.GoString(eventJSON)); err != nil {
		return -1
	}
	return 0
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

func lastError() string {
	msg := C.sa_last_error_message()
	if msg == nil {
		return "(no error message)"
	}
	defer C.sa_string_free(msg)
	return C.GoString(msg)
}

// ErrWorkflow wraps a workflow execution error with its message.
type ErrWorkflow struct {
	Msg string
}

func (e *ErrWorkflow) Error() string { return e.Msg }

// ParseWorkflowOutput parses a raw JSON workflow output into a map.
func ParseWorkflowOutput(raw []byte) (map[string]interface{}, error) {
	if len(raw) == 0 {
		return nil, errors.New("empty output")
	}
	var out map[string]interface{}
	if err := json.Unmarshal(raw, &out); err != nil {
		return nil, fmt.Errorf("unmarshal workflow output: %w", err)
	}
	return out, nil
}
