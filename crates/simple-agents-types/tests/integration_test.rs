//! Integration tests for simple-agents-types.
//!
//! Tests end-to-end usage of all types together.

use simple_agents_types::prelude::*;

#[test]
fn test_complete_request_response_cycle() {
    // Create a request
    let request = CompletionRequest::builder()
        .model("gpt-4")
        .message(Message::system("You are helpful."))
        .message(Message::user("Hello!"))
        .temperature(0.7)
        .max_tokens(100)
        .build()
        .unwrap();

    // Serialize to JSON
    let json = serde_json::to_string(&request).unwrap();
    assert!(json.contains("gpt-4"));
    assert!(json.contains("Hello!"));

    // Deserialize back
    let parsed: CompletionRequest = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.model, "gpt-4");
    assert_eq!(parsed.messages.len(), 2);

    // Create a response
    let response = CompletionResponse {
        id: "resp_123".to_string(),
        model: "gpt-4".to_string(),
        choices: vec![CompletionChoice {
            index: 0,
            message: Message::assistant("Hi there!"),
            finish_reason: FinishReason::Stop,
            logprobs: None,
        }],
        usage: Usage::new(20, 10),
        created: Some(1234567890),
        provider: Some("openai".to_string()),
    };

    // Access response
    assert_eq!(response.content(), Some("Hi there!"));
    assert_eq!(response.usage.total_tokens, 30);

    // Serialize response
    let resp_json = serde_json::to_string(&response).unwrap();
    let parsed_resp: CompletionResponse = serde_json::from_str(&resp_json).unwrap();
    assert_eq!(parsed_resp.id, "resp_123");
}

#[test]
fn test_error_handling_flow() {
    // Provider errors
    let rate_limit = ProviderError::RateLimit {
        retry_after: Some(std::time::Duration::from_secs(60)),
    };
    assert!(rate_limit.is_retryable());

    let invalid_key = ProviderError::InvalidApiKey;
    assert!(!invalid_key.is_retryable());

    // Convert to main error type
    let main_err: SimpleAgentsError = rate_limit.into();
    assert!(matches!(main_err, SimpleAgentsError::Provider(_)));

    // Validation errors
    let validation_err = ValidationError::OutOfRange {
        field: "temperature".to_string(),
        min: 0.0,
        max: 2.0,
    };
    let main_err: SimpleAgentsError = validation_err.into();
    assert!(matches!(main_err, SimpleAgentsError::Validation(_)));
}

#[test]
fn test_coercion_tracking() {
    // Track multiple coercions
    let result = CoercionResult::new("fixed data")
        .with_flag(CoercionFlag::StrippedMarkdown)
        .with_flag(CoercionFlag::FixedTrailingComma)
        .with_flag(CoercionFlag::TypeCoercion {
            from: "string".to_string(),
            to: "number".to_string(),
        })
        .set_confidence(0.85);

    assert!(result.was_coerced());
    assert_eq!(result.flags.len(), 3);
    assert!(result.has_major_coercions());
    assert!(result.is_confident(0.8));
    assert!(!result.is_confident(0.9));

    // Map preserves flags
    let mapped = result.map(|s| s.to_uppercase());
    assert_eq!(mapped.value, "FIXED DATA");
    assert_eq!(mapped.flags.len(), 3);
    assert_eq!(mapped.confidence, 0.85);
}

#[test]
fn test_api_key_security() {
    let key = ApiKey::new("sk-1234567890abcdefghijklmnopqrstuvwxyz").unwrap();

    // Debug is redacted
    let debug = format!("{:?}", key);
    assert!(debug.contains("REDACTED"));
    assert!(!debug.contains("sk-"));

    // Serialization is redacted
    let json = serde_json::to_string(&key).unwrap();
    assert_eq!(json, "\"[REDACTED]\"");

    // Preview shows partial info
    let preview = key.preview();
    assert!(preview.contains("sk-"));
    assert!(preview.contains("chars"));
    assert!(!preview.contains("abcdef"));

    // Only expose() gives raw key
    assert_eq!(key.expose().len(), 39);
}

#[test]
fn test_configuration_types() {
    // Retry config
    let retry = RetryConfig::default();
    assert_eq!(retry.max_attempts, 3);

    let backoff0 = retry.calculate_backoff(0);
    let backoff1 = retry.calculate_backoff(1);
    // Backoff should increase (with or without jitter)
    assert!(backoff1 >= backoff0 || retry.jitter);

    // Healing config
    let strict = HealingConfig::strict();
    assert!(strict.strict_mode);
    assert!(!strict.allow_type_coercion);
    assert_eq!(strict.min_confidence, 0.95);

    let lenient = HealingConfig::lenient();
    assert!(!lenient.strict_mode);
    assert!(lenient.allow_type_coercion);
    assert_eq!(lenient.min_confidence, 0.5);

    // Provider config
    let provider = ProviderConfig::new("test", "https://api.test.com")
        .with_api_key("sk-test")
        .with_default_model("test-model");

    assert_eq!(provider.name, "test");
    assert_eq!(provider.api_key, Some("sk-test".to_string()));
    assert_eq!(provider.default_model, Some("test-model".to_string()));
}

