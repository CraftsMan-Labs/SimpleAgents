use async_trait::async_trait;
use serde_json::{json, Value};
use simple_agents_workflow::{
    inspect_replay_trace, node_timeline, retry_reason_summary, LlmExecutionError,
    LlmExecutionInput, LlmExecutionOutput, LlmExecutor, Node, NodeExecutionPolicy, NodeKind,
    ToolExecutionError, ToolExecutionInput, ToolExecutor, WorkflowDefinition, WorkflowReplayMode,
    WorkflowRuntime, WorkflowRuntimeOptions,
};
use std::sync::atomic::{AtomicBool, Ordering};

struct FlakyLlm {
    failed_once: AtomicBool,
}

#[async_trait]
impl LlmExecutor for FlakyLlm {
    async fn execute(
        &self,
        _input: LlmExecutionInput,
    ) -> Result<LlmExecutionOutput, LlmExecutionError> {
        if !self.failed_once.swap(true, Ordering::Relaxed) {
            return Err(LlmExecutionError::Client(
                "transient upstream error".to_string(),
            ));
        }

        Ok(LlmExecutionOutput {
            content: "debug-success".to_string(),
        })
    }
}

struct DemoTool;

#[async_trait]
impl ToolExecutor for DemoTool {
    async fn execute_tool(&self, _input: ToolExecutionInput) -> Result<Value, ToolExecutionError> {
        Ok(json!({"status": "ok"}))
    }
}

fn workflow() -> WorkflowDefinition {
    WorkflowDefinition {
        version: "v0".to_string(),
        name: "debug-demo".to_string(),
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
                    model: "demo-model".to_string(),
                    prompt: "Generate one line".to_string(),
                    next: Some("tool".to_string()),
                },
            },
            Node {
                id: "tool".to_string(),
                kind: NodeKind::Tool {
                    tool: "demo-tool".to_string(),
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

#[tokio::main]
async fn main() {
    let llm = FlakyLlm {
        failed_once: AtomicBool::new(false),
    };
    let tool = DemoTool;
    let runtime = WorkflowRuntime::new(
        workflow(),
        &llm,
        Some(&tool),
        WorkflowRuntimeOptions {
            replay_mode: WorkflowReplayMode::ValidateRecordedTrace,
            llm_node_policy: NodeExecutionPolicy {
                timeout: None,
                max_retries: 1,
            },
            ..WorkflowRuntimeOptions::default()
        },
    );

    let result = runtime
        .execute(json!({"request_id": "debug-1"}), None)
        .await
        .expect("debug workflow should succeed");

    let timeline = node_timeline(&result);
    let retries = retry_reason_summary(&result.retry_events);

    println!("timeline entries: {}", timeline.len());
    println!("retry groups: {}", retries.len());

    if let Some(trace) = result.trace.as_ref() {
        let replay = inspect_replay_trace(trace);
        println!("replay valid: {}", replay.valid);
        println!("replay events: {}", replay.total_events);
    }
}
