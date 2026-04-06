package simpleagents

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"io"
	"os"
	"testing"
)

func TestParseWorkflowOutputValid(t *testing.T) {
	raw := []byte(`{"workflow_id":"w1","terminal_node":"end"}`)
	m, err := ParseWorkflowOutput(raw)
	if err != nil {
		t.Fatal(err)
	}
	if m["workflow_id"] != "w1" {
		t.Fatalf("workflow_id: %v", m["workflow_id"])
	}
}

func TestParseWorkflowOutputEmpty(t *testing.T) {
	_, err := ParseWorkflowOutput([]byte{})
	if err == nil {
		t.Fatal("expected error")
	}
}

func TestWorkflowInputMarshalJSON(t *testing.T) {
	w := WorkflowInput{
		Messages: []Message{{Role: MessageRoleUser, Content: "hi"}},
		Extra:    map[string]interface{}{"email_text": "test"},
	}
	b, err := json.Marshal(w)
	if err != nil {
		t.Fatal(err)
	}
	var m map[string]interface{}
	if err := json.Unmarshal(b, &m); err != nil {
		t.Fatal(err)
	}
	if m["email_text"] != "test" {
		t.Fatalf("extra: %v", m["email_text"])
	}
}

func TestRunUninitializedClient(t *testing.T) {
	c := &Client{}
	_, err := c.Run(context.Background(), "x.yaml", []byte(`{}`))
	if err == nil {
		t.Fatal("expected error")
	}
}

func TestCompleteUninitializedClient(t *testing.T) {
	c := &Client{}
	_, err := c.Complete(context.Background(), []byte(`{"model":"m","messages":[]}`))
	if err == nil {
		t.Fatal("expected error")
	}
}

func TestStreamCompleteUninitializedClient(t *testing.T) {
	c := &Client{}
	err := c.StreamComplete(context.Background(), []byte(`{"model":"m","messages":[],"stream":true}`), func(string) error { return nil })
	if err == nil {
		t.Fatal("expected error")
	}
}

// captureStdout runs f and returns whatever it printed to os.Stdout.
func captureStdout(f func()) string {
	r, w, _ := os.Pipe()
	origStdout := os.Stdout
	os.Stdout = w
	f()
	w.Close()
	os.Stdout = origStdout
	var buf bytes.Buffer
	io.Copy(&buf, r) //nolint:errcheck
	return buf.String()
}

func TestDefaultOnEventPrintsDelta(t *testing.T) {
	for _, et := range []string{
		EventTypeNodeStreamDelta,
		EventTypeNodeStreamThinkingDelta,
		EventTypeNodeStreamOutputDelta,
	} {
		t.Run(et, func(t *testing.T) {
			eventJSON := fmt.Sprintf(`{"event_type":%q,"delta":"tok"}`, et)
			out := captureStdout(func() {
				if err := DefaultOnEvent(eventJSON); err != nil {
					t.Fatal(err)
				}
			})
			if out != "tok" {
				t.Fatalf("expected %q, got %q", "tok", out)
			}
		})
	}
}

func TestDefaultOnEventSilencesLifecycle(t *testing.T) {
	for _, et := range []string{EventTypeWorkflowStarted, EventTypeWorkflowCompleted} {
		t.Run(et, func(t *testing.T) {
			eventJSON := fmt.Sprintf(`{"event_type":%q}`, et)
			out := captureStdout(func() {
				DefaultOnEvent(eventJSON) //nolint:errcheck
			})
			if out != "" {
				t.Fatalf("expected no output, got %q", out)
			}
		})
	}
}

func TestDefaultOnEventIgnoresInvalidJSON(t *testing.T) {
	if err := DefaultOnEvent("not json"); err != nil {
		t.Fatal("expected nil error for invalid JSON, got:", err)
	}
}

func TestEventTypeConstantsWireNames(t *testing.T) {
	// Spot-check that the canonical wire name is used (not the wrong alias).
	if EventTypeResolvedLlmInput != "resolved_llm_input" {
		t.Fatalf("wrong wire name: %q", EventTypeResolvedLlmInput)
	}
	if EventTypeNodeStreamDelta != "node_stream_delta" {
		t.Fatalf("wrong wire name: %q", EventTypeNodeStreamDelta)
	}
}

func TestWorkflowRunnerEventUnmarshal(t *testing.T) {
	raw := `{"event_type":"node_stream_delta","node_id":"n1","delta":"hello"}`
	var ev WorkflowRunnerEvent
	if err := json.Unmarshal([]byte(raw), &ev); err != nil {
		t.Fatal(err)
	}
	if ev.EventType != "node_stream_delta" {
		t.Fatalf("EventType: %q", ev.EventType)
	}
	if ev.NodeID != "n1" {
		t.Fatalf("NodeID: %q", ev.NodeID)
	}
	if ev.Delta != "hello" {
		t.Fatalf("Delta: %q", ev.Delta)
	}
}
