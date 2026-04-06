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

type summaryItem struct {
	Workflow     string `json:"workflow"`
	Status       string `json:"status"`
	TerminalNode string `json:"terminal_node,omitempty"`
	TotalElapsed any    `json:"total_elapsed_ms,omitempty"`
	Error        string `json:"error,omitempty"`
}

func main() {
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

	emailText := "Please help with a damaged supply order and draft the right response."
	if len(os.Args) > 1 {
		emailText = os.Args[1]
	}

	// Only workflows without custom_worker nodes run with the FFI (no custom executor).
	workflows := []string{
		"examples/workflow_email/email-chat-draft-or-clarify.yaml",
		"examples/workflow_email/email-chat-draft-with-tool-calling.yaml",
		"examples/workflow_email/email-chat-orchestrator-with-subgraph-tool.yaml",
	}

	workflowInput := map[string]any{
		"email_text": emailText,
		"messages": []map[string]any{
			{
				"role":    "system",
				"content": "You are a professional assistant for workflow testing.",
			},
			{
				"role":    "user",
				"content": emailText,
			},
		},
	}
	inputJSON, err := json.Marshal(workflowInput)
	if err != nil {
		log.Fatal(err)
	}

	summary := make([]summaryItem, 0, len(workflows))
	for _, workflowPath := range workflows {
		ctx, cancel := context.WithTimeout(context.Background(), 120*time.Second)
		outJSON, runErr := client.Run(ctx, workflowPath, inputJSON)
		cancel()
		if runErr != nil {
			summary = append(summary, summaryItem{
				Workflow: workflowPath,
				Status:   "error",
				Error:    runErr.Error(),
			})
			continue
		}

		var out map[string]any
		if err := json.Unmarshal(outJSON, &out); err != nil {
			summary = append(summary, summaryItem{
				Workflow: workflowPath,
				Status:   "error",
				Error:    err.Error(),
			})
			continue
		}

		summary = append(summary, summaryItem{
			Workflow:     workflowPath,
			Status:       "ok",
			TerminalNode: strField(out, "terminal_node"),
			TotalElapsed: out["total_elapsed_ms"],
		})
	}

	payload, err := json.MarshalIndent(summary, "", "  ")
	if err != nil {
		log.Fatal(err)
	}

	fmt.Println(string(payload))
}

func strField(m map[string]any, key string) string {
	v, ok := m[key].(string)
	if !ok {
		return ""
	}
	return v
}
