use crate::PySchema;
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use pyo3::types::PyDict;
use serde_json::Value;
use simple_agent_type::prelude::SimpleAgentsError;
use simple_agents_healing::schema::{Field, ObjectSchema};
use simple_agents_healing::Schema;

fn unsigned_integer_hint(map: &serde_json::Map<String, Value>) -> bool {
    let minimum_is_non_negative = map
        .get("minimum")
        .and_then(Value::as_f64)
        .map(|minimum| minimum >= 0.0)
        .unwrap_or(false);
    let format_is_unsigned = map
        .get("format")
        .and_then(Value::as_str)
        .map(|format| matches!(format, "uint" | "uint32" | "uint64"))
        .unwrap_or(false);

    minimum_is_non_negative || format_is_unsigned
}

fn parse_union_schemas(
    map: &serde_json::Map<String, Value>,
    key: &str,
) -> std::result::Result<Option<Schema>, SimpleAgentsError> {
    let Some(raw_union) = map.get(key) else {
        return Ok(None);
    };
    let Value::Array(entries) = raw_union else {
        return Err(SimpleAgentsError::Config(format!(
            "JSON schema keyword '{key}' must be an array"
        )));
    };
    if entries.is_empty() {
        return Err(SimpleAgentsError::Config(format!(
            "JSON schema keyword '{key}' cannot be an empty array"
        )));
    }

    let mut variants = Vec::with_capacity(entries.len());
    for entry in entries {
        variants.push(schema_from_json_schema_value(entry)?);
    }
    Ok(Some(Schema::Union(variants)))
}

pub(crate) fn parse_schema_from_py(
    field_type: &Bound<'_, PyAny>,
    items: Option<&Bound<'_, PyAny>>,
) -> PyResult<Schema> {
    if let Ok(schema_ref) = field_type.extract::<PyRef<PySchema>>() {
        return Ok(schema_ref.schema.clone());
    }

    let type_name: String = field_type.extract().map_err(|_| {
        PyRuntimeError::new_err("field_type must be a string or Schema".to_string())
    })?;

    let schema = match type_name.as_str() {
        "string" => Schema::String,
        "integer" => Schema::Int,
        "number" => Schema::Float,
        "boolean" => Schema::Bool,
        "any" => Schema::Any,
        "array" => {
            let item_schema = if let Some(items_obj) = items {
                parse_schema_from_py(items_obj, None)?
            } else {
                Schema::Any
            };
            Schema::Array(Box::new(item_schema))
        }
        "object" => Schema::Object(ObjectSchema {
            fields: Vec::new(),
            allow_additional_fields: true,
        }),
        other => {
            return Err(PyRuntimeError::new_err(format!(
                "Unknown field_type: {}",
                other
            )))
        }
    };

    Ok(schema)
}

pub(crate) fn pydantic_schema_value(schema: &Bound<'_, PyAny>) -> PyResult<Option<Value>> {
    for attr_name in ["model_json_schema", "schema"] {
        if schema.hasattr(attr_name)? {
            let attr = schema.getattr(attr_name)?;
            if attr.is_callable() {
                let result = attr.call0()?;
                let value: Value = pythonize::depythonize(&result).map_err(|e| {
                    PyRuntimeError::new_err(format!("Invalid Pydantic schema: {}", e))
                })?;
                return Ok(Some(value));
            }
        }
    }
    Ok(None)
}

pub(crate) fn schema_to_json_value(schema: &Bound<'_, PyAny>) -> PyResult<Value> {
    if let Some(value) = pydantic_schema_value(schema)? {
        return Ok(value);
    }

    pythonize::depythonize(schema).map_err(|_| {
        PyRuntimeError::new_err(
            "schema must be JSON-serializable (dict) or a Pydantic model/class".to_string(),
        )
    })
}

