use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};

use thiserror::Error;

use crate::ir::{MergePolicy, NodeKind, WorkflowDefinition, WORKFLOW_IR_V0};

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
        validate_node_kind_fields(node, &mut diagnostics, &mut start_ids, &mut end_count);
    }

    for node in &workflow.nodes {
        match &node.kind {
            NodeKind::Merge { sources, .. } => {
                for source in sources {
                    if !node_index.contains_key(source.as_str()) {
                        diagnostics.push(Diagnostic::error(
                            DiagnosticCode::UnknownTarget,
                            format!("node '{}' references unknown source '{}'", node.id, source),
                            Some(node.id.clone()),
                        ));
                    }
                }
            }
            NodeKind::Reduce { source, .. } => {
                if !node_index.contains_key(source.as_str()) {
                    diagnostics.push(Diagnostic::error(
                        DiagnosticCode::UnknownTarget,
                        format!("node '{}' references unknown source '{}'", node.id, source),
                        Some(node.id.clone()),
                    ));
                }
            }
            _ => {}
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

fn validate_node_kind_fields(
    node: &crate::ir::Node,
    diagnostics: &mut Vec<Diagnostic>,
    start_ids: &mut Vec<String>,
    end_count: &mut usize,
) {
    fn emit_empty_field(diagnostics: &mut Vec<Diagnostic>, node_id: &str, message: &str) {
        diagnostics.push(Diagnostic::error(
            DiagnosticCode::EmptyField,
            message,
            Some(node_id.to_string()),
        ));
    }

    fn require_non_empty(
        diagnostics: &mut Vec<Diagnostic>,
        node_id: &str,
        value: &str,
        message: &str,
    ) {
        if value.is_empty() {
            emit_empty_field(diagnostics, node_id, message);
        }
    }

    fn require_optional_non_empty(
        diagnostics: &mut Vec<Diagnostic>,
        node_id: &str,
        value: Option<&str>,
        message: &str,
    ) {
        if value.is_some_and(str::is_empty) {
            emit_empty_field(diagnostics, node_id, message);
        }
    }

    fn require_non_empty_fields(
        diagnostics: &mut Vec<Diagnostic>,
        node_id: &str,
        fields: &[(&str, &str)],
    ) {
        for (value, message) in fields {
            require_non_empty(diagnostics, node_id, value, message);
        }
    }

    fn require_optional_non_empty_fields(
        diagnostics: &mut Vec<Diagnostic>,
        node_id: &str,
        fields: &[(Option<&str>, &str)],
    ) {
        for (value, message) in fields {
            require_optional_non_empty(diagnostics, node_id, *value, message);
        }
    }

    fn require_positive_when_present<T>(
        diagnostics: &mut Vec<Diagnostic>,
        node_id: &str,
        value: Option<T>,
        message: &str,
    ) where
        T: Copy + PartialEq + From<u8>,
    {
        if value.is_some_and(|limit| limit == T::from(0u8)) {
            emit_empty_field(diagnostics, node_id, message);
        }
    }

    fn require_non_empty_collection<T>(
        diagnostics: &mut Vec<Diagnostic>,
        node_id: &str,
        values: &[T],
        message: &str,
    ) {
        if values.is_empty() {
            emit_empty_field(diagnostics, node_id, message);
        }
    }

    fn require_no_empty_string_entries(
        diagnostics: &mut Vec<Diagnostic>,
        node_id: &str,
        values: &[String],
        message: &str,
    ) {
        if values.iter().any(String::is_empty) {
            emit_empty_field(diagnostics, node_id, message);
        }
    }

    fn require_merge_quorum_matches_policy(
        diagnostics: &mut Vec<Diagnostic>,
        node_id: &str,
        policy: &MergePolicy,
        quorum: Option<usize>,
        source_len: usize,
    ) {
        match policy {
            MergePolicy::Quorum => {
                let invalid_quorum = match quorum {
                    Some(value) => value == 0 || value > source_len,
                    None => true,
                };
                if invalid_quorum {
                    emit_empty_field(
                        diagnostics,
                        node_id,
                        "merge.quorum must be between 1 and merge.sources length for quorum policy",
                    );
                }
            }
            _ => {
                if quorum.is_some() {
                    emit_empty_field(
                        diagnostics,
                        node_id,
                        "merge.quorum is only valid with quorum policy",
                    );
                }
            }
        }
    }

    match &node.kind {
        NodeKind::Start { next } => {
            start_ids.push(node.id.clone());
            require_non_empty(diagnostics, &node.id, next, "start.next must not be empty");
        }
        NodeKind::Llm {
            model,
            prompt,
            next: _,
        } => {
            require_non_empty_fields(
                diagnostics,
                &node.id,
                &[
                    (model, "llm.model must not be empty"),
                    (prompt, "llm.prompt must not be empty"),
                ],
            );
        }
        NodeKind::Tool { tool, .. } => {
            require_non_empty(diagnostics, &node.id, tool, "tool.tool must not be empty");
        }
        NodeKind::Condition {
            expression,
            on_true,
            on_false,
        } => {
            require_non_empty_fields(
                diagnostics,
                &node.id,
                &[
                    (expression, "condition.expression must not be empty"),
                    (on_true, "condition.on_true must not be empty"),
                    (on_false, "condition.on_false must not be empty"),
                ],
            );
        }
        NodeKind::Debounce {
            key_path,
            window_steps,
            next,
            on_suppressed,
        } => {
            require_non_empty_fields(
                diagnostics,
                &node.id,
                &[
                    (key_path, "debounce.key_path must not be empty"),
                    (next, "debounce.next must not be empty"),
                ],
            );
            if *window_steps == 0 {
                emit_empty_field(
                    diagnostics,
                    &node.id,
                    "debounce.window_steps must be greater than zero",
                );
            }
            require_optional_non_empty_fields(
                diagnostics,
                &node.id,
                &[(
                    on_suppressed.as_deref(),
                    "debounce.on_suppressed must not be empty when provided",
                )],
            );
        }
        NodeKind::Throttle {
            key_path,
            window_steps,
            next,
            on_throttled,
        } => {
            require_non_empty_fields(
                diagnostics,
                &node.id,
                &[
                    (key_path, "throttle.key_path must not be empty"),
                    (next, "throttle.next must not be empty"),
                ],
            );
            if *window_steps == 0 {
                emit_empty_field(
                    diagnostics,
                    &node.id,
                    "throttle.window_steps must be greater than zero",
                );
            }
            require_optional_non_empty_fields(
                diagnostics,
                &node.id,
                &[(
                    on_throttled.as_deref(),
                    "throttle.on_throttled must not be empty when provided",
                )],
            );
        }
        NodeKind::RetryCompensate {
            tool,
            input: _,
            max_retries: _,
            compensate_tool,
            compensate_input: _,
            next,
            on_compensated,
        } => {
            require_non_empty_fields(
                diagnostics,
                &node.id,
                &[
                    (tool, "retry_compensate.tool must not be empty"),
                    (
                        compensate_tool,
                        "retry_compensate.compensate_tool must not be empty",
                    ),
                    (next, "retry_compensate.next must not be empty"),
                ],
            );
            require_optional_non_empty_fields(
                diagnostics,
                &node.id,
                &[(
                    on_compensated.as_deref(),
                    "retry_compensate.on_compensated must not be empty when provided",
                )],
            );
        }
        NodeKind::HumanInTheLoop {
            decision_path,
            response_path,
            on_approve,
            on_reject,
        } => {
            require_non_empty_fields(
                diagnostics,
                &node.id,
                &[
                    (
                        decision_path,
                        "human_in_the_loop.decision_path must not be empty",
                    ),
                    (on_approve, "human_in_the_loop.on_approve must not be empty"),
                    (on_reject, "human_in_the_loop.on_reject must not be empty"),
                ],
            );
            require_optional_non_empty_fields(
                diagnostics,
                &node.id,
                &[(
                    response_path.as_deref(),
                    "human_in_the_loop.response_path must not be empty when provided",
                )],
            );
        }
        NodeKind::CacheWrite {
            key_path,
            value_path,
            next,
        } => {
            require_non_empty_fields(
                diagnostics,
                &node.id,
                &[
                    (key_path, "cache_write.key_path must not be empty"),
                    (value_path, "cache_write.value_path must not be empty"),
                    (next, "cache_write.next must not be empty"),
                ],
            );
        }
        NodeKind::CacheRead {
            key_path,
            next,
            on_miss,
        } => {
            require_non_empty_fields(
                diagnostics,
                &node.id,
                &[
                    (key_path, "cache_read.key_path must not be empty"),
                    (next, "cache_read.next must not be empty"),
                ],
            );
            require_optional_non_empty_fields(
                diagnostics,
                &node.id,
                &[(
                    on_miss.as_deref(),
                    "cache_read.on_miss must not be empty when provided",
                )],
            );
        }
        NodeKind::EventTrigger {
            event,
            event_path,
            next,
            on_mismatch,
        } => {
            require_non_empty_fields(
                diagnostics,
                &node.id,
                &[
                    (event, "event_trigger.event must not be empty"),
                    (event_path, "event_trigger.event_path must not be empty"),
                    (next, "event_trigger.next must not be empty"),
                ],
            );
            require_optional_non_empty_fields(
                diagnostics,
                &node.id,
                &[(
                    on_mismatch.as_deref(),
                    "event_trigger.on_mismatch must not be empty when provided",
                )],
            );
        }
        NodeKind::Router { routes, default } => {
            require_non_empty_collection(
                diagnostics,
                &node.id,
                routes,
                "router.routes must contain at least one route",
            );
            if routes
                .iter()
                .any(|route| route.when.is_empty() || route.next.is_empty())
            {
                emit_empty_field(
                    diagnostics,
                    &node.id,
                    "router.routes entries must include non-empty when and next",
                );
            }
            require_non_empty(
                diagnostics,
                &node.id,
                default,
                "router.default must not be empty",
            );
        }
        NodeKind::Transform { expression, next } => {
            require_non_empty_fields(
                diagnostics,
                &node.id,
                &[
                    (expression, "transform.expression must not be empty"),
                    (next, "transform.next must not be empty"),
                ],
            );
        }
        NodeKind::Loop {
            condition,
            body,
            next,
            max_iterations,
        } => {
            require_non_empty_fields(
                diagnostics,
                &node.id,
                &[
                    (condition, "loop.condition must not be empty"),
                    (body, "loop.body must not be empty"),
                    (next, "loop.next must not be empty"),
                ],
            );
            require_positive_when_present(
                diagnostics,
                &node.id,
                *max_iterations,
                "loop.max_iterations must be greater than zero when provided",
            );
        }
        NodeKind::End => {
            *end_count += 1;
        }
        NodeKind::Subgraph { graph, next } => {
            require_non_empty_fields(
                diagnostics,
                &node.id,
                &[(graph, "subgraph.graph must not be empty")],
            );
            require_optional_non_empty_fields(
                diagnostics,
                &node.id,
                &[(
                    next.as_deref(),
                    "subgraph.next must not be empty when provided",
                )],
            );
        }
        NodeKind::Batch { items_path, next } => {
            require_non_empty_fields(
                diagnostics,
                &node.id,
                &[
                    (items_path, "batch.items_path must not be empty"),
                    (next, "batch.next must not be empty"),
                ],
            );
        }
        NodeKind::Filter {
            items_path,
            expression,
            next,
        } => {
            require_non_empty_fields(
                diagnostics,
                &node.id,
                &[
                    (items_path, "filter.items_path must not be empty"),
                    (expression, "filter.expression must not be empty"),
                    (next, "filter.next must not be empty"),
                ],
            );
        }
        NodeKind::Parallel {
            branches,
            next,
            max_in_flight,
        } => {
            require_non_empty_collection(
                diagnostics,
                &node.id,
                branches,
                "parallel.branches must contain at least one node id",
            );
            require_no_empty_string_entries(
                diagnostics,
                &node.id,
                branches,
                "parallel.branches must not contain empty node ids",
            );
            require_non_empty_fields(
                diagnostics,
                &node.id,
                &[(next, "parallel.next must not be empty")],
            );
            require_positive_when_present(
                diagnostics,
                &node.id,
                *max_in_flight,
                "parallel.max_in_flight must be greater than zero when provided",
            );
        }
        NodeKind::Merge {
            sources,
            policy,
            quorum,
            next,
        } => {
            require_non_empty_collection(
                diagnostics,
                &node.id,
                sources,
                "merge.sources must contain at least one node id",
            );
            require_no_empty_string_entries(
                diagnostics,
                &node.id,
                sources,
                "merge.sources must not contain empty node ids",
            );
            require_non_empty(diagnostics, &node.id, next, "merge.next must not be empty");
            require_merge_quorum_matches_policy(
                diagnostics,
                &node.id,
                policy,
                *quorum,
                sources.len(),
            );
        }
        NodeKind::Map {
            tool,
            items_path,
            next,
            max_in_flight,
        } => {
            require_non_empty_fields(
                diagnostics,
                &node.id,
                &[
                    (tool, "map.tool must not be empty"),
                    (items_path, "map.items_path must not be empty"),
                    (next, "map.next must not be empty"),
                ],
            );
            require_positive_when_present(
                diagnostics,
                &node.id,
                *max_in_flight,
                "map.max_in_flight must be greater than zero when provided",
            );
        }
        NodeKind::Reduce {
            source,
            operation: _,
            next,
        } => {
            require_non_empty_fields(
                diagnostics,
                &node.id,
                &[
                    (source, "reduce.source must not be empty"),
                    (next, "reduce.next must not be empty"),
                ],
            );
        }
    }
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

    use crate::ir::{MergePolicy, Node, NodeKind, ReduceOperation, WorkflowDefinition};
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

    #[test]
    fn reports_invalid_merge_quorum_configuration() {
        let workflow = WorkflowDefinition {
            version: "v0".to_string(),
            name: "bad-merge".to_string(),
            nodes: vec![
                Node {
                    id: "start".to_string(),
                    kind: NodeKind::Start {
                        next: "merge".to_string(),
                    },
                },
                Node {
                    id: "source".to_string(),
                    kind: NodeKind::Tool {
                        tool: "echo".to_string(),
                        input: json!({}),
                        next: Some("end".to_string()),
                    },
                },
                Node {
                    id: "merge".to_string(),
                    kind: NodeKind::Merge {
                        sources: vec!["source".to_string()],
                        policy: MergePolicy::Quorum,
                        quorum: Some(2),
                        next: "end".to_string(),
                    },
                },
                Node {
                    id: "end".to_string(),
                    kind: NodeKind::End,
                },
            ],
        };

        let err = validate_and_normalize(&workflow).expect_err("merge quorum should fail");
        assert!(
            err.diagnostics
                .iter()
                .any(|d| d.code == DiagnosticCode::EmptyField
                    && d.node_id.as_deref() == Some("merge"))
        );
    }

    #[test]
    fn reports_unknown_reduce_source() {
        let workflow = WorkflowDefinition {
            version: "v0".to_string(),
            name: "bad-reduce".to_string(),
            nodes: vec![
                Node {
                    id: "start".to_string(),
                    kind: NodeKind::Start {
                        next: "reduce".to_string(),
                    },
                },
                Node {
                    id: "reduce".to_string(),
                    kind: NodeKind::Reduce {
                        source: "missing".to_string(),
                        operation: ReduceOperation::Count,
                        next: "end".to_string(),
                    },
                },
                Node {
                    id: "end".to_string(),
                    kind: NodeKind::End,
                },
            ],
        };

        let err = validate_and_normalize(&workflow).expect_err("reduce source should fail");
        assert!(err
            .diagnostics
            .iter()
            .any(|d| d.code == DiagnosticCode::UnknownTarget
                && d.node_id.as_deref() == Some("reduce")));
    }

    #[test]
    fn reports_invalid_extended_node_configuration() {
        let workflow = WorkflowDefinition {
            version: "v0".to_string(),
            name: "invalid-extended".to_string(),
            nodes: vec![
                Node {
                    id: "start".to_string(),
                    kind: NodeKind::Start {
                        next: "debounce".to_string(),
                    },
                },
                Node {
                    id: "debounce".to_string(),
                    kind: NodeKind::Debounce {
                        key_path: "".to_string(),
                        window_steps: 0,
                        next: "router".to_string(),
                        on_suppressed: None,
                    },
                },
                Node {
                    id: "router".to_string(),
                    kind: NodeKind::Router {
                        routes: vec![],
                        default: "".to_string(),
                    },
                },
                Node {
                    id: "transform".to_string(),
                    kind: NodeKind::Transform {
                        expression: "".to_string(),
                        next: "end".to_string(),
                    },
                },
                Node {
                    id: "end".to_string(),
                    kind: NodeKind::End,
                },
            ],
        };

        let err =
            validate_and_normalize(&workflow).expect_err("extended node validation should fail");
        assert!(err
            .diagnostics
            .iter()
            .any(|d| d.code == DiagnosticCode::EmptyField));
    }

    #[test]
    fn reports_multiple_empty_field_diagnostics_for_parallel_node() {
        let workflow = WorkflowDefinition {
            version: "v0".to_string(),
            name: "bad-parallel".to_string(),
            nodes: vec![
                Node {
                    id: "start".to_string(),
                    kind: NodeKind::Start {
                        next: "parallel".to_string(),
                    },
                },
                Node {
                    id: "parallel".to_string(),
                    kind: NodeKind::Parallel {
                        branches: vec!["".to_string()],
                        next: "".to_string(),
                        max_in_flight: Some(0),
                    },
                },
                Node {
                    id: "end".to_string(),
                    kind: NodeKind::End,
                },
            ],
        };

        let err = validate_and_normalize(&workflow).expect_err("parallel validation should fail");

        assert!(err.diagnostics.iter().any(|d| d.message
            == "parallel.branches must not contain empty node ids"
            && d.node_id.as_deref() == Some("parallel")));
        assert!(err
            .diagnostics
            .iter()
            .any(|d| d.message == "parallel.next must not be empty"
                && d.node_id.as_deref() == Some("parallel")));
        assert!(err.diagnostics.iter().any(|d| d.message
            == "parallel.max_in_flight must be greater than zero when provided"
            && d.node_id.as_deref() == Some("parallel")));
    }

    #[test]
    fn reports_merge_source_unknown_and_quorum_policy_errors_together() {
        let workflow = WorkflowDefinition {
            version: "v0".to_string(),
            name: "bad-merge-sources".to_string(),
            nodes: vec![
                Node {
                    id: "start".to_string(),
                    kind: NodeKind::Start {
                        next: "merge".to_string(),
                    },
                },
                Node {
                    id: "merge".to_string(),
                    kind: NodeKind::Merge {
                        sources: vec!["missing".to_string()],
                        policy: MergePolicy::Quorum,
                        quorum: Some(0),
                        next: "end".to_string(),
                    },
                },
                Node {
                    id: "end".to_string(),
                    kind: NodeKind::End,
                },
            ],
        };

        let err = validate_and_normalize(&workflow).expect_err("merge validation should fail");

        assert!(err.diagnostics.iter().any(|d| {
            d.code == DiagnosticCode::UnknownTarget
                && d.node_id.as_deref() == Some("merge")
                && d.message.contains("unknown source 'missing'")
        }));

        assert!(err.diagnostics.iter().any(|d| {
            d.code == DiagnosticCode::EmptyField
                && d.node_id.as_deref() == Some("merge")
                && d.message
                    == "merge.quorum must be between 1 and merge.sources length for quorum policy"
        }));
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
