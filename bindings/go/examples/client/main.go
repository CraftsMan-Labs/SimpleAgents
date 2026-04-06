package main

import (
	"context"
	"encoding/json"
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

	client, err := simpleagents.NewClient(apiKey, apiBase)
	if err != nil {
		log.Fatal(err)
	}
	defer client.Close()

	ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
	defer cancel()

	maxTokens := uint32(64)
	temp := float32(0.2)
	req := map[string]any{
		"model": model,
		"messages": []map[string]string{
			{"role": "user", "content": "Respond with JSON: {\"status\": \"ok\"}"},
		},
		"max_tokens":  maxTokens,
		"temperature": temp,
		"response_format": map[string]any{
			"type": "json_object",
		},
	}
	reqJSON, err := json.Marshal(req)
	if err != nil {
		log.Fatal(err)
	}

	resJSON, err := client.Complete(ctx, reqJSON)
	if err != nil {
		log.Fatal(err)
	}

	var resp map[string]any
	if err := json.Unmarshal(resJSON, &resp); err != nil {
		log.Fatal(err)
	}
	choices, _ := resp["choices"].([]any)
	if len(choices) == 0 {
		fmt.Println(string(resJSON))
		return
	}
	first, _ := choices[0].(map[string]any)
	msg, _ := first["message"].(map[string]any)
	content, _ := msg["content"].(string)
	fmt.Println("content:", content)
}
