package simpleagents

import (
	"context"
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
