use async_trait::async_trait;
use simple_agent_type::prelude::*;
use simple_agents_core::{
    CompletionMode, CompletionOptions, CompletionOutcome, HealingSettings, SimpleAgentsClient,
};
use simple_agents_healing::schema::Schema;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

struct MockProvider {
    name: &'static str,
    content: &'static str,
    calls: AtomicUsize,
}

impl MockProvider {
    fn new(name: &'static str, content: &'static str) -> Self {
        Self {
            name,
            content,
            calls: AtomicUsize::new(0),
        }
    }
}

#[async_trait]
impl Provider for MockProvider {
    fn name(&self) -> &str {
        self.name
    }

    fn transform_request(&self, _req: &CompletionRequest) -> Result<ProviderRequest> {
        Ok(ProviderRequest::new("http://example.com"))
    }

    async fn execute(&self, _req: ProviderRequest) -> Result<ProviderResponse> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        Ok(ProviderResponse::new(200, serde_json::Value::Null))
    }

    fn transform_response(&self, _resp: ProviderResponse) -> Result<CompletionResponse> {
        Ok(CompletionResponse {
            id: "resp_test".to_string(),
            model: "test-model".to_string(),
            choices: vec![CompletionChoice {
                index: 0,
                message: Message::assistant(self.content),
                finish_reason: FinishReason::Stop,
                logprobs: None,
            }],
            usage: Usage::new(1, 1),
            created: None,
            provider: Some(self.name.to_string()),
            healing_metadata: None,
        })
    }
}

fn build_request() -> CompletionRequest {
    CompletionRequest::builder()
        .model("test-model")
        .message(Message::user("hello"))
        .build()
        .unwrap()
}

#[tokio::test]
async fn complete_json_parses_markdown() {
    let provider = Arc::new(MockProvider::new("p1", "```json\n{\"value\": 42,}\n```"));
    let client = SimpleAgentsClient::with_healing(provider, HealingSettings::default());

    let healed = client
        .complete(
            &build_request(),
            CompletionOptions {
                mode: CompletionMode::HealedJson,
            },
        )
        .await
        .unwrap();
    let healed = match healed {
        CompletionOutcome::HealedJson(healed) => healed,
        _ => panic!("expected healed json"),
    };
    assert_eq!(healed.parsed.value["value"], 42);
}

#[tokio::test]
async fn complete_with_schema_coerces_types() {
    let provider = Arc::new(MockProvider::new("p1", "{\"count\": \"5\"}"));
    let client = SimpleAgentsClient::with_healing(provider, HealingSettings::default());

    let schema = Schema::object(vec![("count".into(), Schema::Int, true)]);
    let healed = client
        .complete(
            &build_request(),
            CompletionOptions {
                mode: CompletionMode::CoercedSchema(schema),
            },
        )
        .await
        .unwrap();
    let healed = match healed {
        CompletionOutcome::CoercedSchema(healed) => healed,
        _ => panic!("expected coerced schema"),
    };

    assert_eq!(healed.coerced.value["count"], 5);
}

#[tokio::test]
async fn complete_returns_standard_response() {
    let provider = Arc::new(MockProvider::new("p1", "ok"));
    let client = SimpleAgentsClient::new(provider);

    let outcome = client
        .complete(&build_request(), CompletionOptions::default())
        .await
        .unwrap();

    let response = match outcome {
        CompletionOutcome::Response(response) => response,
        _ => panic!("expected response"),
    };
    assert_eq!(response.content(), Some("ok"));
}
