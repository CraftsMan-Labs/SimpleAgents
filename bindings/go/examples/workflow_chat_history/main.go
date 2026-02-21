package main

import (
	"bufio"
	"context"
	"encoding/json"
	"errors"
	"flag"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"time"

	"simpleagents"
)

type message struct {
	Role    string `json:"role"`
	Content string `json:"content"`
}

func loadDotEnv(filePath string) {
	content, err := os.ReadFile(filePath)
	if err != nil {
		return
	}
	for _, line := range strings.Split(string(content), "\n") {
		trimmed := strings.TrimSpace(line)
		if trimmed == "" || strings.HasPrefix(trimmed, "#") {
			continue
		}
		splitIndex := strings.Index(trimmed, "=")
		if splitIndex <= 0 {
			continue
		}
		key := strings.TrimSpace(trimmed[:splitIndex])
		value := strings.TrimSpace(trimmed[splitIndex+1:])
		if _, ok := os.LookupEnv(key); !ok {
			_ = os.Setenv(key, value)
		}
	}
}

func loadConfig() (string, string, string, error) {
	loadDotEnv(filepath.Clean("examples/.env"))
	loadDotEnv(filepath.Clean(".env"))
	loadDotEnv(filepath.Clean("../../examples/.env"))
	loadDotEnv(filepath.Clean("../../.env"))

	provider := os.Getenv("WORKFLOW_PROVIDER")
	if provider == "" {
		provider = "openai"
	}

	apiBase := os.Getenv("WORKFLOW_API_BASE")
	if apiBase == "" {
		apiBase = os.Getenv("CUSTOM_API_BASE")
	}

	apiKey := os.Getenv("WORKFLOW_API_KEY")
	if apiKey == "" {
		apiKey = os.Getenv("CUSTOM_API_KEY")
	}

	if apiBase == "" || apiKey == "" {
		return "", "", "", errors.New("set WORKFLOW_API_BASE and WORKFLOW_API_KEY (or CUSTOM_API_BASE/CUSTOM_API_KEY)")
	}

	return provider, apiBase, apiKey, nil
}

func setProviderEnv(provider, apiKey, apiBase string) error {
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
		return fmt.Errorf("unsupported WORKFLOW_PROVIDER: %s", provider)
	}
	return nil
}

func resolveWorkflowPath(workflow string) (string, error) {
	if _, err := os.Stat(workflow); err == nil {
		return workflow, nil
	}

	fromExamplesDir := filepath.Join("examples", workflow)
	if _, err := os.Stat(fromExamplesDir); err == nil {
		return fromExamplesDir, nil
	}

	if strings.HasPrefix(workflow, "examples/") {
		trimmed := strings.TrimPrefix(workflow, "examples/")
		trimmedPath := filepath.Join("examples", trimmed)
		if _, err := os.Stat(trimmedPath); err == nil {
			return trimmedPath, nil
		}
	}

	return "", fmt.Errorf("workflow file not found: %s", workflow)
}

func initialMessages() []message {
	return []message{
		{
			Role:    "system",
			Content: "You are a friendly email drafting assistant for new users. First, explain capabilities clearly when asked what you can do. Then gather missing scenario details and draft concise professional emails. If context is incomplete, ask one specific follow-up question.",
		},
	}
}

func renderAssistantReply(output any) string {
	if output == nil {
		return ""
	}
	if text, ok := output.(string); ok {
		return text
	}
	encoded, err := json.MarshalIndent(output, "", "  ")
	if err != nil {
		return ""
	}
	return string(encoded)
}

