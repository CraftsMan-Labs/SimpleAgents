package simpleagents

import (
	"context"
	"encoding/json"
	"os"
	"testing"
	"time"
)

func requireLiveEnv(t *testing.T) (apiKey, baseURL string) {
	t.Helper()
	key := os.Getenv("CUSTOM_API_KEY")
	base := os.Getenv("CUSTOM_API_BASE")
	if key == "" {
		t.Skip("set CUSTOM_API_KEY for live test")
	}
	return key, base
}

func TestLiveCompleteJSON(t *testing.T) {
	apiKey, baseURL := requireLiveEnv(t)
	model := os.Getenv("CUSTOM_API_MODEL")
	if model == "" {
		t.Skip("set CUSTOM_API_MODEL for live test")
	}

	client, err := NewClient(apiKey, baseURL)
	if err != nil {
		t.Fatalf("new client: %v", err)
	}
	defer client.Close()

	req := map[string]any{
		"model": model,
		"messages": []map[string]string{
			{"role": "user", "content": "Reply with one short sentence saying hello."},
		},
		"max_tokens":  24,
		"temperature": 0.2,
	}
	reqJSON, err := json.Marshal(req)
	if err != nil {
		t.Fatal(err)
	}

	resJSON, err := client.Complete(context.Background(), reqJSON)
	if err != nil {
		t.Fatalf("complete: %v", err)
	}
	var resp map[string]any
	if err := json.Unmarshal(resJSON, &resp); err != nil {
		t.Fatalf("parse response: %v", err)
	}
	choices, ok := resp["choices"].([]any)
	if !ok || len(choices) == 0 {
		t.Fatalf("expected choices: %s", string(resJSON))
	}
}

func TestLiveStreamJSON(t *testing.T) {
	apiKey, baseURL := requireLiveEnv(t)
	model := os.Getenv("CUSTOM_API_MODEL")
	if model == "" {
		t.Skip("set CUSTOM_API_MODEL for live test")
	}

	client, err := NewClient(apiKey, baseURL)
	if err != nil {
		t.Fatalf("new client: %v", err)
	}
	defer client.Close()

	req := map[string]any{
		"model": model,
		"messages": []map[string]string{
			{"role": "user", "content": "Reply with hello in a short sentence."},
		},
		"max_tokens":  32,
		"temperature": 0.2,
		"stream":      true,
	}
	reqJSON, err := json.Marshal(req)
	if err != nil {
		t.Fatal(err)
	}

	ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
	defer cancel()

	chunkCount := 0
	err = client.StreamComplete(ctx, reqJSON, func(chunkJSON string) error {
		var chunk map[string]any
		if err := json.Unmarshal([]byte(chunkJSON), &chunk); err != nil {
			return nil
		}
		choices, ok := chunk["choices"].([]any)
		if !ok || len(choices) == 0 {
			return nil
		}
		first, ok := choices[0].(map[string]any)
		if !ok {
			return nil
		}
		delta, ok := first["delta"].(map[string]any)
		if !ok {
			return nil
		}
		if content, ok := delta["content"].(string); ok && content != "" {
			chunkCount++
		}
		return nil
	})
	if err != nil {
		t.Fatalf("stream: %v", err)
	}
	if chunkCount == 0 {
		t.Fatal("expected at least one content chunk")
	}
}

func TestLiveWorkflowStreamEventTypes(t *testing.T) {
	apiKey, baseURL := requireLiveEnv(t)
	model := os.Getenv("CUSTOM_API_MODEL")
	if model == "" {
		t.Skip("set CUSTOM_API_MODEL for live test")
	}

	client, err := NewClient(apiKey, baseURL)
	if err != nil {
		t.Fatalf("new client: %v", err)
	}
	defer client.Close()

	tmp := t.TempDir()
	workflowPath := tmp + "/live-stream.yaml"
	workflow := `id: live-workflow-stream-test
version: 1.0.0
entry_node: classify
nodes:
  - id: classify
    node_type:
      llm_call:
        model: ` + model + `
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
        model: ` + model + `
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
`
	if err := os.WriteFile(workflowPath, []byte(workflow), 0o600); err != nil {
		t.Fatal(err)
	}

	ctx, cancel := context.WithTimeout(context.Background(), 120*time.Second)
	defer cancel()

	inputJSON, err := json.Marshal(map[string]any{
		"messages": []map[string]string{{
			"role": "user", "content": "Hi",
		}},
	})
	if err != nil {
		t.Fatal(err)
	}

	eventTypes := map[string]int{}
	var completionNerdstats map[string]any

	err = client.Stream(ctx, workflowPath, inputJSON, func(eventJSON string) error {
		var ev map[string]any
		if err := json.Unmarshal([]byte(eventJSON), &ev); err != nil {
			return nil
		}
		et, _ := ev["event_type"].(string)
		eventTypes[et]++
		if et == "workflow_completed" {
			if meta, ok := ev["metadata"].(map[string]any); ok {
				if raw, ok := meta["nerdstats"]; ok {
					if nerdstats, ok := raw.(map[string]any); ok {
						completionNerdstats = nerdstats
					}
				}
			}
		}
		return nil
	})
	if err != nil {
		t.Fatalf("stream workflow: %v", err)
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
