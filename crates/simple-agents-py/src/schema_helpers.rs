use crate::PySchema;
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use pyo3::types::PyDict;
use serde_json::Value;
use simple_agent_type::prelude::SimpleAgentsError;
use simple_agents_healing::schema::{Field, ObjectSchema, StreamAnnotation};
use simple_agents_healing::Schema;

pub(crate) fn parse_schema_from_py(
    py: Python<'_>,
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
                parse_schema_from_py(py, items_obj, None)?
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
            _ => Ok(Schema::Any),
        },
        serde_json::Value::Object(map) => {
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
                                    stream_annotation: StreamAnnotation::Normal,
                                });
                            }
                        }
                        Ok(Schema::Object(ObjectSchema {
                            fields,
                            allow_additional_fields: true,
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
                    "integer" => Ok(Schema::Int),
                    "boolean" => Ok(Schema::Bool),
                    "null" => Ok(Schema::Null),
                    _ => Ok(Schema::Any),
                }
            } else {
                Ok(Schema::Any)
            }
        }
        _ => Ok(Schema::Any),
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
        serde_json::Value::Object(map) => {
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
                    let schema = schema_from_json_schema_value(val)
                        .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
                    fields.push((key.clone(), schema, is_required));
                }
            }
            Ok(Schema::object(fields))
        }
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
