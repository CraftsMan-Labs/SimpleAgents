use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use super::events::WorkflowEvent;

/// The final output of a successful workflow run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowRunOutput {
    /// Identifier for the workflow.
    pub workflow_id: String,
    /// The first node that executed.
    pub entry_node: String,
    /// Ordered list of node IDs that executed.
    pub trace: Vec<String>,
    /// Outputs collected from each node, keyed by node ID.
    pub outputs: BTreeMap<String, Value>,
    /// The last node in the trace.
    pub terminal_node: String,
    /// The output value of the terminal node.
    pub terminal_output: Option<Value>,
    /// Optional performance metadata.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<RunMetadata>,
    /// Collected events if event recording was enabled.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub events: Option<Vec<WorkflowEvent>>,
}

/// Performance and observability metadata for a workflow run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunMetadata {
    /// Total wall-clock time for the workflow.
    pub total_elapsed_ms: u128,
    /// Time to first LLM token across all nodes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ttft_ms: Option<u128>,
    /// Sum of input tokens across all LLM calls.
    pub total_input_tokens: u64,
    /// Sum of output tokens across all LLM calls.
    pub total_output_tokens: u64,
    /// Sum of all tokens across all LLM calls.
    pub total_tokens: u64,
    /// Sum of reasoning tokens (if applicable).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_reasoning_tokens: Option<u64>,
    /// Output tokens per second across the workflow.
    pub tokens_per_second: f64,
    /// Per-step timing details.
    pub step_details: Vec<StepTiming>,
    /// Trace ID if telemetry was enabled.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_id: Option<String>,
}

/// Timing and token details for a single workflow step.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepTiming {
    /// Node ID.
    pub node_id: String,
    /// Node type (e.g. "llm_call", "switch").
    pub node_type: String,
    /// Model used (for LLM nodes).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Wall-clock time for this step.
    pub elapsed_ms: u128,
    /// Input tokens for this step.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_tokens: Option<u64>,
    /// Output tokens for this step.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_tokens: Option<u64>,
    /// Total tokens for this step.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_tokens: Option<u64>,
    /// Reasoning tokens for this step.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_tokens: Option<u64>,
    /// Time to first token for this step.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ttft_ms: Option<u128>,
}

/// Running token counter.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TokenTotals {
    /// Total input tokens so far.
    pub input_tokens: u64,
    /// Total output tokens so far.
    pub output_tokens: u64,
    /// Total tokens so far.
    pub total_tokens: u64,
    /// Total reasoning tokens so far.
    pub reasoning_tokens: Option<u64>,
}

impl TokenTotals {
    /// Add token counts from a step.
    pub fn add(&mut self, input: u64, output: u64, total: u64, reasoning: Option<u64>) {
        self.input_tokens += input;
        self.output_tokens += output;
        self.total_tokens += total;
        if let Some(r) = reasoning {
            *self.reasoning_tokens.get_or_insert(0) += r;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_totals_add() {
        let mut t = TokenTotals::default();
        t.add(10, 20, 30, Some(5));
        t.add(5, 10, 15, None);
        assert_eq!(t.input_tokens, 15);
        assert_eq!(t.output_tokens, 30);
        assert_eq!(t.total_tokens, 45);
        assert_eq!(t.reasoning_tokens, Some(5));
    }

    #[test]
    fn test_output_serialization() {
        let output = WorkflowRunOutput {
            workflow_id: "test".into(),
            entry_node: "start".into(),
            trace: vec!["start".into()],
            outputs: BTreeMap::new(),
            terminal_node: "start".into(),
            terminal_output: Some(serde_json::json!("done")),
            metadata: None,
            events: None,
        };
        let json = serde_json::to_string(&output).unwrap();
        assert!(json.contains("test"));
    }
}
