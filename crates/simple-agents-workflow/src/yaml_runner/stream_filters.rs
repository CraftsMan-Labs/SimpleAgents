use serde_json::Value;
use simple_agents_healing::{CoercionEngine, JsonishParser, Schema};

#[derive(Debug)]
pub(crate) struct StreamedPayloadResolution {
    pub(crate) payload: Value,
    pub(crate) heal_confidence: Option<f32>,
    pub(crate) coerced_confidence: Option<f32>,
}

#[derive(Debug, Default)]
pub(crate) struct StreamJsonAsTextFormatter {
    raw_json: String,
    emitted: bool,
}

impl StreamJsonAsTextFormatter {
    pub(crate) fn push(&mut self, chunk: &str) {
        self.raw_json.push_str(chunk);
    }

    pub(crate) fn emit_if_ready(&mut self, complete: bool) -> Option<String> {
        if self.emitted || !complete {
            return None;
        }
        self.emitted = true;
        Some(render_json_object_as_text(self.raw_json.as_str()))
    }
}

pub(crate) fn render_json_object_as_text(raw_json: &str) -> String {
    let value = match serde_json::from_str::<Value>(raw_json) {
        Ok(value) => value,
        Err(_) => return raw_json.to_string(),
    };
    let Some(object) = value.as_object() else {
        return raw_json.to_string();
    };

    let mut lines = Vec::with_capacity(object.len());
    for (key, value) in object {
        let rendered = match value {
            Value::String(text) => text.clone(),
            _ => value.to_string(),
        };
        lines.push(format!("{key}: {rendered}"));
    }
    lines.join("\n")
}

#[derive(Debug, Default)]
pub(crate) struct StructuredJsonDeltaFilter {
    started: bool,
    completed: bool,
    depth: u32,
    in_string: bool,
    escape: bool,
}

impl StructuredJsonDeltaFilter {
    pub(crate) fn split(&mut self, delta: &str) -> (Option<String>, Option<String>) {
        if delta.is_empty() {
            return (None, None);
        }

        let mut output = String::new();
        let mut thinking = String::new();

        for ch in delta.chars() {
            if self.completed {
                thinking.push(ch);
                continue;
            }

            if !self.started {
                if ch != '{' {
                    thinking.push(ch);
                    continue;
                }
                self.started = true;
                self.depth = 1;
                output.push(ch);
                continue;
            }

            output.push(ch);
            if self.in_string {
                if self.escape {
                    self.escape = false;
                    continue;
                }
                if ch == '\\' {
                    self.escape = true;
                    continue;
                }
                if ch == '"' {
                    self.in_string = false;
                }
                continue;
            }

            match ch {
                '"' => self.in_string = true,
                '{' => self.depth = self.depth.saturating_add(1),
                '}' => {
                    if self.depth > 0 {
                        self.depth -= 1;
                    }
                    if self.depth == 0 {
                        self.completed = true;
                    }
                }
                _ => {}
            }
        }

        let output = if output.is_empty() {
            None
        } else {
            Some(output)
        };
        let thinking = if thinking.is_empty() {
            None
        } else {
            Some(thinking)
        };

        (output, thinking)
    }

    pub(crate) fn completed(&self) -> bool {
        self.completed
    }
}

pub(crate) fn extract_last_fenced_json_block(raw: &str) -> Option<&str> {
    let start = raw.rfind("```json")?;
    let remainder = &raw[start + "```json".len()..];
    let end = remainder.find("```")?;
    let candidate = remainder[..end].trim();
    if candidate.is_empty() {
        return None;
    }
    Some(candidate)
}

pub(crate) fn extract_balanced_object_from(raw: &str, start_index: usize) -> Option<&str> {
    let mut depth = 0u32;
    let mut in_string = false;
    let mut escape = false;

    for (relative_index, ch) in raw[start_index..].char_indices() {
        if in_string {
            if escape {
                escape = false;
                continue;
            }
            if ch == '\\' {
                escape = true;
                continue;
            }
            if ch == '"' {
                in_string = false;
            }
            continue;
        }

        match ch {
            '"' => in_string = true,
            '{' => depth = depth.saturating_add(1),
            '}' => {
                if depth == 0 {
                    return None;
                }
                depth -= 1;
                if depth == 0 {
                    let end_index = start_index + relative_index + ch.len_utf8();
                    return Some(raw[start_index..end_index].trim());
                }
            }
            _ => {}
        }
    }

    None
}

