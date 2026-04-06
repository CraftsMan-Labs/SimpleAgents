package simpleagents

import (
	"context"
	"encoding/json"
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
