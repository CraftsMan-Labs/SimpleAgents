use std::collections::BTreeMap;

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
