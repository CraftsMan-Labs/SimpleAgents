use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};

use thiserror::Error;

use crate::ir::{NodeKind, WorkflowDefinition, WORKFLOW_IR_V0};

/// Validates and returns a deterministic normalized workflow definition.
pub fn validate_and_normalize(
    input: &WorkflowDefinition,
) -> Result<WorkflowDefinition, ValidationErrors> {
    let normalized = input.normalized();
    let diagnostics = validate(&normalized);

    if diagnostics.is_empty() {
        Ok(normalized)
    } else {
        Err(ValidationErrors { diagnostics })
    }
}

/// Validation diagnostic severity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    /// A hard validation failure.
    Error,
}

/// Stable diagnostic codes for workflow validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticCode {
    /// Unsupported IR version.
    UnsupportedVersion,
    /// Empty workflow name.
    EmptyWorkflowName,
    /// Workflow contains no nodes.
    EmptyWorkflow,
    /// Duplicate node id.
    DuplicateNodeId,
    /// Node id is empty.
    EmptyNodeId,
    /// Node edge references unknown node id.
    UnknownTarget,
    /// Missing start node.
    MissingStart,
    /// More than one start node found.
    MultipleStart,
    /// No terminal node found.
    MissingEnd,
    /// Node is unreachable from start.
    UnreachableNode,
    /// Start node cannot reach an end node.
    NoPathToEnd,
    /// Node has an empty required field.
    EmptyField,
}

/// A workflow validation diagnostic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    /// Severity.
    pub severity: Severity,
    /// Stable code.
    pub code: DiagnosticCode,
    /// Human-readable message.
    pub message: String,
    /// Optional node id where the issue occurred.
    pub node_id: Option<String>,
}

impl Diagnostic {
    fn error(code: DiagnosticCode, message: impl Into<String>, node_id: Option<String>) -> Self {
        Self {
            severity: Severity::Error,
            code,
            message: message.into(),
            node_id,
        }
    }
}

/// Aggregated validation errors.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("workflow validation failed")]
pub struct ValidationErrors {
    /// Collected diagnostics.
    pub diagnostics: Vec<Diagnostic>,
}

/// Canonical validation error alias.
pub type ValidationError = ValidationErrors;

