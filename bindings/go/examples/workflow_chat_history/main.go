package main

import (
	"bufio"
	"context"
	"crypto/rand"
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

func defaultWorkflowRegistry(workflowPath string) map[string]any {
	registry := map[string]any{}
	subgraphPath := filepath.Join(filepath.Dir(workflowPath), "hr-warning-email-subgraph.yaml")
	if _, err := os.Stat(subgraphPath); err == nil {
		registry["hr_warning_email_subgraph"] = subgraphPath
	}
	return registry
}

func initialMessages() []message {
	return []message{
		{
			Role:    "system",
			Content: "You are a friendly email drafting assistant for new users. First, explain capabilities clearly when asked what you can do. Then gather missing scenario details and draft concise professional emails. If context is incomplete, ask one specific follow-up question.",
		},
	}
}

func renderAssistantReply(terminalOutput any) string {
	if terminalOutput == nil {
		return ""
	}
	if text, ok := terminalOutput.(string); ok {
		return text
	}
	encoded, err := json.MarshalIndent(terminalOutput, "", "  ")
	if err != nil {
		return ""
	}
	return string(encoded)
}

func randomConversationID() string {
	b := make([]byte, 16)
	if _, err := rand.Read(b); err != nil {
		return fmt.Sprintf("%d", time.Now().UTC().UnixNano())
	}
	b[6] = (b[6] & 0x0f) | 0x40
	b[8] = (b[8] & 0x3f) | 0x80
	return fmt.Sprintf(
		"%08x-%04x-%04x-%04x-%012x",
		b[0:4],
		b[4:6],
		b[6:8],
		b[8:10],
		b[10:16],
	)
}

func main() {
	workflowFlag := flag.String("workflow", "workflow_email/email-chat-draft-or-clarify.yaml", "Path to workflow YAML file")
	maxTurnsFlag := flag.Int("max-turns", 8, "Maximum chat turns before exiting")
	traceDirFlag := flag.String("trace-dir", "examples/workflow_email/traces", "Directory to persist per-turn workflow traces as JSONL")
	conversationIDFlag := flag.String("conversation-id", "", "Conversation UUID used for trace correlation (auto-generated if omitted)")
	flag.Parse()

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
	workflowRegistry := defaultWorkflowRegistry(workflowPath)

	client, err := simpleagents.NewClient(apiKey, apiBase)
	if err != nil {
		panic(err)
	}
	defer client.Close()

	traceDir := filepath.Clean(*traceDirFlag)
	if err := os.MkdirAll(traceDir, 0o755); err != nil {
		panic(err)
	}
	sessionID := time.Now().UTC().Format("20060102T150405Z")
	conversationID := *conversationIDFlag
	if conversationID == "" {
		conversationID = randomConversationID()
	}
	traceFile := filepath.Join(traceDir, fmt.Sprintf("chat-session-%s-%s.jsonl", sessionID, conversationID))

	messages := initialMessages()
	interviewClosed := false

	fmt.Println("Chat Email Assistant")
	fmt.Println("Type your request. Type 'exit' to quit.")
	fmt.Printf("Conversation ID: %s\n", conversationID)
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

		ctx, cancel := context.WithTimeout(context.Background(), 120*time.Second)

		workflowInput := map[string]any{
			"email_text":        userInput,
			"messages":          workflowInputMessages,
			"workflow_registry": workflowRegistry,
		}
		inputJSON, err := json.Marshal(workflowInput)
		if err != nil {
			cancel()
			panic(err)
		}

		outJSON, runErr := client.Run(ctx, workflowPath, inputJSON)
		cancel()
		if runErr != nil {
			panic(runErr)
		}

		var out map[string]any
		if err := json.Unmarshal(outJSON, &out); err != nil {
			panic(err)
		}

		traceRecord := map[string]any{
			"timestamp":        time.Now().UTC().Format(time.RFC3339Nano),
			"turn":             turn,
			"conversation_id":  conversationID,
			"workflow_path":    workflowPath,
			"workflow_id":      out["workflow_id"],
			"terminal_node":    out["terminal_node"],
			"trace":            out["trace"],
			"step_timings":     out["step_timings"],
			"total_elapsed_ms": out["total_elapsed_ms"],
			"user_input":       userInput,
			"assistant_output": out["terminal_output"],
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

		reply := renderAssistantReply(out["terminal_output"])
		fmt.Printf("\nAssistant: %s\n\n", reply)
		messages = append(messages, message{Role: "assistant", Content: reply})

		terminalOutputMap, isMap := out["terminal_output"].(map[string]any)
		decision := ""
		if isMap {
			if rawDecision, ok := terminalOutputMap["decision"].(string); ok {
				decision = rawDecision
			}
		}

		terminalNode, _ := out["terminal_node"].(string)
		if terminalNode == "terminate_candidate" || terminalNode == "already_terminated" || decision == "terminated" {
			interviewClosed = true
			fmt.Println("Interview closed for this session. Start a new run for a new candidate.")
			fmt.Println()
		}

		if terminalNode == "generate_email_draft" {
			fmt.Println("Draft ready. Continue chatting to refine, or type 'exit'.")
			fmt.Println()
		}
	}

	fmt.Println("Reached max turns. Restart to continue.")
}
