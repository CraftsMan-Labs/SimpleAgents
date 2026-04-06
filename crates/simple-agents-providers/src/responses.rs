//! Helpers for building and parsing OpenAI Responses API requests.

use serde_json::{json, Value};
use simple_agent_type::message::Message;
use simple_agent_type::request::CompletionRequest;
use simple_agent_type::response::{
    CompletionChoice, CompletionChunk, CompletionResponse, FinishReason, Usage,
};

/// Build the JSON body for `POST /v1/responses`.
pub fn build_responses_request(req: &CompletionRequest) -> Value {
    let mut body = json!({
        "model": req.model,
    });

    let input: Vec<Value> = req
        .messages
        .iter()
        .map(|m| {
            json!({
                "role": m.role.as_str(),
                "content": m.content_text(),
            })
        })
        .collect();
    body["input"] = Value::Array(input);

    if let Some(ref instructions) = req.instructions {
        body["instructions"] = Value::String(instructions.clone());
    }
    if let Some(ref prev_id) = req.previous_response_id {
        body["previous_response_id"] = Value::String(prev_id.clone());
    }
    if let Some(store) = req.store {
        body["store"] = Value::Bool(store);
    }
    if req.stream == Some(true) {
        body["stream"] = Value::Bool(true);
    }

    body
}

/// Parse a Responses API JSON body into the unified [`CompletionResponse`].
pub fn parse_responses_response(body: Value) -> Result<CompletionResponse, String> {
    let id = body["id"].as_str().unwrap_or("").to_string();

    let output_text = body["output"]
        .as_array()
        .and_then(|outputs| {
            outputs.iter().find_map(|item| {
                if item["type"].as_str() == Some("message") {
                    item["content"].as_array().and_then(|content| {
                        content.iter().find_map(|c| {
                            if c["type"].as_str() == Some("output_text") {
                                c["text"].as_str().map(|s| s.to_string())
                            } else {
                                None
                            }
                        })
                    })
                } else {
                    None
                }
            })
        })
        .unwrap_or_default();

    let usage = body.get("usage").map(|u| Usage {
        prompt_tokens: u["input_tokens"].as_u64().unwrap_or(0) as u32,
        completion_tokens: u["output_tokens"].as_u64().unwrap_or(0) as u32,
        total_tokens: u["total_tokens"].as_u64().unwrap_or(0) as u32,
        reasoning_tokens: None,
    });

    Ok(CompletionResponse {
        id,
        model: body["model"].as_str().unwrap_or("").to_string(),
        choices: vec![CompletionChoice {
            message: Message::assistant(output_text),
            finish_reason: FinishReason::Stop,
            index: 0,
            logprobs: None,
        }],
        usage: usage.unwrap_or_else(|| Usage::new(0, 0)),
        created: None,
        provider: None,
        healing_metadata: None,
    })
}

/// Parse a Responses API SSE event into a [`CompletionChunk`] (placeholder).
pub fn parse_responses_stream_event(_event_type: &str, _data: &str) -> Option<CompletionChunk> {
    // TODO: implement Responses API SSE event parsing
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use simple_agent_type::message::Message;

    #[test]
    fn test_build_responses_request_basic() {
        let req = CompletionRequest::new("gpt-4o")
            .messages(vec![Message::user("hello")])
            .instructions("Be helpful");
        let body = build_responses_request(&req);
        assert_eq!(body["model"], "gpt-4o");
        assert_eq!(body["instructions"], "Be helpful");
        assert!(body["input"].is_array());
    }

    #[test]
    fn test_parse_responses_response_message() {
        let body = serde_json::json!({
            "id": "resp_123",
            "object": "response",
            "status": "completed",
            "model": "gpt-4o",
            "output": [{
                "type": "message",
                "role": "assistant",
                "content": [{"type": "output_text", "text": "Hello!"}],
                "status": "completed"
            }],
            "usage": {"input_tokens": 10, "output_tokens": 5, "total_tokens": 15}
        });
        let resp = parse_responses_response(body).unwrap();
        assert_eq!(resp.content(), Some("Hello!"));
        assert_eq!(resp.id, "resp_123");
        assert_eq!(resp.usage.prompt_tokens, 10);
        assert_eq!(resp.usage.completion_tokens, 5);
    }

    #[test]
    fn test_parse_responses_response_no_output() {
        let body = serde_json::json!({
            "id": "resp_456",
            "object": "response",
            "status": "completed",
            "model": "gpt-4o",
            "output": []
        });
        let resp = parse_responses_response(body).unwrap();
        assert_eq!(resp.content(), Some(""));
    }

    #[test]
    fn test_build_responses_request_with_stream() {
        let mut req = CompletionRequest::new("gpt-4o").messages(vec![Message::user("hello")]);
        req.stream = Some(true);
        let body = build_responses_request(&req);
        assert_eq!(body["stream"], true);
    }

    #[test]
    fn test_build_responses_request_with_previous_response_id() {
        let req = CompletionRequest::new("gpt-4o")
            .messages(vec![Message::user("hello")])
            .previous_response_id("resp_abc");
        let body = build_responses_request(&req);
        assert_eq!(body["previous_response_id"], "resp_abc");
    }

    #[test]
    fn test_parse_responses_stream_event_placeholder() {
        assert!(parse_responses_stream_event("response.done", "{}").is_none());
    }
}