func printStepJSONSummary(out simpleagents.WorkflowYAMLOutput) {
	for _, node := range out.Trace {
		nodeValue, ok := out.Outputs[node]
		if !ok {
			continue
		}
		payload, ok := nodeValue["output"]
		if !ok {
			continue
		}
		fmt.Printf("\nStep: %s\n", node)
		fmt.Println("JSON")
		encoded, err := json.MarshalIndent(payload, "", "  ")
		if err != nil {
			continue
		}
		fmt.Println(string(encoded))
	}

	if out.TerminalNode != "" && out.TerminalOutput != nil {
		fmt.Printf("\nTerminal Step: %s\n", out.TerminalNode)
		fmt.Println("JSON")
		encoded, err := json.MarshalIndent(out.TerminalOutput, "", "  ")
		if err != nil {
			return
		}
		fmt.Println(string(encoded))
	}
}

func main() {
	workflowFlag := flag.String("workflow", "workflow_email/email-chat-draft-or-clarify.yaml", "Path to workflow YAML file")
	includeEventsFlag := flag.Bool("include-events", false, "Include workflow events in each turn response")
	maxTurnsFlag := flag.Int("max-turns", 8, "Maximum chat turns before exiting")
	streamFlag := flag.Bool("stream", false, "Stream workflow node deltas live in terminal when YAML nodes have stream=true")
	showThinkingFlag := flag.Bool("show-thinking", false, "Show raw model stream deltas including thinking tokens")
	traceDirFlag := flag.String("trace-dir", "examples/workflow_email/traces", "Directory to persist per-turn workflow traces as JSONL")
	showStepJSONFlag := flag.Bool("show-step-json", false, "Print per-step JSON summaries after execution")
	flag.Parse()

	if *showThinkingFlag {
		_ = os.Setenv("SIMPLE_AGENTS_WORKFLOW_STREAM_INCLUDE_RAW", "1")
	} else {
		_ = os.Unsetenv("SIMPLE_AGENTS_WORKFLOW_STREAM_INCLUDE_RAW")
	}

	provider, apiBase, apiKey, err := loadConfig()
	if err != nil {
		panic(err)
	}
	if err := setProviderEnv(provider, apiKey, apiBase); err != nil {
		panic(err)
	}

	workflowPath, err := resolveWorkflowPath(*workflowFlag)
	if err != nil {
		panic(err)
	}

	client, err := simpleagents.NewClientFromEnv(provider)
	if err != nil {
		panic(err)
	}
	defer client.Close()

	traceDir := filepath.Clean(*traceDirFlag)
	if err := os.MkdirAll(traceDir, 0o755); err != nil {
		panic(err)
	}
	sessionID := time.Now().UTC().Format("20060102T150405Z")
	traceFile := filepath.Join(traceDir, fmt.Sprintf("chat-session-%s.jsonl", sessionID))

	messages := initialMessages()
	interviewClosed := false

	fmt.Println("Chat Email Assistant")
	fmt.Println("Type your request. Type 'exit' to quit.")
	fmt.Printf("Trace log: %s\n\n", traceFile)

	reader := bufio.NewReader(os.Stdin)

	for turn := 1; turn <= *maxTurnsFlag; turn++ {
		fmt.Print("You: ")
		line, readErr := reader.ReadString('\n')
		if readErr != nil && strings.TrimSpace(line) == "" {
			fmt.Println("Bye!")
			return
		}

		userInput := strings.TrimSpace(line)
		if userInput == "" {
			continue
		}
		lower := strings.ToLower(userInput)
		if lower == "exit" || lower == "quit" {
			fmt.Println("Bye!")
			return
		}
		if interviewClosed {
			fmt.Println()
			fmt.Println("Assistant: This interview session is already closed after termination. Please start a new session with a new run.")
			fmt.Println()
			continue
		}

		messages = append(messages, message{Role: "user", Content: userInput})

		workflowInputMessages := make([]map[string]any, 0, len(messages))
		for _, m := range messages {
			workflowInputMessages = append(workflowInputMessages, map[string]any{
				"role":    m.Role,
				"content": m.Content,
			})
		}

		ctx, cancel := context.WithTimeout(context.Background(), 60*time.Second)

		workflowInput := map[string]any{
			"email_text": userInput,
			"messages":   workflowInputMessages,
		}

		streamedEvents := make([]simpleagents.WorkflowEvent, 0)
		out, runErr := simpleagents.WorkflowYAMLOutput{}, error(nil)
		switch {
		case *streamFlag:
			currentNode := ""
			lineOpen := false
			out, runErr = client.RunWorkflowYAMLStream(ctx, workflowPath, workflowInput, func(event simpleagents.WorkflowEvent) {
				streamedEvents = append(streamedEvents, event)
				expectedEventType := "node_stream_delta"
				if *showThinkingFlag {
					expectedEventType = "node_stream_raw_delta"
				}
				if event.EventType != expectedEventType || event.Delta == nil {
					return
				}
				displayNode := "Workflow"
				if event.NodeID != nil {
					displayNode = *event.NodeID
				} else if event.StepID != nil {
					displayNode = *event.StepID
				}
				if currentNode != displayNode {
					if lineOpen {
						fmt.Println()
					}
					fmt.Printf("\nStep: %s\n", displayNode)
					fmt.Print("Streaming: ")
					currentNode = displayNode
					lineOpen = true
				}
				fmt.Print(*event.Delta)
			})
		case *includeEventsFlag:
			out, runErr = client.RunWorkflowYAMLWithEvents(ctx, workflowPath, workflowInput, nil)
		default:
			out, runErr = client.RunWorkflowYAML(ctx, workflowPath, workflowInput)
		}
		cancel()
		if runErr != nil {
			panic(runErr)
		}
		if *streamFlag && len(streamedEvents) > 0 {
			fmt.Println()
		}

		if *showStepJSONFlag {
			printStepJSONSummary(out)
		}

		eventsValue := any(nil)
		if *streamFlag {
			eventsValue = streamedEvents
		} else if *includeEventsFlag {
			eventsValue = out.Events
		}

		traceRecord := map[string]any{
			"timestamp":        time.Now().UTC().Format(time.RFC3339Nano),
			"turn":             turn,
			"workflow_path":    workflowPath,
			"workflow_id":      out.WorkflowID,
			"terminal_node":    out.TerminalNode,
			"trace":            out.Trace,
			"step_timings":     out.StepTimings,
			"total_elapsed_ms": out.TotalElapsedMS,
			"user_input":       userInput,
			"assistant_output": out.TerminalOutput,
			"events":           eventsValue,
		}
		encodedRecord, marshalErr := json.Marshal(traceRecord)
		if marshalErr != nil {
			panic(marshalErr)
		}
		traceHandle, openErr := os.OpenFile(traceFile, os.O_APPEND|os.O_CREATE|os.O_WRONLY, 0o644)
		if openErr != nil {
			panic(openErr)
		}
		_, writeErr := traceHandle.WriteString(string(encodedRecord) + "\n")
		closeErr := traceHandle.Close()
		if writeErr != nil {
			panic(writeErr)
		}
		if closeErr != nil {
			panic(closeErr)
		}

		reply := renderAssistantReply(out.TerminalOutput)
		fmt.Printf("\nAssistant: %s\n\n", reply)
		messages = append(messages, message{Role: "assistant", Content: reply})

		terminalOutputMap, isMap := out.TerminalOutput.(map[string]any)
		decision := ""
		if isMap {
			if rawDecision, ok := terminalOutputMap["decision"].(string); ok {
				decision = rawDecision
			}
		}

		if out.TerminalNode == "terminate_candidate" || out.TerminalNode == "already_terminated" || decision == "terminated" {
			interviewClosed = true
			fmt.Println("Interview closed for this session. Start a new run for a new candidate.")
			fmt.Println()
		}

		if out.TerminalNode == "generate_email_draft" {
			fmt.Println("Draft ready. Continue chatting to refine, or type 'exit'.")
			fmt.Println()
		}
	}

	fmt.Println("Reached max turns. Restart to continue.")
}
