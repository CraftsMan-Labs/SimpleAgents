use std::collections::BTreeMap;

use serde_json::{json, Value};

use super::{resolve_path, YamlNode};

pub(super) fn apply_set_globals(
    node: &YamlNode,
    outputs: &BTreeMap<String, Value>,
    workflow_input: &Value,
    globals: &mut serde_json::Map<String, Value>,
) {
    let Some(config) = node.config.as_ref() else {
        return;
    };
    let Some(set_globals) = config.set_globals.as_ref() else {
        return;
    };

    let context = json!({
        "input": workflow_input,
        "nodes": outputs,
        "globals": Value::Object(globals.clone())
    });

    for (key, expr) in set_globals {
        let value = resolve_path(&context, expr.as_str())
            .cloned()
            .unwrap_or(Value::Null);
        globals.insert(key.clone(), value);
    }
}

pub(super) fn apply_update_globals(
    node: &YamlNode,
    outputs: &BTreeMap<String, Value>,
    workflow_input: &Value,
    globals: &mut serde_json::Map<String, Value>,
) {
    let Some(config) = node.config.as_ref() else {
        return;
    };
    let Some(update_globals) = config.update_globals.as_ref() else {
        return;
    };

    let context = json!({
        "input": workflow_input,
        "nodes": outputs,
        "globals": Value::Object(globals.clone())
    });

    for (key, update) in update_globals {
        match update.op.as_str() {
            "set" => {
                if let Some(path) = update.from.as_ref() {
                    let value = resolve_path(&context, path.as_str())
                        .cloned()
                        .unwrap_or(Value::Null);
                    globals.insert(key.clone(), value);
                }
            }
            "append" => {
                if let Some(path) = update.from.as_ref() {
                    let value = resolve_path(&context, path.as_str())
                        .cloned()
                        .unwrap_or(Value::Null);
                    let entry = globals
                        .entry(key.clone())
                        .or_insert_with(|| Value::Array(Vec::new()));
                    match entry {
                        Value::Array(items) => items.push(value),
                        other => {
                            let existing = other.clone();
                            *other = Value::Array(vec![existing, value]);
                        }
                    }
                }
            }
            "increment" => {
                let by = update.by.unwrap_or(1.0);
                let current = globals
                    .get(key.as_str())
                    .and_then(Value::as_f64)
                    .unwrap_or(0.0);
                if let Some(next) = serde_json::Number::from_f64(current + by) {
                    globals.insert(key.clone(), Value::Number(next));
                }
            }
            "merge" => {
                if let Some(path) = update.from.as_ref() {
                    let source = resolve_path(&context, path.as_str())
                        .cloned()
                        .unwrap_or(Value::Null);
                    if let Value::Object(source_map) = source {
                        let target = globals
                            .entry(key.clone())
                            .or_insert_with(|| Value::Object(serde_json::Map::new()));
                        if let Value::Object(target_map) = target {
                            target_map.extend(source_map);
                        } else {
                            *target = Value::Object(source_map);
                        }
                    }
                }
            }
            _ => {}
        }
    }
}
