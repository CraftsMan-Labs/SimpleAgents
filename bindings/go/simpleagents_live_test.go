package simpleagents

import (
	"context"
	"fmt"
	"os"
	"testing"
	"time"
)

func requireLiveEnv(t *testing.T) (string, string) {
	t.Helper()
	provider := os.Getenv("PROVIDER")
	model := os.Getenv("CUSTOM_API_MODEL")
	key := os.Getenv("CUSTOM_API_KEY")

	if provider == "" || model == "" || key == "" {
		t.Skip("set PROVIDER, CUSTOM_API_KEY, and CUSTOM_API_MODEL for live test")
	}

	switch provider {
	case "openai":
		os.Setenv("OPENAI_API_KEY", key)
		if base := os.Getenv("CUSTOM_API_BASE"); base != "" {
			os.Setenv("OPENAI_API_BASE", base)
		}
	case "anthropic":
		os.Setenv("ANTHROPIC_API_KEY", key)
	case "openrouter":
		os.Setenv("OPENROUTER_API_KEY", key)
		if base := os.Getenv("CUSTOM_API_BASE"); base != "" {
			os.Setenv("OPENROUTER_API_BASE", base)
		}
	default:
		t.Fatalf("unsupported PROVIDER %q", provider)
	}

	return provider, model
}

func TestLiveCompleteMessages(t *testing.T) {
	provider, model := requireLiveEnv(t)

	client, err := NewClientFromEnv(provider)
	if err != nil {
		t.Fatalf("new client: %v", err)
	}
	defer client.Close()

	maxTokens := int32(24)
	temp := float32(0.2)
	res, err := client.CompleteMessages(
		context.Background(),
		model,
		[]Message{{Role: "user", Content: "Reply with one short sentence saying hello."}},
		CompleteOptions{MaxTokens: &maxTokens, Temperature: &temp},
	)
	if err != nil {
		t.Fatalf("complete messages: %v", err)
	}
	if res.Content == "" && len(res.ToolCalls) == 0 {
		t.Fatal("expected content or tool calls")
	}
}

func TestLiveStreamMessages(t *testing.T) {
	provider, model := requireLiveEnv(t)

	client, err := NewClientFromEnv(provider)
	if err != nil {
		t.Fatalf("new client: %v", err)
	}
	defer client.Close()

	maxTokens := int32(32)
	temp := float32(0.2)
	ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
	defer cancel()

	stream, err := client.StreamMessages(
		ctx,
		model,
		[]Message{{Role: "user", Content: "Reply with hello in a short sentence."}},
		CompleteOptions{MaxTokens: &maxTokens, Temperature: &temp},
	)
	if err != nil {
		t.Fatalf("stream messages: %v", err)
	}

	chunkCount := 0
	for item := range stream {
		if item.Err != nil {
			t.Fatalf("stream item error: %v", item.Err)
		}
		if item.Event.Type == "chunk" {
			chunkCount++
		}
	}

	if chunkCount == 0 {
		t.Fatal("expected at least one stream chunk")
	}
}

func TestLiveWorkflowStreamExplicitEventTypes(t *testing.T) {
	provider, model := requireLiveEnv(t)

	client, err := NewClientFromEnv(provider)
	if err != nil {
		t.Fatalf("new client: %v", err)
	}
	defer client.Close()

	if err := os.Setenv("SIMPLE_AGENTS_WORKFLOW_STREAM_INCLUDE_RAW", "1"); err != nil {
		t.Fatalf("set stream raw env: %v", err)
	}
	t.Cleanup(func() { _ = os.Unsetenv("SIMPLE_AGENTS_WORKFLOW_STREAM_INCLUDE_RAW") })

	workflowFile, err := os.CreateTemp("", "live-workflow-stream-*.yaml")
	if err != nil {
		t.Fatalf("create temp workflow: %v", err)
	}
	t.Cleanup(func() { _ = os.Remove(workflowFile.Name()) })

	workflow := fmt.Sprintf(`id: live-workflow-stream-test
version: 1.0.0
entry_node: classify
nodes:
  - id: classify
    node_type:
      llm_call:
        model: %s
        messages_path: input.messages
        append_prompt_as_user: true
        stream: true
        stream_json_as_text: true
        heal: false
    config:
      prompt: |
        Return JSON only:
        {
          "state": "capabilities_query",
          "reason": "short"
        }
  - id: explain
    node_type:
      llm_call:
        model: %s
        messages_path: input.messages
        append_prompt_as_user: true
        stream: true
        stream_json_as_text: true
        heal: false
    config:
      prompt: |
        Return JSON only:
        {
          "question": "one short sentence"
        }
edges:
  - from: classify
    to: explain
`, model, model)
	if _, err := workflowFile.WriteString(workflow); err != nil {
		t.Fatalf("write temp workflow: %v", err)
	}
	if err := workflowFile.Close(); err != nil {
		t.Fatalf("close temp workflow: %v", err)
	}

	ctx, cancel := context.WithTimeout(context.Background(), 60*time.Second)
	defer cancel()

	workflowInput := map[string]any{
		"messages": []map[string]string{{
			"role":    "user",
			"content": "Hi",
		}},
	}

	eventTypes := map[string]int{}
	var completionNerdstats map[string]any
	_, err = client.RunWorkflowYAMLStreamWithOptions(ctx, workflowFile.Name(), workflowInput, map[string]any{
		"telemetry": map[string]any{"nerdstats": true},
	}, func(event WorkflowEvent) {
		eventTypes[event.EventType]++
		if event.EventType == "workflow_completed" && event.Metadata != nil {
			if raw, ok := event.Metadata["nerdstats"]; ok {
				if nerdstats, ok := raw.(map[string]any); ok {
					completionNerdstats = nerdstats
				}
			}
		}
	})
	if err != nil {
		t.Fatalf("run workflow stream: %v", err)
	}

	if eventTypes["node_stream_delta"] == 0 {
		t.Fatal("expected node_stream_delta events")
	}
	if eventTypes["node_stream_output_delta"] == 0 {
		t.Fatal("expected node_stream_output_delta events")
	}
	if eventTypes["node_stream_raw_delta"] > 0 {
		t.Fatalf("deprecated node_stream_raw_delta should not be emitted: %d", eventTypes["node_stream_raw_delta"])
	}
	if completionNerdstats == nil {
		t.Fatal("expected workflow_completed metadata.nerdstats")
	}
	if _, ok := completionNerdstats["total_elapsed_ms"]; !ok {
		t.Fatal("expected nerdstats.total_elapsed_ms")
	}
	if _, ok := completionNerdstats["token_metrics_available"]; !ok {
		t.Fatal("expected nerdstats.token_metrics_available")
	}
}
