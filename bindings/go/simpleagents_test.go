package simpleagents

import (
	"context"
	"encoding/json"
	"os"
	"path/filepath"
	"testing"
)

func TestValidateMessagesInput(t *testing.T) {
	err := validateMessagesInput("", []Message{{Role: "user", Content: "hi"}})
	if err == nil {
		t.Fatal("expected model validation error")
	}

	err = validateMessagesInput("gpt-4", nil)
	if err == nil {
		t.Fatal("expected messages validation error")
	}

	err = validateMessagesInput("gpt-4", []Message{{Role: "", Content: "hi"}})
	if err == nil {
		t.Fatal("expected role validation error")
	}

	err = validateMessagesInput("gpt-4", []Message{{Role: MessageRole("invalid"), Content: "hi"}})
	if err == nil {
		t.Fatal("expected enum role validation error")
	}
}

func TestValidatePromptInput(t *testing.T) {
	if err := validatePromptInput("", "hello"); err == nil {
		t.Fatal("expected empty model error")
	}
	if err := validatePromptInput("gpt-4", ""); err == nil {
		t.Fatal("expected empty prompt error")
	}
}

type optionCase struct {
	Name      string `json:"name"`
	Mode      string `json:"mode"`
	Schema    bool   `json:"schema"`
	Streaming bool   `json:"streaming"`
	Valid     bool   `json:"valid"`
}

func TestValidateCompleteOptionsGoldenCases(t *testing.T) {
	fixturePath := filepath.Join("testdata", "schema_option_cases.json")
	raw, err := os.ReadFile(fixturePath)
	if err != nil {
		t.Fatalf("read fixture: %v", err)
	}

	var cases []optionCase
	if err := json.Unmarshal(raw, &cases); err != nil {
		t.Fatalf("parse fixture: %v", err)
	}

	for _, tc := range cases {
		t.Run(tc.Name, func(t *testing.T) {
			opts := CompleteOptions{Mode: tc.Mode}
			if tc.Schema {
				opts.Schema = map[string]any{"type": "object"}
			}

			err := validateCompleteOptions(opts, tc.Streaming)
			if tc.Valid && err != nil {
				t.Fatalf("expected valid options, got error: %v", err)
			}
			if !tc.Valid && err == nil {
				t.Fatal("expected validation error")
			}
		})
	}
}

func TestCompleteMessagesUninitializedClient(t *testing.T) {
	c := &Client{}
	_, err := c.CompleteMessages(context.Background(), "gpt-4", []Message{{Role: "user", Content: "hi"}}, CompleteOptions{})
	if err == nil {
		t.Fatal("expected uninitialized client error")
	}
}

func TestCompleteWithContextUninitializedClient(t *testing.T) {
	c := &Client{}
	_, err := c.CompleteWithContext(context.Background(), "gpt-4", "hi", 8, 0.1)
	if err == nil {
		t.Fatal("expected uninitialized client error")
	}
}

func TestStreamMessagesUninitializedClient(t *testing.T) {
	c := &Client{}
	_, err := c.StreamMessages(context.Background(), "gpt-4", []Message{{Role: "user", Content: "hi"}}, CompleteOptions{})
	if err == nil {
		t.Fatal("expected uninitialized client error")
	}
}

func TestRunWorkflowYAMLValidation(t *testing.T) {
	c := &Client{}
	_, err := c.RunWorkflowYAML(context.Background(), "workflow.yaml", nil)
	if err == nil {
		t.Fatal("expected workflowInput validation error")
	}
}

func TestRunWorkflowYAMLUninitializedClient(t *testing.T) {
	c := &Client{}
	_, err := c.RunWorkflowYAML(context.Background(), "workflow.yaml", map[string]any{"email_text": "x"})
	if err == nil {
		t.Fatal("expected uninitialized client error")
	}
}

func TestRunWorkflowYAMLWithOptionsValidation(t *testing.T) {
	c := &Client{}
	_, err := c.RunWorkflowYAMLWithOptions(context.Background(), "workflow.yaml", nil, map[string]any{"telemetry": map[string]any{"enabled": true}})
	if err == nil {
		t.Fatal("expected workflowInput validation error")
	}
}

func TestRunWorkflowYAMLWithOptionsUninitializedClient(t *testing.T) {
	c := &Client{}
	_, err := c.RunWorkflowYAMLWithOptions(
		context.Background(),
		"workflow.yaml",
		map[string]any{"email_text": "x"},
		map[string]any{"trace": map[string]any{"context": map[string]any{"trace_id": "abc"}}},
	)
	if err == nil {
		t.Fatal("expected uninitialized client error")
	}
}

