use super::output::{RunMetadata, StepTiming, TokenTotals};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use simple_agent_type::message::Message;
use std::collections::BTreeMap;

/// Checkpoint captured on workflow failure, enabling retry from the failed node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowCheckpoint {
    /// Path to the workflow YAML file.
    pub workflow_path: String,
    /// ID of the node that failed.
    pub failed_node_id: String,
    /// Ordered list of nodes that completed before failure.
    pub completed_trace: Vec<String>,
    /// Outputs from completed nodes.
    pub completed_outputs: BTreeMap<String, Value>,
    /// Global variable state at time of failure.
    pub globals: BTreeMap<String, Value>,
    /// The original input messages.
    pub original_messages: Vec<Message>,
    /// Step timings collected before failure.
    pub step_timings: Vec<StepTiming>,
    /// Token usage collected before failure.
    pub token_totals: TokenTotals,
}

/// Partial output returned when a workflow fails after some nodes succeed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartialWorkflowOutput {
    /// Workflow identifier.
    pub workflow_id: String,
    /// Nodes that completed before failure.
    pub completed_trace: Vec<String>,
    /// Outputs from completed nodes.
    pub completed_outputs: BTreeMap<String, Value>,
    /// The node that failed.
    pub failed_node_id: String,
    /// Error description.
    pub error: String,
    /// Checkpoint for retrying.
    pub checkpoint: WorkflowCheckpoint,
    /// Optional performance metadata.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nerdstats: Option<RunMetadata>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_checkpoint_serialization_roundtrip() {
        let cp = WorkflowCheckpoint {
            workflow_path: "test.yaml".into(),
            failed_node_id: "node_b".into(),
            completed_trace: vec!["node_a".into()],
            completed_outputs: BTreeMap::from([(
                "node_a".into(),
                serde_json::json!({"result": 1}),
            )]),
            globals: BTreeMap::new(),
            original_messages: vec![Message::user("hello")],
            step_timings: vec![],
            token_totals: TokenTotals::default(),
        };
        let json = serde_json::to_string(&cp).unwrap();
        let parsed: WorkflowCheckpoint = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.failed_node_id, "node_b");
        assert_eq!(parsed.completed_trace, vec!["node_a"]);
        assert_eq!(parsed.original_messages.len(), 1);
    }
}
