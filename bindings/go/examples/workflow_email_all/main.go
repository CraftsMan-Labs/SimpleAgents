package main

import (
	"context"
	"encoding/json"
	"fmt"
	"log"
	"os"
	"path/filepath"
	"sort"
	"time"

	"simpleagents"
)

type summaryItem struct {
	Workflow     string `json:"workflow"`
	Status       string `json:"status"`
	TerminalNode string `json:"terminal_node,omitempty"`
	TotalElapsed uint64 `json:"total_elapsed_ms,omitempty"`
	Error        string `json:"error,omitempty"`
}

func setProviderEnv(provider string, apiKey string, apiBase string) {
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

func listWorkflows(baseDir string) ([]string, error) {
	matches, err := filepath.Glob(filepath.Join(baseDir, "*.yaml"))
	if err != nil {
		return nil, err
	}
	sort.Strings(matches)
	return matches, nil
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

	emailText := "Please help with a damaged supply order and draft the right response."
	if len(os.Args) > 1 {
		emailText = os.Args[1]
	}

	workflows, err := listWorkflows("examples/workflow_email")
	if err != nil {
		log.Fatal(err)
	}

	client, err := simpleagents.NewClientFromEnv(provider)
	if err != nil {
		log.Fatal(err)
	}
	defer client.Close()

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

	summary := make([]summaryItem, 0, len(workflows))
	for _, workflowPath := range workflows {
		ctx, cancel := context.WithTimeout(context.Background(), 90*time.Second)
		out, runErr := client.RunWorkflowYAML(ctx, workflowPath, workflowInput)
		cancel()
		if runErr != nil {
			summary = append(summary, summaryItem{
				Workflow: workflowPath,
				Status:   "error",
				Error:    runErr.Error(),
			})
			continue
		}

		summary = append(summary, summaryItem{
			Workflow:     workflowPath,
			Status:       "ok",
			TerminalNode: out.TerminalNode,
			TotalElapsed: out.TotalElapsedMS,
		})
	}

	payload, err := json.MarshalIndent(summary, "", "  ")
	if err != nil {
		log.Fatal(err)
	}

	fmt.Println(string(payload))
}
