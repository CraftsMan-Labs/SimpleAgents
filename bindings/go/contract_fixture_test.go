package simpleagents

import (
	"encoding/json"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"testing"
)

type contractFixture struct {
	Go struct {
		RequiredAPISymbols []string `json:"required_api_symbols"`
	} `json:"go"`
	SharedCases map[string]any `json:"shared_cases"`
}

type workflowFixture struct {
	WorkflowDSL struct {
		Entry string                 `json:"entry"`
		Nodes map[string]interface{} `json:"nodes"`
	} `json:"workflow_dsl"`
	CanonicalIR struct {
		Nodes []map[string]interface{} `json:"nodes"`
	} `json:"canonical_ir"`
	RequiredNodeTypes []string `json:"required_node_types"`
	WireExpectations  []struct {
		NodeID   string   `json:"node_id"`
		Outgoing []string `json:"outgoing"`
	} `json:"wire_expectations"`
	MergeSourceExpectations []struct {
		NodeID  string   `json:"node_id"`
		Sources []string `json:"sources"`
	} `json:"merge_source_expectations"`
}

func outgoingEdgesForNode(node map[string]interface{}) []string {
	nodeType, _ := node["type"].(string)
	switch nodeType {
	case "start":
		return []string{node["next"].(string)}
	case "llm", "tool", "subgraph":
		if next, ok := node["next"].(string); ok && next != "" {
			return []string{next}
		}
		return []string{}
	case "condition":
		return []string{node["on_true"].(string), node["on_false"].(string)}
	case "loop":
		return []string{node["body"].(string), node["next"].(string)}
	case "parallel":
		branchesAny, _ := node["branches"].([]interface{})
		outgoing := make([]string, 0, len(branchesAny)+1)
		for _, branch := range branchesAny {
			outgoing = append(outgoing, branch.(string))
		}
		outgoing = append(outgoing, node["next"].(string))
		return outgoing
	case "batch", "filter", "merge", "map", "reduce":
		return []string{node["next"].(string)}
	case "end":
		return []string{}
	default:
		return []string{}
	}
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

func TestWorkflowDSLFixturePreservesCanonicalIRWires(t *testing.T) {
	root := filepath.Join("..", "..")
	fixturePath := filepath.Join(root, "parity-fixtures", "workflow_dsl_ir_golden.json")
	data, err := os.ReadFile(fixturePath)
	if err != nil {
		t.Fatalf("read workflow fixture: %v", err)
	}

	var fixture workflowFixture
	if err := json.Unmarshal(data, &fixture); err != nil {
		t.Fatalf("parse workflow fixture: %v", err)
	}

	canonicalByID := map[string]map[string]interface{}{}
	canonicalTypes := map[string]struct{}{}
	for _, node := range fixture.CanonicalIR.Nodes {
		id, _ := node["id"].(string)
		canonicalByID[id] = node
		if nodeType, ok := node["type"].(string); ok {
			canonicalTypes[nodeType] = struct{}{}
		}
	}

	if _, ok := canonicalByID[fixture.WorkflowDSL.Entry]; !ok {
		t.Fatalf("dsl entry node %q should exist in canonical IR", fixture.WorkflowDSL.Entry)
	}

	if len(fixture.WorkflowDSL.Nodes) != len(canonicalByID) {
		t.Fatalf("dsl and canonical node counts should match: dsl=%d canonical=%d", len(fixture.WorkflowDSL.Nodes), len(canonicalByID))
	}
	for dslID := range fixture.WorkflowDSL.Nodes {
		if _, ok := canonicalByID[dslID]; !ok {
			t.Fatalf("dsl node %q missing from canonical IR", dslID)
		}
	}

	requiredTypes := map[string]struct{}{}
	for _, nodeType := range fixture.RequiredNodeTypes {
		requiredTypes[nodeType] = struct{}{}
	}
	if len(requiredTypes) != len(canonicalTypes) {
		t.Fatalf("required node types and canonical node types should match")
	}
	for nodeType := range requiredTypes {
		if _, ok := canonicalTypes[nodeType]; !ok {
			t.Fatalf("canonical fixture missing node type %q", nodeType)
		}
	}

	for _, expectedWire := range fixture.WireExpectations {
		node, ok := canonicalByID[expectedWire.NodeID]
		if !ok {
			t.Fatalf("missing node %q for wire assertion", expectedWire.NodeID)
		}

		actual := outgoingEdgesForNode(node)
		expected := append([]string{}, expectedWire.Outgoing...)
		sort.Strings(actual)
		sort.Strings(expected)
		if strings.Join(actual, ",") != strings.Join(expected, ",") {
			t.Fatalf("node %q outgoing wires mismatch: got %v want %v", expectedWire.NodeID, actual, expected)
		}
	}

	for _, expectedMerge := range fixture.MergeSourceExpectations {
		node, ok := canonicalByID[expectedMerge.NodeID]
		if !ok {
			t.Fatalf("missing merge node %q", expectedMerge.NodeID)
		}
		nodeType, _ := node["type"].(string)
		if nodeType != "merge" {
			t.Fatalf("node %q should be merge, got %q", expectedMerge.NodeID, nodeType)
		}

		sourcesAny, _ := node["sources"].([]interface{})
		actual := make([]string, 0, len(sourcesAny))
		for _, source := range sourcesAny {
			actual = append(actual, source.(string))
		}
		expected := append([]string{}, expectedMerge.Sources...)
		sort.Strings(actual)
		sort.Strings(expected)
		if strings.Join(actual, ",") != strings.Join(expected, ",") {
			t.Fatalf("merge sources mismatch for %q: got %v want %v", expectedMerge.NodeID, actual, expected)
		}
	}
}
