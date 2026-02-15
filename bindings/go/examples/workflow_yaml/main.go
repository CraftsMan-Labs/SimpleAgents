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

func setProviderEnv(provider, apiKey, apiBase string) {
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
}

func main() {
	provider := os.Getenv("WORKFLOW_PROVIDER")
	if provider == "" {
		provider = "openai"
	}
	apiKey := os.Getenv("WORKFLOW_API_KEY")
	if apiKey == "" {
		apiKey = os.Getenv("CUSTOM_API_KEY")
	}
	apiBase := os.Getenv("WORKFLOW_API_BASE")
	if apiBase == "" {
		apiBase = os.Getenv("CUSTOM_API_BASE")
	}

	if apiKey == "" {
		log.Fatal("set WORKFLOW_API_KEY (or CUSTOM_API_KEY)")
	}

	setProviderEnv(provider, apiKey, apiBase)

	workflowPath := "../../examples/workflow_email/email-intake-classification.yaml"
	if len(os.Args) > 1 {
		workflowPath = os.Args[1]
	}

	emailText := "Termination request, second warning already issued."
	if len(os.Args) > 2 {
		emailText = os.Args[2]
	}

	client, err := simpleagents.NewClientFromEnv(provider)
	if err != nil {
		log.Fatal(err)
	}
	defer client.Close()

	ctx, cancel := context.WithTimeout(context.Background(), 45*time.Second)
	defer cancel()

	out, err := client.RunEmailWorkflowYAML(ctx, workflowPath, emailText)
	if err != nil {
		log.Fatal(err)
	}

	payload, err := json.MarshalIndent(out, "", "  ")
	if err != nil {
		log.Fatal(err)
	}

	fmt.Println(string(payload))
}
