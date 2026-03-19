use std::collections::{BTreeMap, HashMap};
use std::sync::{Once, OnceLock};

use opentelemetry::global;
use opentelemetry::propagation::{Extractor, TextMapPropagator};
use opentelemetry::trace::{Span as OtelSpan, Tracer};
use opentelemetry::{Context, KeyValue};
use opentelemetry_otlp::{Protocol, WithExportConfig};
use opentelemetry_sdk::trace::TracerProvider as SdkTracerProvider;
use opentelemetry_sdk::{propagation::TraceContextPropagator, Resource};
use tonic::metadata::{AsciiMetadataKey, MetadataMap, MetadataValue};

pub const ENV_TRACING_ENABLED: &str = "SIMPLE_AGENTS_TRACING_ENABLED";
pub const ENV_OTLP_ENDPOINT: &str = "OTEL_EXPORTER_OTLP_ENDPOINT";
pub const ENV_OTLP_PROTOCOL: &str = "OTEL_EXPORTER_OTLP_PROTOCOL";
pub const ENV_OTLP_HEADERS: &str = "OTEL_EXPORTER_OTLP_HEADERS";
pub const ENV_SERVICE_NAME: &str = "OTEL_SERVICE_NAME";

const DEFAULT_SERVICE_NAME: &str = "simple-agents-workflow";
const DEFAULT_GRPC_ENDPOINT: &str = "http://localhost:4317";
const DEFAULT_HTTP_ENDPOINT: &str = "http://localhost:4318";

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OtlpProtocol {
    Grpc,
    HttpProtobuf,
}

impl OtlpProtocol {
    fn parse(value: &str) -> Result<Self, String> {
        let normalized = value.trim().to_ascii_lowercase();
        match normalized.as_str() {
            "grpc" => Ok(Self::Grpc),
            "http/protobuf" => Ok(Self::HttpProtobuf),
            _ => Err(format!(
                "{ENV_OTLP_PROTOCOL} must be one of: grpc, http/protobuf"
            )),
        }
    }

    fn to_otlp_protocol(self) -> Protocol {
        match self {
            Self::Grpc => Protocol::Grpc,
            Self::HttpProtobuf => Protocol::HttpBinary,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TracingConfig {
    enabled: bool,
    endpoint: String,
    protocol: OtlpProtocol,
    headers: HashMap<String, String>,
    service_name: String,
}

impl TracingConfig {
    fn from_env() -> Result<Self, String> {
        Self::from_lookup(|key| std::env::var(key).ok())
    }

    fn from_lookup<F>(lookup: F) -> Result<Self, String>
    where
        F: Fn(&str) -> Option<String>,
    {
        let enabled = lookup(ENV_TRACING_ENABLED)
            .as_deref()
            .map(parse_bool)
            .transpose()?
            .unwrap_or(false);

        let protocol = lookup(ENV_OTLP_PROTOCOL)
            .as_deref()
            .map(OtlpProtocol::parse)
            .transpose()?
            .unwrap_or(OtlpProtocol::Grpc);

        let endpoint = lookup(ENV_OTLP_ENDPOINT).unwrap_or_else(|| match protocol {
            OtlpProtocol::Grpc => DEFAULT_GRPC_ENDPOINT.to_string(),
            OtlpProtocol::HttpProtobuf => DEFAULT_HTTP_ENDPOINT.to_string(),
        });

        let service_name = lookup(ENV_SERVICE_NAME)
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_SERVICE_NAME.to_string());

        let headers = lookup(ENV_OTLP_HEADERS)
            .as_deref()
            .map(parse_headers)
            .transpose()?
            .unwrap_or_default();

        Ok(Self {
            enabled,
            endpoint,
            protocol,
            headers,
            service_name,
        })
    }
}

fn parse_bool(value: &str) -> Result<bool, String> {
    let normalized = value.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "1" | "true" | "yes" | "on" => Ok(true),
        "0" | "false" | "no" | "off" => Ok(false),
        _ => Err(format!(
            "{ENV_TRACING_ENABLED} must be a boolean-like value (true/false/1/0/yes/no/on/off)"
        )),
    }
}

fn parse_headers(raw: &str) -> Result<HashMap<String, String>, String> {
    let mut headers = HashMap::new();
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(headers);
    }