fn validate(workflow: &WorkflowDefinition) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    if workflow.version != WORKFLOW_IR_V0 {
        diagnostics.push(Diagnostic::error(
            DiagnosticCode::UnsupportedVersion,
            format!(
                "unsupported workflow IR version '{}'; expected '{}'",
                workflow.version, WORKFLOW_IR_V0
            ),
            None,
        ));
    }

    if workflow.name.is_empty() {
        diagnostics.push(Diagnostic::error(
            DiagnosticCode::EmptyWorkflowName,
            "workflow name must not be empty",
            None,
        ));
    }

    if workflow.nodes.is_empty() {
        diagnostics.push(Diagnostic::error(
            DiagnosticCode::EmptyWorkflow,
            "workflow must contain at least one node",
            None,
        ));
        return diagnostics;
    }

    let mut node_index = HashMap::with_capacity(workflow.nodes.len());
    let mut duplicates = BTreeSet::new();
    let mut start_ids = Vec::new();
    let mut end_count = 0usize;

    for node in &workflow.nodes {
        if node.id.is_empty() {
            diagnostics.push(Diagnostic::error(
                DiagnosticCode::EmptyNodeId,
                "node id must not be empty",
                Some(node.id.clone()),
            ));
        }

        if let Some(previous_id) = node_index.insert(node.id.as_str(), node) {
            duplicates.insert(previous_id.id.clone());
            duplicates.insert(node.id.clone());
        }

        match &node.kind {
            NodeKind::Start { next } => {
                start_ids.push(node.id.clone());
                if next.is_empty() {
                    diagnostics.push(Diagnostic::error(
                        DiagnosticCode::EmptyField,
                        "start.next must not be empty",
                        Some(node.id.clone()),
                    ));
                }
            }
            NodeKind::Llm {
                model,
                prompt,
                next: _,
            } => {
                if model.is_empty() {
                    diagnostics.push(Diagnostic::error(
                        DiagnosticCode::EmptyField,
                        "llm.model must not be empty",
                        Some(node.id.clone()),
                    ));
                }
                if prompt.is_empty() {
                    diagnostics.push(Diagnostic::error(
                        DiagnosticCode::EmptyField,
                        "llm.prompt must not be empty",
                        Some(node.id.clone()),
                    ));
                }
            }
            NodeKind::Tool { tool, .. } => {
                if tool.is_empty() {
                    diagnostics.push(Diagnostic::error(
                        DiagnosticCode::EmptyField,
                        "tool.tool must not be empty",
                        Some(node.id.clone()),
                    ));
                }
            }
            NodeKind::Condition {
                expression,
                on_true,
                on_false,
            } => {
                if expression.is_empty() {
                    diagnostics.push(Diagnostic::error(
                        DiagnosticCode::EmptyField,
                        "condition.expression must not be empty",
                        Some(node.id.clone()),
                    ));
                }
                if on_true.is_empty() {
                    diagnostics.push(Diagnostic::error(
                        DiagnosticCode::EmptyField,
                        "condition.on_true must not be empty",
                        Some(node.id.clone()),
                    ));
                }
                if on_false.is_empty() {
                    diagnostics.push(Diagnostic::error(
                        DiagnosticCode::EmptyField,
                        "condition.on_false must not be empty",
                        Some(node.id.clone()),
                    ));
                }
            }
            NodeKind::Loop {
                condition,
                body,
                next,
                max_iterations,
            } => {
                if condition.is_empty() {
                    diagnostics.push(Diagnostic::error(
                        DiagnosticCode::EmptyField,
                        "loop.condition must not be empty",
                        Some(node.id.clone()),
                    ));
                }
                if body.is_empty() {
                    diagnostics.push(Diagnostic::error(
                        DiagnosticCode::EmptyField,
                        "loop.body must not be empty",
                        Some(node.id.clone()),
                    ));
                }
                if next.is_empty() {
                    diagnostics.push(Diagnostic::error(
                        DiagnosticCode::EmptyField,
                        "loop.next must not be empty",
                        Some(node.id.clone()),
                    ));
                }
                if max_iterations.is_some_and(|limit| limit == 0) {
                    diagnostics.push(Diagnostic::error(
                        DiagnosticCode::EmptyField,
                        "loop.max_iterations must be greater than zero when provided",
                        Some(node.id.clone()),
                    ));
                }
            }
            NodeKind::End => {
                end_count += 1;
            }
        }
    }

    for duplicate_id in duplicates {
        diagnostics.push(Diagnostic::error(
            DiagnosticCode::DuplicateNodeId,
            format!("duplicate node id '{}'", duplicate_id),
            Some(duplicate_id),
        ));
    }

    if start_ids.is_empty() {
        diagnostics.push(Diagnostic::error(
            DiagnosticCode::MissingStart,
            "workflow must contain exactly one start node",
            None,
        ));
    } else if start_ids.len() > 1 {
        diagnostics.push(Diagnostic::error(
            DiagnosticCode::MultipleStart,
            format!(
                "workflow must contain exactly one start node, found {}",
                start_ids.len()
            ),
            None,
        ));
    }

    if end_count == 0 {
        diagnostics.push(Diagnostic::error(
            DiagnosticCode::MissingEnd,
            "workflow must contain at least one end node",
            None,
        ));
    }

    for node in &workflow.nodes {
        for edge in node.outgoing_edges() {
            if !node_index.contains_key(edge) {
                diagnostics.push(Diagnostic::error(
                    DiagnosticCode::UnknownTarget,
                    format!("node '{}' references unknown target '{}'", node.id, edge),
                    Some(node.id.clone()),
                ));
            }
        }
    }

    if start_ids.len() == 1 {
        let start_id = start_ids[0].as_str();
        let reachable = reachable_nodes(start_id, &node_index);

        for node in &workflow.nodes {
            if !reachable.contains(node.id.as_str()) {
                diagnostics.push(Diagnostic::error(
                    DiagnosticCode::UnreachableNode,
                    format!(
                        "node '{}' is unreachable from start node '{}'",
                        node.id, start_id
                    ),
                    Some(node.id.clone()),
                ));
            }
        }

        let has_path_to_end = reachable.iter().any(|id| {
            node_index
                .get(*id)
                .is_some_and(|node| matches!(node.kind, NodeKind::End))
        });

        if !has_path_to_end {
            diagnostics.push(Diagnostic::error(
                DiagnosticCode::NoPathToEnd,
                format!("start node '{}' cannot reach any end node", start_id),
                Some(start_id.to_string()),
            ));
        }
    }

    diagnostics
}

