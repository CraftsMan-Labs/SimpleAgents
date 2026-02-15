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
    #[serde(alias = "switch", alias = "if")]
    Condition {
        expression: String,
        on_true: String,
        on_false: String,
    },
    /// Debounces repeated events for a key across nearby steps.
    Debounce {
        key_path: String,
        window_steps: u32,
        next: String,
        on_suppressed: Option<String>,
    },
    /// Throttles repeated events for a key to one pass per window.
    Throttle {
        key_path: String,
        window_steps: u32,
        next: String,
        on_throttled: Option<String>,
    },
    /// Explicit retry node with compensation fallback.
    RetryCompensate {
        tool: String,
        #[serde(default)]
        input: Value,
        max_retries: usize,
        compensate_tool: String,
        #[serde(default)]
        compensate_input: Value,
        next: String,
        on_compensated: Option<String>,
    },
    /// Human decision gate that routes on approve/reject.
    HumanInTheLoop {
        decision_path: String,
        response_path: Option<String>,
        on_approve: String,
        on_reject: String,
    },
    /// Writes a value into runtime cache.
    CacheWrite {
        key_path: String,
        value_path: String,
        next: String,
    },
    /// Reads a value from runtime cache.
    CacheRead {
        key_path: String,
        next: String,
        on_miss: Option<String>,
    },
    /// Event gate for schedule/cron/webhook style entry checks.
    EventTrigger {
        event: String,
        event_path: String,
        next: String,
        on_mismatch: Option<String>,
    },
    /// Router/selector node that picks first matching route.
    Router {
        routes: Vec<RouterRoute>,
        default: String,
    },
    /// Deterministic transform node using JSON literal or path expression.
    Transform { expression: String, next: String },
    /// Loop node with explicit body and exit transitions.
    Loop {
        condition: String,
        body: String,
        next: String,
        max_iterations: Option<u32>,
    },
    /// Executes a registered subgraph in an isolated child scope.
    Subgraph { graph: String, next: Option<String> },
    /// Captures an array payload from scope for downstream processing.
    Batch { items_path: String, next: String },
    /// Filters an array payload deterministically with a boolean expression.
    Filter {
        items_path: String,
        expression: String,
        next: String,
    },
    /// Concurrently executes branch nodes and continues to next.
    Parallel {
        branches: Vec<String>,
        next: String,
        max_in_flight: Option<usize>,
    },
    /// Merges outputs from source nodes according to policy.
    Merge {
        sources: Vec<String>,
        policy: MergePolicy,
        quorum: Option<usize>,
        next: String,
    },
    /// Applies a tool to each item in an array from scoped input.
    Map {
        tool: String,
        items_path: String,
        next: String,
        max_in_flight: Option<usize>,
    },
    /// Reduces an array output into a single value.
    Reduce {
        source: String,
        operation: ReduceOperation,
        next: String,
    },
    /// Terminal node.
    End,
}

/// Merge behavior policy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MergePolicy {
    First,
    All,
    Quorum,
}

/// Reduce operation selector.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReduceOperation {
    Count,
    Sum,
}

