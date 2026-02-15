use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::time::Instant;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use simple_agent_type::message::Message;
use simple_agent_type::request::CompletionRequest;
use simple_agents_core::{CompletionOptions, CompletionOutcome, SimpleAgentsClient};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct YamlStepTiming {
    pub node_id: String,
    pub node_kind: String,
    pub elapsed_ms: u128,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct YamlWorkflowRunOutput {
    pub workflow_id: String,
    pub entry_node: String,
    pub email_text: String,
    pub trace: Vec<String>,
    pub outputs: BTreeMap<String, Value>,
    pub terminal_node: String,
    pub terminal_output: Option<Value>,
    pub step_timings: Vec<YamlStepTiming>,
    pub total_elapsed_ms: u128,
}

#[derive(Debug, Error)]
pub enum YamlWorkflowRunError {
    #[error("failed to read workflow yaml '{path}': {source}")]
    Read {
        path: String,
        source: std::io::Error,
    },
    #[error("failed to parse workflow yaml '{path}': {source}")]
    Parse {
        path: String,
        source: serde_yaml::Error,
    },
    #[error("workflow '{workflow_id}' has no nodes")]
    EmptyNodes { workflow_id: String },
    #[error("entry node '{entry_node}' does not exist")]
    MissingEntry { entry_node: String },
    #[error("unknown node id '{node_id}'")]
    MissingNode { node_id: String },
    #[error("unsupported node type in '{node_id}'")]
    UnsupportedNodeType { node_id: String },
    #[error("unsupported switch condition format: {condition}")]
    UnsupportedCondition { condition: String },
    #[error("switch node '{node_id}' has no valid next target")]
    InvalidSwitchTarget { node_id: String },
    #[error("llm schema mapping not defined for node '{node_id}'")]
    MissingLlmSchema { node_id: String },
    #[error("llm returned non-object payload for node '{node_id}'")]
    LlmPayloadNotObject { node_id: String },
    #[error("custom worker handler '{handler}' is not supported")]
    UnsupportedCustomHandler { handler: String },
    #[error("llm execution failed for node '{node_id}': {message}")]
    Llm {
        node_id: String,
        message: String,
    },
}

#[async_trait]
pub trait YamlWorkflowLlmExecutor: Send + Sync {
    async fn complete_structured(
        &self,
        model: &str,
        prompt: &str,
        schema: &Value,
    ) -> Result<Value, String>;
}

pub async fn run_email_workflow_yaml_file(
    workflow_path: &Path,
    email_text: &str,
    executor: &dyn YamlWorkflowLlmExecutor,
) -> Result<YamlWorkflowRunOutput, YamlWorkflowRunError> {
    let contents = std::fs::read_to_string(workflow_path).map_err(|source| YamlWorkflowRunError::Read {
        path: workflow_path.display().to_string(),
        source,
    })?;

    let workflow: YamlWorkflow =
        serde_yaml::from_str(&contents).map_err(|source| YamlWorkflowRunError::Parse {
            path: workflow_path.display().to_string(),
            source,
        })?;

    run_email_workflow_yaml(&workflow, email_text, executor).await
}

pub async fn run_email_workflow_yaml_file_with_client(
    workflow_path: &Path,
    email_text: &str,
    client: &SimpleAgentsClient,
) -> Result<YamlWorkflowRunOutput, YamlWorkflowRunError> {
    let contents = std::fs::read_to_string(workflow_path).map_err(|source| YamlWorkflowRunError::Read {
        path: workflow_path.display().to_string(),
        source,
    })?;

    let workflow: YamlWorkflow =
        serde_yaml::from_str(&contents).map_err(|source| YamlWorkflowRunError::Parse {
            path: workflow_path.display().to_string(),
            source,
        })?;

    run_email_workflow_yaml_with_client(&workflow, email_text, client).await
}

pub async fn run_email_workflow_yaml_with_client(
    workflow: &YamlWorkflow,
    email_text: &str,
    client: &SimpleAgentsClient,
) -> Result<YamlWorkflowRunOutput, YamlWorkflowRunError> {
    struct BorrowedClientExecutor<'a> {
        client: &'a SimpleAgentsClient,
    }

    #[async_trait]
    impl<'a> YamlWorkflowLlmExecutor for BorrowedClientExecutor<'a> {
        async fn complete_structured(
            &self,
            model: &str,
            prompt: &str,
            _schema: &Value,
        ) -> Result<Value, String> {
            let request = CompletionRequest::builder()
                .model(model)
                .messages(vec![
                    Message::system("You execute workflow classification steps."),
                    Message::user(prompt),
                ])
                .build()
                .map_err(|error| format!("failed to build completion request: {error}"))?;

            let outcome = self
                .client
                .complete(&request, CompletionOptions::default())
                .await
                .map_err(|error| error.to_string())?;

            let response = match outcome {
                CompletionOutcome::Response(response) => response,
                CompletionOutcome::Stream(_) => {
                    return Err("streaming completion returned for structured run".to_string())
                }
                CompletionOutcome::HealedJson(healed) => healed.response,
                CompletionOutcome::CoercedSchema(coerced) => coerced.response,
            };

            let content = response
                .content()
                .ok_or_else(|| "completion returned empty content".to_string())?;
            serde_json::from_str(content)
                .map_err(|error| format!("failed to parse structured completion JSON: {error}"))
        }
    }

    let executor = BorrowedClientExecutor { client };
    run_email_workflow_yaml(workflow, email_text, &executor).await
}