fn reachable_nodes<'a>(
    start_id: &'a str,
    node_index: &HashMap<&'a str, &'a crate::ir::Node>,
) -> HashSet<&'a str> {
    let mut visited = HashSet::new();
    let mut queue = VecDeque::from([start_id]);

    while let Some(current) = queue.pop_front() {
        if !visited.insert(current) {
            continue;
        }

        if let Some(node) = node_index.get(current) {
            for edge in node.outgoing_edges() {
                if node_index.contains_key(edge) {
                    queue.push_back(edge);
                }
            }
        }
    }

    visited
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;
    use serde_json::json;

    use crate::ir::{Node, NodeKind, WorkflowDefinition};
    use crate::validation::{validate_and_normalize, DiagnosticCode};

    fn valid_workflow() -> WorkflowDefinition {
        WorkflowDefinition {
            version: "v0".to_string(),
            name: "basic".to_string(),
            nodes: vec![
                Node {
                    id: "start".to_string(),
                    kind: NodeKind::Start {
                        next: "llm".to_string(),
                    },
                },
                Node {
                    id: "llm".to_string(),
                    kind: NodeKind::Llm {
                        model: "gpt-4".to_string(),
                        prompt: "Say hi".to_string(),
                        next: Some("tool".to_string()),
                    },
                },
                Node {
                    id: "tool".to_string(),
                    kind: NodeKind::Tool {
                        tool: "validator".to_string(),
                        input: json!({"strict": true}),
                        next: Some("end".to_string()),
                    },
                },
                Node {
                    id: "end".to_string(),
                    kind: NodeKind::End,
                },
            ],
        }
    }

    #[test]
    fn validates_and_normalizes_valid_workflow() {
        let workflow = valid_workflow();
        let normalized = validate_and_normalize(&workflow).expect("workflow should validate");

        assert_eq!(normalized.nodes.first().map(|n| n.id.as_str()), Some("end"));
        assert_eq!(normalized.nodes.last().map(|n| n.id.as_str()), Some("tool"));
    }

    #[test]
    fn reports_unknown_target() {
        let mut workflow = valid_workflow();
        workflow.nodes[0].kind = NodeKind::Start {
            next: "missing".to_string(),
        };

        let err = validate_and_normalize(&workflow).expect_err("should fail validation");
        assert!(err
            .diagnostics
            .iter()
            .any(|d| d.code == DiagnosticCode::UnknownTarget));
    }

    #[test]
    fn reports_unreachable_node() {
        let mut workflow = valid_workflow();
        workflow.nodes.push(Node {
            id: "orphan".to_string(),
            kind: NodeKind::End,
        });

        let err = validate_and_normalize(&workflow).expect_err("should fail validation");
        assert!(err
            .diagnostics
            .iter()
            .any(|d| d.code == DiagnosticCode::UnreachableNode
                && d.node_id.as_deref() == Some("orphan")));
    }

    #[test]
    fn reports_duplicate_node_id() {
        let mut workflow = valid_workflow();
        workflow.nodes.push(Node {
            id: "llm".to_string(),
            kind: NodeKind::End,
        });

        let err = validate_and_normalize(&workflow).expect_err("should fail validation");
        assert!(err
            .diagnostics
            .iter()
            .any(|d| d.code == DiagnosticCode::DuplicateNodeId));
    }

    #[test]
    fn reports_no_path_to_end() {
        let workflow = WorkflowDefinition {
            version: "v0".to_string(),
            name: "no-end-path".to_string(),
            nodes: vec![
                Node {
                    id: "start".to_string(),
                    kind: NodeKind::Start {
                        next: "llm".to_string(),
                    },
                },
                Node {
                    id: "llm".to_string(),
                    kind: NodeKind::Llm {
                        model: "gpt-4".to_string(),
                        prompt: "test".to_string(),
                        next: None,
                    },
                },
                Node {
                    id: "end".to_string(),
                    kind: NodeKind::End,
                },
            ],
        };

        let err = validate_and_normalize(&workflow).expect_err("should fail validation");
        assert!(err
            .diagnostics
            .iter()
            .any(|d| d.code == DiagnosticCode::NoPathToEnd));
    }

    #[test]
    fn reports_invalid_loop_configuration() {
        let workflow = WorkflowDefinition {
            version: "v0".to_string(),
            name: "bad-loop".to_string(),
            nodes: vec![
                Node {
                    id: "start".to_string(),
                    kind: NodeKind::Start {
                        next: "loop".to_string(),
                    },
                },
                Node {
                    id: "loop".to_string(),
                    kind: NodeKind::Loop {
                        condition: "".to_string(),
                        body: "".to_string(),
                        next: "end".to_string(),
                        max_iterations: Some(0),
                    },
                },
                Node {
                    id: "end".to_string(),
                    kind: NodeKind::End,
                },
            ],
        };

        let err = validate_and_normalize(&workflow).expect_err("loop validation should fail");
        assert!(err
            .diagnostics
            .iter()
            .any(|d| d.code == DiagnosticCode::EmptyField));
    }

    proptest! {
        #[test]
        fn validate_and_normalize_never_panics(name in ".*", version in ".*") {
            let workflow = WorkflowDefinition {
                version,
                name,
                nodes: vec![
                    Node {
                        id: "start".to_string(),
                        kind: NodeKind::Start { next: "end".to_string() },
                    },
                    Node {
                        id: "end".to_string(),
                        kind: NodeKind::End,
                    },
                ],
            };

            let _ = validate_and_normalize(&workflow);
        }
    }
}