    for part in trimmed.split(',') {
        let entry = part.trim();
        if entry.is_empty() {
            continue;
        }
        let Some((name, value)) = entry.split_once('=') else {
            return Err(format!(
                "{ENV_OTLP_HEADERS} must contain comma-separated key=value entries"
            ));
        };
        let key = name.trim();
        let val = value.trim();
        if key.is_empty() {
            return Err(format!("{ENV_OTLP_HEADERS} contains an empty header name"));
        }
        if val.is_empty() {
            return Err(format!(
                "{ENV_OTLP_HEADERS} header '{key}' has an empty value"
            ));
        }
        headers.insert(key.to_string(), val.to_string());
    }

    Ok(headers)
}

struct TracerProviderFactory;

impl TracerProviderFactory {
    fn build(config: &TracingConfig) -> Result<SdkTracerProvider, String> {
        let exporter = OtlpExporterFactory::build_span_exporter(config)?;

        let provider = SdkTracerProvider::builder()
            .with_batch_exporter(exporter, opentelemetry_sdk::runtime::Tokio)
            .with_config(
                opentelemetry_sdk::trace::Config::default().with_resource(Resource::new(vec![
                    KeyValue::new("service.name", config.service_name.clone()),
                ])),
            )
            .build();

        Ok(provider)
    }
}

struct OtlpExporterFactory;

impl OtlpExporterFactory {
    fn build_span_exporter(
        config: &TracingConfig,
    ) -> Result<opentelemetry_otlp::SpanExporter, String> {
        match config.protocol {
            OtlpProtocol::Grpc => {
                let builder = opentelemetry_otlp::new_exporter()
                    .tonic()
                    .with_protocol(config.protocol.to_otlp_protocol())
                    .with_endpoint(config.endpoint.clone());
                let builder = if config.headers.is_empty() {
                    builder
                } else {
                    builder.with_metadata(grpc_metadata_from_headers(&config.headers)?)
                };
                builder
                    .build_span_exporter()
                    .map_err(|error| error.to_string())
            }
            OtlpProtocol::HttpProtobuf => opentelemetry_otlp::new_exporter()
                .http()
                .with_protocol(config.protocol.to_otlp_protocol())
                .with_endpoint(normalize_http_traces_endpoint(&config.endpoint))
                .with_headers(config.headers.clone())
                .build_span_exporter()
                .map_err(|error| error.to_string()),
        }
    }
}

fn normalize_http_traces_endpoint(endpoint: &str) -> String {
    let trimmed = endpoint.trim();
    if trimmed.is_empty() {
        return "/v1/traces".to_string();
    }
    let without_trailing_slash = trimmed.trim_end_matches('/');
    if without_trailing_slash.ends_with("/v1/traces") {
        return without_trailing_slash.to_string();
    }
    format!("{without_trailing_slash}/v1/traces")
}