pub async fn run_email_workflow_yaml(
    workflow: &YamlWorkflow,
    email_text: &str,
    executor: &dyn YamlWorkflowLlmExecutor,
) -> Result<YamlWorkflowRunOutput, YamlWorkflowRunError> {
    if workflow.nodes.is_empty() {
        return Err(YamlWorkflowRunError::EmptyNodes {
            workflow_id: workflow.id.clone(),
        });
    }

    let node_map: HashMap<&str, &YamlNode> =
        workflow.nodes.iter().map(|node| (node.id.as_str(), node)).collect();
    if !node_map.contains_key(workflow.entry_node.as_str()) {
        return Err(YamlWorkflowRunError::MissingEntry {
            entry_node: workflow.entry_node.clone(),
        });
    }

    let edge_map: HashMap<&str, &str> = workflow
        .edges
        .iter()
        .map(|edge| (edge.from.as_str(), edge.to.as_str()))
        .collect();

    let mut current = workflow.entry_node.clone();
    let mut trace = Vec::new();
    let mut outputs: BTreeMap<String, Value> = BTreeMap::new();
    let mut step_timings = Vec::new();
    let started = Instant::now();

    loop {
        let node = *node_map
            .get(current.as_str())
            .ok_or_else(|| YamlWorkflowRunError::MissingNode {
                node_id: current.clone(),
            })?;

        trace.push(node.id.clone());
        let step_started = Instant::now();

        let next = if let Some(llm) = &node.node_type.llm_call {
            let prompt_template = node
                .config
                .as_ref()
                .and_then(|cfg| cfg.prompt.as_deref())
                .unwrap_or_default();
            let prompt = prompt_template.replace("{{ input.email_text }}", email_text);
            let schema = schema_for_node(node.id.as_str())
                .ok_or_else(|| YamlWorkflowRunError::MissingLlmSchema {
                    node_id: node.id.clone(),
                })?;

            let payload = executor
                .complete_structured(llm.model.as_str(), prompt.as_str(), &schema)
                .await
                .map_err(|message| YamlWorkflowRunError::Llm {
                    node_id: node.id.clone(),
                    message,
                })?;

            if !payload.is_object() {
                return Err(YamlWorkflowRunError::LlmPayloadNotObject {
                    node_id: node.id.clone(),
                });
            }

            outputs.insert(node.id.clone(), json!({ "output": payload }));
            edge_map.get(node.id.as_str()).map(|value| value.to_string())
        } else if let Some(switch) = &node.node_type.switch {
            let context = json!({ "input": { "email_text": email_text }, "nodes": outputs });
            let mut chosen = Some(switch.default.clone());
            for branch in &switch.branches {
                if evaluate_switch_condition(branch.condition.as_str(), &context)? {
                    chosen = Some(branch.target.clone());
                    break;
                }
            }
            let chosen = chosen.ok_or_else(|| YamlWorkflowRunError::InvalidSwitchTarget {
                node_id: node.id.clone(),
            })?;
            Some(chosen)
        } else if let Some(custom) = &node.node_type.custom_worker {
            if custom.handler != "GetRagData" {
                return Err(YamlWorkflowRunError::UnsupportedCustomHandler {
                    handler: custom.handler.clone(),
                });
            }

            let topic = node
                .config
                .as_ref()
                .and_then(|cfg| cfg.payload.as_ref())
                .and_then(|payload| payload.get("topic"))
                .and_then(Value::as_str)
                .unwrap_or("clarification");
            outputs.insert(node.id.clone(), json!({ "output": mock_rag(topic) }));
            None
        } else {
            return Err(YamlWorkflowRunError::UnsupportedNodeType {
                node_id: node.id.clone(),
            });
        };

        let node_kind = node.kind_name().to_string();
        step_timings.push(YamlStepTiming {
            node_id: node.id.clone(),
            node_kind,
            elapsed_ms: step_started.elapsed().as_millis(),
        });

        if let Some(next) = next {
            current = next;
            continue;
        }
        break;
    }

    let terminal_node = trace
        .last()
        .cloned()
        .ok_or_else(|| YamlWorkflowRunError::EmptyNodes {
            workflow_id: workflow.id.clone(),
        })?;

    let terminal_output = outputs
        .get(terminal_node.as_str())
        .and_then(|value| value.get("output"))
        .cloned();

    Ok(YamlWorkflowRunOutput {
        workflow_id: workflow.id.clone(),
        entry_node: workflow.entry_node.clone(),
        email_text: email_text.to_string(),
        trace,
        outputs,
        terminal_node,
        terminal_output,
        step_timings,
        total_elapsed_ms: started.elapsed().as_millis(),
    })
}

