//! Typed N-API shapes for `workflow_options` (mirrors Go `TypedWorkflowRunOptions` JSON).

use std::collections::HashMap;

use napi::bindgen_prelude::{Error, Result};
use napi_derive::napi;
use serde_json::{Map, Value as JsonValue};

#[napi(object)]
#[derive(Clone, Debug, Default)]
pub struct WorkflowTelemetryConfigNapi {
    pub enabled: Option<bool>,
    pub nerdstats: Option<bool>,
    pub sample_rate: Option<f64>,
    pub payload_mode: Option<String>,
    pub retention_days: Option<u32>,
    pub multi_tenant: Option<bool>,
    pub tool_trace_mode: Option<String>,
}

#[napi(object)]
#[derive(Clone, Debug, Default)]
pub struct WorkflowTraceContextNapi {
    pub trace_id: Option<String>,
    pub span_id: Option<String>,
    pub parent_span_id: Option<String>,
    pub traceparent: Option<String>,
    pub tracestate: Option<String>,
    pub baggage: Option<HashMap<String, String>>,
}

#[napi(object)]
#[derive(Clone, Debug, Default)]
pub struct WorkflowTraceTenantNapi {
    pub workspace_id: Option<String>,
    pub user_id: Option<String>,
    pub conversation_id: Option<String>,
    pub request_id: Option<String>,
    pub run_id: Option<String>,
}

#[napi(object)]
#[derive(Clone, Debug, Default)]
pub struct WorkflowTraceConfigNapi {
    pub context: Option<WorkflowTraceContextNapi>,
    pub tenant: Option<WorkflowTraceTenantNapi>,
}

/// Run options for [`super::WorkflowYamlRunRequest`] and related APIs (no arbitrary top-level keys;
/// matches Rust `YamlWorkflowRunOptions` + `include_events` for request parsing).
#[napi(object)]
#[derive(Clone, Debug, Default)]
pub struct WorkflowRunOptionsNapi {
    pub model: Option<String>,
    pub telemetry: Option<WorkflowTelemetryConfigNapi>,
    pub trace: Option<WorkflowTraceConfigNapi>,
    pub include_events: Option<bool>,
}

fn telemetry_to_value(t: &WorkflowTelemetryConfigNapi) -> JsonValue {
    let mut m = Map::new();
    if let Some(v) = t.enabled {
        m.insert("enabled".into(), JsonValue::Bool(v));
    }
    if let Some(v) = t.nerdstats {
        m.insert("nerdstats".into(), JsonValue::Bool(v));
    }
    if let Some(v) = t.sample_rate {
        m.insert("sample_rate".into(), serde_json::json!(v));
    }
    if let Some(ref s) = t.payload_mode {
        m.insert("payload_mode".into(), JsonValue::String(s.clone()));
    }
    if let Some(v) = t.retention_days {
        m.insert("retention_days".into(), JsonValue::Number(v.into()));
    }
    if let Some(v) = t.multi_tenant {
        m.insert("multi_tenant".into(), JsonValue::Bool(v));
    }
    if let Some(ref s) = t.tool_trace_mode {
        m.insert("tool_trace_mode".into(), JsonValue::String(s.clone()));
    }
    JsonValue::Object(m)
}

fn trace_context_to_value(c: &WorkflowTraceContextNapi) -> Result<JsonValue> {
    let mut m = Map::new();
    if let Some(ref s) = c.trace_id {
        m.insert("trace_id".into(), JsonValue::String(s.clone()));
    }
    if let Some(ref s) = c.span_id {
        m.insert("span_id".into(), JsonValue::String(s.clone()));
    }
    if let Some(ref s) = c.parent_span_id {
        m.insert("parent_span_id".into(), JsonValue::String(s.clone()));
    }
    if let Some(ref s) = c.traceparent {
        m.insert("traceparent".into(), JsonValue::String(s.clone()));
    }
    if let Some(ref s) = c.tracestate {
        m.insert("tracestate".into(), JsonValue::String(s.clone()));
    }
    if let Some(ref b) = c.baggage {
        if !b.is_empty() {
            m.insert(
                "baggage".into(),
                serde_json::to_value(b).map_err(|error| {
                    Error::from_reason(format!("trace.context.baggage serialization failed: {error}"))
                })?,
            );
        }
    }
    Ok(JsonValue::Object(m))
}

fn trace_tenant_to_value(t: &WorkflowTraceTenantNapi) -> JsonValue {
    let mut m = Map::new();
    if let Some(ref s) = t.workspace_id {
        m.insert("workspace_id".into(), JsonValue::String(s.clone()));
    }
    if let Some(ref s) = t.user_id {
        m.insert("user_id".into(), JsonValue::String(s.clone()));
    }
    if let Some(ref s) = t.conversation_id {
        m.insert("conversation_id".into(), JsonValue::String(s.clone()));
    }
    if let Some(ref s) = t.request_id {
        m.insert("request_id".into(), JsonValue::String(s.clone()));
    }
    if let Some(ref s) = t.run_id {
        m.insert("run_id".into(), JsonValue::String(s.clone()));
    }
    JsonValue::Object(m)
}

fn trace_to_value(t: &WorkflowTraceConfigNapi) -> Result<JsonValue> {
    let mut m = Map::new();
    if let Some(ref ctx) = t.context {
        m.insert("context".into(), trace_context_to_value(ctx)?);
    }
    match &t.tenant {
        Some(tenant) => {
            m.insert("tenant".into(), trace_tenant_to_value(tenant));
        }
        None => {
            m.insert("tenant".into(), JsonValue::Object(Map::new()));
        }
    }
    Ok(JsonValue::Object(m))
}

/// Serialize typed options to JSON for [`crate::workflow_helpers::parse_workflow_request_options`].
pub(crate) fn workflow_run_options_napi_to_json(
    opt: Option<WorkflowRunOptionsNapi>,
) -> Result<Option<JsonValue>> {
    let Some(opts) = opt else {
        return Ok(None);
    };
    let mut map = Map::new();
    if let Some(m) = opts.model {
        map.insert("model".into(), JsonValue::String(m));
    }
    if let Some(ref t) = opts.telemetry {
        map.insert("telemetry".into(), telemetry_to_value(t));
    }
    if let Some(ref tr) = opts.trace {
        map.insert("trace".into(), trace_to_value(tr)?);
    }
    if let Some(ie) = opts.include_events {
        map.insert("include_events".into(), JsonValue::Bool(ie));
    }
    if map.is_empty() {
        Ok(None)
    } else {
        Ok(Some(JsonValue::Object(map)))
    }
}
