use crate::ir::{NodeKind, WorkflowDefinition};

/// Renders a workflow definition as a Mermaid flowchart.
pub fn workflow_to_mermaid(workflow: &WorkflowDefinition) -> String {
    let mut lines = Vec::new();
    lines.push("flowchart TD".to_string());

    for node in &workflow.nodes {
        let kind = node_kind_label(&node.kind);
        lines.push(format!(
            "  {}[\"{}\\n({})\"]",
            sanitize_id(&node.id),
            escape_label(&node.id),
            kind
        ));
    }

    for node in &workflow.nodes {
        let from = sanitize_id(&node.id);
        for (label, to) in edge_specs(&node.kind) {
            let edge = if label.is_empty() {
                format!("  {} --> {}", from, sanitize_id(&to))
            } else {
                format!(
                    "  {} -- \"{}\" --> {}",
                    from,
                    escape_label(&label),
                    sanitize_id(&to)
                )
            };
            lines.push(edge);
        }
    }

    lines.join("\n")
}

fn edge_specs(kind: &NodeKind) -> Vec<(String, String)> {
    match kind {
        NodeKind::Start { next } => vec![("".to_string(), next.clone())],
        NodeKind::Llm { next, .. } => next
            .as_ref()
            .map(|n| vec![("".to_string(), n.clone())])
            .unwrap_or_default(),
        NodeKind::Tool { next, .. } => next
            .as_ref()
            .map(|n| vec![("".to_string(), n.clone())])
            .unwrap_or_default(),
        NodeKind::Condition {
            on_true, on_false, ..
        } => vec![
            ("true".to_string(), on_true.clone()),
            ("false".to_string(), on_false.clone()),
        ],
        NodeKind::Debounce {
            next,
            on_suppressed,
            ..
        } => {
            let mut edges = vec![("emit".to_string(), next.clone())];
            if let Some(target) = on_suppressed.as_ref() {
                edges.push(("suppressed".to_string(), target.clone()));
            }
            edges
        }
        NodeKind::Throttle {
            next, on_throttled, ..
        } => {
            let mut edges = vec![("emit".to_string(), next.clone())];
            if let Some(target) = on_throttled.as_ref() {
                edges.push(("throttled".to_string(), target.clone()));
            }
            edges
        }
        NodeKind::RetryCompensate {
            next,
            on_compensated,
            ..
        } => {
            let mut edges = vec![("success".to_string(), next.clone())];
            if let Some(target) = on_compensated.as_ref() {
                edges.push(("compensated".to_string(), target.clone()));
            }
            edges
        }
        NodeKind::HumanInTheLoop {
            on_approve,
            on_reject,
            ..
        } => {
            vec![
                ("approve".to_string(), on_approve.clone()),
                ("reject".to_string(), on_reject.clone()),
            ]
        }
        NodeKind::CacheWrite { next, .. } => vec![("".to_string(), next.clone())],
        NodeKind::CacheRead { next, on_miss, .. } => {
            let mut edges = vec![("hit".to_string(), next.clone())];
            if let Some(target) = on_miss.as_ref() {
                edges.push(("miss".to_string(), target.clone()));
            }
            edges
        }
        NodeKind::EventTrigger {
            next, on_mismatch, ..
        } => {
            let mut edges = vec![("match".to_string(), next.clone())];
            if let Some(target) = on_mismatch.as_ref() {
                edges.push(("mismatch".to_string(), target.clone()));
            }
            edges
        }
        NodeKind::Router { routes, default } => {
            let mut edges: Vec<(String, String)> = routes
                .iter()
                .enumerate()
                .map(|(i, route)| {
                    let mut label = String::from("route");
                    label.push_str(&(i + 1).to_string());
                    (label, route.next.clone())
                })
                .collect();
            edges.push(("default".to_string(), default.clone()));
            edges
        }
        NodeKind::Transform { next, .. } => vec![("".to_string(), next.clone())],
        NodeKind::Loop { body, next, .. } => vec![
            ("continue".to_string(), body.clone()),
            ("done".to_string(), next.clone()),
        ],
        NodeKind::Subgraph { next, .. } => next
            .as_ref()
            .map(|n| vec![("".to_string(), n.clone())])
            .unwrap_or_default(),
        NodeKind::Batch { next, .. } => vec![("".to_string(), next.clone())],
        NodeKind::Filter { next, .. } => vec![("".to_string(), next.clone())],
        NodeKind::Parallel { branches, next, .. } => {
            let mut edges = branches
                .iter()
                .map(|branch| ("branch".to_string(), branch.clone()))
                .collect::<Vec<(String, String)>>();
            edges.push(("join".to_string(), next.clone()));
            edges
        }
        NodeKind::Merge { sources, next, .. } => {
            let mut edges = sources
                .iter()
                .map(|source| ("source".to_string(), source.clone()))
                .collect::<Vec<(String, String)>>();
            edges.push(("next".to_string(), next.clone()));
            edges
        }
        NodeKind::Map { next, .. } => vec![("".to_string(), next.clone())],
        NodeKind::Reduce { next, .. } => vec![("".to_string(), next.clone())],
        NodeKind::End => Vec::new(),
    }
}

