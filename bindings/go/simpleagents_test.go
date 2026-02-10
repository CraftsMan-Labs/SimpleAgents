package simpleagents

import (
	"context"
	"os"
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

func TestLiveCompleteMessages(t *testing.T) {
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
