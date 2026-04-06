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

// Default workflow has no custom_worker nodes (Go FFI does not inject a custom worker executor).
// Workflows like email-intake-classification.yaml require custom_worker and will not complete via FFI alone.

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

	client, err := simpleagents.NewClient(apiKey, apiBase)
	if err != nil {
		log.Fatal(err)
	}
	defer client.Close()

	workflowPath := "../../examples/workflow_email/email-chat-draft-or-clarify.yaml"
	if len(os.Args) > 1 {
		workflowPath = os.Args[1]
	}

	emailText := "Termination request, second warning already issued."
	if len(os.Args) > 2 {
		emailText = os.Args[2]
	}

	input := map[string]any{
		"email_text": emailText,
		"messages": []map[string]any{
			{"role": "user", "content": emailText},
		},
	}
	inputJSON, err := json.Marshal(input)
	if err != nil {
		log.Fatal(err)
	}

	ctx, cancel := context.WithTimeout(context.Background(), 120*time.Second)
	defer cancel()

	outJSON, err := client.Run(ctx, workflowPath, inputJSON)
	if err != nil {
		log.Fatal(err)
	}

	var out map[string]any
	if err := json.Unmarshal(outJSON, &out); err != nil {
		log.Fatal(err)
	}

	payload, err := json.MarshalIndent(out, "", "  ")
	if err != nil {
		log.Fatal(err)
	}

	fmt.Println(string(payload))
}
