use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Canonical workflow version identifier for the current minimal IR.
pub const WORKFLOW_IR_V0: &str = "v0";

/// A workflow definition in canonical IR form.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkflowDefinition {
    /// IR version.
    #[serde(default = "default_version")]
    pub version: String,
    /// Human-readable workflow name.
    pub name: String,
    /// Node list that defines the workflow graph.
    pub nodes: Vec<Node>,
}

fn default_version() -> String {
    WORKFLOW_IR_V0.to_string()
}

impl WorkflowDefinition {
    /// Returns a deterministic copy by sorting nodes by id and normalizing strings.
    pub fn normalized(&self) -> Self {
        let mut normalized = self.clone();
        normalized.version = normalized.version.trim().to_string();
        normalized.name = normalized.name.trim().to_string();

        normalized.nodes = normalized
            .nodes
            .iter()
            .cloned()
            .map(|node| node.normalized())
            .collect();
        normalized.nodes.sort_by(|a, b| a.id.cmp(&b.id));
        normalized
    }
}

/// A named workflow node.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Node {
    /// Unique node id within a workflow.
    pub id: String,
    /// Node behavior and edge declarations.
    #[serde(flatten)]
    pub kind: NodeKind,
}

impl Node {
    /// Returns a deterministic copy with normalized string fields.
    pub fn normalized(mut self) -> Self {
        self.id = self.id.trim().to_string();
        self.kind = self.kind.normalized();
        self
    }

    /// Returns all referenced outgoing edge ids.
    pub fn outgoing_edges(&self) -> Vec<&str> {
        self.kind.outgoing_edges()
    }
}

/// Canonical node taxonomy for v0.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum NodeKind {
    /// Entry node. Must have exactly one in the workflow.
    Start { next: String },
    /// LLM call node.
    Llm {
        model: String,
        prompt: String,
        next: Option<String>,
    },
    /// Tool invocation node.
    Tool {
        tool: String,
        #[serde(default)]
        input: Value,
        next: Option<String>,
    },
    /// Conditional branching node.
    Condition {
        expression: String,
        on_true: String,
        on_false: String,
    },
    /// Loop node with explicit body and exit transitions.
    Loop {
        condition: String,
        body: String,
        next: String,
        max_iterations: Option<u32>,
    },
    /// Terminal node.
    End,
}

impl NodeKind {
    fn normalized(self) -> Self {
        match self {
            Self::Start { next } => Self::Start {
                next: next.trim().to_string(),
            },
            Self::Llm {
                model,
                prompt,
                next,
            } => Self::Llm {
                model: model.trim().to_string(),
                prompt: prompt.trim().to_string(),
                next: next.map(|edge| edge.trim().to_string()),
            },
            Self::Tool { tool, input, next } => Self::Tool {
                tool: tool.trim().to_string(),
                input,
                next: next.map(|edge| edge.trim().to_string()),
            },
            Self::Condition {
                expression,
                on_true,
                on_false,
            } => Self::Condition {
                expression: expression.trim().to_string(),
                on_true: on_true.trim().to_string(),
                on_false: on_false.trim().to_string(),
            },
            Self::Loop {
                condition,
                body,
                next,
                max_iterations,
            } => Self::Loop {
                condition: condition.trim().to_string(),
                body: body.trim().to_string(),
                next: next.trim().to_string(),
                max_iterations,
            },
            Self::End => Self::End,
        }
    }

    fn outgoing_edges(&self) -> Vec<&str> {
        match self {
            Self::Start { next } => vec![next.as_str()],
            Self::Llm { next, .. } | Self::Tool { next, .. } => {
                next.as_deref().map_or_else(Vec::new, |edge| vec![edge])
            }
            Self::Condition {
                on_true, on_false, ..
            } => vec![on_true.as_str(), on_false.as_str()],
            Self::Loop { body, next, .. } => vec![body.as_str(), next.as_str()],
            Self::End => Vec::new(),
        }
    }
}
