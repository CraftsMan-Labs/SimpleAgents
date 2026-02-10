package main

import (
	"context"
	"fmt"
	"log"
	"os"
	"time"

	"simpleagents"
)

func main() {
	provider := os.Getenv("PROVIDER")
	model := os.Getenv("CUSTOM_API_MODEL")
	apiKey := os.Getenv("CUSTOM_API_KEY")
	apiBase := os.Getenv("CUSTOM_API_BASE")

	if provider == "" || model == "" || apiKey == "" {
		log.Fatal("set PROVIDER, CUSTOM_API_KEY, and CUSTOM_API_MODEL")
	}

	switch provider {
	case "openai":
		_ = os.Setenv("OPENAI_API_KEY", apiKey)
		if apiBase != "" {
			_ = os.Setenv("OPENAI_API_BASE", apiBase)
		}
	case "anthropic":
		_ = os.Setenv("ANTHROPIC_API_KEY", apiKey)
	case "openrouter":
		_ = os.Setenv("OPENROUTER_API_KEY", apiKey)
		if apiBase != "" {
			_ = os.Setenv("OPENROUTER_API_BASE", apiBase)
		}
	default:
		log.Fatalf("unsupported provider %q", provider)
	}

	client, err := simpleagents.NewClientFromEnv(provider)
	if err != nil {
		log.Fatal(err)
	}
	defer client.Close()

	ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
	defer cancel()

	maxTokens := int32(64)
	temperature := float32(0.2)
	result, err := client.CompleteMessages(
		ctx,
		model,
		[]simpleagents.Message{{Role: "user", Content: "Respond with JSON: {\"status\": \"ok\"}"}},
		simpleagents.CompleteOptions{
			Mode:        "healed_json",
			MaxTokens:   &maxTokens,
			Temperature: &temperature,
		},
	)
	if err != nil {
		log.Fatal(err)
	}

	fmt.Println("content:", result.Content)
	if result.Healed != nil {
		fmt.Printf("healed value: %#v\n", result.Healed.Value)
	}
}