pub(crate) fn schema_from_json_schema_value(
    value: &serde_json::Value,
) -> std::result::Result<Schema, SimpleAgentsError> {
    match value {
        serde_json::Value::String(type_name) => match type_name.as_str() {
            "string" => Ok(Schema::String),
            "number" => Ok(Schema::Float),
            "integer" => Ok(Schema::Int),
            "boolean" => Ok(Schema::Bool),
            _ => Err(SimpleAgentsError::Config(format!(
                "unsupported JSON schema type '{type_name}'"
            ))),
        },
        serde_json::Value::Object(map) => {
            if let Some(union) = parse_union_schemas(map, "oneOf")? {
                return Ok(union);
            }
            if let Some(union) = parse_union_schemas(map, "anyOf")? {
                return Ok(union);
            }
            if let Some(union) = parse_union_schemas(map, "allOf")? {
                return Ok(union);
            }

            if let Some(serde_json::Value::String(type_name)) = map.get("type") {
                match type_name.as_str() {
                    "object" => {
                        let mut fields = Vec::new();
                        if let Some(serde_json::Value::Object(props_obj)) = map.get("properties") {
                            let required_fields = map.get("required").and_then(|r| {
                                if let serde_json::Value::Array(arr) = r {
                                    Some(arr.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>())
                                } else {
                                    None
                                }
                            });

                            for (key, val) in props_obj {
                                let is_required = required_fields
                                    .as_ref()
                                    .map(|req| req.contains(&key.as_str()))
                                    .unwrap_or(false);
                                let schema = schema_from_json_schema_value(val)?;
                                fields.push(Field {
                                    name: key.clone(),
                                    schema,
                                    required: is_required,
                                    aliases: Vec::new(),
                                    default: None,
                                    description: None,
                                });
                            }
                        }
                        let allow_additional_fields = map
                            .get("additionalProperties")
                            .and_then(Value::as_bool)
                            .unwrap_or(true);
                        Ok(Schema::Object(ObjectSchema {
                            fields,
                            allow_additional_fields,
                        }))
                    }
                    "array" => {
                        if let Some(items) = map.get("items") {
                            Ok(Schema::Array(Box::new(schema_from_json_schema_value(
                                items,
                            )?)))
                        } else {
                            Ok(Schema::Array(Box::new(Schema::Any)))
                        }
                    }
                    "string" => Ok(Schema::String),
                    "number" => Ok(Schema::Float),
                    "integer" => {
                        if unsigned_integer_hint(map) {
                            Ok(Schema::UInt)
                        } else {
                            Ok(Schema::Int)
                        }
                    }
                    "boolean" => Ok(Schema::Bool),
                    "null" => Ok(Schema::Null),
                    _ => Err(SimpleAgentsError::Config(format!(
                        "unsupported JSON schema object type '{type_name}'"
                    ))),
                }
            } else if let Some(serde_json::Value::Array(type_names)) = map.get("type") {
                let mut variants = Vec::with_capacity(type_names.len());
                for type_name in type_names {
                    let serde_json::Value::String(type_name) = type_name else {
                        return Err(SimpleAgentsError::Config(
                            "JSON schema type array must contain only strings".to_string(),
                        ));
                    };
                    variants.push(schema_from_json_schema_value(&Value::String(
                        type_name.clone(),
                    ))?);
                }
                if variants.is_empty() {
                    return Err(SimpleAgentsError::Config(
                        "JSON schema type array cannot be empty".to_string(),
                    ));
                }
                Ok(Schema::Union(variants))
            } else {
                Err(SimpleAgentsError::Config(
                    "unsupported JSON schema object: expected 'type', 'oneOf', 'anyOf', or 'allOf'"
                        .to_string(),
                ))
            }
        }
        _ => Err(SimpleAgentsError::Config(
            "unsupported JSON schema value: expected object or type string".to_string(),
        )),
    }
}

pub(crate) fn schema_from_python_input(schema: &Bound<'_, PyAny>) -> PyResult<Schema> {
    if let Ok(schema_ref) = schema.extract::<PyRef<PySchema>>() {
        return Ok(schema_ref.schema.clone());
    }

    let schema_value: serde_json::Value = if let Some(value) = pydantic_schema_value(schema)? {
        value
    } else {
        let schema_dict: &Bound<'_, PyDict> = schema.downcast().map_err(|_| {
            PyRuntimeError::new_err(
                "schema must be a dict, Schema, or Pydantic model/class".to_string(),
            )
        })?;
        pythonize::depythonize(schema_dict)
            .map_err(|e| PyRuntimeError::new_err(format!("Invalid schema: {}", e)))?
    };

    match schema_value {
        serde_json::Value::Object(_) => schema_from_json_schema_value(&schema_value)
            .map_err(|e| PyRuntimeError::new_err(e.to_string())),
        serde_json::Value::Array(arr) if !arr.is_empty() => {
            let schema = schema_from_json_schema_value(&arr[0])
                .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
            Ok(Schema::Array(Box::new(schema)))
        }
        _ => Err(PyRuntimeError::new_err(format!(
            "Unsupported schema format: {}",
            schema_value
        ))),
    }
}
