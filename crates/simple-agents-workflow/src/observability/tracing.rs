use std::collections::{BTreeMap, HashMap};
use std::sync::{Once, OnceLock};

use opentelemetry::global;
use opentelemetry::propagation::{Extractor, TextMapPropagator};
use opentelemetry::trace::{Span as OtelSpan, Tracer};
use opentelemetry::{Context, KeyValue};
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::trace::TracerProvider as SdkTracerProvider;
use opentelemetry_sdk::{propagation::TraceContextPropagator, Resource};

/// OpenTelemetry-friendly trace context carrier.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TraceContext {
    pub trace_id: Option<String>,
    pub span_id: Option<String>,
    pub parent_span_id: Option<String>,
    pub traceparent: Option<String>,
    pub tracestate: Option<String>,
    pub baggage: BTreeMap<String, String>,
}

/// Span operation kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpanKind {
    Workflow,
    Node,
}

/// Trait for a mutable span handle.
pub trait WorkflowSpan: Send {
    fn set_attribute(&mut self, key: &str, value: &str);
    fn add_event(&mut self, name: &str);
    fn end(self: Box<Self>);
}

/// Workflow-level tracing adapter surface.
pub trait WorkflowTracer: Send + Sync {
    fn start_span(
        &self,
        name: &str,
        kind: SpanKind,
        parent: Option<&TraceContext>,
    ) -> (TraceContext, Box<dyn WorkflowSpan>);
}

static WORKFLOW_TRACER: OnceLock<Box<dyn WorkflowTracer>> = OnceLock::new();
static OTEL_PROVIDER: OnceLock<Result<SdkTracerProvider, String>> = OnceLock::new();
static OTEL_GLOBAL_SET: Once = Once::new();

pub fn workflow_tracer() -> &'static dyn WorkflowTracer {
    WORKFLOW_TRACER
        .get_or_init(|| {
            if OtelWorkflowTracer::init_from_env().is_ok() {
                Box::new(OtelWorkflowTracer)
            } else {
                Box::new(NoopWorkflowTracer)
            }
        })
        .as_ref()
}

pub fn flush_workflow_tracer() {
    if let Some(Ok(provider)) = OTEL_PROVIDER.get() {
        let _ = provider.force_flush();
    }
}

/// No-op span for deployments without tracing backends.
#[derive(Debug, Default)]
pub struct NoopWorkflowSpan;

impl WorkflowSpan for NoopWorkflowSpan {
    fn set_attribute(&mut self, _key: &str, _value: &str) {}

    fn add_event(&mut self, _name: &str) {}

    fn end(self: Box<Self>) {}
}

/// No-op tracer for deployments without OpenTelemetry.
#[derive(Debug, Default)]
pub struct NoopWorkflowTracer;

impl WorkflowTracer for NoopWorkflowTracer {
    fn start_span(
        &self,
        _name: &str,
        _kind: SpanKind,
        parent: Option<&TraceContext>,
    ) -> (TraceContext, Box<dyn WorkflowSpan>) {
        (
            parent.cloned().unwrap_or_default(),
            Box::<NoopWorkflowSpan>::default(),
        )
    }
}

#[derive(Debug)]
pub struct OtelWorkflowTracer;

#[derive(Debug)]
pub struct OtelWorkflowSpan {
    inner: opentelemetry::global::BoxedSpan,
}

impl WorkflowSpan for OtelWorkflowSpan {
    fn set_attribute(&mut self, key: &str, value: &str) {
        self.inner
            .set_attribute(KeyValue::new(key.to_string(), value.to_string()));
    }

    fn add_event(&mut self, name: &str) {
        self.inner.add_event(name.to_string(), Vec::new());
    }

    fn end(self: Box<Self>) {
        let mut inner = self.inner;
        inner.end();
    }
}