pub(crate) fn extract_first_parsable_object(raw: &str) -> Option<&str> {
    for (index, ch) in raw.char_indices() {
        if ch != '{' {
            continue;
        }
        let Some(candidate) = extract_balanced_object_from(raw, index) else {
            continue;
        };
        if serde_json::from_str::<Value>(candidate).is_ok() {
            return Some(candidate);
        }
    }

    None
}

pub(crate) fn resolve_structured_json_candidate(raw: &str) -> Option<&str> {
    extract_last_fenced_json_block(raw).or_else(|| extract_first_parsable_object(raw))
}

/// When models stream `{"a":1}, "b": "c"` the first parsable object omits `b`; merge before the heal path.
fn try_merge_leading_object_comma_continuation(raw: &str, candidate: &str) -> Option<String> {
    let inner = candidate.strip_prefix('{')?.strip_suffix('}')?;
    let start = raw.find(candidate)?;
    let after_candidate = raw.get(start + candidate.len()..)?;
    let after = after_candidate.trim_start().strip_prefix(',')?.trim_start();
    if after.is_empty() {
        return None;
    }
    let merged = format!("{{{inner}, {after}}}");
    serde_json::from_str::<Value>(&merged).ok()?;
    Some(merged)
}

fn heal_and_coerce_stream_payload(
    text: &str,
    schema: Option<&Value>,
) -> Result<StreamedPayloadResolution, String> {
    let parser = JsonishParser::new();
    let healed = parser
        .parse(text)
        .map_err(|error| format!("failed to heal streamed structured completion JSON: {error}"))?;

    let mut payload = healed.value;
    let mut coerced_confidence = None;
    if let Some(schema_value) = schema {
        let schema = convert_json_schema_to_healing_schema(schema_value)?;
        let engine = CoercionEngine::new();
        let coerced = engine.coerce(&payload, &schema).map_err(|error| {
            format!("failed to coerce streamed structured completion JSON: {error}")
        })?;
        payload = coerced.value;
        coerced_confidence = Some(coerced.confidence);
    }

    Ok(StreamedPayloadResolution {
        payload,
        heal_confidence: Some(healed.confidence),
        coerced_confidence,
    })
}

pub(crate) fn parse_streamed_structured_payload(
    raw: &str,
    heal: bool,
    schema: Option<&Value>,
) -> Result<StreamedPayloadResolution, String> {
    if !heal {
        if let Ok(payload) = serde_json::from_str::<Value>(raw) {
            return Ok(StreamedPayloadResolution {
                payload,
                heal_confidence: None,
                coerced_confidence: None,
            });
        }

        let candidate = resolve_structured_json_candidate(raw).ok_or_else(|| {
            "failed to parse streamed structured completion JSON: no JSON object candidate found"
                .to_string()
        })?;
        let payload = serde_json::from_str::<Value>(candidate).map_err(|error| {
            format!(
                "failed to parse streamed structured completion JSON: {error}; candidate={candidate}"
            )
        })?;
        return Ok(StreamedPayloadResolution {
            payload,
            heal_confidence: None,
            coerced_confidence: None,
        });
    }

    let resolved = resolve_structured_json_candidate(raw);
    if let Some(candidate) = resolved {
        if let Some(ref merged) = try_merge_leading_object_comma_continuation(raw, candidate) {
            if let Ok(resolution) = heal_and_coerce_stream_payload(merged.as_str(), schema) {
                return Ok(resolution);
            }
        }
    }

    let primary = resolved.unwrap_or(raw);
    match heal_and_coerce_stream_payload(primary, schema) {
        Ok(resolution) => Ok(resolution),
        Err(primary_err) => match resolved {
            Some(candidate) if candidate != raw => {
                heal_and_coerce_stream_payload(raw, schema).map_err(|raw_err| {
                    format!("{primary_err}; full-text retry: {raw_err}")
                })
            }
            _ => Err(primary_err),
        },
    }
}