func TestRunWorkflowYAMLWithEventsValidation(t *testing.T) {
	c := &Client{}
	_, err := c.RunWorkflowYAMLWithEvents(context.Background(), "workflow.yaml", nil, nil)
	if err == nil {
		t.Fatal("expected workflowInput validation error")
	}
}

func TestRunWorkflowYAMLWithEventsUninitializedClient(t *testing.T) {
	c := &Client{}
	_, err := c.RunWorkflowYAMLWithEvents(context.Background(), "workflow.yaml", map[string]any{"email_text": "x"}, nil)
	if err == nil {
		t.Fatal("expected uninitialized client error")
	}
}

func TestTypedWorkflowRunOptionsToMapNil(t *testing.T) {
	actual, err := typedWorkflowRunOptionsToMap(nil)
	if err != nil {
		t.Fatalf("expected nil error, got %v", err)
	}
	if actual != nil {
		t.Fatal("expected nil map for nil typed options")
	}
}

func TestTypedWorkflowInputToMapNil(t *testing.T) {
	actual, err := typedWorkflowInputToMap(nil)
	if err == nil {
		t.Fatal("expected workflowInput validation error")
	}
	if actual != nil {
		t.Fatal("expected nil map for nil typed workflow input")
	}
}

func TestTypedWorkflowInputToMapIncludesAdditionalFields(t *testing.T) {
	actual, err := typedWorkflowInputToMap(&TypedWorkflowInput{
		Additional: map[string]json.RawMessage{
			"priority": json.RawMessage(`"high"`),
			"context":  json.RawMessage(`{"channel":"support"}`),
		},
	})
	if err != nil {
		t.Fatalf("expected nil error, got %v", err)
	}

	priority, ok := actual["priority"].(json.RawMessage)
	if !ok {
		t.Fatalf("expected priority as json.RawMessage, got %T", actual["priority"])
	}
	if string(priority) != `"high"` {
		t.Fatalf("expected priority raw message %q, got %q", `"high"`, string(priority))
	}

	contextValue, ok := actual["context"].(json.RawMessage)
	if !ok {
		t.Fatalf("expected context as json.RawMessage, got %T", actual["context"])
	}
	if string(contextValue) != `{"channel":"support"}` {
		t.Fatalf("expected context raw message %q, got %q", `{"channel":"support"}`, string(contextValue))
	}
}

func TestTypedWorkflowInputToMapRejectsEmptyAdditionalJSON(t *testing.T) {
	_, err := typedWorkflowInputToMap(&TypedWorkflowInput{
		Additional: map[string]json.RawMessage{
			"invalid": json.RawMessage{},
		},
	})
	if err == nil {
		t.Fatal("expected error for empty additional JSON payload")
	}
}

func TestTypedWorkflowInputToMapRejectsMalformedAdditionalJSON(t *testing.T) {
	_, err := typedWorkflowInputToMap(&TypedWorkflowInput{
		Additional: map[string]json.RawMessage{
			"invalid": json.RawMessage(`{"bad":`),
		},
	})
	if err == nil {
		t.Fatal("expected error for malformed additional JSON payload")
	}
}

func TestTypedWorkflowInputToMapRejectsReservedAdditionalMessages(t *testing.T) {
	_, err := typedWorkflowInputToMap(&TypedWorkflowInput{
		Additional: map[string]json.RawMessage{
			"messages": json.RawMessage(`[]`),
		},
	})
	if err == nil {
		t.Fatal("expected error for reserved additional messages field")
	}
}

func TestTypedWorkflowInputToMapIncludesExplicitEmptyMessages(t *testing.T) {
	actual, err := typedWorkflowInputToMap(&TypedWorkflowInput{
		Messages: []WorkflowInputMessage{},
	})
	if err != nil {
		t.Fatalf("expected nil error, got %v", err)
	}

	messagesValue, ok := actual["messages"]
	if !ok {
		t.Fatal("expected messages key to be present")
	}
	messages, ok := messagesValue.([]WorkflowInputMessage)
	if !ok {
		t.Fatalf("expected messages slice, got %T", messagesValue)
	}
	if len(messages) != 0 {
		t.Fatalf("expected explicit empty messages slice, got len %d", len(messages))
	}
}

