//! Telemetry and tracing configuration types for OpenTelemetry integration.

use serde::{Deserialize, Serialize};

/// Which API format to use when communicating with a provider.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub enum ApiFormat {
    /// OpenAI Chat Completions API format.
    #[default]
    ChatCompletions,
    /// OpenAI Responses API format.
    Responses,
}

/// The OTLP export protocol to use.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub enum OtelProtocol {
    /// gRPC protocol.
    #[default]
    Grpc,
    /// HTTP/protobuf protocol.
    HttpProtobuf,
}

/// Configuration for OpenTelemetry telemetry export.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryConfig {
    /// Whether telemetry collection is enabled.
    pub enabled: bool,
    /// OTLP collector endpoint URL.
    pub endpoint: String,
    /// OTLP export protocol (gRPC or HTTP/protobuf).
    pub protocol: OtelProtocol,
    /// Additional headers sent with OTLP exports.
    pub headers: Vec<(String, String)>,
    /// The `service.name` resource attribute.
    pub service_name: String,
}

impl Default for TelemetryConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            endpoint: "http://localhost:4317".into(),
            protocol: OtelProtocol::Grpc,
            headers: vec![],
            service_name: "simple-agents".into(),
        }
    }
}

impl TelemetryConfig {
    /// Build a [`TelemetryConfig`] from standard OpenTelemetry environment variables.
    pub fn from_env() -> Self {
        let enabled = std::env::var("SIMPLE_AGENTS_TRACING_ENABLED")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        let endpoint = std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT")
            .unwrap_or_else(|_| "http://localhost:4317".into());
        let protocol = match std::env::var("OTEL_EXPORTER_OTLP_PROTOCOL").as_deref() {
            Ok("http/protobuf") => OtelProtocol::HttpProtobuf,
            _ => OtelProtocol::Grpc,
        };
        let headers = std::env::var("OTEL_EXPORTER_OTLP_HEADERS")
            .map(|h| {
                h.split(',')
                    .filter_map(|pair| {
                        let (k, v) = pair.split_once('=')?;
                        Some((k.trim().to_string(), v.trim().to_string()))
                    })
                    .collect()
            })
            .unwrap_or_default();
        let service_name =
            std::env::var("OTEL_SERVICE_NAME").unwrap_or_else(|_| "simple-agents".into());
        Self {
            enabled,
            endpoint,
            protocol,
            headers,
            service_name,
        }
    }
}

/// Distributed trace context propagated through agent operations.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TraceContext {
    /// W3C Trace ID.
    pub trace_id: Option<String>,
    /// W3C Span ID.
    pub span_id: Option<String>,
    /// Logical session identifier.
    pub session_id: Option<String>,
    /// User identifier for attribution.
    pub user_id: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_format_default() {
        assert_eq!(ApiFormat::default(), ApiFormat::ChatCompletions);
    }

    #[test]
    fn test_telemetry_config_default() {
        let cfg = TelemetryConfig::default();
        assert!(!cfg.enabled);
        assert_eq!(cfg.endpoint, "http://localhost:4317");
    }

    #[test]
    fn test_telemetry_config_serialization() {
        let cfg = TelemetryConfig::default();
        let json = serde_json::to_string(&cfg).unwrap();
        let parsed: TelemetryConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.endpoint, cfg.endpoint);
    }
}
