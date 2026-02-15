package simpleagents

import (
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

type contractFixture struct {
	Go struct {
		RequiredAPISymbols []string `json:"required_api_symbols"`
	} `json:"go"`
	SharedCases map[string]any `json:"shared_cases"`
}

func TestGoBindingsFollowSharedContractFixture(t *testing.T) {
	root := filepath.Join("..", "..")
	fixturePath := filepath.Join(root, "parity-fixtures", "binding_contract.json")
	data, err := os.ReadFile(fixturePath)
	if err != nil {
		t.Fatalf("read fixture: %v", err)
	}

	var fixture contractFixture
	if err := json.Unmarshal(data, &fixture); err != nil {
		t.Fatalf("parse fixture: %v", err)
	}

	if _, ok := fixture.SharedCases["request"]; !ok {
		t.Fatal("shared_cases.request must exist")
	}
	if _, ok := fixture.SharedCases["response"]; !ok {
		t.Fatal("shared_cases.response must exist")
	}
	if _, ok := fixture.SharedCases["healing"]; !ok {
		t.Fatal("shared_cases.healing must exist")
	}
	if _, ok := fixture.SharedCases["streaming"]; !ok {
		t.Fatal("shared_cases.streaming must exist")
	}
	if _, ok := fixture.SharedCases["tool_call"]; !ok {
		t.Fatal("shared_cases.tool_call must exist")
	}

	apiSource, err := os.ReadFile(filepath.Join("simpleagents.go"))
	if err != nil {
		t.Fatalf("read go bindings source: %v", err)
	}
	source := string(apiSource)
	for _, symbol := range fixture.Go.RequiredAPISymbols {
		if !strings.Contains(source, symbol) {
			t.Fatalf("simpleagents.go should include symbol %q", symbol)
		}
	}
}
