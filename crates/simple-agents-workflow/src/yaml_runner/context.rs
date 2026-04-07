use serde_json::Value;
use simple_agent_type::message::{parse_messages_value, Message};

use super::{YamlTemplateBinding, YamlWorkflowRunError};

pub(super) fn evaluate_switch_condition(
    condition: &str,
    context: &Value,
) -> Result<bool, YamlWorkflowRunError> {
    let (left, right) =
        condition
            .split_once("==")
            .ok_or_else(|| YamlWorkflowRunError::UnsupportedCondition {
                condition: condition.to_string(),
            })?;

    let left_path = left.trim().trim_start_matches("$.");
    let right_literal = right.trim().trim_matches('"').trim_matches('\'');
    let left_value = resolve_path(context, left_path);
    Ok(left_value
        .and_then(Value::as_str)
        .map(|value| value == right_literal)
        .unwrap_or(false))
}

pub(super) fn parse_messages_from_context(
    path: &str,
    context: &Value,
) -> Result<Vec<Message>, String> {
    let normalized_path = path.trim().trim_start_matches("$.");
    let value = resolve_path(context, normalized_path)
        .ok_or_else(|| format!("messages_path not found: {path}"))?;
    if value.as_array().is_some_and(|messages| messages.is_empty()) {
        return Err(format!(
            "messages_path must not resolve to an empty list: {path}"
        ));
    }
    parse_messages_value(value)
        .map_err(|err| format!("messages_path must resolve to a list of messages: {path}; {err}"))
}

pub(super) fn resolve_path<'a>(value: &'a Value, path: &str) -> Option<&'a Value> {
    path.split('.')
        .filter(|segment| !segment.is_empty())
        .try_fold(value, |current, segment| {
            if let Ok(index) = segment.parse::<usize>() {
                return current.get(index);
            }
            current.get(segment)
        })
}

pub(crate) fn interpolate_template(template: &str, context: &Value) -> String {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;

    loop {
        let Some(start) = rest.find("{{") else {
            out.push_str(rest);
            break;
        };

        out.push_str(&rest[..start]);
        let after_start = &rest[start + 2..];
        let Some(end) = after_start.find("}}") else {
            out.push_str(&rest[start..]);
            break;
        };

        let expr = after_start[..end].trim();
        let source_path = expr.trim_start_matches("$.");
        let replacement = resolve_path(context, source_path)
            .map(value_to_template_string)
            .unwrap_or_default();
        out.push_str(replacement.as_str());

        rest = &after_start[end + 2..];
    }

    out
}

/// Recursively interpolates `{{ ... }}` templates in every string leaf of `value`, using the same
/// rules as [`interpolate_template`] (e.g. `nodes.foo.output.bar` and `$.input.x`).
pub(crate) fn interpolate_json(value: &Value, context: &Value) -> Value {
    match value {
        Value::String(s) => Value::String(interpolate_template(s, context)),
        Value::Array(items) => Value::Array(
            items
                .iter()
                .map(|item| interpolate_json(item, context))
                .collect(),
        ),
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(k, v)| (k.clone(), interpolate_json(v, context)))
                .collect(),
        ),
        other => other.clone(),
    }
}

pub(super) fn collect_template_bindings(
    template: &str,
    context: &Value,
) -> Vec<YamlTemplateBinding> {
    let mut bindings = Vec::new();
    let mut rest = template;

    loop {
        let Some(start) = rest.find("{{") else {
            break;
        };

        let after_start = &rest[start + 2..];
        let Some(end) = after_start.find("}}") else {
            break;
        };

        let expr = after_start[..end].trim();
        let source_path = expr.trim_start_matches("$.").to_string();
        let resolved = resolve_path(context, source_path.as_str()).cloned();
        let missing = resolved.is_none();
        let resolved_value = resolved.unwrap_or(Value::Null);
        bindings.push(YamlTemplateBinding {
            index: bindings.len(),
            expression: expr.to_string(),
            source_path,
            resolved_type: json_type_name(&resolved_value).to_string(),
            missing,
            resolved: resolved_value,
        });

        rest = &after_start[end + 2..];
    }

    bindings
}

pub(super) fn json_type_name(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "bool",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

fn value_to_template_string(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::Bool(v) => v.to_string(),
        Value::Number(v) => v.to_string(),
        Value::String(v) => v.clone(),
        Value::Array(_) | Value::Object(_) => serde_json::to_string(value).unwrap_or_default(),
    }
}
