use serde_json::{json, Value};

use super::types::YamlWorkflowRunOutput;

pub(crate) fn workflow_nerdstats(output: &YamlWorkflowRunOutput) -> Value {
    let llm_nodes_without_usage: Vec<String> = output
        .step_timings
        .iter()
        .filter(|step| step.node_kind == "llm_call" && step.total_tokens.is_none())
        .map(|step| step.node_id.clone())
        .collect();
    let token_metrics_available = llm_nodes_without_usage.is_empty();
    let token_metrics_source = if token_metrics_available {
        "provider_usage"
    } else {
        "provider_stream_usage_unavailable"
    };
    let total_input_tokens = if token_metrics_available {
        json!(output.total_input_tokens)
    } else {
        Value::Null
    };
    let total_output_tokens = if token_metrics_available {
        json!(output.total_output_tokens)
    } else {
        Value::Null
    };
    let total_tokens = if token_metrics_available {
        json!(output.total_tokens)
    } else {
        Value::Null
    };
    let total_reasoning_tokens = if token_metrics_available {
        json!(output.total_reasoning_tokens)
    } else {
        Value::Null
    };
    let tokens_per_second = if token_metrics_available {
        json!(output.tokens_per_second)
    } else {
        Value::Null
    };

    json!({
        "workflow_id": output.workflow_id,
        "terminal_node": output.terminal_node,
        "total_elapsed_ms": output.total_elapsed_ms,
        "ttft_ms": output.ttft_ms,
        "step_details": output.step_timings,
        "total_input_tokens": total_input_tokens,
        "total_output_tokens": total_output_tokens,
        "total_tokens": total_tokens,
        "total_reasoning_tokens": total_reasoning_tokens,
        "tokens_per_second": tokens_per_second,
        "trace_id": output.trace_id,
        "token_metrics_available": token_metrics_available,
        "token_metrics_source": token_metrics_source,
        "llm_nodes_without_usage": llm_nodes_without_usage,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    use super::super::types::YamlStepTiming;

    #[test]
    fn workflow_nerdstats_marks_stream_token_metrics_unavailable() {
        let output = YamlWorkflowRunOutput {
            workflow_id: "nerdstats-test".to_string(),
            entry_node: "classify".to_string(),
            trace: vec!["classify".to_string()],
            outputs: BTreeMap::new(),
            terminal_node: "classify".to_string(),
            terminal_output: Some(json!({"ok": true})),
            step_timings: vec![YamlStepTiming {
                node_id: "classify".to_string(),
                node_kind: "llm_call".to_string(),
                model_name: Some("gpt-4.1".to_string()),
                elapsed_ms: 100,
                prompt_tokens: None,
                completion_tokens: None,
                total_tokens: None,
                reasoning_tokens: None,
                tokens_per_second: None,
            }],
            llm_node_metrics: BTreeMap::new(),
            llm_node_models: BTreeMap::new(),
            total_elapsed_ms: 100,
            ttft_ms: None,
            total_input_tokens: 0,
            total_output_tokens: 0,
            total_tokens: 0,
            total_reasoning_tokens: None,
            tokens_per_second: 0.0,
            trace_id: None,
            metadata: None,
        };

        let nerdstats = workflow_nerdstats(&output);

        assert_eq!(
            nerdstats
                .get("token_metrics_available")
                .and_then(|v| v.as_bool()),
            Some(false)
        );
        assert_eq!(
            nerdstats
                .get("token_metrics_source")
                .and_then(|v| v.as_str()),
            Some("provider_stream_usage_unavailable")
        );
        assert!(nerdstats.get("total_input_tokens").unwrap().is_null());
        assert!(nerdstats.get("total_output_tokens").unwrap().is_null());
        assert!(nerdstats.get("total_tokens").unwrap().is_null());
        assert!(nerdstats.get("total_reasoning_tokens").unwrap().is_null());
        assert!(nerdstats.get("tokens_per_second").unwrap().is_null());
        let missing_nodes = nerdstats
            .get("llm_nodes_without_usage")
            .and_then(|v| v.as_array())
            .expect("llm_nodes_without_usage should be an array");
        assert_eq!(missing_nodes.len(), 1);
        assert_eq!(missing_nodes[0].as_str(), Some("classify"));
    }

    #[test]
    fn workflow_nerdstats_includes_ttft_when_available() {
        let output = YamlWorkflowRunOutput {
            workflow_id: "ttft-test".to_string(),
            entry_node: "step".to_string(),
            trace: vec!["step".to_string()],
            outputs: BTreeMap::new(),
            terminal_node: "step".to_string(),
            terminal_output: Some(json!({"ok": true})),
            step_timings: vec![YamlStepTiming {
                node_id: "step".to_string(),
                node_kind: "llm_call".to_string(),
                model_name: Some("gpt-4.1".to_string()),
                elapsed_ms: 200,
                prompt_tokens: Some(10),
                completion_tokens: Some(5),
                total_tokens: Some(15),
                reasoning_tokens: None,
                tokens_per_second: None,
            }],
            llm_node_metrics: BTreeMap::new(),
            llm_node_models: BTreeMap::new(),
            total_elapsed_ms: 200,
            ttft_ms: Some(42),
            total_input_tokens: 10,
            total_output_tokens: 5,
            total_tokens: 15,
            total_reasoning_tokens: None,
            tokens_per_second: 25.0,
            trace_id: None,
            metadata: None,
        };

        let nerdstats = workflow_nerdstats(&output);

        assert_eq!(nerdstats.get("ttft_ms").and_then(|v| v.as_u64()), Some(42));
    }

    #[test]
    fn workflow_nerdstats_schema_contract_is_stable() {
        let output = YamlWorkflowRunOutput {
            workflow_id: "schema-contract".to_string(),
            entry_node: "start".to_string(),
            trace: vec!["start".to_string()],
            outputs: BTreeMap::new(),
            terminal_node: "start".to_string(),
            terminal_output: Some(json!({"ok": true})),
            step_timings: vec![YamlStepTiming {
                node_id: "start".to_string(),
                node_kind: "llm_call".to_string(),
                model_name: Some("gpt-4.1".to_string()),
                elapsed_ms: 50,
                prompt_tokens: Some(10),
                completion_tokens: Some(5),
                total_tokens: Some(15),
                reasoning_tokens: None,
                tokens_per_second: None,
            }],
            llm_node_metrics: BTreeMap::new(),
            llm_node_models: BTreeMap::new(),
            total_elapsed_ms: 50,
            ttft_ms: None,
            total_input_tokens: 10,
            total_output_tokens: 5,
            total_tokens: 15,
            total_reasoning_tokens: None,
            tokens_per_second: 100.0,
            trace_id: Some("abc123".to_string()),
            metadata: None,
        };

        let nerdstats = workflow_nerdstats(&output);
        let obj = nerdstats
            .as_object()
            .expect("nerdstats should be an object");

        let required_keys = [
            "workflow_id",
            "terminal_node",
            "total_elapsed_ms",
            "ttft_ms",
            "step_details",
            "total_input_tokens",
            "total_output_tokens",
            "total_tokens",
            "total_reasoning_tokens",
            "tokens_per_second",
            "trace_id",
            "token_metrics_available",
            "token_metrics_source",
            "llm_nodes_without_usage",
        ];
        for key in required_keys {
            assert!(
                obj.contains_key(key),
                "nerdstats missing expected key '{key}'"
            );
        }
    }
}
