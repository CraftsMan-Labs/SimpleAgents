package simpleagents

/*
#cgo CFLAGS: -I${SRCDIR}/../../crates/simple-agents-ffi/include
#cgo LDFLAGS: -lsimple_agents_ffi

#include <stdlib.h>
#include "simple_agents.h"
*/
import "C"

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"unsafe"
)

// Message represents a chat message for message-based completions.
type Message struct {
	Role       string
	Content    string
	Name       string
	ToolCallID string
}

// CompleteOptions controls completion behavior for CompleteMessages.
type CompleteOptions struct {
	MaxTokens   *int32
	Temperature *float32
	TopP        *float32
	// Mode supports: standard, healed_json, schema
	Mode string
	// Schema is required when Mode is "schema".
	Schema any
}

// ToolCallFunction is a function payload emitted by tool calls.
type ToolCallFunction struct {
	Name      string `json:"name"`
	Arguments string `json:"arguments"`
}

// ToolCall is a tool-call response emitted by the model.
type ToolCall struct {
	ID       string           `json:"id"`
	ToolType string           `json:"tool_type"`
	Function ToolCallFunction `json:"function"`
}

// CompletionUsage contains token accounting for a completion.
type CompletionUsage struct {
	PromptTokens     int32 `json:"prompt_tokens"`
	CompletionTokens int32 `json:"completion_tokens"`
	TotalTokens      int32 `json:"total_tokens"`
}

// HealingData contains parsed/coerced data with transparency metadata.
type HealingData struct {
	Value      any     `json:"value"`
	Flags      any     `json:"flags"`
	Confidence float32 `json:"confidence"`
}

// CompletionResult is the structured result payload returned by CompleteMessages.
type CompletionResult struct {
	ID           string          `json:"id"`
	Model        string          `json:"model"`
	Role         string          `json:"role"`
	Content      string          `json:"content"`
	ToolCalls    []ToolCall      `json:"tool_calls"`
	FinishReason string          `json:"finish_reason"`
	Usage        CompletionUsage `json:"usage"`
	Raw          string          `json:"raw"`
	Healed       *HealingData    `json:"healed"`
	Coerced      *HealingData    `json:"coerced"`
}

type Client struct {
	ptr *C.SAClient
}

func NewClientFromEnv(provider string) (*Client, error) {
	cProvider := C.CString(provider)
	defer C.free(unsafe.Pointer(cProvider))

	ptr := C.sa_client_new_from_env(cProvider)
	if ptr == nil {
		return nil, lastError()
	}

	return &Client{ptr: ptr}, nil
}

func (c *Client) Close() {
	if c == nil || c.ptr == nil {
		return
	}
	C.sa_client_free(c.ptr)
	c.ptr = nil
}

// Complete preserves the original prompt-based API.
func (c *Client) Complete(model, prompt string, maxTokens int32, temperature float32) (string, error) {
	return c.CompleteWithContext(context.Background(), model, prompt, maxTokens, temperature)
}

// CompleteWithContext executes prompt-based completion with context cancellation support.
func (c *Client) CompleteWithContext(
	ctx context.Context,
	model, prompt string,
	maxTokens int32,
	temperature float32,
) (string, error) {
	if err := c.validateClient(); err != nil {
		return "", err
	}
	if err := validatePromptInput(model, prompt); err != nil {
		return "", err
	}
	if ctx == nil {
		ctx = context.Background()
	}

	cModel := C.CString(model)
	defer C.free(unsafe.Pointer(cModel))
	cPrompt := C.CString(prompt)
	defer C.free(unsafe.Pointer(cPrompt))

	type result struct {
		value string
		err   error
	}
	resultCh := make(chan result, 1)
	go func() {
		response := C.sa_complete(c.ptr, cModel, cPrompt, C.int32_t(maxTokens), C.float(temperature))
		if response == nil {
			resultCh <- result{"", lastError()}
			return
		}
		defer C.sa_string_free(response)
		resultCh <- result{C.GoString(response), nil}
	}()

	select {
	case res := <-resultCh:
		return res.value, res.err
	case <-ctx.Done():
		// C calls are not cancellable here. Wait for the goroutine to finish so
		// deferred C.free calls in this function do not race with in-flight cgo.
		<-resultCh
		return "", ctx.Err()
	}
}