pub(crate) fn convert_json_schema_to_healing_schema(schema: &Value) -> Result<Schema, String> {
    let type_value = schema.get("type");
    match type_value {
        Some(Value::String(type_str)) => convert_typed_schema(schema, type_str),
        Some(Value::Array(types)) => {
            let mut variants = Vec::with_capacity(types.len());
            for t in types {
                let Some(type_str) = t.as_str() else {
                    return Err("invalid JSON Schema union type entry".to_string());
                };
                let mut single = schema.clone();
                single["type"] = Value::String(type_str.to_string());
                variants.push(convert_json_schema_to_healing_schema(&single)?);
            }
            Ok(Schema::Union(variants))
        }
        None => Ok(Schema::Any),
        _ => Err("invalid JSON Schema `type` field".to_string()),
    }
}

fn convert_typed_schema(schema: &Value, type_str: &str) -> Result<Schema, String> {
    match type_str {
        "string" => Ok(Schema::String),
        "integer" => Ok(Schema::Int),
        "number" => Ok(Schema::Float),
        "boolean" => Ok(Schema::Bool),
        "null" => Ok(Schema::Null),
        "array" => {
            let items = schema
                .get("items")
                .ok_or_else(|| "array schema requires `items`".to_string())?;
            Ok(Schema::array(convert_json_schema_to_healing_schema(items)?))
        }
        "object" => {
            let props = schema
                .get("properties")
                .and_then(Value::as_object)
                .cloned()
                .unwrap_or_default();
            let required = schema
                .get("required")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let required_names: Vec<String> = required
                .into_iter()
                .filter_map(|v| v.as_str().map(ToString::to_string))
                .collect();
            let mut fields = Vec::with_capacity(props.len());
            for (name, field_schema) in props {
                let is_required = required_names.iter().any(|required| required == &name);
                fields.push((
                    name,
                    convert_json_schema_to_healing_schema(&field_schema)?,
                    is_required,
                ));
            }
            Ok(Schema::object(fields))
        }
        _ => Ok(Schema::Any),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn streamed_payload_heal_full_raw_when_first_object_omits_trailing_required() {
        let raw = r#"{"state":"capabilities_query"}, "reason": "short""#;
        let schema = serde_json::json!({
            "type": "object",
            "required": ["state", "reason"],
            "properties": {
                "state": { "type": "string" },
                "reason": { "type": "string" }
            }
        });
        let result = parse_streamed_structured_payload(raw, true, Some(&schema)).expect("full retry");
        assert_eq!(
            result.payload.get("state").and_then(Value::as_str),
            Some("capabilities_query")
        );
        assert_eq!(result.payload.get("reason").and_then(Value::as_str), Some("short"));
    }

    #[test]
    fn streamed_payload_parser_extracts_first_json_object() {
        let raw = "Some thinking text here...\n\n{\"subject\":\"hello\",\"body\":\"world\"}";
        let result = parse_streamed_structured_payload(raw, false, None).expect("should parse");
        assert_eq!(
            result.payload.get("subject").unwrap().as_str(),
            Some("hello")
        );
        assert!(result.heal_confidence.is_none());
    }

    #[test]
    fn streamed_payload_parser_prefers_outermost_object_over_nested() {
        let raw = r#"{"status":"success","message":"done","details":{"id":"123","name":"test"}}"#;
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "status": { "type": "string" },
                "message": { "type": "string" },
                "details": {
                    "type": "object",
                    "properties": {
                        "id": { "type": "string" },
                        "name": { "type": "string" }
                    },
                    "required": ["id", "name"]
                }
            },
            "required": ["status", "message", "details"]
        });
        let result = parse_streamed_structured_payload(raw, true, Some(&schema))
            .expect("should coerce outer object");
        assert_eq!(
            result.payload.get("status").and_then(Value::as_str),
            Some("success")
        );
        assert_eq!(
            result.payload.get("message").and_then(Value::as_str),
            Some("done")
        );
        assert!(result.payload.get("details").unwrap().is_object());
    }

    #[test]
    fn streamed_payload_parser_handles_unbalanced_reasoning_before_json() {
        let raw = "{ thoughts that don't close... ```json\n{\"key\":\"value\"}\n```";
        let result = parse_streamed_structured_payload(raw, false, None).expect("should parse");
        assert_eq!(result.payload.get("key").unwrap().as_str(), Some("value"));
    }

    #[test]
    fn streamed_payload_parser_handles_markdown_with_heal() {
        let raw = "Let me think... ```json\n{\"result\": \"ok\"}\n```";
        let result = parse_streamed_structured_payload(raw, true, None).expect("should parse");
        assert_eq!(result.payload.get("result").unwrap().as_str(), Some("ok"));
        assert!(result.heal_confidence.is_some());
        assert!(result.coerced_confidence.is_none());
    }

    #[test]
    fn streamed_payload_parser_heal_with_schema_coerces_and_sets_coerced_confidence() {
        let raw = r#"{"age":"30","label":"ok"}"#;
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "age": { "type": "integer" },
                "label": { "type": "string" }
            },
            "required": ["age", "label"]
        });
        let result =
            parse_streamed_structured_payload(raw, true, Some(&schema)).expect("should coerce");
        assert_eq!(result.payload.get("age").and_then(Value::as_i64), Some(30));
        assert_eq!(
            result.payload.get("label").and_then(Value::as_str),
            Some("ok")
        );
        assert!(result.heal_confidence.is_some());
        assert!(result.coerced_confidence.is_some());
    }

    #[test]
    fn convert_json_schema_to_healing_schema_rejects_invalid_union_entry() {
        let schema = serde_json::json!({
            "type": ["string", 42]
        });
        let err = convert_json_schema_to_healing_schema(&schema)
            .expect_err("invalid union entry should fail");
        assert!(err.contains("invalid JSON Schema union type entry"));
    }

    #[test]
    fn streamed_payload_parser_errors_when_no_json_candidate_exists() {
        let raw = "plain text with no json at all";
        let err = parse_streamed_structured_payload(raw, false, None)
            .expect_err("should fail without JSON");
        assert!(err.contains("no JSON object candidate found"));
    }

    #[test]
    fn structured_json_delta_filter_strips_reasoning_prefix_and_suffix() {
        let mut filter = StructuredJsonDeltaFilter::default();
        let (out, think) = filter.split("thinking...");
        assert!(out.is_none());
        assert_eq!(think.as_deref(), Some("thinking..."));

        let (out, think) = filter.split("{\"key\":\"value\"}");
        assert_eq!(out.as_deref(), Some("{\"key\":\"value\"}"));
        assert!(think.is_none());
        assert!(filter.completed());

        let (out, think) = filter.split("more text");
        assert!(out.is_none());
        assert_eq!(think.as_deref(), Some("more text"));
    }

    #[test]
    fn structured_json_delta_filter_handles_braces_inside_strings() {
        let mut filter = StructuredJsonDeltaFilter::default();
        let (out, _) = filter.split("{\"brace\":\"{}\"}");
        assert_eq!(out.as_deref(), Some("{\"brace\":\"{}\"}"));
        assert!(filter.completed());
    }

    #[test]
    fn render_json_object_as_text_converts_top_level_fields() {
        let json = "{\"subject\":\"hello\",\"body\":\"world\"}";
        let text = render_json_object_as_text(json);
        assert!(text.contains("subject: hello"));
        assert!(text.contains("body: world"));
    }

    #[test]
    fn stream_json_as_text_formatter_emits_once_when_complete() {
        let mut formatter = StreamJsonAsTextFormatter::default();
        formatter.push("{\"a\":\"1\"");
        assert!(formatter.emit_if_ready(false).is_none());
        formatter.push("}");
        let first = formatter.emit_if_ready(true);
        assert!(first.is_some());
        assert!(formatter.emit_if_ready(true).is_none());
    }
}
