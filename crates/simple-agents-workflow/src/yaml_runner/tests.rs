use std::path::Path;

use serde_json::{json, Value};

// Bring in all `pub` items from yaml_runner (contracts, events, output, types, etc.)
use super::*;
// Items that are pub(crate) or pub(super) — must be imported explicitly.
use super::context::interpolate_template;
use super::dispatch_yaml_workflow_execution;
use super::execute;
use super::loader::load_workflow_yaml_file;
use super::types::{
    YamlLlmExecutionResult, YamlLlmTokenUsage, YamlWorkflowRunOptions, YamlWorkflowRunOutput,
    YamlWorkflowTelemetryConfig, YamlWorkflowTraceContextInput, YamlWorkflowTraceOptions,
    YamlWorkflowTraceTenantContext,
};
use async_trait::async_trait;
use simple_agent_type::message::{Message, Role};
use simple_agent_type::provider::{Provider, ProviderRequest, ProviderResponse};
use simple_agent_type::request::CompletionRequest;
use simple_agent_type::response::{CompletionChoice, CompletionResponse, FinishReason, Usage};
use simple_agent_type::tool::{ToolCall, ToolCallFunction, ToolType};
use simple_agent_type::{Result as SaResult, SimpleAgentsError};

// ---------------------------------------------------------------------------
// Test helpers — thin wrappers over internal impls so test call-sites don't
// need to construct a full YamlWorkflowExecutionRequest.
// ---------------------------------------------------------------------------

async fn run_workflow_yaml(
    workflow: &YamlWorkflow,
    workflow_input: &Value,
    executor: &dyn YamlWorkflowLlmExecutor,
) -> Result<YamlWorkflowRunOutput, YamlWorkflowRunError> {
    execute::run_workflow_yaml_with_custom_worker_and_events_and_options_impl(
        workflow,
        workflow_input,
        executor,
        None,
        None,
        &YamlWorkflowRunOptions::default(),
        YamlWorkflowExecutionFlags::default(),
    )
    .await
}

async fn run_workflow_yaml_with_custom_worker_and_events_and_options(
    workflow: &YamlWorkflow,
    workflow_input: &Value,
    executor: &dyn YamlWorkflowLlmExecutor,
    custom_worker: Option<&dyn YamlWorkflowCustomWorkerExecutor>,
    event_sink: Option<&dyn YamlWorkflowEventSink>,
    options: &YamlWorkflowRunOptions,
    flags: YamlWorkflowExecutionFlags,
) -> Result<YamlWorkflowRunOutput, YamlWorkflowRunError> {
    execute::run_workflow_yaml_with_custom_worker_and_events_and_options_impl(
        workflow,
        workflow_input,
        executor,
        custom_worker,
        event_sink,
        options,
        flags,
    )
    .await
}

async fn run_workflow_yaml_with_client_and_custom_worker_and_events_and_options(
    workflow: &YamlWorkflow,
    workflow_input: &Value,
    client: &simple_agents_core::SimpleAgentsClient,
    custom_worker: Option<&dyn YamlWorkflowCustomWorkerExecutor>,
    event_sink: Option<&dyn YamlWorkflowEventSink>,
    options: &YamlWorkflowRunOptions,
    flags: YamlWorkflowExecutionFlags,
) -> Result<YamlWorkflowRunOutput, YamlWorkflowRunError> {
    dispatch_yaml_workflow_execution(
        workflow,
        workflow_input,
        YamlWorkflowExecutorBinding::Client(client),
        custom_worker,
        event_sink,
        options,
        flags,
    )
    .await
}

async fn run_workflow_yaml_file(
    path: &Path,
    workflow_input: &Value,
    executor: &dyn YamlWorkflowLlmExecutor,
) -> Result<YamlWorkflowRunOutput, YamlWorkflowRunError> {
    let (_path, workflow) = load_workflow_yaml_file(path)?;
    execute::run_workflow_yaml_with_custom_worker_and_events_and_options_impl(
        &workflow,
        workflow_input,
        executor,
        None,
        None,
        &YamlWorkflowRunOptions::default(),
        YamlWorkflowExecutionFlags::default(),
    )
    .await
}

use simple_agents_core::SimpleAgentsClient;
use std::fs;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

struct MockExecutor;

struct RecordingSink {
    events: Mutex<Vec<YamlWorkflowEvent>>,
}

struct CancelAfterFirstEventSink {
    cancelled: AtomicBool,
}

impl YamlWorkflowEventSink for CancelAfterFirstEventSink {
    fn emit(&self, _event: &YamlWorkflowEvent) {
        self.cancelled.store(true, Ordering::SeqCst);
    }

    fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }
}

struct CountingExecutor {
    call_count: AtomicUsize,
}

struct CapturingWorker {
    context: Mutex<Option<Value>>,
}

struct HandlerFileCapturingWorker {
    handler_file: Mutex<Option<String>>,
}

struct ToolLoopProvider;

struct UnknownToolProvider;

struct ReasoningUsageProvider;

struct ToolLoopReasoningProvider;

#[async_trait]
impl Provider for ToolLoopProvider {
    fn name(&self) -> &str {
        "openai"
    }

    fn transform_request(&self, req: &CompletionRequest) -> SaResult<ProviderRequest> {
        let body = serde_json::to_value(req).map_err(SimpleAgentsError::from)?;
        Ok(ProviderRequest::new("mock://tool-loop").with_body(body))
    }

    async fn execute(&self, req: ProviderRequest) -> SaResult<ProviderResponse> {
        let request: CompletionRequest =
            serde_json::from_value(req.body).map_err(SimpleAgentsError::from)?;

        let has_tools = request
            .tools
            .as_ref()
            .is_some_and(|tools| !tools.is_empty());
        let has_tool_result = request.messages.iter().any(|m| m.role == Role::Tool);

        let response = if has_tools && !has_tool_result {
            CompletionResponse {
                id: "resp_tool_1".to_string(),
                model: request.model.clone(),
                choices: vec![CompletionChoice {
                    index: 0,
                    message: Message::assistant("").with_tool_calls(vec![ToolCall {
                        id: "call_get_context".to_string(),
                        tool_type: ToolType::Function,
                        function: ToolCallFunction {
                            name: "get_customer_context".to_string(),
                            arguments: "{\"order_id\":\"123\"}".to_string(),
                        },
                    }]),
                    finish_reason: FinishReason::ToolCalls,
                    logprobs: None,
                }],
                usage: Usage::new(10, 5),
                created: None,
                provider: Some(self.name().to_string()),
                healing_metadata: None,
            }
        } else if has_tools && has_tool_result {
            CompletionResponse {
                id: "resp_tool_2".to_string(),
                model: request.model.clone(),
                choices: vec![CompletionChoice {
                    index: 0,
                    message: Message::assistant("{\"state\":\"done\"}"),
                    finish_reason: FinishReason::Stop,
                    logprobs: None,
                }],
                usage: Usage::new(12, 6),
                created: None,
                provider: Some(self.name().to_string()),
                healing_metadata: None,
            }
        } else {
            let prompt = request
                .messages
                .iter()
                .rev()
                .find(|m| m.role == Role::User)
                .map(|m| m.content_text().to_string())
                .unwrap_or_default();
            let payload = json!({
                "subject": "ok",
                "body": prompt,
            })
            .to_string();
            CompletionResponse {
                id: "resp_final".to_string(),
                model: request.model.clone(),
                choices: vec![CompletionChoice {
                    index: 0,
                    message: Message::assistant(payload),
                    finish_reason: FinishReason::Stop,
                    logprobs: None,
                }],
                usage: Usage::new(8, 4),
                created: None,
                provider: Some(self.name().to_string()),
                healing_metadata: None,
            }
        };

        let body = serde_json::to_value(response).map_err(SimpleAgentsError::from)?;
        Ok(ProviderResponse::new(200, body))
    }

    fn transform_response(&self, resp: ProviderResponse) -> SaResult<CompletionResponse> {
        serde_json::from_value(resp.body).map_err(SimpleAgentsError::from)
    }
}

#[async_trait]
impl Provider for UnknownToolProvider {
    fn name(&self) -> &str {
        "openai"
    }

    fn transform_request(&self, req: &CompletionRequest) -> SaResult<ProviderRequest> {
        let body = serde_json::to_value(req).map_err(SimpleAgentsError::from)?;
        Ok(ProviderRequest::new("mock://unknown-tool").with_body(body))
    }