fn grpc_metadata_from_headers(headers: &HashMap<String, String>) -> Result<MetadataMap, String> {
    let mut metadata = MetadataMap::new();
    for (name, value) in headers {
        let key = AsciiMetadataKey::from_bytes(name.as_bytes())
            .map_err(|_| format!("{ENV_OTLP_HEADERS} has invalid gRPC header key '{name}'"))?;
        let metadata_value = MetadataValue::try_from(value.as_str()).map_err(|_| {
            format!("{ENV_OTLP_HEADERS} has invalid gRPC header value for key '{name}'")
        })?;
        metadata.insert(key, metadata_value);
    }
    Ok(metadata)
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
        let config = TracingConfig::from_env()?;
        if !config.enabled {
            return Err("tracing disabled".to_string());
        }

        let provider_result = OTEL_PROVIDER.get_or_init(|| TracerProviderFactory::build(&config));

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
            traceparent: Some(format!(
                "00-{}-{}-{:02x}",
                span_context.trace_id(),
                span_context.span_id(),
                span_context.trace_flags().to_u8()
            )),
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
    use std::collections::HashMap;

    use super::{
        grpc_metadata_from_headers, normalize_http_traces_endpoint, parse_headers,
        NoopWorkflowTracer, OtlpProtocol, SpanKind, TraceContext, TracingConfig, WorkflowTracer,
        ENV_OTLP_ENDPOINT, ENV_OTLP_HEADERS, ENV_OTLP_PROTOCOL, ENV_TRACING_ENABLED,
    };

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

    #[test]
    fn parse_headers_accepts_comma_separated_pairs() {
        let headers = parse_headers("a=b, c=d").expect("headers should parse");
        assert_eq!(headers.get("a").map(String::as_str), Some("b"));
        assert_eq!(headers.get("c").map(String::as_str), Some("d"));
    }

    #[test]
    fn parse_headers_rejects_invalid_entry() {
        let error = parse_headers("invalid").expect_err("invalid header should fail");
        assert!(error.contains(ENV_OTLP_HEADERS));
    }

    #[test]
    fn tracing_config_defaults_to_disabled_grpc() {
        let config = TracingConfig::from_lookup(|_| None).expect("config should parse");
        assert!(!config.enabled);
        assert_eq!(config.protocol, OtlpProtocol::Grpc);
        assert_eq!(config.endpoint, "http://localhost:4317");
        assert!(config.headers.is_empty());
    }

    #[test]
    fn tracing_config_parses_http_protocol_and_headers() {
        let mut values = HashMap::new();
        values.insert(ENV_TRACING_ENABLED.to_string(), "true".to_string());
        values.insert(ENV_OTLP_PROTOCOL.to_string(), "http/protobuf".to_string());
        values.insert(
            ENV_OTLP_ENDPOINT.to_string(),
            "https://cloud.langfuse.com/api/public/otel".to_string(),
        );
        values.insert(
            ENV_OTLP_HEADERS.to_string(),
            "Authorization=Basic test,x-langfuse-ingestion-version=4".to_string(),
        );

        let config = TracingConfig::from_lookup(|key| values.get(key).cloned())
            .expect("config should parse");

        assert!(config.enabled);
        assert_eq!(config.protocol, OtlpProtocol::HttpProtobuf);
        assert_eq!(
            config.endpoint,
            "https://cloud.langfuse.com/api/public/otel"
        );
        assert_eq!(
            config
                .headers
                .get("x-langfuse-ingestion-version")
                .map(String::as_str),
            Some("4")
        );
    }

    #[test]
    fn tracing_config_rejects_invalid_protocol() {
        let mut values = HashMap::new();
        values.insert(ENV_OTLP_PROTOCOL.to_string(), "http/json".to_string());
        let error = TracingConfig::from_lookup(|key| values.get(key).cloned())
            .expect_err("invalid protocol should fail");
        assert!(error.contains(ENV_OTLP_PROTOCOL));
    }

    #[test]
    fn http_endpoint_normalizer_appends_v1_traces() {
        assert_eq!(
            normalize_http_traces_endpoint("http://localhost:4318"),
            "http://localhost:4318/v1/traces"
        );
        assert_eq!(
            normalize_http_traces_endpoint("http://localhost:4318/"),
            "http://localhost:4318/v1/traces"
        );
        assert_eq!(
            normalize_http_traces_endpoint("https://cloud.langfuse.com/api/public/otel"),
            "https://cloud.langfuse.com/api/public/otel/v1/traces"
        );
        assert_eq!(
            normalize_http_traces_endpoint("https://example.com/v1/traces"),
            "https://example.com/v1/traces"
        );
    }

    #[test]
    fn grpc_metadata_builder_rejects_invalid_keys() {
        let mut headers = HashMap::new();
        headers.insert("Invalid Header".to_string(), "value".to_string());
        let error =
            grpc_metadata_from_headers(&headers).expect_err("invalid metadata key should fail");
        assert!(error.contains(ENV_OTLP_HEADERS));
    }
}
