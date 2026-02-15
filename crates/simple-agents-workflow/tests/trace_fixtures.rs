use async_trait::async_trait;
use serde_json::{json, Value};
use simple_agents_workflow::ir::{Node, NodeKind, WorkflowDefinition};
use simple_agents_workflow::runtime::{
    LlmExecutionError, LlmExecutionInput, LlmExecutionOutput, LlmExecutor, ToolExecutionError,
    ToolExecutionInput, ToolExecutor, WorkflowReplayMode, WorkflowRuntime, WorkflowRuntimeOptions,
};
use simple_agents_workflow::trace::WorkflowTrace;
use simple_agents_workflow::{replay_trace, ReplayViolationCode};

struct MockLlmExecutor;

#[async_trait]
impl LlmExecutor for MockLlmExecutor {
    async fn execute(&self, _input: LlmExecutionInput) -> Result<LlmExecutionOutput, LlmExecutionError> {
        Ok(LlmExecutionOutput {
            content: "ok".to_string(),
        })
    }
}

struct MockToolExecutor;

#[async_trait]
impl ToolExecutor for MockToolExecutor {
    async fn execute_tool(&self, _input: ToolExecutionInput) -> Result<Value, ToolExecutionError> {
        Ok(json!({"status": "done"}))
    }
}

fn linear_workflow() -> WorkflowDefinition {
    WorkflowDefinition {
        version: "v0".to_string(),
        name: "linear".to_string(),
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
                    prompt: "Summarize".to_string(),
                    next: Some("tool".to_string()),
                },
            },
            Node {
                id: "tool".to_string(),
                kind: NodeKind::Tool {
                    tool: "extract".to_string(),
                    input: json!({"k": "v"}),
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

fn fixture(name: &str) -> String {
    let path = format!(
        "{}/tests/fixtures/{}",
        env!("CARGO_MANIFEST_DIR"),
        name
    );
    std::fs::read_to_string(path).expect("fixture file should be readable")
}

#[tokio::test]
async fn recorded_trace_matches_linear_golden_fixture() {
    let llm = MockLlmExecutor;
    let tool = MockToolExecutor;
    let runtime = WorkflowRuntime::new(
        linear_workflow(),
        &llm,
        Some(&tool),
        WorkflowRuntimeOptions {
            replay_mode: WorkflowReplayMode::ValidateRecordedTrace,
            ..WorkflowRuntimeOptions::default()
        },
    );

    let result = runtime
        .execute(json!({"request_id": "r1"}), None)
        .await
        .expect("workflow should succeed");

    let expected: WorkflowTrace =
        serde_json::from_str(&fixture("linear_trace.json")).expect("fixture should parse");
    assert_eq!(result.trace, Some(expected));
    assert_eq!(result.replay_report.as_ref().map(|r| r.total_events), Some(9));
}

#[test]
fn invalid_fixture_fails_replay_validation() {
    let trace: WorkflowTrace = serde_json::from_str(&fixture("invalid_missing_terminal_trace.json"))
        .expect("fixture should parse");
    let error = replay_trace(&trace).expect_err("fixture should fail replay");

    assert!(error
        .violations
        .iter()
        .any(|violation| violation.code == ReplayViolationCode::MissingTerminalEvent));
}