#[test]
fn test_request_validation() {
    // Valid request
    let valid = CompletionRequest::builder()
        .model("gpt-4")
        .message(Message::user("test"))
        .build();
    assert!(valid.is_ok());

    // Missing model
    let no_model = CompletionRequest::builder()
        .message(Message::user("test"))
        .build();
    assert!(no_model.is_err());

    // Invalid temperature
    let bad_temp = CompletionRequest::builder()
        .model("gpt-4")
        .message(Message::user("test"))
        .temperature(3.0)
        .build();
    assert!(bad_temp.is_err());

    // Invalid top_p
    let bad_top_p = CompletionRequest::builder()
        .model("gpt-4")
        .message(Message::user("test"))
        .top_p(1.5)
        .build();
    assert!(bad_top_p.is_err());

    // Invalid model name
    let bad_model = CompletionRequest::builder()
        .model("gpt-4!")
        .message(Message::user("test"))
        .build();
    assert!(bad_model.is_err());
}

#[test]
fn test_streaming_types() {
    // Create a streaming chunk
    let chunk = CompletionChunk {
        id: "chunk_123".to_string(),
        model: "gpt-4".to_string(),
        choices: vec![ChoiceDelta {
            index: 0,
            delta: MessageDelta {
                role: Some(Role::Assistant),
                content: Some("Hello".to_string()),
            },
            finish_reason: None,
        }],
        created: Some(1234567890),
    };

    // Serialize and deserialize
    let json = serde_json::to_string(&chunk).unwrap();
    let parsed: CompletionChunk = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.id, "chunk_123");
    assert_eq!(parsed.choices[0].delta.content, Some("Hello".to_string()));
}

#[test]
fn test_provider_request_response() {
    // Build provider request
    let req = ProviderRequest::new("https://api.example.com/v1/chat")
        .with_header("Authorization", "Bearer sk-test")
        .with_header("Content-Type", "application/json")
        .with_body(serde_json::json!({"model": "test"}))
        .with_timeout(std::time::Duration::from_secs(30));

    assert_eq!(req.url, "https://api.example.com/v1/chat");
    assert_eq!(req.headers.len(), 2);
    assert_eq!(req.timeout, Some(std::time::Duration::from_secs(30)));

    // Test provider response
    let resp = ProviderResponse::new(200, serde_json::json!({"status": "ok"}));
    assert!(resp.is_success());
    assert!(!resp.is_client_error());
    assert!(!resp.is_server_error());

    let error_resp = ProviderResponse::new(500, serde_json::json!({"error": "server error"}));
    assert!(!error_resp.is_success());
    assert!(error_resp.is_server_error());
}

#[test]
fn test_router_types() {
    // Provider metrics
    let mut metrics = ProviderMetrics::default();
    assert_eq!(metrics.success_rate(), 1.0);

    metrics.total_requests = 100;
    metrics.successful_requests = 95;
    metrics.failed_requests = 5;

    assert!((metrics.success_rate() - 0.95).abs() < 0.01);
    assert!((metrics.failure_rate() - 0.05).abs() < 0.01);

    // Provider health
    assert!(ProviderHealth::Healthy.is_available());
    assert!(ProviderHealth::Degraded.is_available());
    assert!(!ProviderHealth::Unavailable.is_available());

    // Routing modes
    assert!(!RoutingMode::Priority.description().is_empty());
    assert!(!RoutingMode::RoundRobin.description().is_empty());
}

#[test]
fn test_cache_key_generation() {
    use simple_agents_types::cache::CacheKey;

    // Keys should be deterministic
    let key1 = CacheKey::from_parts("openai", "gpt-4", "test content");
    let key2 = CacheKey::from_parts("openai", "gpt-4", "test content");
    assert_eq!(key1, key2);

    // Different content = different key
    let key3 = CacheKey::from_parts("openai", "gpt-4", "different");
    assert_ne!(key1, key3);

    // Keys contain provider and model
    assert!(key1.starts_with("openai:"));
    assert!(key1.contains("gpt-4"));

    // Namespace keys
    let ns_key = CacheKey::with_namespace("responses", "abc123");
    assert_eq!(ns_key, "responses:abc123");
}

#[test]
fn test_all_types_are_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}

    // Core types
    assert_send_sync::<Message>();
    assert_send_sync::<CompletionRequest>();
    assert_send_sync::<CompletionResponse>();
    assert_send_sync::<Usage>();

    // Config types
    assert_send_sync::<RetryConfig>();
    assert_send_sync::<HealingConfig>();
    assert_send_sync::<Capabilities>();
    assert_send_sync::<ProviderConfig>();

    // Error types
    assert_send_sync::<SimpleAgentsError>();
    assert_send_sync::<ProviderError>();
    assert_send_sync::<HealingError>();
    assert_send_sync::<ValidationError>();

    // Coercion types
    assert_send_sync::<CoercionFlag>();
    assert_send_sync::<CoercionResult<String>>();

    // Provider types
    assert_send_sync::<ProviderRequest>();
    assert_send_sync::<ProviderResponse>();

    // Router types
    assert_send_sync::<ProviderMetrics>();
}
