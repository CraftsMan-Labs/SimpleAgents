use std::collections::{HashMap, HashSet};

use serde_json::Value;

use super::*;

pub(crate) fn validate_json_schema(schema: &Value) -> Result<(), String> {
    jsonschema::JSONSchema::compile(schema)
        .map(|_| ())
        .map_err(|error| format!("invalid JSON schema: {error}"))
}

pub(crate) fn validate_schema_instance(schema: &Value, instance: &Value) -> Result<(), String> {
    let validator = jsonschema::JSONSchema::compile(schema)
        .map_err(|error| format!("invalid JSON schema: {error}"))?;
    if let Err(errors) = validator.validate(instance) {
        let message = errors
            .into_iter()
            .next()
            .map(|error| error.to_string())
            .unwrap_or_else(|| "unknown schema validation error".to_string());
        return Err(format!("schema validation failed: {message}"));
    }
    Ok(())
}

pub(crate) fn schema_type(schema: &Value) -> Option<&str> {
    schema.get("type").and_then(Value::as_str)
}

pub(crate) fn schema_expects_object(schema: &Value) -> bool {
    schema_type(schema) == Some("object")
}

pub fn verify_yaml_workflow(workflow: &YamlWorkflow) -> Vec<YamlWorkflowDiagnostic> {
    let mut diagnostics = Vec::new();
    let known_ids: HashMap<&str, &YamlNode> = workflow
        .nodes
        .iter()
        .map(|node| (node.id.as_str(), node))
        .collect();

    if !known_ids.contains_key(workflow.entry_node.as_str()) {
        diagnostics.push(YamlWorkflowDiagnostic {
            node_id: None,
            code: "missing_entry".to_string(),
            severity: YamlWorkflowDiagnosticSeverity::Error,
            message: format!("entry node '{}' does not exist", workflow.entry_node),
        });
    }

    for edge in &workflow.edges {
        if !known_ids.contains_key(edge.from.as_str()) {
            diagnostics.push(YamlWorkflowDiagnostic {
                node_id: Some(edge.from.clone()),
                code: "unknown_edge_from".to_string(),
                severity: YamlWorkflowDiagnosticSeverity::Error,
                message: format!("edge.from '{}' does not exist", edge.from),
            });
        }
        if !known_ids.contains_key(edge.to.as_str()) {
            diagnostics.push(YamlWorkflowDiagnostic {
                node_id: Some(edge.to.clone()),
                code: "unknown_edge_to".to_string(),
                severity: YamlWorkflowDiagnosticSeverity::Error,
                message: format!("edge.to '{}' does not exist", edge.to),
            });
        }
    }

    for node in &workflow.nodes {
        if let Some(llm) = &node.node_type.llm_call {
            if llm.model.trim().is_empty() {
                diagnostics.push(YamlWorkflowDiagnostic {
                    node_id: Some(node.id.clone()),
                    code: "empty_model".to_string(),
                    severity: YamlWorkflowDiagnosticSeverity::Error,
                    message: "llm_call.model must not be empty".to_string(),
                });
            }
            if llm.stream.unwrap_or(false) && llm.heal.unwrap_or(false) {
                diagnostics.push(YamlWorkflowDiagnostic {
                    node_id: Some(node.id.clone()),
                    code: "stream_heal_conflict".to_string(),
                    severity: YamlWorkflowDiagnosticSeverity::Warning,
                    message:
                        "llm_call.stream=true with heal=true is not streamable; runtime will disable streaming"
                            .to_string(),
                });
            }

            if llm.max_tool_roundtrips.unwrap_or(1) == 0 {
                diagnostics.push(YamlWorkflowDiagnostic {
                    node_id: Some(node.id.clone()),
                    code: "invalid_max_tool_roundtrips".to_string(),
                    severity: YamlWorkflowDiagnosticSeverity::Error,
                    message: "llm_call.max_tool_roundtrips must be >= 1".to_string(),
                });
            }

            if let Some(global_key) = llm.tool_calls_global_key.as_ref() {
                if global_key.trim().is_empty() {
                    diagnostics.push(YamlWorkflowDiagnostic {
                        node_id: Some(node.id.clone()),
                        code: "empty_tool_calls_global_key".to_string(),
                        severity: YamlWorkflowDiagnosticSeverity::Error,
                        message: "llm_call.tool_calls_global_key must not be empty".to_string(),
                    });
                }
            }

            match normalize_tool_choice(llm.tool_choice.clone()) {
                Ok(choice) => {
                    if let Some(ToolChoice::Tool(choice_tool)) = choice.as_ref() {
                        if !llm.tools.iter().any(|tool| match (llm.tools_format, tool) {
                            (YamlToolFormat::Openai, YamlToolDeclaration::OpenAi(openai)) => {
                                openai.function.name == choice_tool.function.name
                            }
                            (
                                YamlToolFormat::Simplified,
                                YamlToolDeclaration::Simplified(simple),
                            ) => simple.name == choice_tool.function.name,
                            _ => false,
                        }) {
                            diagnostics.push(YamlWorkflowDiagnostic {
                                node_id: Some(node.id.clone()),
                                code: "unknown_tool_choice_function".to_string(),
                                severity: YamlWorkflowDiagnosticSeverity::Error,
                                message: format!(
                                    "llm_call.tool_choice references unknown function '{}'",
                                    choice_tool.function.name
                                ),
                            });
                        }
                    }
                }
                Err(message) => {
                    diagnostics.push(YamlWorkflowDiagnostic {
                        node_id: Some(node.id.clone()),
                        code: "invalid_tool_choice".to_string(),
                        severity: YamlWorkflowDiagnosticSeverity::Error,
                        message,
                    });
                }
            }

            let normalized_tools = match normalize_llm_tools(llm) {
                Ok(tools) => tools,
                Err(message) => {
                    diagnostics.push(YamlWorkflowDiagnostic {
                        node_id: Some(node.id.clone()),
                        code: "invalid_tools_format".to_string(),
                        severity: YamlWorkflowDiagnosticSeverity::Error,
                        message,
                    });
                    Vec::new()
                }
            };

            let mut seen_tool_names = HashSet::new();
            for tool in &normalized_tools {
                let name = tool.definition.function.name.trim();
                if name.is_empty() {
                    diagnostics.push(YamlWorkflowDiagnostic {
                        node_id: Some(node.id.clone()),
                        code: "empty_tool_name".to_string(),
                        severity: YamlWorkflowDiagnosticSeverity::Error,
                        message: "tool function name must not be empty".to_string(),
                    });
                }
                if !seen_tool_names.insert(tool.definition.function.name.clone()) {
                    diagnostics.push(YamlWorkflowDiagnostic {
                        node_id: Some(node.id.clone()),
                        code: "duplicate_tool_name".to_string(),
                        severity: YamlWorkflowDiagnosticSeverity::Error,
                        message: format!(
                            "duplicate tool function name '{}' in node",
                            tool.definition.function.name
                        ),
                    });
                }

                let schema = tool
                    .definition
                    .function
                    .parameters
                    .clone()
                    .unwrap_or(Value::Null);
                if schema.is_null() {
                    diagnostics.push(YamlWorkflowDiagnostic {
                        node_id: Some(node.id.clone()),
                        code: "missing_tool_input_schema".to_string(),
                        severity: YamlWorkflowDiagnosticSeverity::Error,
                        message: format!(
                            "tool '{}' is missing input schema",
                            tool.definition.function.name
                        ),
                    });
                } else if let Err(message) = validate_json_schema(&schema) {
                    diagnostics.push(YamlWorkflowDiagnostic {
                        node_id: Some(node.id.clone()),
                        code: "invalid_tool_input_schema".to_string(),
                        severity: YamlWorkflowDiagnosticSeverity::Error,
                        message: format!(
                            "tool '{}' has invalid input schema: {}",
                            tool.definition.function.name, message
                        ),
                    });
                }

                if let Some(output_schema) = tool.output_schema.as_ref() {
                    if let Err(message) = validate_json_schema(output_schema) {
                        diagnostics.push(YamlWorkflowDiagnostic {
                            node_id: Some(node.id.clone()),
                            code: "invalid_tool_output_schema".to_string(),
                            severity: YamlWorkflowDiagnosticSeverity::Error,
                            message: format!(
                                "tool '{}' has invalid output schema: {}",
                                tool.definition.function.name, message
                            ),
                        });
                    }
                }
            }
        }

        if let Some(switch) = &node.node_type.switch {
            for branch in &switch.branches {
                if !known_ids.contains_key(branch.target.as_str()) {
                    diagnostics.push(YamlWorkflowDiagnostic {
                        node_id: Some(node.id.clone()),
                        code: "unknown_switch_target".to_string(),
                        severity: YamlWorkflowDiagnosticSeverity::Error,
                        message: format!("switch branch target '{}' does not exist", branch.target),
                    });
                }
            }
            if !known_ids.contains_key(switch.default.as_str()) {
                diagnostics.push(YamlWorkflowDiagnostic {
                    node_id: Some(node.id.clone()),
                    code: "unknown_switch_default".to_string(),
                    severity: YamlWorkflowDiagnosticSeverity::Error,
                    message: format!("switch default target '{}' does not exist", switch.default),
                });
            }
        }

        if let Some(config) = node.config.as_ref() {
            if let Some(update_globals) = config.update_globals.as_ref() {
                for (key, update) in update_globals {
                    let is_valid_op =
                        matches!(update.op.as_str(), "set" | "append" | "increment" | "merge");
                    if !is_valid_op {
                        diagnostics.push(YamlWorkflowDiagnostic {
                            node_id: Some(node.id.clone()),
                            code: "unknown_update_op".to_string(),
                            severity: YamlWorkflowDiagnosticSeverity::Error,
                            message: format!(
                                "update_globals key '{}' has unknown op '{}'; expected set|append|increment|merge",
                                key, update.op
                            ),
                        });
                    }

                    if update.op != "increment" && update.from.is_none() {
                        diagnostics.push(YamlWorkflowDiagnostic {
                            node_id: Some(node.id.clone()),
                            code: "missing_update_from".to_string(),
                            severity: YamlWorkflowDiagnosticSeverity::Error,
                            message: format!(
                                "update_globals key '{}' with op '{}' requires 'from'",
                                key, update.op
                            ),
                        });
                    }
                }
            }
        }
    }

    diagnostics
}
