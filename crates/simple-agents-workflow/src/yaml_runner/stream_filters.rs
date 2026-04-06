use serde_json::Value;
use simple_agents_healing::JsonishParser;

#[derive(Debug)]
pub(crate) struct StreamedPayloadResolution {
    pub(crate) payload: Value,
    pub(crate) heal_confidence: Option<f32>,
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

pub(crate) fn extract_last_parsable_object(raw: &str) -> Option<&str> {
    let starts: Vec<usize> = raw
        .char_indices()
        .filter_map(|(index, ch)| if ch == '{' { Some(index) } else { None })
        .collect();

    for start in starts.into_iter().rev() {
        let Some(candidate) = extract_balanced_object_from(raw, start) else {
            continue;
        };
        if serde_json::from_str::<Value>(candidate).is_ok() {
            return Some(candidate);
        }
    }

    None
}

pub(crate) fn resolve_structured_json_candidate(raw: &str) -> Option<&str> {
    extract_last_fenced_json_block(raw).or_else(|| extract_last_parsable_object(raw))
}

pub(crate) fn parse_streamed_structured_payload(
    raw: &str,
    heal: bool,
) -> Result<StreamedPayloadResolution, String> {
    if !heal {
        if let Ok(payload) = serde_json::from_str::<Value>(raw) {
            return Ok(StreamedPayloadResolution {
                payload,
                heal_confidence: None,
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
        });
    }

    let candidate = resolve_structured_json_candidate(raw).unwrap_or(raw);
    let parser = JsonishParser::new();
    let healed = parser
        .parse(candidate)
        .map_err(|error| format!("failed to heal streamed structured completion JSON: {error}"))?;

    Ok(StreamedPayloadResolution {
        payload: healed.value,
        heal_confidence: Some(healed.confidence),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn streamed_payload_parser_extracts_last_json_object() {
        let raw = "Some thinking text here...\n\n{\"subject\":\"hello\",\"body\":\"world\"}";
        let result = parse_streamed_structured_payload(raw, false).expect("should parse");
        assert_eq!(
            result.payload.get("subject").unwrap().as_str(),
            Some("hello")
        );
        assert!(result.heal_confidence.is_none());
    }

    #[test]
    fn streamed_payload_parser_handles_unbalanced_reasoning_before_json() {
        let raw = "{ thoughts that don't close... ```json\n{\"key\":\"value\"}\n```";
        let result = parse_streamed_structured_payload(raw, false).expect("should parse");
        assert_eq!(result.payload.get("key").unwrap().as_str(), Some("value"));
    }

    #[test]
    fn streamed_payload_parser_handles_markdown_with_heal() {
        let raw = "Let me think... ```json\n{\"result\": \"ok\"}\n```";
        let result = parse_streamed_structured_payload(raw, true).expect("should parse");
        assert_eq!(result.payload.get("result").unwrap().as_str(), Some("ok"));
        assert!(result.heal_confidence.is_some());
    }

    #[test]
    fn streamed_payload_parser_errors_when_no_json_candidate_exists() {
        let raw = "plain text with no json at all";
        let err =
            parse_streamed_structured_payload(raw, false).expect_err("should fail without JSON");
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