func TestTypedWorkflowRunOptionsToMapWithTraceContext(t *testing.T) {
	traceparent := "00-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-bbbbbbbbbbbbbbbb-01"
	conversationID := "conv-123"
	enabled := true
	sampleRate := float32(1.0)
	options := &TypedWorkflowRunOptions{
		Telemetry: &WorkflowTelemetryConfig{
			Enabled:    &enabled,
			Nerdstats:  &enabled,
			SampleRate: &sampleRate,
		},
		Trace: &WorkflowTraceConfig{
			Context: &WorkflowTraceContext{
				Traceparent: &traceparent,
				Baggage: map[string]string{
					"tenant": "acme",
				},
			},
			Tenant: &WorkflowTraceTenant{
				ConversationID: &conversationID,
			},
		},
	}

	actual, err := typedWorkflowRunOptionsToMap(options)
	if err != nil {
		t.Fatalf("expected nil error, got %v", err)
	}

	trace, ok := actual["trace"].(map[string]any)
	if !ok {
		t.Fatal("expected trace map")
	}
	telemetry, ok := actual["telemetry"].(map[string]any)
	if !ok {
		t.Fatal("expected telemetry map")
	}
	sampleRateValue, ok := telemetry["sample_rate"].(float64)
	if !ok {
		t.Fatalf("expected sample_rate number, got %T", telemetry["sample_rate"])
	}
	if sampleRateValue != float64(sampleRate) {
		t.Fatalf("expected sample_rate %v, got %v", sampleRate, sampleRateValue)
	}
	contextMap, ok := trace["context"].(map[string]any)
	if !ok {
		t.Fatal("expected trace.context map")
	}
	if contextMap["traceparent"] != traceparent {
		t.Fatalf("expected traceparent %q, got %v", traceparent, contextMap["traceparent"])
	}

	tenantMap, ok := trace["tenant"].(map[string]any)
	if !ok {
		t.Fatal("expected trace.tenant map")
	}
	if tenantMap["conversation_id"] != conversationID {
		t.Fatalf("expected conversation_id %q, got %v", conversationID, tenantMap["conversation_id"])
	}
}

func TestRunWorkflowYAMLWithRunOptionsUninitializedClient(t *testing.T) {
	c := &Client{}
	traceparent := "00-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-bbbbbbbbbbbbbbbb-01"
	options := &TypedWorkflowRunOptions{
		Trace: &WorkflowTraceConfig{
			Context: &WorkflowTraceContext{Traceparent: &traceparent},
		},
	}
	_, err := c.RunWorkflowYAMLWithRunOptions(context.Background(), "workflow.yaml", map[string]any{"email_text": "x"}, options)
	if err == nil {
		t.Fatal("expected uninitialized client error")
	}
}

func TestRunWorkflowYAMLWithTypedInputUninitializedClient(t *testing.T) {
	c := &Client{}
	_, err := c.RunWorkflowYAMLWithTypedInput(context.Background(), "workflow.yaml", &TypedWorkflowInput{
		Messages: []WorkflowInputMessage{{Role: MessageRoleUser, Content: "x"}},
	})
	if err == nil {
		t.Fatal("expected uninitialized client error")
	}
}

func TestRunWorkflowYAMLWithTypedInputNilValidation(t *testing.T) {
	c := &Client{}
	_, err := c.RunWorkflowYAMLWithTypedInput(context.Background(), "workflow.yaml", nil)
	if err == nil {
		t.Fatal("expected workflowInput validation error")
	}
}

func TestRunWorkflowYAMLStreamValidation(t *testing.T) {
	c := &Client{}
	_, err := c.RunWorkflowYAMLStream(context.Background(), "workflow.yaml", nil, nil)
	if err == nil {
		t.Fatal("expected workflowInput validation error")
	}
}

func TestRunWorkflowYAMLStreamUninitializedClient(t *testing.T) {
	c := &Client{}
	_, err := c.RunWorkflowYAMLStream(context.Background(), "workflow.yaml", map[string]any{"email_text": "x"}, nil)
	if err == nil {
		t.Fatal("expected uninitialized client error")
	}
}

func TestRunWorkflowYAMLStreamWithRunOptionsUninitializedClient(t *testing.T) {
	c := &Client{}
	_, err := c.RunWorkflowYAMLStreamWithRunOptions(
		context.Background(),
		"workflow.yaml",
		map[string]any{"email_text": "x"},
		nil,
		nil,
		nil,
	)
	if err == nil {
		t.Fatal("expected uninitialized client error")
	}
}

func TestWorkflowOutputToTypedOutputProjectsNodeOutputs(t *testing.T) {
	output := WorkflowYAMLOutput{
		WorkflowID:   "wf-1",
		EntryNode:    "start",
		Trace:        []string{"start", "classify"},
		TerminalNode: "classify",
		NodeOutputs: []WorkflowNodeOutputRecord{
			{NodeID: "start", NodeKind: WorkflowNodeKindCustomWorker, Value: map[string]any{"ok": true}},
			{NodeID: "classify", NodeKind: WorkflowNodeKindLlmCall, Value: map[string]any{"state": "ready"}},
		},
		TerminalOutput: map[string]any{
			"state": "ready",
		},
	}

	typed := output.ToTypedOutput()
	if typed.WorkflowID != "wf-1" {
		t.Fatalf("expected workflow id wf-1, got %s", typed.WorkflowID)
	}
	if len(typed.NodeOutputs) != 2 {
		t.Fatalf("expected 2 node outputs, got %d", len(typed.NodeOutputs))
	}
	if typed.TerminalOutput == nil {
		t.Fatal("expected terminal output record")
	}
	if typed.TerminalOutput.NodeKind != WorkflowNodeKindLlmCall {
		t.Fatalf("expected llm_call terminal node kind, got %s", typed.TerminalOutput.NodeKind)
	}
}