    async fn execute(&self, req: ProviderRequest) -> SaResult<ProviderResponse> {
        let request: CompletionRequest =
            serde_json::from_value(req.body).map_err(SimpleAgentsError::from)?;

        let response = CompletionResponse {
            id: "resp_unknown_tool".to_string(),
            model: request.model,
            choices: vec![CompletionChoice {
                index: 0,
                message: Message::assistant("").with_tool_calls(vec![ToolCall {
                    id: "call_unknown".to_string(),
                    tool_type: ToolType::Function,
                    function: ToolCallFunction {
                        name: "unknown_tool".to_string(),
                        arguments: "{}".to_string(),
                    },
                }]),
                finish_reason: FinishReason::ToolCalls,
                logprobs: None,
            }],
            usage: Usage::new(10, 5),
            created: None,
            provider: Some(self.name().to_string()),
            healing_metadata: None,
        };

        let body = serde_json::to_value(response).map_err(SimpleAgentsError::from)?;
        Ok(ProviderResponse::new(200, body))
    }

    fn transform_response(&self, resp: ProviderResponse) -> SaResult<CompletionResponse> {
        serde_json::from_value(resp.body).map_err(SimpleAgentsError::from)
    }
}

#[async_trait]
impl Provider for ReasoningUsageProvider {
    fn name(&self) -> &str {
        "openai"
    }

    fn transform_request(&self, req: &CompletionRequest) -> SaResult<ProviderRequest> {
        let body = serde_json::to_value(req).map_err(SimpleAgentsError::from)?;
        Ok(ProviderRequest::new("mock://reasoning").with_body(body))
    }

    async fn execute(&self, req: ProviderRequest) -> SaResult<ProviderResponse> {
        let request: CompletionRequest =
            serde_json::from_value(req.body).map_err(SimpleAgentsError::from)?;

        let mut usage = Usage::new(20, 10);
        usage.reasoning_tokens = Some(5);

        let response = CompletionResponse {
            id: "resp_reasoning".to_string(),
            model: request.model,
            choices: vec![CompletionChoice {
                index: 0,
                message: Message::assistant("{\"ok\":true}"),
                finish_reason: FinishReason::Stop,
                logprobs: None,
            }],
            usage,
            created: None,
            provider: Some(self.name().to_string()),
            healing_metadata: None,
        };

        let body = serde_json::to_value(response).map_err(SimpleAgentsError::from)?;
        Ok(ProviderResponse::new(200, body))
    }

    fn transform_response(&self, resp: ProviderResponse) -> SaResult<CompletionResponse> {
        serde_json::from_value(resp.body).map_err(SimpleAgentsError::from)
    }
}

#[async_trait]
impl Provider for ToolLoopReasoningProvider {
    fn name(&self) -> &str {
        "openai"
    }

    fn transform_request(&self, req: &CompletionRequest) -> SaResult<ProviderRequest> {
        let body = serde_json::to_value(req).map_err(SimpleAgentsError::from)?;
        Ok(ProviderRequest::new("mock://tool-loop-reasoning").with_body(body))
    }

    async fn execute(&self, req: ProviderRequest) -> SaResult<ProviderResponse> {
        let request: CompletionRequest =
            serde_json::from_value(req.body).map_err(SimpleAgentsError::from)?;

        let has_tools = request
            .tools
            .as_ref()
            .is_some_and(|tools| !tools.is_empty());
        let has_tool_result = request.messages.iter().any(|m| m.role == Role::Tool);

        let response = if has_tools && !has_tool_result {
            let mut usage = Usage::new(15, 7);
            usage.reasoning_tokens = Some(3);
            CompletionResponse {
                id: "resp_tool_reasoning_1".to_string(),
                model: request.model.clone(),
                choices: vec![CompletionChoice {
                    index: 0,
                    message: Message::assistant("").with_tool_calls(vec![ToolCall {
                        id: "call_reason_tool".to_string(),
                        tool_type: ToolType::Function,
                        function: ToolCallFunction {
                            name: "get_customer_context".to_string(),
                            arguments: "{\"order_id\":\"456\"}".to_string(),
                        },
                    }]),
                    finish_reason: FinishReason::ToolCalls,
                    logprobs: None,
                }],
                usage,
                created: None,
                provider: Some(self.name().to_string()),
                healing_metadata: None,
            }
        } else {
            let mut usage = Usage::new(20, 10);
            usage.reasoning_tokens = Some(4);
            CompletionResponse {
                id: "resp_tool_reasoning_2".to_string(),
                model: request.model.clone(),
                choices: vec![CompletionChoice {
                    index: 0,
                    message: Message::assistant("{\"state\":\"reasoning_done\"}"),
                    finish_reason: FinishReason::Stop,
                    logprobs: None,
                }],
                usage,
                created: None,
                provider: Some(self.name().to_string()),
                healing_metadata: None,
            }
        };

        let body = serde_json::to_value(response).map_err(SimpleAgentsError::from)?;
        Ok(ProviderResponse::new(200, body))
    }

    fn transform_response(&self, resp: ProviderResponse) -> SaResult<CompletionResponse> {
        serde_json::from_value(resp.body).map_err(SimpleAgentsError::from)
    }
}

struct FixedToolWorker {
    payload: Value,
}

struct CountingToolWorker {
    execute_calls: AtomicUsize,
}

#[async_trait]
impl YamlWorkflowCustomWorkerExecutor for FixedToolWorker {
    async fn execute(
        &self,
        _handler: &str,
        _handler_file: Option<&str>,
        _payload: &Value,
        _context: &Value,
    ) -> Result<Value, String> {
        Ok(self.payload.clone())
    }
}

#[async_trait]
impl YamlWorkflowCustomWorkerExecutor for CountingToolWorker {
    async fn execute(
        &self,
        _handler: &str,
        _handler_file: Option<&str>,
        _payload: &Value,
        _context: &Value,
    ) -> Result<Value, String> {
        self.execute_calls.fetch_add(1, Ordering::SeqCst);
        Ok(json!({"ok": true}))
    }
}

#[async_trait]
impl YamlWorkflowLlmExecutor for CountingExecutor {
    async fn complete_structured(
        &self,
        _request: YamlLlmExecutionRequest,
        _event_sink: Option<&dyn YamlWorkflowEventSink>,
    ) -> Result<YamlLlmExecutionResult, String> {
        self.call_count.fetch_add(1, Ordering::SeqCst);
        Ok(YamlLlmExecutionResult {
            payload: json!({"state":"ok"}),
            usage: None,
            ttft_ms: None,
            tool_calls: Vec::new(),
        })
    }
}

impl YamlWorkflowEventSink for RecordingSink {
    fn emit(&self, event: &YamlWorkflowEvent) {
        self.events
            .lock()
            .expect("recording sink lock should not be poisoned")
            .push(event.clone());
    }
}

#[async_trait]
impl YamlWorkflowCustomWorkerExecutor for CapturingWorker {
    async fn execute(
        &self,
        _handler: &str,
        _handler_file: Option<&str>,
        _payload: &Value,
        context: &Value,
    ) -> Result<Value, String> {
        let mut guard = self
            .context
            .lock()
            .map_err(|_| "capturing worker lock should not be poisoned".to_string())?;
        *guard = Some(context.clone());
        Ok(json!({"ok": true}))
    }
}

#[async_trait]
impl YamlWorkflowCustomWorkerExecutor for HandlerFileCapturingWorker {
    async fn execute(
        &self,
        _handler: &str,
        handler_file: Option<&str>,
        _payload: &Value,
        _context: &Value,
    ) -> Result<Value, String> {
        let mut guard = self
            .handler_file
            .lock()
            .map_err(|_| "handler-file lock should not be poisoned".to_string())?;
        *guard = handler_file.map(ToString::to_string);
        Ok(json!({"ok": true}))
    }
}

