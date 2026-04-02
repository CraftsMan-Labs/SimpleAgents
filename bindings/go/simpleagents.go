package simpleagents

/*
#cgo CFLAGS: -I${SRCDIR}/../../crates/simple-agents-ffi/include
#cgo LDFLAGS: -lsimple_agents_ffi

#include <stdlib.h>
#include "simple_agents.h"

char *sa_run_email_workflow_yaml(
    SAClient *client,
    const char *workflow_path,
    const char *email_text
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

char *sa_run_workflow_yaml_stream_events(
    SAClient *client,
    const char *workflow_path,
    const char *workflow_input_json,
    const char *workflow_options_json,
    int32_t (*callback)(const char *event_json, void *user_data),
    void *user_data
);

extern int32_t sa_go_stream_callback_export(char *event_json, void *user_data);
extern int32_t sa_go_workflow_event_callback_export(char *event_json, void *user_data);

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

static char *sa_run_email_workflow_yaml_go(
    SAClient *client,
    const char *workflow_path,
    const char *email_text
) {
    return sa_run_email_workflow_yaml(client, workflow_path, email_text);
}

static char *sa_run_workflow_yaml_with_options_go(
    SAClient *client,
    const char *workflow_path,
    const char *workflow_input_json,
    const char *workflow_options_json
) {
    return sa_run_workflow_yaml_with_options(
        client,
        workflow_path,
        workflow_input_json,
        workflow_options_json
    );
}

static char *sa_run_workflow_yaml_with_events_go(
    SAClient *client,
    const char *workflow_path,
    const char *workflow_input_json,
    const char *workflow_options_json
) {
    return sa_run_workflow_yaml_with_events(
        client,
        workflow_path,
        workflow_input_json,
        workflow_options_json
    );
}

static int32_t sa_go_workflow_event_callback_bridge(const char *event_json, void *user_data) {
    return sa_go_workflow_event_callback_export((char *)event_json, user_data);
}

static char *sa_run_workflow_yaml_stream_events_go(
    SAClient *client,
    const char *workflow_path,
    const char *workflow_input_json,
    const char *workflow_options_json,
    void *user_data
) {
    return sa_run_workflow_yaml_stream_events(
        client,
        workflow_path,
        workflow_input_json,
        workflow_options_json,
        sa_go_workflow_event_callback_bridge,
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
	"sort"
	"sync"
	"unsafe"
)

// Message represents a chat message for message-based completions.
type MessageRole string

const (
	MessageRoleSystem    MessageRole = "system"
	MessageRoleUser      MessageRole = "user"
	MessageRoleAssistant MessageRole = "assistant"
	MessageRoleTool      MessageRole = "tool"
)

type Message struct {
	Role       MessageRole
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

type WorkflowStepTiming struct {
	NodeID           string   `json:"node_id"`
	NodeKind         string   `json:"node_kind"`
	ElapsedMS        uint64   `json:"elapsed_ms"`
	PromptTokens     *uint32  `json:"prompt_tokens,omitempty"`
	CompletionTokens *uint32  `json:"completion_tokens,omitempty"`
	TotalTokens      *uint32  `json:"total_tokens,omitempty"`
	ReasoningTokens  *uint32  `json:"reasoning_tokens,omitempty"`
	TokensPerSecond  *float64 `json:"tokens_per_second,omitempty"`
}

type WorkflowLlmNodeMetrics struct {
	ElapsedMS        uint64  `json:"elapsed_ms"`
	PromptTokens     uint32  `json:"prompt_tokens"`
	CompletionTokens uint32  `json:"completion_tokens"`
	TotalTokens      uint32  `json:"total_tokens"`
	ReasoningTokens  *uint32 `json:"reasoning_tokens,omitempty"`
	TokensPerSecond  float64 `json:"tokens_per_second"`
}

type WorkflowEvent struct {
	EventType           string         `json:"event_type"`
	NodeID              *string        `json:"node_id,omitempty"`
	StepID              *string        `json:"step_id,omitempty"`
	NodeKind            *string        `json:"node_kind,omitempty"`
	Streamable          *bool          `json:"streamable,omitempty"`
	Message             *string        `json:"message,omitempty"`
	Delta               *string        `json:"delta,omitempty"`
	TokenKind           *string        `json:"token_kind,omitempty"`
	IsTerminalNodeToken *bool          `json:"is_terminal_node_token,omitempty"`
	ElapsedMS           *uint64        `json:"elapsed_ms,omitempty"`
	Metadata            map[string]any `json:"metadata,omitempty"`
}

type WorkflowNodeKind string

const (
	WorkflowNodeKindLlmCall      WorkflowNodeKind = "llm_call"
	WorkflowNodeKindSwitch       WorkflowNodeKind = "switch"
	WorkflowNodeKindCustomWorker WorkflowNodeKind = "custom_worker"
	WorkflowNodeKindUnknown      WorkflowNodeKind = "unknown"
)

type WorkflowNodeOutputRecord struct {
	NodeID   string           `json:"node_id"`
	NodeKind WorkflowNodeKind `json:"node_kind"`
	Value    any              `json:"value"`
}

type WorkflowRunTypedOutput struct {
	WorkflowID     string                     `json:"workflow_id"`
	EntryNode      string                     `json:"entry_node"`
	Trace          []string                   `json:"trace"`
	TerminalNode   string                     `json:"terminal_node"`
	TerminalOutput *WorkflowNodeOutputRecord  `json:"terminal_output,omitempty"`
	NodeOutputs    []WorkflowNodeOutputRecord `json:"node_outputs"`
}

type WorkflowEventType string

const (
	WorkflowEventTypeWorkflowStarted       WorkflowEventType = "workflow_started"
	WorkflowEventTypeWorkflowCompleted     WorkflowEventType = "workflow_completed"
	WorkflowEventTypeNodeStarted           WorkflowEventType = "node_started"
	WorkflowEventTypeNodeCompleted         WorkflowEventType = "node_completed"
	WorkflowEventTypeNodeLlmInputResolved  WorkflowEventType = "node_llm_input_resolved"
	WorkflowEventTypeNodeStreamDelta       WorkflowEventType = "node_stream_delta"
	WorkflowEventTypeNodeStreamThinking    WorkflowEventType = "node_stream_thinking_delta"
	WorkflowEventTypeNodeStreamOutput      WorkflowEventType = "node_stream_output_delta"
	WorkflowEventTypeNodeToolCallRequested WorkflowEventType = "node_tool_call_requested"
	WorkflowEventTypeNodeToolCallCompleted WorkflowEventType = "node_tool_call_completed"
	WorkflowEventTypeNodeToolCallFailed    WorkflowEventType = "node_tool_call_failed"
	WorkflowEventTypeNodeToolRoundtrip     WorkflowEventType = "node_tool_roundtrip_completed"
	WorkflowEventTypeNodeHealed            WorkflowEventType = "node_healed"
	WorkflowEventTypeUnknown               WorkflowEventType = "unknown"
)

type WorkflowTypedEvent struct {
	EventType           WorkflowEventType `json:"event_type"`
	RawEventType        string            `json:"raw_event_type"`
	NodeID              *string           `json:"node_id,omitempty"`
	StepID              *string           `json:"step_id,omitempty"`
	NodeKind            *string           `json:"node_kind,omitempty"`
	Streamable          *bool             `json:"streamable,omitempty"`
	Message             *string           `json:"message,omitempty"`
	Delta               *string           `json:"delta,omitempty"`
	TokenKind           *string           `json:"token_kind,omitempty"`
	IsTerminalNodeToken *bool             `json:"is_terminal_node_token,omitempty"`
	ElapsedMS           *uint64           `json:"elapsed_ms,omitempty"`
	Metadata            map[string]any    `json:"metadata,omitempty"`
}

func workflowEventTypeFromString(value string) WorkflowEventType {
	switch value {
	case string(WorkflowEventTypeWorkflowStarted):
		return WorkflowEventTypeWorkflowStarted
	case string(WorkflowEventTypeWorkflowCompleted):
		return WorkflowEventTypeWorkflowCompleted
	case string(WorkflowEventTypeNodeStarted):
		return WorkflowEventTypeNodeStarted
	case string(WorkflowEventTypeNodeCompleted):
		return WorkflowEventTypeNodeCompleted
	case string(WorkflowEventTypeNodeLlmInputResolved):
		return WorkflowEventTypeNodeLlmInputResolved
	case string(WorkflowEventTypeNodeStreamDelta):
		return WorkflowEventTypeNodeStreamDelta
	case string(WorkflowEventTypeNodeStreamThinking):
		return WorkflowEventTypeNodeStreamThinking
	case string(WorkflowEventTypeNodeStreamOutput):
		return WorkflowEventTypeNodeStreamOutput
	case string(WorkflowEventTypeNodeToolCallRequested):
		return WorkflowEventTypeNodeToolCallRequested
	case string(WorkflowEventTypeNodeToolCallCompleted):
		return WorkflowEventTypeNodeToolCallCompleted
	case string(WorkflowEventTypeNodeToolCallFailed):
		return WorkflowEventTypeNodeToolCallFailed
	case string(WorkflowEventTypeNodeToolRoundtrip):
		return WorkflowEventTypeNodeToolRoundtrip
	case string(WorkflowEventTypeNodeHealed):
		return WorkflowEventTypeNodeHealed
	default:
		return WorkflowEventTypeUnknown
	}
}

func (event WorkflowEvent) ToTypedEvent() WorkflowTypedEvent {
	return WorkflowTypedEvent{
		EventType:           workflowEventTypeFromString(event.EventType),
		RawEventType:        event.EventType,
		NodeID:              event.NodeID,
		StepID:              event.StepID,
		NodeKind:            event.NodeKind,
		Streamable:          event.Streamable,
		Message:             event.Message,
		Delta:               event.Delta,
		TokenKind:           event.TokenKind,
		IsTerminalNodeToken: event.IsTerminalNodeToken,
		ElapsedMS:           event.ElapsedMS,
		Metadata:            event.Metadata,
	}
}

type WorkflowYAMLOutput struct {
	WorkflowID           string                            `json:"workflow_id"`
	EntryNode            string                            `json:"entry_node"`
	EmailText            string                            `json:"email_text"`
	Trace                []string                          `json:"trace"`
	Outputs              map[string]map[string]any         `json:"outputs"`
	NodeOutputs          []WorkflowNodeOutputRecord        `json:"node_outputs,omitempty"`
	TerminalNode         string                            `json:"terminal_node"`
	TerminalOutput       any                               `json:"terminal_output"`
	StepTimings          []WorkflowStepTiming              `json:"step_timings"`
	LlmNodeMetrics       map[string]WorkflowLlmNodeMetrics `json:"llm_node_metrics"`
	TotalElapsedMS       uint64                            `json:"total_elapsed_ms"`
	TTFTMS               *uint64                           `json:"ttft_ms,omitempty"`
	TotalInputTokens     uint64                            `json:"total_input_tokens"`
	TotalOutputTokens    uint64                            `json:"total_output_tokens"`
	TotalTokens          uint64                            `json:"total_tokens"`
	TotalReasoningTokens *uint64                           `json:"total_reasoning_tokens,omitempty"`
	TokensPerSecond      float64                           `json:"tokens_per_second"`
	TraceID              string                            `json:"trace_id,omitempty"`
	Metadata             map[string]any                    `json:"metadata,omitempty"`
	Events               []WorkflowEvent                   `json:"events,omitempty"`
}

func (out WorkflowYAMLOutput) ToTypedOutput() WorkflowRunTypedOutput {
	nodeOutputs := out.NodeOutputs
	if len(nodeOutputs) == 0 {
		nodeOutputs = make([]WorkflowNodeOutputRecord, 0, len(out.Outputs))
		for nodeID, value := range out.Outputs {
			nodeOutputs = append(nodeOutputs, WorkflowNodeOutputRecord{
				NodeID:   nodeID,
				NodeKind: WorkflowNodeKindUnknown,
				Value:    value,
			})
		}
		sort.Slice(nodeOutputs, func(i, j int) bool {
			return nodeOutputs[i].NodeID < nodeOutputs[j].NodeID
		})
	}

	var terminalOutput *WorkflowNodeOutputRecord
	if out.TerminalOutput != nil {
		terminalOutput = &WorkflowNodeOutputRecord{
			NodeID:   out.TerminalNode,
			NodeKind: WorkflowNodeKindUnknown,
			Value:    out.TerminalOutput,
		}
	}

	return WorkflowRunTypedOutput{
		WorkflowID:     out.WorkflowID,
		EntryNode:      out.EntryNode,
		Trace:          out.Trace,
		TerminalNode:   out.TerminalNode,
		TerminalOutput: terminalOutput,
		NodeOutputs:    nodeOutputs,
	}
}

type WorkflowRunOptions struct {
	Telemetry map[string]any `json:"telemetry,omitempty"`
	Trace     map[string]any `json:"trace,omitempty"`
	Model     string         `json:"model,omitempty"`
}

// WorkflowTraceContext carries optional upstream trace propagation fields.
type WorkflowTraceContext struct {
	TraceID      *string           `json:"trace_id,omitempty"`
	SpanID       *string           `json:"span_id,omitempty"`
	ParentSpanID *string           `json:"parent_span_id,omitempty"`
	Traceparent  *string           `json:"traceparent,omitempty"`
	Tracestate   *string           `json:"tracestate,omitempty"`
	Baggage      map[string]string `json:"baggage,omitempty"`
}

// WorkflowTraceTenant carries optional multi-tenant correlation metadata.
type WorkflowTraceTenant struct {
	WorkspaceID    *string `json:"workspace_id,omitempty"`
	UserID         *string `json:"user_id,omitempty"`
	ConversationID *string `json:"conversation_id,omitempty"`
	RequestID      *string `json:"request_id,omitempty"`
	RunID          *string `json:"run_id,omitempty"`
}

// WorkflowTraceConfig nests trace context and tenant attributes.
type WorkflowTraceConfig struct {
	Context *WorkflowTraceContext `json:"context,omitempty"`
	Tenant  *WorkflowTraceTenant  `json:"tenant,omitempty"`
}

// WorkflowTelemetryConfig controls workflow telemetry behavior.
type WorkflowTelemetryConfig struct {
	Enabled       *bool    `json:"enabled,omitempty"`
	Nerdstats     *bool    `json:"nerdstats,omitempty"`
	SampleRate    *float32 `json:"sample_rate,omitempty"`
	PayloadMode   *string  `json:"payload_mode,omitempty"`
	RetentionDays *uint32  `json:"retention_days,omitempty"`
	MultiTenant   *bool    `json:"multi_tenant,omitempty"`
	ToolTraceMode *string  `json:"tool_trace_mode,omitempty"`
}

// TypedWorkflowRunOptions is a typed alternative to map-based workflow options.
type TypedWorkflowRunOptions struct {
	Telemetry *WorkflowTelemetryConfig `json:"telemetry,omitempty"`
	Trace     *WorkflowTraceConfig     `json:"trace,omitempty"`
	Model     *string                  `json:"model,omitempty"`
}

func typedWorkflowRunOptionsToMap(options *TypedWorkflowRunOptions) (map[string]any, error) {
	if options == nil {
		return nil, nil
	}

	encoded, err := json.Marshal(options)
	if err != nil {
		return nil, fmt.Errorf("marshal typed workflow options: %w", err)
	}

	var decoded map[string]any
	if err := json.Unmarshal(encoded, &decoded); err != nil {
		return nil, fmt.Errorf("unmarshal typed workflow options: %w", err)
	}

	if len(decoded) == 0 {
		return nil, nil
	}

	return decoded, nil
}

type streamBridge struct {
	ctx context.Context
	out chan StreamResult
}

type workflowEventBridge struct {
	ctx     context.Context
	onEvent func(WorkflowEvent)
	out     chan WorkflowEventResult
}

type Client struct {
	mu       sync.Mutex
	ptr      *C.SAClient
	closed   bool
	inFlight sync.WaitGroup
}

type WorkflowEventResult struct {
	Event WorkflowEvent
	Err   error
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
	c.ptr = nil
	c.mu.Unlock()

	go func() {
		c.inFlight.Wait()
		C.sa_client_free(ptr)
	}()
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

// RunEmailWorkflowYAML executes the Rust workflow YAML runner and returns structured output.
func (c *Client) RunEmailWorkflowYAML(
	ctx context.Context,
	workflowPath string,
	emailText string,
) (WorkflowYAMLOutput, error) {
	return c.RunWorkflowYAML(ctx, workflowPath, map[string]any{"email_text": emailText})
}

// RunEmailWorkflowYAMLTyped executes email workflow YAML and projects output into typed records.
func (c *Client) RunEmailWorkflowYAMLTyped(
	ctx context.Context,
	workflowPath string,
	emailText string,
) (WorkflowRunTypedOutput, error) {
	out, err := c.RunEmailWorkflowYAML(ctx, workflowPath, emailText)
	if err != nil {
		return WorkflowRunTypedOutput{}, err
	}
	return out.ToTypedOutput(), nil
}

// RunEmailWorkflowYAMLWithRunOptions executes email workflow YAML with typed workflow options.
func (c *Client) RunEmailWorkflowYAMLWithRunOptions(
	ctx context.Context,
	workflowPath string,
	emailText string,
	options *TypedWorkflowRunOptions,
) (WorkflowYAMLOutput, error) {
	workflowOptions, err := typedWorkflowRunOptionsToMap(options)
	if err != nil {
		return WorkflowYAMLOutput{}, err
	}

	return c.RunWorkflowYAMLWithOptions(
		ctx,
		workflowPath,
		map[string]any{"email_text": emailText},
		workflowOptions,
	)
}

// RunEmailWorkflowYAMLWithRunOptionsTyped executes email workflow YAML with typed run options and typed output projection.
func (c *Client) RunEmailWorkflowYAMLWithRunOptionsTyped(
	ctx context.Context,
	workflowPath string,
	emailText string,
	options *TypedWorkflowRunOptions,
) (WorkflowRunTypedOutput, error) {
	out, err := c.RunEmailWorkflowYAMLWithRunOptions(ctx, workflowPath, emailText, options)
	if err != nil {
		return WorkflowRunTypedOutput{}, err
	}
	return out.ToTypedOutput(), nil
}

// RunWorkflowYAML executes the Rust workflow YAML runner with arbitrary workflow input.
func (c *Client) RunWorkflowYAML(
	ctx context.Context,
	workflowPath string,
	workflowInput map[string]any,
) (WorkflowYAMLOutput, error) {
	return c.RunWorkflowYAMLWithOptions(ctx, workflowPath, workflowInput, nil)
}

// RunWorkflowYAMLTyped executes workflow YAML and projects output into typed records.
func (c *Client) RunWorkflowYAMLTyped(
	ctx context.Context,
	workflowPath string,
	workflowInput map[string]any,
) (WorkflowRunTypedOutput, error) {
	out, err := c.RunWorkflowYAML(ctx, workflowPath, workflowInput)
	if err != nil {
		return WorkflowRunTypedOutput{}, err
	}
	return out.ToTypedOutput(), nil
}

// RunWorkflowYAMLWithRunOptions executes workflow YAML with typed workflow options.
func (c *Client) RunWorkflowYAMLWithRunOptions(
	ctx context.Context,
	workflowPath string,
	workflowInput map[string]any,
	options *TypedWorkflowRunOptions,
) (WorkflowYAMLOutput, error) {
	workflowOptions, err := typedWorkflowRunOptionsToMap(options)
	if err != nil {
		return WorkflowYAMLOutput{}, err
	}

	return c.RunWorkflowYAMLWithOptions(ctx, workflowPath, workflowInput, workflowOptions)
}

// RunWorkflowYAMLWithRunOptionsTyped executes workflow YAML with typed run options and typed output projection.
func (c *Client) RunWorkflowYAMLWithRunOptionsTyped(
	ctx context.Context,
	workflowPath string,
	workflowInput map[string]any,
	options *TypedWorkflowRunOptions,
) (WorkflowRunTypedOutput, error) {
	out, err := c.RunWorkflowYAMLWithRunOptions(ctx, workflowPath, workflowInput, options)
	if err != nil {
		return WorkflowRunTypedOutput{}, err
	}
	return out.ToTypedOutput(), nil
}

// RunWorkflowYAMLWithOptions executes the Rust workflow YAML runner with telemetry options.
func (c *Client) RunWorkflowYAMLWithOptions(
	ctx context.Context,
	workflowPath string,
	workflowInput map[string]any,
	options map[string]any,
) (WorkflowYAMLOutput, error) {
	if workflowPath == "" {
		return WorkflowYAMLOutput{}, errors.New("workflowPath cannot be empty")
	}
	if workflowInput == nil {
		return WorkflowYAMLOutput{}, errors.New("workflowInput cannot be nil")
	}
	if ctx == nil {
		ctx = context.Background()
	}

	workflowInputJSON, err := json.Marshal(workflowInput)
	if err != nil {
		return WorkflowYAMLOutput{}, fmt.Errorf("marshal workflow input: %w", err)
	}

	workflowOptionsJSON := ""
	if options != nil {
		encoded, marshalErr := json.Marshal(options)
		if marshalErr != nil {
			return WorkflowYAMLOutput{}, fmt.Errorf("marshal workflow options: %w", marshalErr)
		}
		workflowOptionsJSON = string(encoded)
	}

	ptr, err := c.beginCall()
	if err != nil {
		return WorkflowYAMLOutput{}, err
	}

	resultCh := make(chan workflowRunResult, 1)
	go func() {
		defer c.endCall()
		cWorkflowPath := C.CString(workflowPath)
		cWorkflowInputJSON := C.CString(string(workflowInputJSON))
		cWorkflowOptionsJSON := C.CString(workflowOptionsJSON)
		defer C.free(unsafe.Pointer(cWorkflowPath))
		defer C.free(unsafe.Pointer(cWorkflowInputJSON))
		defer C.free(unsafe.Pointer(cWorkflowOptionsJSON))

		response := C.sa_run_workflow_yaml_with_options_go(
			ptr,
			cWorkflowPath,
			cWorkflowInputJSON,
			cWorkflowOptionsJSON,
		)
		if response == nil {
			sendIfWaiting(resultCh, workflowRunResult{WorkflowYAMLOutput{}, lastError()})
			return
		}
		defer C.sa_string_free(response)

		var output WorkflowYAMLOutput
		if err := json.Unmarshal([]byte(C.GoString(response)), &output); err != nil {
			sendIfWaiting(resultCh, workflowRunResult{WorkflowYAMLOutput{}, err})
			return
		}

		sendIfWaiting(resultCh, workflowRunResult{output, nil})
	}()

	select {
	case <-ctx.Done():
		return WorkflowYAMLOutput{}, ctx.Err()
	case result := <-resultCh:
		if result.err != nil {
			return WorkflowYAMLOutput{}, result.err
		}
		return result.value, nil
	}
}

// RunWorkflowYAMLWithOptionsTyped executes workflow YAML with options and returns typed output records.
func (c *Client) RunWorkflowYAMLWithOptionsTyped(
	ctx context.Context,
	workflowPath string,
	workflowInput map[string]any,
	options map[string]any,
) (WorkflowRunTypedOutput, error) {
	out, err := c.RunWorkflowYAMLWithOptions(ctx, workflowPath, workflowInput, options)
	if err != nil {
		return WorkflowRunTypedOutput{}, err
	}
	return out.ToTypedOutput(), nil
}

// RunWorkflowYAMLWithEvents executes workflow YAML and includes recorded runtime events in output.events.
func (c *Client) RunWorkflowYAMLWithEvents(
	ctx context.Context,
	workflowPath string,
	workflowInput map[string]any,
	options map[string]any,
) (WorkflowYAMLOutput, error) {
	if workflowPath == "" {
		return WorkflowYAMLOutput{}, errors.New("workflowPath cannot be empty")
	}
	if workflowInput == nil {
		return WorkflowYAMLOutput{}, errors.New("workflowInput cannot be nil")
	}
	if ctx == nil {
		ctx = context.Background()
	}

	workflowInputJSON, err := json.Marshal(workflowInput)
	if err != nil {
		return WorkflowYAMLOutput{}, fmt.Errorf("marshal workflow input: %w", err)
	}

	workflowOptionsJSON := ""
	if options != nil {
		encoded, marshalErr := json.Marshal(options)
		if marshalErr != nil {
			return WorkflowYAMLOutput{}, fmt.Errorf("marshal workflow options: %w", marshalErr)
		}
		workflowOptionsJSON = string(encoded)
	}

	ptr, err := c.beginCall()
	if err != nil {
		return WorkflowYAMLOutput{}, err
	}

	resultCh := make(chan workflowRunResult, 1)
	go func() {
		defer c.endCall()
		cWorkflowPath := C.CString(workflowPath)
		cWorkflowInputJSON := C.CString(string(workflowInputJSON))
		cWorkflowOptionsJSON := C.CString(workflowOptionsJSON)
		defer C.free(unsafe.Pointer(cWorkflowPath))
		defer C.free(unsafe.Pointer(cWorkflowInputJSON))
		defer C.free(unsafe.Pointer(cWorkflowOptionsJSON))

		response := C.sa_run_workflow_yaml_with_events_go(
			ptr,
			cWorkflowPath,
			cWorkflowInputJSON,
			cWorkflowOptionsJSON,
		)
		if response == nil {
			sendIfWaiting(resultCh, workflowRunResult{WorkflowYAMLOutput{}, lastError()})
			return
		}
		defer C.sa_string_free(response)

		var output WorkflowYAMLOutput
		if err := json.Unmarshal([]byte(C.GoString(response)), &output); err != nil {
			sendIfWaiting(resultCh, workflowRunResult{WorkflowYAMLOutput{}, err})
			return
		}

		sendIfWaiting(resultCh, workflowRunResult{output, nil})
	}()

	select {
	case <-ctx.Done():
		return WorkflowYAMLOutput{}, ctx.Err()
	case result := <-resultCh:
		if result.err != nil {
			return WorkflowYAMLOutput{}, result.err
		}
		return result.value, nil
	}
}

// RunWorkflowYAMLWithEventsAndRunOptions executes workflow YAML with events using typed workflow options.
func (c *Client) RunWorkflowYAMLWithEventsAndRunOptions(
	ctx context.Context,
	workflowPath string,
	workflowInput map[string]any,
	options *TypedWorkflowRunOptions,
) (WorkflowYAMLOutput, error) {
	workflowOptions, err := typedWorkflowRunOptionsToMap(options)
	if err != nil {
		return WorkflowYAMLOutput{}, err
	}

	return c.RunWorkflowYAMLWithEvents(ctx, workflowPath, workflowInput, workflowOptions)
}

// RunWorkflowYAMLStream emits live workflow events to onEvent while returning final workflow output.
func (c *Client) RunWorkflowYAMLStream(
	ctx context.Context,
	workflowPath string,
	workflowInput map[string]any,
	onEvent func(WorkflowEvent),
) (WorkflowYAMLOutput, error) {
	return c.RunWorkflowYAMLStreamWithOptions(ctx, workflowPath, workflowInput, nil, onEvent)
}

// RunWorkflowYAMLStreamWithOptions emits live workflow events to onEvent with workflow options.
func (c *Client) RunWorkflowYAMLStreamWithOptions(
	ctx context.Context,
	workflowPath string,
	workflowInput map[string]any,
	options map[string]any,
	onEvent func(WorkflowEvent),
) (WorkflowYAMLOutput, error) {
	if workflowPath == "" {
		return WorkflowYAMLOutput{}, errors.New("workflowPath cannot be empty")
	}
	if workflowInput == nil {
		return WorkflowYAMLOutput{}, errors.New("workflowInput cannot be nil")
	}
	if ctx == nil {
		ctx = context.Background()
	}

	workflowInputJSON, err := json.Marshal(workflowInput)
	if err != nil {
		return WorkflowYAMLOutput{}, fmt.Errorf("marshal workflow input: %w", err)
	}

	workflowOptionsJSON := ""
	if options != nil {
		encoded, marshalErr := json.Marshal(options)
		if marshalErr != nil {
			return WorkflowYAMLOutput{}, fmt.Errorf("marshal workflow options: %w", marshalErr)
		}
		workflowOptionsJSON = string(encoded)
	}

	ptr, err := c.beginCall()
	if err != nil {
		return WorkflowYAMLOutput{}, err
	}

	bridge := &workflowEventBridge{
		ctx:     ctx,
		onEvent: onEvent,
		out:     make(chan WorkflowEventResult, 16),
	}
	handle := cgo.NewHandle(bridge)

	resultCh := make(chan workflowRunResult, 1)
	go func() {
		defer c.endCall()
		defer close(bridge.out)
		defer handle.Delete()

		cWorkflowPath := C.CString(workflowPath)
		cWorkflowInputJSON := C.CString(string(workflowInputJSON))
		cWorkflowOptionsJSON := C.CString(workflowOptionsJSON)
		defer C.free(unsafe.Pointer(cWorkflowPath))
		defer C.free(unsafe.Pointer(cWorkflowInputJSON))
		defer C.free(unsafe.Pointer(cWorkflowOptionsJSON))

		response := C.sa_run_workflow_yaml_stream_events_go(
			ptr,
			cWorkflowPath,
			cWorkflowInputJSON,
			cWorkflowOptionsJSON,
			unsafe.Pointer(uintptr(handle)),
		)
		if response == nil {
			sendIfWaiting(resultCh, workflowRunResult{WorkflowYAMLOutput{}, lastError()})
			return
		}
		defer C.sa_string_free(response)

		var output WorkflowYAMLOutput
		if err := json.Unmarshal([]byte(C.GoString(response)), &output); err != nil {
			sendIfWaiting(resultCh, workflowRunResult{WorkflowYAMLOutput{}, err})
			return
		}

		sendIfWaiting(resultCh, workflowRunResult{output, nil})
	}()

	for {
		select {
		case <-ctx.Done():
			return WorkflowYAMLOutput{}, ctx.Err()
		case ev, ok := <-bridge.out:
			if !ok {
				bridge.out = nil
				continue
			}
			if ev.Err != nil {
				return WorkflowYAMLOutput{}, ev.Err
			}
		case result := <-resultCh:
			if result.err != nil {
				return WorkflowYAMLOutput{}, result.err
			}
			return result.value, nil
		}
	}
}

// RunWorkflowYAMLStreamWithRunOptions emits workflow events with typed workflow options.
func (c *Client) RunWorkflowYAMLStreamWithRunOptions(
	ctx context.Context,
	workflowPath string,
	workflowInput map[string]any,
	options *TypedWorkflowRunOptions,
	onEvent func(WorkflowEvent),
) (WorkflowYAMLOutput, error) {
	workflowOptions, err := typedWorkflowRunOptionsToMap(options)
	if err != nil {
		return WorkflowYAMLOutput{}, err
	}

	return c.RunWorkflowYAMLStreamWithOptions(ctx, workflowPath, workflowInput, workflowOptions, onEvent)
}

type completeResult struct {
	value string
	err   error
}

type completeMessagesResult struct {
	value CompletionResult
	err   error
}

type workflowRunResult struct {
	value WorkflowYAMLOutput
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
	if err := validateCompleteOptions(opts, false); err != nil {
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
			role := C.CString(string(msg.Role))
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
	if err := validateCompleteOptions(opts, true); err != nil {
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
			role := C.CString(string(msg.Role))
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

//export sa_go_workflow_event_callback_export
func sa_go_workflow_event_callback_export(eventJSON *C.char, userData unsafe.Pointer) C.int32_t {
	handle := cgo.Handle(uintptr(userData))
	bridge, ok := handle.Value().(*workflowEventBridge)
	if !ok || bridge == nil {
		return 1
	}

	select {
	case <-bridge.ctx.Done():
		return 1
	default:
	}

	var event WorkflowEvent
	if err := json.Unmarshal([]byte(C.GoString(eventJSON)), &event); err != nil {
		sendIfWaiting(bridge.out, WorkflowEventResult{Err: fmt.Errorf("unmarshal workflow event: %w", err)})
		return 1
	}

	if bridge.onEvent != nil {
		bridge.onEvent(event)
	}

	sendIfWaiting(bridge.out, WorkflowEventResult{Event: event})
	return 0
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
		switch msg.Role {
		case MessageRoleSystem, MessageRoleUser, MessageRoleAssistant, MessageRoleTool:
		default:
			return fmt.Errorf("messages[%d].role must be one of: system, user, assistant, tool", i)
		}
		if msg.Content == "" {
			return fmt.Errorf("messages[%d].content cannot be empty", i)
		}
	}
	return nil
}

func validateCompleteOptions(opts CompleteOptions, streaming bool) error {
	mode := opts.Mode
	if mode == "" {
		mode = "standard"
	}

	switch mode {
	case "standard", "healed_json", "schema":
	default:
		return fmt.Errorf("unsupported mode %q", mode)
	}

	if mode == "schema" && opts.Schema == nil {
		return errors.New("schema mode requires schema")
	}

	if mode != "schema" && opts.Schema != nil {
		return errors.New("schema is only valid when mode is \"schema\"")
	}

	if streaming && mode != "standard" {
		return errors.New("streaming only supports mode \"standard\"")
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