// CompleteMessages executes message-based completion and returns structured output.
func (c *Client) CompleteMessages(
	ctx context.Context,
	model string,
	messages []Message,
	opts CompleteOptions,
) (CompletionResult, error) {
	if err := c.validateClient(); err != nil {
		return CompletionResult{}, err
	}
	if err := validateMessagesInput(model, messages); err != nil {
		return CompletionResult{}, err
	}
	if ctx == nil {
		ctx = context.Background()
	}

	cModel := C.CString(model)
	defer C.free(unsafe.Pointer(cModel))

	cMessages := make([]C.SAMessage, len(messages))
	allocated := make([]*C.char, 0, len(messages)*4)
	freeAll := func() {
		for _, p := range allocated {
			if p != nil {
				C.free(unsafe.Pointer(p))
			}
		}
	}
	for i, msg := range messages {
		role := C.CString(msg.Role)
		content := C.CString(msg.Content)
		allocated = append(allocated, role, content)
		cMessages[i].role = role
		cMessages[i].content = content

		if msg.Name != "" {
			name := C.CString(msg.Name)
			allocated = append(allocated, name)
			cMessages[i].name = name
		}
		if msg.ToolCallID != "" {
			toolCallID := C.CString(msg.ToolCallID)
			allocated = append(allocated, toolCallID)
			cMessages[i].tool_call_id = toolCallID
		}
	}
	defer freeAll()

	maxTokens := int32(0)
	if opts.MaxTokens != nil {
		maxTokens = *opts.MaxTokens
	}
	temperature := float32(-1)
	if opts.Temperature != nil {
		temperature = *opts.Temperature
	}
	topP := float32(-1)
	if opts.TopP != nil {
		topP = *opts.TopP
	}

	modeValue := opts.Mode
	if modeValue == "" {
		modeValue = "standard"
	}
	cMode := C.CString(modeValue)
	defer C.free(unsafe.Pointer(cMode))

	var cSchemaJSON *C.char
	if opts.Schema != nil {
		schemaJSON, err := json.Marshal(opts.Schema)
		if err != nil {
			return CompletionResult{}, fmt.Errorf("marshal schema: %w", err)
		}
		cSchemaJSON = C.CString(string(schemaJSON))
		defer C.free(unsafe.Pointer(cSchemaJSON))
	}

	type result struct {
		value CompletionResult
		err   error
	}
	resultCh := make(chan result, 1)
	go func() {
		response := C.sa_complete_messages_json(
			c.ptr,
			cModel,
			(*C.SAMessage)(unsafe.Pointer(&cMessages[0])),
			C.size_t(len(cMessages)),
			C.int32_t(maxTokens),
			C.float(temperature),
			C.float(topP),
			cMode,
			cSchemaJSON,
		)
		if response == nil {
			resultCh <- result{CompletionResult{}, lastError()}
			return
		}
		defer C.sa_string_free(response)

		var parsed CompletionResult
		if err := json.Unmarshal([]byte(C.GoString(response)), &parsed); err != nil {
			resultCh <- result{CompletionResult{}, fmt.Errorf("unmarshal completion result: %w", err)}
			return
		}
		resultCh <- result{parsed, nil}
	}()

	select {
	case res := <-resultCh:
		return res.value, res.err
	case <-ctx.Done():
		// C calls are not cancellable here. Wait for the goroutine to finish so
		// deferred C.free calls in this function do not race with in-flight cgo.
		<-resultCh
		return CompletionResult{}, ctx.Err()
	}
}

func (c *Client) validateClient() error {
	if c == nil || c.ptr == nil {
		return errors.New("client is not initialized")
	}
	return nil
}

func validatePromptInput(model, prompt string) error {
	if model == "" {
		return errors.New("model cannot be empty")
	}
	if prompt == "" {
		return errors.New("prompt cannot be empty")
	}
	return nil
}

func validateMessagesInput(model string, messages []Message) error {
	if model == "" {
		return errors.New("model cannot be empty")
	}
	if len(messages) == 0 {
		return errors.New("messages cannot be empty")
	}
	for i, msg := range messages {
		if msg.Role == "" {
			return fmt.Errorf("messages[%d].role cannot be empty", i)
		}
		if msg.Content == "" {
			return fmt.Errorf("messages[%d].content cannot be empty", i)
		}
	}
	return nil
}

func lastError() error {
	msg := C.sa_last_error_message()
	if msg == nil {
		return errors.New("unknown error")
	}
	defer C.sa_string_free(msg)
	return errors.New(C.GoString(msg))
}