fn evaluate_switch_condition(condition: &str, context: &Value) -> Result<bool, YamlWorkflowRunError> {
    let (left, right) = condition
        .split_once("==")
        .ok_or_else(|| YamlWorkflowRunError::UnsupportedCondition {
            condition: condition.to_string(),
        })?;

    let left_path = left.trim().trim_start_matches("$.");
    let right_literal = right
        .trim()
        .trim_matches('"')
        .trim_matches('\'');
    let left_value = resolve_path(context, left_path);
    Ok(left_value
        .and_then(Value::as_str)
        .map(|value| value == right_literal)
        .unwrap_or(false))
}

fn resolve_path<'a>(value: &'a Value, path: &str) -> Option<&'a Value> {
    path.split('.')
        .filter(|segment| !segment.is_empty())
        .try_fold(value, |current, segment| current.get(segment))
}

fn schema_for_node(node_id: &str) -> Option<Value> {
    if node_id == "classify_top_level" {
        return Some(json!({
            "type": "object",
            "properties": {
                "category": {
                    "type": "string",
                    "enum": [
                        "probation",
                        "termination",
                        "leave_request",
                        "supply_chain_request",
                        "clarification"
                    ]
                },
                "reason": { "type": "string" }
            },
            "required": ["category", "reason"],
            "additionalProperties": false
        }));
    }

    if node_id == "classify_supply_chain_subtype" {
        return Some(json!({
            "type": "object",
            "properties": {
                "subtype": {
                    "type": "string",
                    "enum": ["order_assessment", "order_replacement", "clarification"]
                },
                "reason": { "type": "string" }
            },
            "required": ["subtype", "reason"],
            "additionalProperties": false
        }));
    }

    if node_id == "classify_termination_subtype" {
        return Some(json!({
            "type": "object",
            "properties": {
                "subtype": {
                    "type": "string",
                    "enum": ["first_time_offense", "repeated_offense", "clarification"]
                },
                "reason": { "type": "string" }
            },
            "required": ["subtype", "reason"],
            "additionalProperties": false
        }));
    }

    None
}

fn mock_rag(topic: &str) -> Value {
    let (kb_source, playbook) = match topic {
        "probation" => (
            "hr_policy/probation.md",
            "Collect manager review, performance evidence, and probation timeline.",
        ),
        "leave_request" => (
            "hr_policy/leave.md",
            "Validate leave balance, manager approval, and blackout dates.",
        ),
        "supply_chain_order_assessment" => (
            "supply_chain/order_assessment.md",
            "Review order specs, inventory risk, and vendor lead-time guidance.",
        ),
        "supply_chain_order_replacement" => (
            "supply_chain/order_replacement.md",
            "Collect order id, damage proof, and replacement SLA policy.",
        ),
        "termination_first_time_offense" => (
            "hr_policy/termination_first_offense.md",
            "Validate first-incident criteria and route to HRBP review.",
        ),
        "termination_repeated_offense" => (
            "hr_policy/termination_repeated_offense.md",
            "Collect prior warnings and escalation approvals before final action.",
        ),
        _ => (
            "shared/request_clarification.md",
            "Request clarifying details before routing.",
        ),
    };

    json!({
        "kb_source": kb_source,
        "playbook": playbook,
    })
}

