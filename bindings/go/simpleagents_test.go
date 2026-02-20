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
