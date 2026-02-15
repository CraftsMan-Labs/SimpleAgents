package simpleagents

/*
#cgo CFLAGS: -I${SRCDIR}/../../crates/simple-agents-ffi/include
#cgo LDFLAGS: -lsimple_agents_ffi

#include <stdlib.h>
#include "simple_agents.h"

extern int32_t sa_go_stream_callback_export(char *event_json, void *user_data);

static int32_t sa_go_stream_callback_bridge(const char *event_json, void *user_data) {
    return sa_go_stream_callback_export((char *)event_json, user_data);
}

static int32_t sa_stream_messages_go(
    SAClient *client,
    const char *model,
    const SAMessage *messages,
    size_t messages_len,
    int32_t max_tokens,
    float temperature,
    float top_p,
    void *user_data
) {
    return sa_stream_messages(
        client,
        model,
        messages,
        messages_len,
        max_tokens,
        temperature,
        top_p,
        sa_go_stream_callback_bridge,
        user_data
    );
}
*/
import "C"

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"runtime/cgo"
	"sync"
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

// StreamMessageDelta is the message delta payload for one streamed choice.
type StreamMessageDelta struct {
	Role    string `json:"role,omitempty"`
	Content string `json:"content,omitempty"`
}

// StreamChoiceDelta is one streamed choice delta.
type StreamChoiceDelta struct {
	Index        uint32             `json:"index"`
	Delta        StreamMessageDelta `json:"delta"`
	FinishReason string             `json:"finish_reason,omitempty"`
}

// StreamChunk mirrors the Rust CompletionChunk payload for streaming.
type StreamChunk struct {
	ID      string              `json:"id"`
	Model   string              `json:"model"`
	Choices []StreamChoiceDelta `json:"choices"`
	Created *int64              `json:"created,omitempty"`
}

// StreamEvent is emitted by StreamMessages.
type StreamEvent struct {
	Type    string       `json:"type"`
	Chunk   *StreamChunk `json:"chunk,omitempty"`
	Message string       `json:"message,omitempty"`
}

// StreamResult represents one streamed item or terminal error.
type StreamResult struct {
	Event StreamEvent
	Err   error
}

type streamBridge struct {
	ctx context.Context
	out chan StreamResult
}

type Client struct {
	mu       sync.Mutex
	ptr      *C.SAClient
	closed   bool
	inFlight sync.WaitGroup
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
	if c == nil {
		return
	}

	c.mu.Lock()
	if c.ptr == nil || c.closed {
		c.mu.Unlock()
		return
	}
	c.closed = true
	ptr := c.ptr
	c.mu.Unlock()

	c.inFlight.Wait()

	c.mu.Lock()
	if c.ptr == ptr {
		C.sa_client_free(c.ptr)
		c.ptr = nil
	}
	c.mu.Unlock()
}

func (c *Client) beginCall() (*C.SAClient, error) {
	if c == nil {
		return nil, errors.New("client is not initialized")
	}

	c.mu.Lock()
	defer c.mu.Unlock()

	if c.ptr == nil || c.closed {
		return nil, errors.New("client is not initialized")
	}

	c.inFlight.Add(1)
	return c.ptr, nil
}

func (c *Client) endCall() {
	c.inFlight.Done()
}

// Complete preserves the original prompt-based API.
func (c *Client) Complete(model, prompt string, maxTokens int32, temperature float32) (string, error) {
	return c.CompleteWithContext(context.Background(), model, prompt, maxTokens, temperature)
}

// CompletePrompt is the canonical prompt-based API with explicit context.
func (c *Client) CompletePrompt(
	ctx context.Context,
	model, prompt string,
	maxTokens int32,
	temperature float32,
) (string, error) {
	return c.CompleteWithContext(ctx, model, prompt, maxTokens, temperature)
}

type completeResult struct {
	value string
	err   error
}

type completeMessagesResult struct {
	value CompletionResult
	err   error
}

func sendIfWaiting[T any](ch chan<- T, value T) {
	select {
	case ch <- value:
	default:
	}
}