impl OtelWorkflowTracer {
    fn init_from_env() -> Result<(), String> {
        if !std::env::var("SIMPLE_AGENTS_OTEL_ENABLED")
            .map(|value| value == "1" || value.eq_ignore_ascii_case("true"))
            .unwrap_or(false)
        {
            return Err("otel disabled".to_string());
        }

        let provider_result = OTEL_PROVIDER.get_or_init(|| {
            let endpoint = std::env::var("SIMPLE_AGENTS_OTEL_ENDPOINT")
                .unwrap_or_else(|_| "http://localhost:4317".to_string());
            let service_name = std::env::var("SIMPLE_AGENTS_SERVICE_NAME")
                .unwrap_or_else(|_| "simple-agents-workflow".to_string());

            let exporter = opentelemetry_otlp::new_exporter()
                .tonic()
                .with_endpoint(endpoint)
                .build_span_exporter()
                .map_err(|error| error.to_string())?;

            let provider = SdkTracerProvider::builder()
                .with_batch_exporter(exporter, opentelemetry_sdk::runtime::Tokio)
                .with_config(opentelemetry_sdk::trace::Config::default().with_resource(
                    Resource::new(vec![KeyValue::new("service.name", service_name)]),
                ))
                .build();

            Ok(provider)
        });

        match provider_result {
            Ok(provider) => {
                OTEL_GLOBAL_SET.call_once(|| {
                    global::set_tracer_provider(provider.clone());
                });
                Ok(())
            }
            Err(error) => Err(error.clone()),
        }
    }

    fn parse_parent_context(parent: &TraceContext) -> Option<Context> {
        let mut headers = HashMap::new();
        if let Some(traceparent) = parent.traceparent.as_ref() {
            headers.insert("traceparent".to_string(), traceparent.clone());
        }
        if let Some(tracestate) = parent.tracestate.as_ref() {
            headers.insert("tracestate".to_string(), tracestate.clone());
        }
        if !headers.contains_key("traceparent") {
            if let (Some(trace_id), Some(span_id)) =
                (parent.trace_id.as_ref(), parent.span_id.as_ref())
            {
                let value = format!("00-{trace_id}-{span_id}-01");
                headers.insert("traceparent".to_string(), value);
            }
        }
        if headers.is_empty() {
            return None;
        }

        struct HeaderExtractor<'a> {
            inner: &'a HashMap<String, String>,
        }

        impl Extractor for HeaderExtractor<'_> {
            fn get(&self, key: &str) -> Option<&str> {
                self.inner.get(key).map(String::as_str)
            }

            fn keys(&self) -> Vec<&str> {
                self.inner.keys().map(String::as_str).collect()
            }
        }

        let propagator = TraceContextPropagator::new();
        let extractor = HeaderExtractor { inner: &headers };
        Some(propagator.extract(&extractor))
    }
}

impl WorkflowTracer for OtelWorkflowTracer {
    fn start_span(
        &self,
        name: &str,
        _kind: SpanKind,
        parent: Option<&TraceContext>,
    ) -> (TraceContext, Box<dyn WorkflowSpan>) {
        let tracer = global::tracer("simple-agents-workflow");
        let span = match parent.and_then(Self::parse_parent_context) {
            Some(parent_context) => tracer.start_with_context(name.to_string(), &parent_context),
            None => tracer.start(name.to_string()),
        };
        let span_context = span.span_context().clone();
        let context = TraceContext {
            trace_id: Some(span_context.trace_id().to_string()),
            span_id: Some(span_context.span_id().to_string()),
            parent_span_id: parent.and_then(|value| value.span_id.clone()),
            traceparent: parent.and_then(|value| value.traceparent.clone()),
            tracestate: parent.and_then(|value| value.tracestate.clone()),
            baggage: parent
                .map(|value| value.baggage.clone())
                .unwrap_or_default(),
        };

        (context, Box::new(OtelWorkflowSpan { inner: span }))
    }
}

#[cfg(test)]
mod tests {
    use super::{NoopWorkflowTracer, SpanKind, TraceContext, WorkflowTracer};

    #[test]
    fn noop_tracer_supports_span_lifecycle() {
        let tracer = NoopWorkflowTracer;
        let parent = TraceContext {
            trace_id: Some("trace-1".to_string()),
            ..TraceContext::default()
        };
        let (ctx, mut span) = tracer.start_span("node.llm", SpanKind::Node, Some(&parent));
        assert_eq!(ctx.trace_id.as_deref(), Some("trace-1"));
        span.set_attribute("node.id", "llm");
        span.add_event("start");
        span.end();
    }
}