#[async_trait]
impl YamlWorkflowLlmExecutor for MockExecutor {
    async fn complete_structured(
        &self,
        request: YamlLlmExecutionRequest,
        _event_sink: Option<&dyn YamlWorkflowEventSink>,
    ) -> Result<YamlLlmExecutionResult, String> {
        let prompt = request.prompt;
        if prompt.contains("exactly one category") {
            return Ok(YamlLlmExecutionResult {
                payload: json!({"category":"termination","reason":"mock"}),
                usage: Some(YamlLlmTokenUsage {
                    prompt_tokens: 10,
                    completion_tokens: 5,
                    total_tokens: 15,
                    reasoning_tokens: None,
                }),
                ttft_ms: None,
                tool_calls: Vec::new(),
            });
        }
        if prompt.contains("Determine termination subtype") {
            return Ok(YamlLlmExecutionResult {
                payload: json!({"subtype":"repeated_offense","reason":"mock"}),
                usage: Some(YamlLlmTokenUsage {
                    prompt_tokens: 12,
                    completion_tokens: 6,
                    total_tokens: 18,
                    reasoning_tokens: None,
                }),
                ttft_ms: None,
                tool_calls: Vec::new(),
            });
        }
        if prompt.contains("Determine supply chain subtype") {
            return Ok(YamlLlmExecutionResult {
                payload: json!({"subtype":"order_replacement","reason":"mock"}),
                usage: Some(YamlLlmTokenUsage {
                    prompt_tokens: 11,
                    completion_tokens: 4,
                    total_tokens: 15,
                    reasoning_tokens: None,
                }),
                ttft_ms: None,
                tool_calls: Vec::new(),
            });
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
        Classify the following email into exactly one category:
        - termination
        - supply_chain

        Email:
        {{ input.email_text }}
      output_schema:
        type: object
        properties:
          category:
            type: string
            enum: ["termination", "supply_chain"]
          reason:
            type: string
        required: [category, reason]

  - id: termination_sub_classify
    node_type:
      llm_call:
        model: gpt-4.1
    config:
      prompt: |
        Determine termination subtype for:
        {{ input.email_text }}
      output_schema:
        type: object
        properties:
          subtype:
            type: string
          reason:
            type: string
        required: [subtype, reason]

  - id: supply_chain_sub_classify
    node_type:
      llm_call:
        model: gpt-4.1
    config:
      prompt: |
        Determine supply chain subtype for:
        {{ input.email_text }}
      output_schema:
        type: object
        properties:
          subtype:
            type: string
          reason:
            type: string
        required: [subtype, reason]

  - id: router
    node_type:
      switch:
        branches:
          - condition: '$.nodes.classify_top_level.output.category == "termination"'
            target: termination_sub_classify
        default: supply_chain_sub_classify

edges:
  - from: classify_top_level
    to: router
    "#;

    let workflow: YamlWorkflow = serde_yaml::from_str(yaml).expect("yaml should parse");
    let output = run_workflow_yaml_with_custom_worker_and_events_and_options(
        &workflow,
        &json!({"email_text": "You are terminated."}),
        &MockExecutor,
        None,
        None,
        &YamlWorkflowRunOptions::default(),
        YamlWorkflowExecutionFlags::default(),
    )
    .await
    .expect("workflow should execute");

    assert_eq!(output.workflow_id, "email-intake-classification");
    assert_eq!(output.entry_node, "classify_top_level");
    assert_eq!(output.terminal_node, "termination_sub_classify");
    assert!(output.terminal_output.is_some());
    let _ = output.total_elapsed_ms;
    assert!(!output.step_timings.is_empty());
    for step in &output.step_timings {
        assert!(!step.node_id.is_empty());
        assert!(!step.node_kind.is_empty());
    }
    assert_eq!(output.total_input_tokens, 22);
    assert_eq!(output.total_output_tokens, 11);
    assert_eq!(output.total_tokens, 33);
}

#[tokio::test]
async fn workflow_runner_executes_inline_workflow_with_executor() {
    let yaml = r#"
id: simple
entry_node: classify
nodes:
  - id: classify
    node_type:
      llm_call:
        model: gpt-4.1
    config:
      prompt: "Classify the following email into exactly one category: {{ input.email_text }}"
      output_schema:
        type: object
        properties:
          category: { type: string }
        required: [category]
    "#;

    let workflow: YamlWorkflow = serde_yaml::from_str(yaml).expect("yaml should parse");
    let output = run_workflow_yaml(&workflow, &json!({"email_text": "hello"}), &MockExecutor)
        .await
        .expect("workflow should execute");

    assert_eq!(output.workflow_id, "simple");
    assert_eq!(output.terminal_node, "classify");
    assert!(output.terminal_output.is_some());
}

#[tokio::test]
async fn workflow_runner_requires_executor() {
    let yaml = r#"
id: wf
entry_node: step
nodes:
  - id: step
    node_type:
      custom_worker:
        handler: my_handler
    "#;

    let workflow: YamlWorkflow = serde_yaml::from_str(yaml).expect("yaml should parse");
    let err = run_workflow_yaml_with_custom_worker_and_events_and_options(
        &workflow,
        &json!({"email_text": "hello"}),
        &MockExecutor,
        None,
        None,
        &YamlWorkflowRunOptions::default(),
        YamlWorkflowExecutionFlags::default(),
    )
    .await
    .expect_err("custom_worker without executor should fail");
    let rendered = err.to_string();
    assert!(rendered.contains("no custom worker executor is configured"));
}

#[tokio::test]
async fn emits_resolved_llm_input_event_with_bindings() {
    let yaml = r#"
id: binding-test
entry_node: classify
nodes:
  - id: classify
    node_type:
      llm_call:
        model: gpt-4.1
    config:
      prompt: |
        Classify the following email into exactly one category:
        {{ input.email_text }}
      output_schema:
        type: object
        properties:
          category: { type: string }
        required: [category]
    "#;

    let workflow: YamlWorkflow = serde_yaml::from_str(yaml).expect("yaml should parse");
    let sink = RecordingSink {
        events: Mutex::new(Vec::new()),
    };
    run_workflow_yaml_with_custom_worker_and_events_and_options(
        &workflow,
        &json!({"email_text": "hello"}),
        &MockExecutor,
        None,
        Some(&sink),
        &YamlWorkflowRunOptions::default(),
        YamlWorkflowExecutionFlags::default(),
    )
    .await
    .expect("workflow should execute");

    let events = sink.events.lock().expect("events should be available");
    let resolved_event = events
        .iter()
        .find(|event| event.event_type == "resolved_llm_input")
        .expect("resolved_llm_input event should be emitted");

    let metadata = resolved_event
        .metadata
        .as_ref()
        .expect("event metadata should be present");
    let bindings = metadata
        .get("bindings")
        .and_then(Value::as_array)
        .expect("bindings should be an array");

    assert!(!bindings.is_empty());
    let first_binding = &bindings[0];
    assert_eq!(
        first_binding.get("source_path").and_then(Value::as_str),
        Some("input.email_text")
    );
    assert_eq!(
        first_binding.get("resolved").and_then(Value::as_str),
        Some("hello")
    );
}

#[tokio::test]
async fn workflow_completed_event_includes_nerdstats_by_default() {
    let yaml = r#"
id: nerdstats-test
entry_node: classify
nodes:
  - id: classify
    node_type:
      llm_call:
        model: gpt-4.1
    config:
      prompt: "Classify the following email into exactly one category: {{ input.email_text }}"
      output_schema:
        type: object
        properties:
          category: { type: string }
        required: [category]
    "#;

    let workflow: YamlWorkflow = serde_yaml::from_str(yaml).expect("yaml should parse");
    let sink = RecordingSink {
        events: Mutex::new(Vec::new()),
    };
    run_workflow_yaml_with_custom_worker_and_events_and_options(
        &workflow,
        &json!({"email_text": "hello"}),
        &MockExecutor,
        None,
        Some(&sink),
        &YamlWorkflowRunOptions::default(),
        YamlWorkflowExecutionFlags::default(),
    )
    .await
    .expect("workflow should execute");

    let events = sink.events.lock().expect("events should be available");
    let completed_event = events
        .iter()
        .find(|event| event.event_type == "workflow_completed")
        .expect("workflow_completed event should be emitted");

    let metadata = completed_event
        .metadata
        .as_ref()
        .expect("workflow_completed metadata should be present");
    let nerdstats = metadata
        .get("nerdstats")
        .expect("nerdstats should be present");
    assert!(nerdstats.get("workflow_id").is_some());
    assert!(nerdstats.get("total_elapsed_ms").is_some());
}

#[tokio::test]
async fn workflow_completed_event_omits_nerdstats_when_disabled() {
    let yaml = r#"
id: nerdstats-disabled
entry_node: classify
nodes:
  - id: classify
    node_type:
      llm_call:
        model: gpt-4.1
    config:
      prompt: "Classify the following email into exactly one category: {{ input.email_text }}"
      output_schema:
        type: object
        properties:
          category: { type: string }
        required: [category]
    "#;

    let workflow: YamlWorkflow = serde_yaml::from_str(yaml).expect("yaml should parse");
    let sink = RecordingSink {
        events: Mutex::new(Vec::new()),
    };
    let options = YamlWorkflowRunOptions {
        telemetry: YamlWorkflowTelemetryConfig {
            nerdstats: false,
            ..Default::default()
        },
        ..Default::default()
    };
    run_workflow_yaml_with_custom_worker_and_events_and_options(
        &workflow,
        &json!({"email_text": "hello"}),
        &MockExecutor,
        None,
        Some(&sink),
        &options,
        YamlWorkflowExecutionFlags::default(),
    )
    .await
    .expect("workflow should execute");

    let events = sink.events.lock().expect("events should be available");
    let completed_event = events
        .iter()
        .find(|event| event.event_type == "workflow_completed")
        .expect("workflow_completed event should be emitted");

    let has_nerdstats = completed_event
        .metadata
        .as_ref()
        .and_then(|m| m.get("nerdstats"))
        .is_some();
    assert!(!has_nerdstats);
}

struct StreamAwareMockExecutor;

#[async_trait]
impl YamlWorkflowLlmExecutor for StreamAwareMockExecutor {
    async fn complete_structured(
        &self,
        request: YamlLlmExecutionRequest,
        _event_sink: Option<&dyn YamlWorkflowEventSink>,
    ) -> Result<YamlLlmExecutionResult, String> {
        Ok(YamlLlmExecutionResult {
            payload: json!({"category":"test","reason":"mock"}),
            usage: if request.stream {
                None
            } else {
                Some(YamlLlmTokenUsage {
                    prompt_tokens: 10,
                    completion_tokens: 5,
                    total_tokens: 15,
                    reasoning_tokens: None,
                })
            },
            ttft_ms: None,
            tool_calls: Vec::new(),
        })
    }
}

#[tokio::test]
async fn workflow_completed_event_includes_nerdstats_for_streaming_nodes() {
    let yaml = r#"
id: stream-nerdstats
entry_node: classify
nodes:
  - id: classify
    node_type:
      llm_call:
        model: gpt-4.1
        stream: true
    config:
      prompt: "hello"
      output_schema:
        type: object
        properties:
          category: { type: string }
        required: [category]
    "#;

    let workflow: YamlWorkflow = serde_yaml::from_str(yaml).expect("yaml should parse");
    let sink = RecordingSink {
        events: Mutex::new(Vec::new()),
    };
    run_workflow_yaml_with_custom_worker_and_events_and_options(
        &workflow,
        &json!({"email_text": "hello"}),
        &StreamAwareMockExecutor,
        None,
        Some(&sink),
        &YamlWorkflowRunOptions::default(),
        YamlWorkflowExecutionFlags::default(),
    )
    .await
    .expect("workflow should execute");

    let events = sink.events.lock().expect("events should be available");
    let completed_event = events
        .iter()
        .find(|event| event.event_type == "workflow_completed")
        .expect("workflow_completed event should be emitted");

    let nerdstats = completed_event
        .metadata
        .as_ref()
        .and_then(|meta| meta.get("nerdstats"))
        .expect("nerdstats should be present for streaming");
    assert_eq!(
        nerdstats
            .get("token_metrics_source")
            .and_then(|v| v.as_str()),
        Some("provider_stream_usage_unavailable")
    );
}

struct MessageHistoryExecutor;

#[async_trait]
impl YamlWorkflowLlmExecutor for MessageHistoryExecutor {
    async fn complete_structured(
        &self,
        request: YamlLlmExecutionRequest,
        _event_sink: Option<&dyn YamlWorkflowEventSink>,
    ) -> Result<YamlLlmExecutionResult, String> {
        let messages = request
            .messages
            .as_ref()
            .ok_or("messages should be present")?;
        let count = messages.len();
        Ok(YamlLlmExecutionResult {
            payload: json!({"message_count": count}),
            usage: None,
            ttft_ms: None,
            tool_calls: Vec::new(),
        })
    }
}

#[tokio::test]
async fn supports_messages_path_in_workflow_input() {
    let yaml = r#"
id: messages-path-test
entry_node: classify
nodes:
  - id: classify
    node_type:
      llm_call:
        model: gpt-4.1
        messages_path: input.messages
    config:
      prompt: ""
      output_schema:
        type: object
        properties:
          message_count: { type: integer }
        required: [message_count]
    "#;

    let workflow: YamlWorkflow = serde_yaml::from_str(yaml).expect("yaml should parse");
    let output = run_workflow_yaml_with_custom_worker_and_events_and_options(
        &workflow,
        &json!({
            "messages": [
                {"role":"user","content":"hello"},
                {"role":"assistant","content":"hi"}
            ]
        }),
        &MessageHistoryExecutor,
        None,
        None,
        &YamlWorkflowRunOptions::default(),
        YamlWorkflowExecutionFlags::default(),
    )
    .await
    .expect("workflow should execute");

    assert_eq!(output.terminal_node, "classify");
}

#[tokio::test]
async fn wrapper_entrypoints_produce_equivalent_outputs() {
    let yaml = r#"
id: wrapper-test
entry_node: classify
nodes:
  - id: classify
    node_type:
      llm_call:
        model: gpt-4.1
    config:
      prompt: "Classify the following email into exactly one category: {{ input.email_text }}"
      output_schema:
        type: object
        properties:
          category: { type: string }
        required: [category]
    "#;

    let workflow: YamlWorkflow = serde_yaml::from_str(yaml).expect("yaml should parse");
    let input = json!({"email_text": "hello"});

    let output_1 = run_workflow_yaml(&workflow, &input, &MockExecutor)
        .await
        .expect("run_workflow_yaml should succeed");

    let output_2 = run_workflow_yaml_with_custom_worker_and_events_and_options(
        &workflow,
        &input,
        &MockExecutor,
        None,
        None,
        &YamlWorkflowRunOptions::default(),
        YamlWorkflowExecutionFlags::default(),
    )
    .await
    .expect("full wrapper should succeed");

    assert_eq!(output_1.workflow_id, output_2.workflow_id);
    assert_eq!(output_1.terminal_node, output_2.terminal_node);
}

#[tokio::test]
async fn yaml_llm_tool_calling_captures_traces_and_supports_globals_reference() {
    let yaml = r#"
id: tool-test
entry_node: chat
nodes:
  - id: chat
    node_type:
      llm_call:
        model: gpt-4.1
        tools_format: simplified
        tools:
          - name: get_customer_context
            input_schema:
              type: object
              properties:
                order_id: { type: string }
              required: [order_id]
        max_tool_roundtrips: 2
    config:
      prompt: "Handle: {{ input.email_text }}"
      output_schema:
        type: object
        properties:
          subject: { type: string }
          body: { type: string }
        required: [subject, body]
edges: []
    "#;

    let workflow: YamlWorkflow = serde_yaml::from_str(yaml).expect("yaml should parse");
    let worker = FixedToolWorker {
        payload: json!({"customer_name":"Acme Corp","order_status":"shipped"}),
    };

    let client = SimpleAgentsClient::new(Arc::new(ToolLoopProvider));

    let output = run_workflow_yaml_with_client_and_custom_worker_and_events_and_options(
        &workflow,
        &json!({"email_text": "Where is my order?"}),
        &client,
        Some(&worker),
        None,
        &YamlWorkflowRunOptions::default(),
        YamlWorkflowExecutionFlags::default(),
    )
    .await
    .expect("tool workflow should execute");

    assert_eq!(output.terminal_node, "chat");
    assert!(output.terminal_output.is_some());
    assert!(output.total_input_tokens > 0);
    assert!(output.total_output_tokens > 0);
}

#[tokio::test]
async fn workflow_with_client_preserves_reasoning_tokens_in_output_and_nerdstats() {
    let yaml = r#"
id: reasoning-test
entry_node: classify
nodes:
  - id: classify
    node_type:
      llm_call:
        model: o3-mini
    config:
      prompt: "Classify: {{ input.email_text }}"
      output_schema:
        type: object
        properties:
          ok: { type: boolean }
        required: [ok]
    "#;

    let workflow: YamlWorkflow = serde_yaml::from_str(yaml).expect("yaml should parse");

    let client = SimpleAgentsClient::new(Arc::new(ReasoningUsageProvider));

    let sink = RecordingSink {
        events: Mutex::new(Vec::new()),
    };
    let output = run_workflow_yaml_with_client_and_custom_worker_and_events_and_options(
        &workflow,
        &json!({"email_text": "hello"}),
        &client,
        None,
        Some(&sink),
        &YamlWorkflowRunOptions::default(),
        YamlWorkflowExecutionFlags::default(),
    )
    .await
    .expect("workflow should succeed");

    assert_eq!(output.total_reasoning_tokens, Some(5));
    assert_eq!(output.total_input_tokens, 20);
    assert_eq!(output.total_output_tokens, 10);
    assert_eq!(output.total_tokens, 30);
}

#[tokio::test]
async fn workflow_with_tools_accumulates_reasoning_tokens_across_roundtrips() {
    let yaml = r#"
id: tool-reasoning-test
entry_node: chat
nodes:
  - id: chat
    node_type:
      llm_call:
        model: o3-mini
        tools_format: simplified
        tools:
          - name: get_customer_context
            input_schema:
              type: object
              properties:
                order_id: { type: string }
              required: [order_id]
        max_tool_roundtrips: 2
    config:
      prompt: "Handle: {{ input.email_text }}"
      output_schema:
        type: object
        properties:
          state: { type: string }
        required: [state]
edges: []
    "#;

    let workflow: YamlWorkflow = serde_yaml::from_str(yaml).expect("yaml should parse");
    let worker = FixedToolWorker {
        payload: json!({"info":"ok"}),
    };

    let client = SimpleAgentsClient::new(Arc::new(ToolLoopReasoningProvider));

    let output = run_workflow_yaml_with_client_and_custom_worker_and_events_and_options(
        &workflow,
        &json!({"email_text": "reasoning test"}),
        &client,
        Some(&worker),
        None,
        &YamlWorkflowRunOptions::default(),
        YamlWorkflowExecutionFlags::default(),
    )
    .await
    .expect("tool+reasoning workflow should execute");

    assert_eq!(output.total_reasoning_tokens, Some(7));
    assert_eq!(output.total_input_tokens, 35);
    assert_eq!(output.total_output_tokens, 17);
}

#[tokio::test]
async fn yaml_llm_tool_output_schema_mismatch_hard_fails_node() {
    let yaml = r#"
id: tool-output-mismatch
entry_node: chat
nodes:
  - id: chat
    node_type:
      llm_call:
        model: gpt-4.1
        tools_format: simplified
        tools:
          - name: get_customer_context
            input_schema:
              type: object
              properties:
                order_id: { type: string }
              required: [order_id]
            output_schema:
              type: object
              properties:
                customer_name: { type: string }
                order_status: { type: string }
              required: [customer_name, order_status]
        max_tool_roundtrips: 2
    config:
      prompt: "Handle: {{ input.email_text }}"
      output_schema:
        type: object
        properties:
          state: { type: string }
        required: [state]
edges: []
    "#;

    let workflow: YamlWorkflow = serde_yaml::from_str(yaml).expect("yaml should parse");
    let worker = FixedToolWorker {
        payload: json!({"wrong_key": "wrong_value"}),
    };

    let client = SimpleAgentsClient::new(Arc::new(ToolLoopProvider));

    let err = run_workflow_yaml_with_client_and_custom_worker_and_events_and_options(
        &workflow,
        &json!({"email_text": "mismatch test"}),
        &client,
        Some(&worker),
        None,
        &YamlWorkflowRunOptions::default(),
        YamlWorkflowExecutionFlags::default(),
    )
    .await
    .expect_err("tool output schema mismatch should fail");

    let rendered = err.to_string();
    assert!(rendered.contains("schema validation failed"));
}

#[tokio::test]
async fn yaml_llm_unknown_tool_is_rejected_before_custom_worker_execution() {
    let yaml = r#"
id: unknown-tool-test
entry_node: chat
nodes:
  - id: chat
    node_type:
      llm_call:
        model: gpt-4.1
        tools_format: simplified
        tools:
          - name: known_tool
            input_schema:
              type: object
              properties:
                order_id: { type: string }
              required: [order_id]
        max_tool_roundtrips: 2
    config:
      prompt: "Handle: {{ input.email_text }}"
      output_schema:
        type: object
        properties:
          state: { type: string }
        required: [state]
edges: []
    "#;

    let workflow: YamlWorkflow = serde_yaml::from_str(yaml).expect("yaml should parse");
    let worker_calls = Arc::new(CountingToolWorker {
        execute_calls: AtomicUsize::new(0),
    });

    let client = SimpleAgentsClient::new(Arc::new(UnknownToolProvider));

    let err = run_workflow_yaml_with_client_and_custom_worker_and_events_and_options(
        &workflow,
        &json!({"email_text": "unknown tool test"}),
        &client,
        Some(worker_calls.as_ref()),
        None,
        &YamlWorkflowRunOptions::default(),
        YamlWorkflowExecutionFlags::default(),
    )
    .await
    .expect_err("unknown tool should cause error");

    assert_eq!(worker_calls.execute_calls.load(Ordering::SeqCst), 0);
    let rendered = err.to_string();
    assert!(rendered.contains("unknown_tool"));
}

#[test]
fn validates_tools_format_mismatch() {
    let yaml = r#"
id: mismatch
entry_node: generate
nodes:
  - id: generate
    node_type:
      llm_call:
        model: gpt-4.1
        tools_format: openai
        tools:
          - name: get_customer_context
            input_schema:
              type: object
              properties:
                order_id: { type: string }
              required: [order_id]
    "#;

    let workflow: YamlWorkflow = serde_yaml::from_str(yaml).expect("yaml should parse");
    let diagnostics = verify_yaml_workflow(&workflow);
    assert!(diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "invalid_tools_format"));
}

#[tokio::test]
async fn custom_worker_node_requires_executor() {
    let yaml = r#"
id: worker-requires-executor
entry_node: handler
nodes:
  - id: handler
    node_type:
      custom_worker:
        handler: my_handler
    "#;

    let workflow: YamlWorkflow = serde_yaml::from_str(yaml).expect("yaml should parse");
    let err = run_workflow_yaml_with_custom_worker_and_events_and_options(
        &workflow,
        &json!({"email_text": "hello"}),
        &MockExecutor,
        None,
        None,
        &YamlWorkflowRunOptions::default(),
        YamlWorkflowExecutionFlags::default(),
    )
    .await
    .expect_err("custom_worker without executor should fail");

    assert!(err
        .to_string()
        .contains("no custom worker executor is configured"));
}

#[tokio::test]
async fn custom_worker_receives_trace_context_block() {
    let yaml = r#"
id: trace-worker
entry_node: handler
nodes:
  - id: handler
    node_type:
      custom_worker:
        handler: my_handler
    "#;

    let workflow: YamlWorkflow = serde_yaml::from_str(yaml).expect("yaml should parse");
    let worker = CapturingWorker {
        context: Mutex::new(None),
    };
    let options = YamlWorkflowRunOptions {
        telemetry: YamlWorkflowTelemetryConfig {
            enabled: true,
            ..Default::default()
        },
        trace: YamlWorkflowTraceOptions {
            context: Some(YamlWorkflowTraceContextInput {
                trace_id: Some("explicit-trace-123".to_string()),
                ..Default::default()
            }),
            tenant: YamlWorkflowTraceTenantContext {
                workspace_id: Some("ws-1".to_string()),
                user_id: Some("u-1".to_string()),
                ..Default::default()
            },
        },
        model: None,
    };

    run_workflow_yaml_with_custom_worker_and_events_and_options(
        &workflow,
        &json!({"email_text": "hello"}),
        &MockExecutor,
        Some(&worker),
        None,
        &options,
        YamlWorkflowExecutionFlags::default(),
    )
    .await
    .expect("worker workflow should succeed");

    let captured_context = worker
        .context
        .lock()
        .expect("context lock should not be poisoned")
        .clone()
        .expect("context should be captured");

    let trace_block = captured_context
        .get("trace")
        .expect("trace block should be present");
    assert!(trace_block.get("context").is_some());
    assert!(trace_block.get("tenant").is_some());
}

#[tokio::test]
async fn event_sink_cancellation_stops_workflow_before_llm_execution() {
    let yaml = r#"
id: cancellation-test
entry_node: classify
nodes:
  - id: classify
    node_type:
      llm_call:
        model: gpt-4.1
    config:
      prompt: "Classify the following email into exactly one category: {{ input.email_text }}"
      output_schema:
        type: object
        properties:
          category: { type: string }
        required: [category]
  - id: second
    node_type:
      llm_call:
        model: gpt-4.1
    config:
      prompt: "Classify the following email into exactly one category: {{ input.email_text }}"
      output_schema:
        type: object
        properties:
          category: { type: string }
        required: [category]

edges:
  - from: classify
    to: second
    "#;

    let workflow: YamlWorkflow = serde_yaml::from_str(yaml).expect("yaml should parse");
    let executor = CountingExecutor {
        call_count: AtomicUsize::new(0),
    };
    let sink = CancelAfterFirstEventSink {
        cancelled: AtomicBool::new(false),
    };

    let err = run_workflow_yaml_with_custom_worker_and_events_and_options(
        &workflow,
        &json!({"email_text": "hello"}),
        &executor,
        None,
        Some(&sink),
        &YamlWorkflowRunOptions::default(),
        YamlWorkflowExecutionFlags::default(),
    )
    .await
    .expect_err("cancelled workflow should return error");

    let rendered = err.to_string();
    assert!(rendered.contains("cancelled"));
}

#[tokio::test]
async fn rejects_invalid_messages_path_shape() {
    let yaml = r#"
id: invalid-messages
entry_node: classify
nodes:
  - id: classify
    node_type:
      llm_call:
        model: gpt-4.1
        messages_path: input.messages
    config:
      prompt: ""
      output_schema:
        type: object
        properties:
          ok: { type: boolean }
    "#;

    let workflow: YamlWorkflow = serde_yaml::from_str(yaml).expect("yaml should parse");
    let err = run_workflow_yaml_with_custom_worker_and_events_and_options(
        &workflow,
        &json!({"messages": "not-an-array"}),
        &MockExecutor,
        None,
        None,
        &YamlWorkflowRunOptions::default(),
        YamlWorkflowExecutionFlags::default(),
    )
    .await
    .expect_err("invalid messages_path value should fail");

    let rendered = err.to_string();
    assert!(rendered.contains("messages_path") || rendered.contains("array"));
}

#[tokio::test]
async fn rejects_non_yaml_workflow_file_extension() {
    let file_path = std::env::temp_dir().join(format!(
        "simple_agents_workflow_{}.txt",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("unix epoch")
            .as_nanos()
    ));
    fs::write(&file_path, "not yaml").expect("temporary file should be created");

    let result =
        run_workflow_yaml_file(&file_path, &json!({"email_text": "hello"}), &MockExecutor).await;

    match result {
        Err(YamlWorkflowRunError::FileRejected { path, reason }) => {
            assert!(path.ends_with(".txt"));
            assert!(reason.contains(".yaml or .yml"));
        }
        other => panic!("expected file rejection error, got {other:?}"),
    }

    fs::remove_file(&file_path).ok();
}

#[tokio::test]
async fn ir_custom_worker_path_preserves_handler_file() {
    let yaml = r#"
id: worker-handler-file
entry_node: worker
nodes:
  - id: worker
    node_type:
      custom_worker:
        handler: GetRagData
        handler_file: handlers.py
    config:
      payload:
        topic: clarification
    "#;

    let workflow: YamlWorkflow = serde_yaml::from_str(yaml).expect("yaml should parse");
    let worker = HandlerFileCapturingWorker {
        handler_file: Mutex::new(None),
    };

    run_workflow_yaml_with_custom_worker_and_events_and_options(
        &workflow,
        &json!({"email_text":"hello"}),
        &MockExecutor,
        Some(&worker),
        None,
        &YamlWorkflowRunOptions::default(),
        YamlWorkflowExecutionFlags::default(),
    )
    .await
    .expect("workflow should execute");

    let captured = worker
        .handler_file
        .lock()
        .expect("handler-file lock should not be poisoned")
        .clone();
    assert_eq!(captured.as_deref(), Some("handlers.py"));
}

#[tokio::test]
async fn workflow_output_contains_trace_id_in_both_locations() {
    let yaml = r#"
id: trace-test
entry_node: classify
nodes:
  - id: classify
    node_type:
      llm_call:
        model: gpt-4.1
    config:
      prompt: "Classify the following email into exactly one category: {{ input.email_text }}"
      output_schema:
        type: object
        properties:
          category: { type: string }
        required: [category]
    "#;

    let workflow: YamlWorkflow = serde_yaml::from_str(yaml).expect("yaml should parse");
    let output = run_workflow_yaml_with_custom_worker_and_events_and_options(
        &workflow,
        &json!({"email_text": "hello"}),
        &MockExecutor,
        None,
        None,
        &YamlWorkflowRunOptions::default(),
        YamlWorkflowExecutionFlags::default(),
    )
    .await
    .expect("workflow should execute");

    let trace_id = output
        .trace_id
        .as_ref()
        .expect("trace_id should be present");
    assert!(!trace_id.is_empty());

    let metadata = output
        .metadata
        .as_ref()
        .expect("metadata should be present");
    let metadata_trace_id = metadata
        .get("telemetry")
        .and_then(|t| t.get("trace_id"))
        .and_then(Value::as_str)
        .expect("metadata.telemetry.trace_id should be present");
    assert_eq!(trace_id, metadata_trace_id);
}

#[tokio::test]
async fn workflow_run_options_sample_rate_zero_marks_trace_unsampled() {
    let yaml = r#"
id: unsampled-trace
entry_node: classify
nodes:
  - id: classify
    node_type:
      llm_call:
        model: gpt-4.1
    config:
      prompt: "Classify the following email into exactly one category: {{ input.email_text }}"
      output_schema:
        type: object
        properties:
          category: { type: string }
        required: [category]
    "#;

    let workflow: YamlWorkflow = serde_yaml::from_str(yaml).expect("yaml should parse");
    let options = YamlWorkflowRunOptions {
        telemetry: YamlWorkflowTelemetryConfig {
            sample_rate: 0.0,
            ..Default::default()
        },
        ..Default::default()
    };

    let output = run_workflow_yaml_with_custom_worker_and_events_and_options(
        &workflow,
        &json!({"email_text": "hello"}),
        &MockExecutor,
        None,
        None,
        &options,
        YamlWorkflowExecutionFlags::default(),
    )
    .await
    .expect("workflow should execute");

    let metadata = output
        .metadata
        .as_ref()
        .expect("metadata should be present");
    let sampled = metadata
        .get("telemetry")
        .and_then(|t| t.get("sampled"))
        .and_then(Value::as_bool)
        .expect("sampled field should be present");
    assert!(!sampled);
}

#[tokio::test]
async fn workflow_run_options_derives_trace_id_from_traceparent_when_trace_id_missing() {
    let yaml = r#"
id: traceparent-derived
entry_node: classify
nodes:
  - id: classify
    node_type:
      llm_call:
        model: gpt-4.1
    config:
      prompt: "Classify the following email into exactly one category: {{ input.email_text }}"
      output_schema:
        type: object
        properties:
          category: { type: string }
        required: [category]
    "#;

    let workflow: YamlWorkflow = serde_yaml::from_str(yaml).expect("yaml should parse");
    let options = YamlWorkflowRunOptions {
        trace: YamlWorkflowTraceOptions {
            context: Some(YamlWorkflowTraceContextInput {
                traceparent: Some(
                    "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01".to_string(),
                ),
                ..Default::default()
            }),
            ..Default::default()
        },
        ..Default::default()
    };

    let output = run_workflow_yaml_with_custom_worker_and_events_and_options(
        &workflow,
        &json!({"email_text": "hello"}),
        &MockExecutor,
        None,
        None,
        &options,
        YamlWorkflowExecutionFlags::default(),
    )
    .await
    .expect("workflow should execute");

    let trace_id = output.trace_id.as_ref().expect("trace_id should be set");
    assert_eq!(trace_id, "4bf92f3577b34da6a3ce929d0e0e4736");

    let metadata = output
        .metadata
        .as_ref()
        .expect("metadata should be present");
    let source = metadata
        .get("telemetry")
        .and_then(|t| t.get("trace_id_source"))
        .and_then(Value::as_str)
        .expect("trace_id_source should be present");
    assert_eq!(source, "traceparent");
}

#[tokio::test]
async fn workflow_run_options_sample_rate_one_marks_trace_sampled() {
    let yaml = r#"
id: sampled-trace
entry_node: classify
nodes:
  - id: classify
    node_type:
      llm_call:
        model: gpt-4.1
    config:
      prompt: "Classify the following email into exactly one category: {{ input.email_text }}"
      output_schema:
        type: object
        properties:
          category: { type: string }
        required: [category]
    "#;

    let workflow: YamlWorkflow = serde_yaml::from_str(yaml).expect("yaml should parse");
    let options = YamlWorkflowRunOptions {
        telemetry: YamlWorkflowTelemetryConfig {
            sample_rate: 1.0,
            ..Default::default()
        },
        ..Default::default()
    };

    let output = run_workflow_yaml_with_custom_worker_and_events_and_options(
        &workflow,
        &json!({"email_text": "hello"}),
        &MockExecutor,
        None,
        None,
        &options,
        YamlWorkflowExecutionFlags::default(),
    )
    .await
    .expect("workflow should execute");

    let metadata = output
        .metadata
        .as_ref()
        .expect("metadata should be present");
    let sampled = metadata
        .get("telemetry")
        .and_then(|t| t.get("sampled"))
        .and_then(Value::as_bool)
        .expect("sampled field should be present");
    assert!(sampled);
}

#[tokio::test]
async fn workflow_run_options_sampling_is_deterministic_for_fixed_trace_id() {
    let yaml = r#"
id: deterministic-sampling
entry_node: classify
nodes:
  - id: classify
    node_type:
      llm_call:
        model: gpt-4.1
    config:
      prompt: "Classify the following email into exactly one category: {{ input.email_text }}"
      output_schema:
        type: object
        properties:
          category: { type: string }
        required: [category]
    "#;

    let workflow: YamlWorkflow = serde_yaml::from_str(yaml).expect("yaml should parse");
    let options = YamlWorkflowRunOptions {
        telemetry: YamlWorkflowTelemetryConfig {
            sample_rate: 0.5,
            ..Default::default()
        },
        trace: YamlWorkflowTraceOptions {
            context: Some(YamlWorkflowTraceContextInput {
                trace_id: Some("deterministic-trace-id-12345".to_string()),
                ..Default::default()
            }),
            ..Default::default()
        },
        ..Default::default()
    };

    let output_1 = run_workflow_yaml_with_custom_worker_and_events_and_options(
        &workflow,
        &json!({"email_text": "hello"}),
        &MockExecutor,
        None,
        None,
        &options,
        YamlWorkflowExecutionFlags::default(),
    )
    .await
    .expect("first run should succeed");

    let output_2 = run_workflow_yaml_with_custom_worker_and_events_and_options(
        &workflow,
        &json!({"email_text": "hello"}),
        &MockExecutor,
        None,
        None,
        &options,
        YamlWorkflowExecutionFlags::default(),
    )
    .await
    .expect("second run should succeed");

    let sampled_1 = output_1
        .metadata
        .as_ref()
        .and_then(|m| m.get("telemetry"))
        .and_then(|t| t.get("sampled"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let sampled_2 = output_2
        .metadata
        .as_ref()
        .and_then(|m| m.get("telemetry"))
        .and_then(|t| t.get("sampled"))
        .and_then(Value::as_bool)
        .unwrap_or(false);

    assert_eq!(sampled_1, sampled_2);
}

#[tokio::test]
async fn workflow_run_options_reject_invalid_sample_rate() {
    let yaml = r#"
id: invalid-sample-rate
entry_node: classify
nodes:
  - id: classify
    node_type:
      llm_call:
        model: gpt-4.1
    config:
      prompt: "hello"
      output_schema:
        type: object
        properties:
          category: { type: string }
    "#;

    let workflow: YamlWorkflow = serde_yaml::from_str(yaml).expect("yaml should parse");
    let options = YamlWorkflowRunOptions {
        telemetry: YamlWorkflowTelemetryConfig {
            sample_rate: 2.0,
            ..Default::default()
        },
        ..Default::default()
    };

    let err = run_workflow_yaml_with_custom_worker_and_events_and_options(
        &workflow,
        &json!({"email_text": "hello"}),
        &MockExecutor,
        None,
        None,
        &options,
        YamlWorkflowExecutionFlags::default(),
    )
    .await
    .expect_err("invalid sample_rate should fail");
    assert!(err.to_string().contains("sample_rate"));
}

#[tokio::test]
async fn workflow_run_options_reject_nan_sample_rate() {
    let yaml = r#"
id: nan-sample-rate
entry_node: classify
nodes:
  - id: classify
    node_type:
      llm_call:
        model: gpt-4.1
    config:
      prompt: "hello"
      output_schema:
        type: object
        properties:
          category: { type: string }
    "#;

    let workflow: YamlWorkflow = serde_yaml::from_str(yaml).expect("yaml should parse");
    let options = YamlWorkflowRunOptions {
        telemetry: YamlWorkflowTelemetryConfig {
            sample_rate: f32::NAN,
            ..Default::default()
        },
        ..Default::default()
    };

    let err = run_workflow_yaml_with_custom_worker_and_events_and_options(
        &workflow,
        &json!({"email_text": "hello"}),
        &MockExecutor,
        None,
        None,
        &options,
        YamlWorkflowExecutionFlags::default(),
    )
    .await
    .expect_err("NaN sample_rate should fail");
    assert!(err.to_string().contains("sample_rate"));
}

#[tokio::test]
async fn workflow_run_options_use_explicit_trace_id_and_payload_mode() {
    let yaml = r#"
id: explicit-options
entry_node: classify
nodes:
  - id: classify
    node_type:
      llm_call:
        model: gpt-4.1
    config:
      prompt: "Classify the following email into exactly one category: {{ input.email_text }}"
      output_schema:
        type: object
        properties:
          category: { type: string }
        required: [category]
    "#;

    let workflow: YamlWorkflow = serde_yaml::from_str(yaml).expect("yaml should parse");
    let options = YamlWorkflowRunOptions {
        telemetry: YamlWorkflowTelemetryConfig {
            payload_mode: YamlWorkflowPayloadMode::RedactedPayload,
            ..Default::default()
        },
        trace: YamlWorkflowTraceOptions {
            context: Some(YamlWorkflowTraceContextInput {
                trace_id: Some("explicit-trace-abc123".to_string()),
                ..Default::default()
            }),
            ..Default::default()
        },
        ..Default::default()
    };

    let output = run_workflow_yaml_with_custom_worker_and_events_and_options(
        &workflow,
        &json!({"email_text": "hello"}),
        &MockExecutor,
        None,
        None,
        &options,
        YamlWorkflowExecutionFlags::default(),
    )
    .await
    .expect("workflow should execute with explicit options");

    assert_eq!(output.trace_id.as_deref(), Some("explicit-trace-abc123"));
    let metadata = output
        .metadata
        .as_ref()
        .expect("metadata should be present");
    let source = metadata
        .get("telemetry")
        .and_then(|t| t.get("trace_id_source"))
        .and_then(Value::as_str);
    assert_eq!(source, Some("explicit_trace_id"));
}

#[test]
fn yaml_llm_call_rejects_unknown_provider_field() {
    let yaml = r#"
id: wf
entry_node: classify
nodes:
  - id: classify
    node_type:
      llm_call:
        provider: openai
        model: gpt-4.1-mini
    config:
      prompt: hi
    "#;

    let err = serde_yaml::from_str::<YamlWorkflow>(yaml)
        .expect_err("unknown llm_call fields should fail parsing");
    let message = err.to_string();
    assert!(message.contains("provider"));
    assert!(message.contains("unknown field"));
}

#[test]
fn yaml_custom_worker_rejects_unknown_language_field() {
    let yaml = r#"
id: wf
entry_node: worker
nodes:
  - id: worker
    node_type:
      custom_worker:
        language: python
        handler: get_rag_data
    "#;

    let err = serde_yaml::from_str::<YamlWorkflow>(yaml)
        .expect_err("unknown custom_worker fields should fail parsing");
    let message = err.to_string();
    assert!(message.contains("language"));
    assert!(message.contains("unknown field"));
}

#[test]
fn yaml_workflow_accepts_top_level_metadata_block() {
    let yaml = r#"
id: wf
version: 1.0.0
metadata:
  name: Example Workflow
  tags:
    - example
entry_node: classify
nodes:
  - id: classify
    node_type:
      llm_call:
        model: gpt-4.1-mini
    config:
      prompt: hi
    "#;

    let workflow =
        serde_yaml::from_str::<YamlWorkflow>(yaml).expect("top-level metadata block should parse");
    assert_eq!(workflow.id, "wf");
    assert!(workflow.metadata.is_some());
}

#[test]
fn yaml_node_accepts_name_field() {
    let yaml = r#"
id: wf
entry_node: classify
nodes:
  - id: classify
    name: Classify Input
    node_type:
      llm_call:
        model: gpt-4.1-mini
    config:
      prompt: hi
    "#;

    let workflow =
        serde_yaml::from_str::<YamlWorkflow>(yaml).expect("node name field should parse");
    assert_eq!(workflow.nodes.len(), 1);
    assert_eq!(workflow.nodes[0].name.as_deref(), Some("Classify Input"));
}

#[tokio::test]
async fn custom_worker_handler_file_requires_executor() {
    let yaml = r#"
id: wf
entry_node: lookup
nodes:
  - id: lookup
    node_type:
      custom_worker:
        handler: get_rag_data
        handler_file: handlers.py
    "#;

    let workflow: YamlWorkflow = serde_yaml::from_str(yaml).expect("yaml should parse");
    let err = run_workflow_yaml_with_custom_worker_and_events_and_options(
        &workflow,
        &json!({"email_text": "hello"}),
        &MockExecutor,
        None,
        None,
        &YamlWorkflowRunOptions::default(),
        YamlWorkflowExecutionFlags::default(),
    )
    .await
    .expect_err("handler_file without executor should fail");

    let rendered = err.to_string();
    assert!(rendered.contains("handler_file"));
    assert!(rendered.contains("no custom worker executor configured"));
}

#[tokio::test]
async fn validates_workflow_input_before_ir_runtime_path() {
    let yaml = r#"
id: chat-workflow
entry_node: classify
nodes:
  - id: classify
    node_type:
      llm_call:
        model: gpt-4.1
    config:
      prompt: |
        classify
    "#;

    let workflow: YamlWorkflow = serde_yaml::from_str(yaml).expect("yaml should parse");
    let err = run_workflow_yaml(&workflow, &json!("not-an-object"), &MockExecutor)
        .await
        .expect_err("non-object input should fail before execution");

    assert!(matches!(err, YamlWorkflowRunError::InvalidInput { .. }));
}

#[test]
fn interpolate_template_supports_dollar_prefixed_paths() {
    let context = json!({
        "input": {
            "email_text": "hello"
        }
    });

    let result = interpolate_template("{{ $.input.email_text }}", &context);
    assert_eq!(result, "hello");
}

#[tokio::test]
async fn workflow_run_options_reject_unknown_top_level_field() {
    let yaml = r#"
id: options-unknown-top-level
entry_node: classify
nodes:
  - id: classify
    node_type:
      llm_call:
        model: gpt-4.1
    config:
      prompt: |
        {{ input.email_text }}
    "#;

    let _workflow: YamlWorkflow = serde_yaml::from_str(yaml).expect("yaml should parse");
    let options = serde_json::from_value::<YamlWorkflowRunOptions>(json!({
        "telemetry": {"enabled": true},
        "unexpected": true
    }))
    .expect_err("unknown options key should fail parsing");

    let message = options.to_string();
    assert!(message.contains("unknown field"));
    assert!(message.contains("unexpected"));
}

#[cfg(test)]
mod yaml_workflow_execution_contract_tests {
    use std::sync::Mutex;

    use super::{
        is_workflow_stream_delta_event, validate_yaml_workflow_execution, YamlWorkflow,
        YamlWorkflowEvent, YamlWorkflowEventSink, YamlWorkflowExecutionFlags,
        YamlWorkflowExecutionSurface, YamlWorkflowStreamFilterSink,
    };

    fn minimal_workflow() -> YamlWorkflow {
        let yaml = r#"
id: contract-test
entry_node: n
nodes:
  - id: n
    node_type:
      llm_call:
        model: m
    config:
      prompt: "x"
    "#;
        serde_yaml::from_str(yaml).expect("workflow yaml")
    }

    #[test]
    fn execution_flags_default_preserves_yaml_streaming_opt_in() {
        let f = YamlWorkflowExecutionFlags::default();
        assert!(!f.healing);
        assert!(!f.workflow_streaming);
        assert!(f.node_llm_streaming);
        assert!(!f.split_stream_deltas);
    }

    #[test]
    fn validate_rejects_run_with_workflow_streaming() {
        let wf = minimal_workflow();
        let flags = YamlWorkflowExecutionFlags {
            workflow_streaming: true,
            ..YamlWorkflowExecutionFlags::default()
        };
        let err = validate_yaml_workflow_execution(&wf, flags, YamlWorkflowExecutionSurface::Run)
            .expect_err("run surface should reject workflow_streaming");
        let msg = err.to_string();
        assert!(
            msg.contains("workflow_streaming=true"),
            "unexpected message: {msg}"
        );
    }

    #[test]
    fn validate_accepts_stream_surface_with_workflow_streaming() {
        let wf = minimal_workflow();
        let flags = YamlWorkflowExecutionFlags {
            workflow_streaming: true,
            ..YamlWorkflowExecutionFlags::default()
        };
        validate_yaml_workflow_execution(&wf, flags, YamlWorkflowExecutionSurface::Stream)
            .expect("stream surface allows workflow_streaming");
    }

    #[test]
    fn validate_rejects_healing_with_node_llm_streaming() {
        let wf = minimal_workflow();
        let flags = YamlWorkflowExecutionFlags {
            healing: true,
            node_llm_streaming: true,
            ..YamlWorkflowExecutionFlags::default()
        };
        let err = validate_yaml_workflow_execution(&wf, flags, YamlWorkflowExecutionSurface::Run)
            .expect_err("global heal+stream flags conflict");
        assert!(
            err.to_string().contains("healing and node_llm_streaming"),
            "{err}"
        );
    }

    #[test]
    fn stream_delta_event_predicate_matches_workflow_stream_types() {
        assert!(is_workflow_stream_delta_event("node_stream_delta"));
        assert!(is_workflow_stream_delta_event("node_stream_thinking_delta"));
        assert!(is_workflow_stream_delta_event("node_stream_output_delta"));
        assert!(!is_workflow_stream_delta_event("workflow_started"));
    }

    struct RecordingSink {
        types: Mutex<Vec<String>>,
    }

    impl YamlWorkflowEventSink for RecordingSink {
        fn emit(&self, event: &YamlWorkflowEvent) {
            self.types
                .lock()
                .expect("lock")
                .push(event.event_type.clone());
        }
    }

    fn sample_delta_event() -> YamlWorkflowEvent {
        YamlWorkflowEvent {
            event_type: "node_stream_delta".to_string(),
            node_id: None,
            step_id: None,
            node_kind: None,
            streamable: None,
            message: None,
            delta: Some("x".to_string()),
            token_kind: None,
            is_terminal_node_token: None,
            elapsed_ms: None,
            metadata: None,
        }
    }

    fn sample_lifecycle_event() -> YamlWorkflowEvent {
        YamlWorkflowEvent {
            event_type: "workflow_started".to_string(),
            node_id: None,
            step_id: None,
            node_kind: None,
            streamable: None,
            message: None,
            delta: None,
            token_kind: None,
            is_terminal_node_token: None,
            elapsed_ms: None,
            metadata: None,
        }
    }

    #[test]
    fn stream_filter_suppresses_deltas_when_workflow_streaming_disabled() {
        let inner = RecordingSink {
            types: Mutex::new(Vec::new()),
        };
        let filter = YamlWorkflowStreamFilterSink::new(&inner, false);
        filter.emit(&sample_delta_event());
        filter.emit(&sample_lifecycle_event());
        let seen = inner.types.lock().expect("lock");
        assert_eq!(seen.as_slice(), &["workflow_started".to_string()]);
    }

    #[test]
    fn stream_filter_forwards_deltas_when_workflow_streaming_enabled() {
        let inner = RecordingSink {
            types: Mutex::new(Vec::new()),
        };
        let filter = YamlWorkflowStreamFilterSink::new(&inner, true);
        filter.emit(&sample_delta_event());
        let seen = inner.types.lock().expect("lock");
        assert_eq!(seen.as_slice(), &["node_stream_delta".to_string()]);
    }
}