#[derive(Debug, Clone, Deserialize)]
pub struct YamlWorkflow {
    pub id: String,
    pub entry_node: String,
    #[serde(default)]
    pub nodes: Vec<YamlNode>,
    #[serde(default)]
    pub edges: Vec<YamlEdge>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct YamlNode {
    pub id: String,
    pub node_type: YamlNodeType,
    pub config: Option<YamlNodeConfig>,
}

impl YamlNode {
    fn kind_name(&self) -> &'static str {
        if self.node_type.llm_call.is_some() {
            "llm_call"
        } else if self.node_type.switch.is_some() {
            "switch"
        } else if self.node_type.custom_worker.is_some() {
            "custom_worker"
        } else {
            "unknown"
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct YamlNodeType {
    pub llm_call: Option<YamlLlmCall>,
    pub switch: Option<YamlSwitch>,
    pub custom_worker: Option<YamlCustomWorker>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct YamlLlmCall {
    pub model: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct YamlSwitch {
    #[serde(default)]
    pub branches: Vec<YamlSwitchBranch>,
    pub default: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct YamlSwitchBranch {
    pub condition: String,
    pub target: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct YamlCustomWorker {
    pub handler: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct YamlNodeConfig {
    pub prompt: Option<String>,
    pub payload: Option<Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct YamlEdge {
    pub from: String,
    pub to: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockExecutor;

    #[async_trait]
    impl YamlWorkflowLlmExecutor for MockExecutor {
        async fn complete_structured(
            &self,
            _model: &str,
            prompt: &str,
            _schema: &Value,
        ) -> Result<Value, String> {
            if prompt.contains("exactly one category") {
                return Ok(json!({"category":"termination","reason":"mock"}));
            }
            if prompt.contains("Determine termination subtype") {
                return Ok(json!({"subtype":"repeated_offense","reason":"mock"}));
            }
            if prompt.contains("Determine supply chain subtype") {
                return Ok(json!({"subtype":"order_replacement","reason":"mock"}));
            }
            Err("unexpected prompt".to_string())
        }
    }

    #[tokio::test]
    async fn runs_yaml_workflow_and_returns_step_timings() {
        let yaml = r#"
id: email-intake-classification
entry_node: classify_top_level
nodes:
  - id: classify_top_level
    node_type:
      llm_call:
        model: gpt-4.1
    config:
      prompt: |
        Classify this email into exactly one category:
        {{ input.email_text }}
  - id: route_top_level
    node_type:
      switch:
        branches:
          - condition: '$.nodes.classify_top_level.output.category == "termination"'
            target: classify_termination_subtype
        default: rag_clarification
  - id: classify_termination_subtype
    node_type:
      llm_call:
        model: gpt-4.1
    config:
      prompt: |
        Determine termination subtype:
        {{ input.email_text }}
  - id: route_termination_subtype
    node_type:
      switch:
        branches:
          - condition: '$.nodes.classify_termination_subtype.output.subtype == "repeated_offense"'
            target: rag_termination_repeated_offense
        default: rag_clarification
  - id: rag_termination_repeated_offense
    node_type:
      custom_worker:
        handler: GetRagData
    config:
      payload:
        topic: termination_repeated_offense
edges:
  - from: classify_top_level
    to: route_top_level
  - from: classify_termination_subtype
    to: route_termination_subtype
"#;

        let workflow: YamlWorkflow = serde_yaml::from_str(yaml).expect("yaml should parse");
        let output = run_email_workflow_yaml(&workflow, "test", &MockExecutor)
            .await
            .expect("yaml workflow should execute");

        assert_eq!(output.workflow_id, "email-intake-classification");
        assert_eq!(output.terminal_node, "rag_termination_repeated_offense");
        assert!(!output.step_timings.is_empty());
        assert_eq!(output.step_timings.len(), output.trace.len());
        assert!(output.outputs.contains_key("rag_termination_repeated_offense"));
    }
}