// CompleteWithContext executes prompt-based completion with context cancellation support.
func (c *Client) CompleteWithContext(
	ctx context.Context,
	model, prompt string,
	maxTokens int32,
	temperature float32,
) (string, error) {
	if err := validatePromptInput(model, prompt); err != nil {
		return "", err
	}
	if ctx == nil {
		ctx = context.Background()
	}

	ptr, err := c.beginCall()
	if err != nil {
		return "", err
	}

	resultCh := make(chan completeResult, 1)
	go func() {
		defer c.endCall()
		cModel := C.CString(model)
		defer C.free(unsafe.Pointer(cModel))
		cPrompt := C.CString(prompt)
		defer C.free(unsafe.Pointer(cPrompt))

		response := C.sa_complete(ptr, cModel, cPrompt, C.int32_t(maxTokens), C.float(temperature))
		if response == nil {
			sendIfWaiting(resultCh, completeResult{"", lastError()})
			return
		}
		defer C.sa_string_free(response)
		sendIfWaiting(resultCh, completeResult{C.GoString(response), nil})
	}()

	select {
	case res := <-resultCh:
		return res.value, res.err
	case <-ctx.Done():
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
	if err := validateMessagesInput(model, messages); err != nil {
		return CompletionResult{}, err
	}
	if ctx == nil {
		ctx = context.Background()
	}

	ptr, err := c.beginCall()
	if err != nil {
		return CompletionResult{}, err
	}

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

	var schemaJSONString string
	if opts.Schema != nil {
		schemaJSON, err := json.Marshal(opts.Schema)
		if err != nil {
			return CompletionResult{}, fmt.Errorf("marshal schema: %w", err)
		}
		schemaJSONString = string(schemaJSON)
	}

	messagesCopy := append([]Message(nil), messages...)

	resultCh := make(chan completeMessagesResult, 1)
	go func() {
		defer c.endCall()
		cModel := C.CString(model)
		defer C.free(unsafe.Pointer(cModel))
		cMode := C.CString(modeValue)
		defer C.free(unsafe.Pointer(cMode))

		var cSchemaJSON *C.char
		if schemaJSONString != "" {
			cSchemaJSON = C.CString(schemaJSONString)
			defer C.free(unsafe.Pointer(cSchemaJSON))
		}

		cMessages := make([]C.SAMessage, len(messagesCopy))
		allocated := make([]*C.char, 0, len(messagesCopy)*4)
		freeAll := func() {
			for _, p := range allocated {
				if p != nil {
					C.free(unsafe.Pointer(p))
				}
			}
		}
		defer freeAll()

		for i, msg := range messagesCopy {
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

		response := C.sa_complete_messages_json(
			ptr,
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
			sendIfWaiting(resultCh, completeMessagesResult{CompletionResult{}, lastError()})
			return
		}
		defer C.sa_string_free(response)

		var parsed CompletionResult
		if err := json.Unmarshal([]byte(C.GoString(response)), &parsed); err != nil {
			sendIfWaiting(resultCh, completeMessagesResult{CompletionResult{}, fmt.Errorf("unmarshal completion result: %w", err)})
			return
		}
		sendIfWaiting(resultCh, completeMessagesResult{parsed, nil})
	}()

	select {
	case res := <-resultCh:
		return res.value, res.err
	case <-ctx.Done():
		return CompletionResult{}, ctx.Err()
	}
}

// StreamMessages executes message-based completion in streaming mode.
//
// The returned channel is closed on completion, cancellation, or error.
// Callers should range over the channel until closed.
func (c *Client) StreamMessages(
	ctx context.Context,
	model string,
	messages []Message,
	opts CompleteOptions,
) (<-chan StreamResult, error) {
	if err := validateMessagesInput(model, messages); err != nil {
		return nil, err
	}
	if ctx == nil {
		ctx = context.Background()
	}

	ptr, err := c.beginCall()
	if err != nil {
		return nil, err
	}

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

	messagesCopy := append([]Message(nil), messages...)
	out := make(chan StreamResult, 16)
	bridge := &streamBridge{ctx: ctx, out: out}
	handle := cgo.NewHandle(bridge)

	go func() {
		defer c.endCall()
		defer close(out)
		defer handle.Delete()

		cModel := C.CString(model)
		defer C.free(unsafe.Pointer(cModel))

		cMessages := make([]C.SAMessage, len(messagesCopy))
		allocated := make([]*C.char, 0, len(messagesCopy)*4)
		freeAll := func() {
			for _, p := range allocated {
				if p != nil {
					C.free(unsafe.Pointer(p))
				}
			}
		}
		defer freeAll()

		for i, msg := range messagesCopy {
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

		status := C.sa_stream_messages_go(
			ptr,
			cModel,
			(*C.SAMessage)(unsafe.Pointer(&cMessages[0])),
			C.size_t(len(cMessages)),
			C.int32_t(maxTokens),
			C.float(temperature),
			C.float(topP),
			unsafe.Pointer(uintptr(handle)),
		)

		if status != 0 {
			err := lastError()
			if ctx.Err() != nil {
				err = ctx.Err()
			}
			sendIfWaiting(out, StreamResult{Err: err})
		}
	}()

	return out, nil
}

//export sa_go_stream_callback_export
func sa_go_stream_callback_export(eventJSON *C.char, userData unsafe.Pointer) C.int32_t {
	handle := cgo.Handle(uintptr(userData))
	bridge, ok := handle.Value().(*streamBridge)
	if !ok || bridge == nil {
		return 1
	}

	select {
	case <-bridge.ctx.Done():
		return 1
	default:
	}

	var event StreamEvent
	if err := json.Unmarshal([]byte(C.GoString(eventJSON)), &event); err != nil {
		sendIfWaiting(bridge.out, StreamResult{Err: fmt.Errorf("unmarshal stream event: %w", err)})
		return 1
	}

	if event.Type == "error" {
		sendIfWaiting(bridge.out, StreamResult{Err: errors.New(event.Message)})
		return 1
	}

	select {
	case bridge.out <- StreamResult{Event: event}:
		return 0
	case <-bridge.ctx.Done():
		return 1
	}
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