fn node_kind_label(kind: &NodeKind) -> &'static str {
    match kind {
        NodeKind::Start { .. } => "start",
        NodeKind::Llm { .. } => "llm",
        NodeKind::Tool { .. } => "tool",
        NodeKind::Condition { .. } => "condition",
        NodeKind::Debounce { .. } => "debounce",
        NodeKind::Throttle { .. } => "throttle",
        NodeKind::RetryCompensate { .. } => "retry_compensate",
        NodeKind::HumanInTheLoop { .. } => "human_in_the_loop",
        NodeKind::CacheWrite { .. } => "cache_write",
        NodeKind::CacheRead { .. } => "cache_read",
        NodeKind::EventTrigger { .. } => "event_trigger",
        NodeKind::Router { .. } => "router",
        NodeKind::Transform { .. } => "transform",
        NodeKind::Loop { .. } => "loop",
        NodeKind::Subgraph { .. } => "subgraph",
        NodeKind::Batch { .. } => "batch",
        NodeKind::Filter { .. } => "filter",
        NodeKind::Parallel { .. } => "parallel",
        NodeKind::Merge { .. } => "merge",
        NodeKind::Map { .. } => "map",
        NodeKind::Reduce { .. } => "reduce",
        NodeKind::End => "end",
    }
}

fn sanitize_id(id: &str) -> String {
    let mut out = String::with_capacity(id.len() + 1);
    if id
        .chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_alphabetic() || ch == '_')
    {
        out.push_str(id);
    } else {
        out.push('n');
        out.push('_');
        out.push_str(id);
    }
    out.chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

fn escape_label(label: &str) -> String {
    label.replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::ir::{Node, NodeKind, RouterRoute, WorkflowDefinition};
    use crate::visualize::workflow_to_mermaid;

    #[test]
    fn renders_condition_edges_with_labels() {
        let workflow = WorkflowDefinition {
            version: "v0".to_string(),
            name: "cond".to_string(),
            nodes: vec![
                Node {
                    id: "start".to_string(),
                    kind: NodeKind::Start {
                        next: "route".to_string(),
                    },
                },
                Node {
                    id: "route".to_string(),
                    kind: NodeKind::Condition {
                        expression: "input.ok == true".to_string(),
                        on_true: "yes".to_string(),
                        on_false: "no".to_string(),
                    },
                },
                Node {
                    id: "yes".to_string(),
                    kind: NodeKind::End,
                },
                Node {
                    id: "no".to_string(),
                    kind: NodeKind::End,
                },
            ],
        };

        let mermaid = workflow_to_mermaid(&workflow);
        assert!(mermaid.contains("route -- \"true\" --> yes"));
        assert!(mermaid.contains("route -- \"false\" --> no"));
    }

    #[test]
    fn renders_parallel_and_router_shapes() {
        let workflow = WorkflowDefinition {
            version: "v0".to_string(),
            name: "advanced".to_string(),
            nodes: vec![
                Node {
                    id: "fanout".to_string(),
                    kind: NodeKind::Parallel {
                        branches: vec!["a".to_string(), "b".to_string()],
                        next: "join".to_string(),
                        max_in_flight: Some(2),
                    },
                },
                Node {
                    id: "pick".to_string(),
                    kind: NodeKind::Router {
                        routes: vec![RouterRoute {
                            when: "input.x == 1".to_string(),
                            next: "a".to_string(),
                        }],
                        default: "b".to_string(),
                    },
                },
                Node {
                    id: "a".to_string(),
                    kind: NodeKind::Tool {
                        tool: "t".to_string(),
                        input: json!({}),
                        next: Some("join".to_string()),
                    },
                },
                Node {
                    id: "b".to_string(),
                    kind: NodeKind::End,
                },
                Node {
                    id: "join".to_string(),
                    kind: NodeKind::Merge {
                        sources: vec!["a".to_string(), "b".to_string()],
                        policy: crate::ir::MergePolicy::All,
                        quorum: None,
                        next: "b".to_string(),
                    },
                },
            ],
        };

        let mermaid = workflow_to_mermaid(&workflow);
        assert!(mermaid.contains("fanout -- \"branch\" --> a"));
        assert!(mermaid.contains("fanout -- \"join\" --> join"));
        assert!(mermaid.contains("pick -- \"route1\" --> a"));
        assert!(mermaid.contains("pick -- \"default\" --> b"));
    }
}
