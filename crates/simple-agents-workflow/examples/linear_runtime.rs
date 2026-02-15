use async_trait::async_trait;
use serde_json::{json, Value};
use simple_agents_workflow::{
    LlmExecutionError, LlmExecutionInput, LlmExecutionOutput, LlmExecutor, Node, NodeKind,
    ToolExecutionError, ToolExecutionInput, ToolExecutor, WorkflowDefinition, WorkflowReplayMode,
    WorkflowRuntime, WorkflowRuntimeOptions,
};

struct DemoLlm;

#[async_trait]
impl LlmExecutor for DemoLlm {
    async fn execute(
        &self,
        _input: LlmExecutionInput,
    ) -> Result<LlmExecutionOutput, LlmExecutionError> {
        Ok(LlmExecutionOutput {
            content: "hello from llm".to_string(),
        })
    }
}

struct DemoTool;

#[async_trait]
impl ToolExecutor for DemoTool {
    async fn execute_tool(&self, _input: ToolExecutionInput) -> Result<Value, ToolExecutionError> {
        Ok(json!({ "status": "ok" }))
    }
}

fn workflow() -> WorkflowDefinition {
    WorkflowDefinition {
        version: "v0".to_string(),
        name: "linear-demo".to_string(),
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
                    prompt: "Say hello".to_string(),
                    next: Some("tool".to_string()),
                },
            },
            Node {
                id: "tool".to_string(),
                kind: NodeKind::Tool {
                    tool: "demo-tool".to_string(),
                    input: json!({ "strict": true }),
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
    let llm = DemoLlm;
    let tool = DemoTool;
    let runtime = WorkflowRuntime::new(
        workflow(),
        &llm,
        Some(&tool),
        WorkflowRuntimeOptions {
            replay_mode: WorkflowReplayMode::ValidateRecordedTrace,
            ..WorkflowRuntimeOptions::default()
        },
    );

    let result = runtime
        .execute(json!({ "request_id": "demo-1" }), None)
        .await
        .expect("demo workflow should succeed");

    println!("terminal node: {}", result.terminal_node_id);
    println!("recorded events: {}", result.events.len());
}