/// One router route definition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RouterRoute {
    /// Boolean expression evaluated against scoped input.
    pub when: String,
    /// Next node id when the route matches.
    pub next: String,
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
            Self::Debounce {
                key_path,
                window_steps,
                next,
                on_suppressed,
            } => Self::Debounce {
                key_path: key_path.trim().to_string(),
                window_steps,
                next: next.trim().to_string(),
                on_suppressed: on_suppressed.map(|edge| edge.trim().to_string()),
            },
            Self::Throttle {
                key_path,
                window_steps,
                next,
                on_throttled,
            } => Self::Throttle {
                key_path: key_path.trim().to_string(),
                window_steps,
                next: next.trim().to_string(),
                on_throttled: on_throttled.map(|edge| edge.trim().to_string()),
            },
            Self::RetryCompensate {
                tool,
                input,
                max_retries,
                compensate_tool,
                compensate_input,
                next,
                on_compensated,
            } => Self::RetryCompensate {
                tool: tool.trim().to_string(),
                input,
                max_retries,
                compensate_tool: compensate_tool.trim().to_string(),
                compensate_input,
                next: next.trim().to_string(),
                on_compensated: on_compensated.map(|edge| edge.trim().to_string()),
            },
            Self::HumanInTheLoop {
                decision_path,
                response_path,
                on_approve,
                on_reject,
            } => Self::HumanInTheLoop {
                decision_path: decision_path.trim().to_string(),
                response_path: response_path.map(|path| path.trim().to_string()),
                on_approve: on_approve.trim().to_string(),
                on_reject: on_reject.trim().to_string(),
            },
            Self::CacheWrite {
                key_path,
                value_path,
                next,
            } => Self::CacheWrite {
                key_path: key_path.trim().to_string(),
                value_path: value_path.trim().to_string(),
                next: next.trim().to_string(),
            },
            Self::CacheRead {
                key_path,
                next,
                on_miss,
            } => Self::CacheRead {
                key_path: key_path.trim().to_string(),
                next: next.trim().to_string(),
                on_miss: on_miss.map(|edge| edge.trim().to_string()),
            },
            Self::EventTrigger {
                event,
                event_path,
                next,
                on_mismatch,
            } => Self::EventTrigger {
                event: event.trim().to_string(),
                event_path: event_path.trim().to_string(),
                next: next.trim().to_string(),
                on_mismatch: on_mismatch.map(|edge| edge.trim().to_string()),
            },
            Self::Router { routes, default } => Self::Router {
                routes: routes
                    .into_iter()
                    .map(|route| RouterRoute {
                        when: route.when.trim().to_string(),
                        next: route.next.trim().to_string(),
                    })
                    .collect(),
                default: default.trim().to_string(),
            },
            Self::Transform { expression, next } => Self::Transform {
                expression: expression.trim().to_string(),
                next: next.trim().to_string(),
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
            Self::Subgraph { graph, next } => Self::Subgraph {
                graph: graph.trim().to_string(),
                next: next.map(|edge| edge.trim().to_string()),
            },
            Self::Batch { items_path, next } => Self::Batch {
                items_path: items_path.trim().to_string(),
                next: next.trim().to_string(),
            },
            Self::Filter {
                items_path,
                expression,
                next,
            } => Self::Filter {
                items_path: items_path.trim().to_string(),
                expression: expression.trim().to_string(),
                next: next.trim().to_string(),
            },
            Self::Parallel {
                branches,
                next,
                max_in_flight,
            } => Self::Parallel {
                branches: branches
                    .into_iter()
                    .map(|edge| edge.trim().to_string())
                    .collect(),
                next: next.trim().to_string(),
                max_in_flight,
            },
            Self::Merge {
                sources,
                policy,
                quorum,
                next,
            } => Self::Merge {
                sources: sources
                    .into_iter()
                    .map(|id| id.trim().to_string())
                    .collect(),
                policy,
                quorum,
                next: next.trim().to_string(),
            },
            Self::Map {
                tool,
                items_path,
                next,
                max_in_flight,
            } => Self::Map {
                tool: tool.trim().to_string(),
                items_path: items_path.trim().to_string(),
                next: next.trim().to_string(),
                max_in_flight,
            },
            Self::Reduce {
                source,
                operation,
                next,
            } => Self::Reduce {
                source: source.trim().to_string(),
                operation,
                next: next.trim().to_string(),
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
            Self::Debounce {
                next,
                on_suppressed,
                ..
            } => {
                let mut edges = vec![next.as_str()];
                if let Some(edge) = on_suppressed.as_deref() {
                    edges.push(edge);
                }
                edges
            }
            Self::Throttle {
                next, on_throttled, ..
            } => {
                let mut edges = vec![next.as_str()];
                if let Some(edge) = on_throttled.as_deref() {
                    edges.push(edge);
                }
                edges
            }
            Self::RetryCompensate {
                next,
                on_compensated,
                ..
            }
            | Self::CacheRead {
                next,
                on_miss: on_compensated,
                ..
            }
            | Self::EventTrigger {
                next,
                on_mismatch: on_compensated,
                ..
            } => {
                let mut edges = vec![next.as_str()];
                if let Some(edge) = on_compensated.as_deref() {
                    edges.push(edge);
                }
                edges
            }
            Self::HumanInTheLoop {
                on_approve,
                on_reject,
                ..
            } => vec![on_approve.as_str(), on_reject.as_str()],
            Self::CacheWrite { next, .. } | Self::Transform { next, .. } => vec![next.as_str()],
            Self::Router { routes, default } => {
                let mut edges = routes
                    .iter()
                    .map(|route| route.next.as_str())
                    .collect::<Vec<_>>();
                edges.push(default.as_str());
                edges
            }
            Self::Loop { body, next, .. } => vec![body.as_str(), next.as_str()],
            Self::Subgraph { next, .. } => next.as_deref().map_or_else(Vec::new, |edge| vec![edge]),
            Self::Batch { next, .. } | Self::Filter { next, .. } => vec![next.as_str()],
            Self::Parallel { branches, next, .. } => {
                let mut edges = branches.iter().map(String::as_str).collect::<Vec<_>>();
                edges.push(next.as_str());
                edges
            }
            Self::Merge { next, .. } | Self::Map { next, .. } | Self::Reduce { next, .. } => {
                vec![next.as_str()]
            }
            Self::End => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::NodeKind;

    #[test]
    fn condition_deserializes_switch_alias() {
        let kind: NodeKind = serde_json::from_value(json!({
            "type": "switch",
            "expression": "input.ok == true",
            "on_true": "end_true",
            "on_false": "end_false"
        }))
        .expect("switch alias should deserialize");

        assert!(matches!(kind, NodeKind::Condition { .. }));
    }

    #[test]
    fn condition_deserializes_if_alias() {
        let kind: NodeKind = serde_json::from_value(json!({
            "type": "if",
            "expression": "input.ok == true",
            "on_true": "end_true",
            "on_false": "end_false"
        }))
        .expect("if alias should deserialize");

        assert!(matches!(kind, NodeKind::Condition { .. }));
    }
}
