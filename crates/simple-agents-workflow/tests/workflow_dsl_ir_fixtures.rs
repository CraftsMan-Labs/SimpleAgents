use serde::Deserialize;
use serde_json::Value;
use simple_agents_workflow::ir::{NodeKind, WorkflowDefinition};
use simple_agents_workflow::validate_and_normalize;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Deserialize)]
struct WireExpectation {
    node_id: String,
    outgoing: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct MergeSourceExpectation {
    node_id: String,
    sources: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct WorkflowDslFixture {
    entry: String,
    nodes: BTreeMap<String, Value>,
}

#[derive(Debug, Deserialize)]
struct WorkflowDslIrFixture {
    workflow_dsl: WorkflowDslFixture,
    canonical_ir: Value,
    required_node_types: Vec<String>,
    wire_expectations: Vec<WireExpectation>,
    merge_source_expectations: Vec<MergeSourceExpectation>,
}

fn fixture() -> WorkflowDslIrFixture {
    let path = format!(
        "{}/../../parity-fixtures/workflow_dsl_ir_golden.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let contents = std::fs::read_to_string(path).expect("workflow DSL fixture should be readable");
    serde_json::from_str(&contents).expect("workflow DSL fixture should parse")
}

fn node_type_name(kind: &NodeKind) -> &'static str {
    match kind {
        NodeKind::Start { .. } => "start",
        NodeKind::Llm { .. } => "llm",
        NodeKind::Tool { .. } => "tool",
        NodeKind::Condition { .. } => "condition",
        NodeKind::Loop { .. } => "loop",
        NodeKind::Parallel { .. } => "parallel",
        NodeKind::Merge { .. } => "merge",
        NodeKind::Map { .. } => "map",
        NodeKind::Reduce { .. } => "reduce",
        NodeKind::Subgraph { .. } => "subgraph",
        NodeKind::Batch { .. } => "batch",
        NodeKind::Filter { .. } => "filter",
        NodeKind::Debounce { .. } => "debounce",
        NodeKind::Throttle { .. } => "throttle",
        NodeKind::RetryCompensate { .. } => "retry_compensate",
        NodeKind::HumanInTheLoop { .. } => "human_in_the_loop",
        NodeKind::CacheWrite { .. } => "cache_write",
        NodeKind::CacheRead { .. } => "cache_read",
        NodeKind::EventTrigger { .. } => "event_trigger",
        NodeKind::Router { .. } => "router",
        NodeKind::Transform { .. } => "transform",
        NodeKind::End => "end",
    }
}

#[test]
fn advanced_workflow_fixture_has_expected_node_coverage_and_wires() {
    let fixture = fixture();
    let workflow: WorkflowDefinition =
        serde_json::from_value(fixture.canonical_ir).expect("canonical IR should parse");
    let normalized = validate_and_normalize(&workflow).expect("canonical IR should validate");

    let actual_types: BTreeSet<String> = normalized
        .nodes
        .iter()
        .map(|node| node_type_name(&node.kind).to_string())
        .collect();
    let expected_types: BTreeSet<String> = fixture.required_node_types.into_iter().collect();
    assert_eq!(actual_types, expected_types);

    let dsl_ids: BTreeSet<String> = fixture.workflow_dsl.nodes.keys().cloned().collect();
    let canonical_ids: BTreeSet<String> = normalized
        .nodes
        .iter()
        .map(|node| node.id.clone())
        .collect();
    assert_eq!(dsl_ids, canonical_ids);
    assert!(
        canonical_ids.contains(&fixture.workflow_dsl.entry),
        "DSL entry node should exist in canonical IR"
    );

    let outgoing_by_node: BTreeMap<String, Vec<String>> = normalized
        .nodes
        .iter()
        .map(|node| {
            let mut outgoing = node
                .outgoing_edges()
                .into_iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>();
            outgoing.sort();
            (node.id.clone(), outgoing)
        })
        .collect();

    for expectation in fixture.wire_expectations {
        let mut expected = expectation.outgoing;
        expected.sort();
        let actual = outgoing_by_node
            .get(&expectation.node_id)
            .unwrap_or_else(|| panic!("missing node '{}'", expectation.node_id));
        assert_eq!(
            actual, &expected,
            "node '{}' should keep expected outgoing wires",
            expectation.node_id
        );
    }

    for expectation in fixture.merge_source_expectations {
        let merge_node = normalized
            .nodes
            .iter()
            .find(|node| node.id == expectation.node_id)
            .unwrap_or_else(|| panic!("missing merge node '{}'", expectation.node_id));

        let NodeKind::Merge { sources, .. } = &merge_node.kind else {
            panic!("node '{}' should be merge", expectation.node_id);
        };

        let mut actual = sources.clone();
        let mut expected = expectation.sources;
        actual.sort();
        expected.sort();
        assert_eq!(actual, expected);
    }
}