func TestWorkflowEventToTypedEventMapsKnownAndUnknown(t *testing.T) {
	nodeID := "classify"
	event := WorkflowEvent{EventType: "node_completed", NodeID: &nodeID}
	typed := event.ToTypedEvent()
	if typed.EventType != WorkflowEventTypeNodeCompleted {
		t.Fatalf("expected node_completed type, got %s", typed.EventType)
	}
	if typed.RawEventType != "node_completed" {
		t.Fatalf("expected raw event type node_completed, got %s", typed.RawEventType)
	}

	unknown := WorkflowEvent{EventType: "custom_event"}.ToTypedEvent()
	if unknown.EventType != WorkflowEventTypeUnknown {
		t.Fatalf("expected unknown event type, got %s", unknown.EventType)
	}
}

func TestRunWorkflowYAMLTypedUninitializedClient(t *testing.T) {
	c := &Client{}
	_, err := c.RunWorkflowYAMLTyped(context.Background(), "workflow.yaml", map[string]any{"email_text": "x"})
	if err == nil {
		t.Fatal("expected uninitialized client error")
	}
}

func TestValidateWorkflowRunRequest(t *testing.T) {
	err := validateWorkflowRunRequest(WorkflowRunRequest{})
	if err == nil {
		t.Fatal("expected empty path error")
	}
	err = validateWorkflowRunRequest(WorkflowRunRequest{WorkflowPath: "w.yaml"})
	if err == nil {
		t.Fatal("expected nil input error")
	}
	err = validateWorkflowRunRequest(WorkflowRunRequest{
		WorkflowPath: "w.yaml",
		Input:        &TypedWorkflowInput{},
	})
	if err == nil {
		t.Fatal("expected missing messages error")
	}
	if err := validateWorkflowRunRequest(WorkflowRunRequest{
		WorkflowPath: "w.yaml",
		Input: &TypedWorkflowInput{
			Messages: []WorkflowInputMessage{{Role: MessageRoleUser, Content: "hi"}},
		},
	}); err != nil {
		t.Fatalf("expected valid request, got %v", err)
	}
}

func TestWorkflowYAMLDeclaresCustomWorker(t *testing.T) {
	dir := t.TempDir()
	plainPath := filepath.Join(dir, "plain.yaml")
	if err := os.WriteFile(plainPath, []byte("workflow:\n  entry: start\n"), 0o600); err != nil {
		t.Fatal(err)
	}
	ok, err := workflowYAMLDeclaresCustomWorker(plainPath)
	if err != nil {
		t.Fatalf("plain workflow: %v", err)
	}
	if ok {
		t.Fatal("expected no custom_worker match")
	}

	cwPath := filepath.Join(dir, "cw.yaml")
	if err := os.WriteFile(cwPath, []byte("nodes:\n  n1:\n    custom_worker:\n      runtime: python\n"), 0o600); err != nil {
		t.Fatal(err)
	}
	ok, err = workflowYAMLDeclaresCustomWorker(cwPath)
	if err != nil {
		t.Fatalf("custom worker workflow: %v", err)
	}
	if !ok {
		t.Fatal("expected custom_worker match")
	}
}

func TestNewClientWithProviderValidation(t *testing.T) {
	_, err := NewClientWithProvider(ProviderConfig{})
	if err == nil {
		t.Fatal("expected provider validation error")
	}
	_, err = NewClientWithProvider(ProviderConfig{Provider: "openai"})
	if err == nil {
		t.Fatal("expected api key validation error")
	}
}

func TestRunRejectsCustomWorkerBeforeClient(t *testing.T) {
	dir := t.TempDir()
	cwPath := filepath.Join(dir, "cw.yaml")
	if err := os.WriteFile(cwPath, []byte("  custom_worker:\n"), 0o600); err != nil {
		t.Fatal(err)
	}
	c := &Client{}
	req := WorkflowRunRequest{
		WorkflowPath: cwPath,
		Input: &TypedWorkflowInput{
			Messages: []WorkflowInputMessage{{Role: MessageRoleUser, Content: "hi"}},
		},
	}
	_, err := c.Run(context.Background(), req, WorkflowRunFlags{})
	if err == nil {
		t.Fatal("expected custom_worker error before client use")
	}
}
