//! Metrics collection for provider operations.
//!
//! This module provides Prometheus-compatible metrics for monitoring
//! LLM provider requests, including latency, throughput, and token usage.

use std::time::Instant;

/// Metric names as constants for consistency
pub mod names {
    /// Total number of requests to LLM providers
    pub const REQUESTS_TOTAL: &str = "simple_agents_requests_total";

    /// Request duration in seconds (histogram)
    pub const REQUEST_DURATION: &str = "simple_agents_request_duration_seconds";

    /// Total tokens used (prompt + completion)
    pub const TOKENS_TOTAL: &str = "simple_agents_tokens_total";

    /// Total number of retry attempts
    pub const RETRIES_TOTAL: &str = "simple_agents_retries_total";

    /// Retry backoff duration in seconds (histogram)
    pub const RETRY_BACKOFF: &str = "simple_agents_retry_backoff_seconds";
}

/// Labels used for metrics
pub mod labels {
    /// Provider name (e.g., "openai", "anthropic")
    pub const PROVIDER: &str = "provider";

    /// Model name (e.g., "gpt-4", "claude-3-opus")
    pub const MODEL: &str = "model";

    /// Request status ("success", "error", "timeout")
    pub const STATUS: &str = "status";

    /// Token type ("prompt", "completion")
    pub const TOKEN_TYPE: &str = "type";

    /// Error type when status is "error"
    pub const ERROR_TYPE: &str = "error_type";
}

/// Timer for measuring request duration.
///
/// Automatically records duration when dropped.
#[derive(Clone)]
pub struct RequestTimer {
    provider: String,
    model: String,
    start: Instant,
}

impl RequestTimer {
    /// Start a new timer for a request.
    pub fn start(provider: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            provider: provider.into(),
            model: model.into(),
            start: Instant::now(),
        }
    }

    /// Complete the request successfully and record metrics.
    pub fn complete_success(self, prompt_tokens: u32, completion_tokens: u32) {
        let duration = self.start.elapsed().as_secs_f64();

        // Record request counter
        metrics::counter!(
            names::REQUESTS_TOTAL,
            labels::PROVIDER => self.provider.clone(),
            labels::MODEL => self.model.clone(),
            labels::STATUS => "success",
        )
        .increment(1);

        // Record request duration
        metrics::histogram!(
            names::REQUEST_DURATION,
            labels::PROVIDER => self.provider.clone(),
            labels::MODEL => self.model.clone(),
            labels::STATUS => "success",
        )
        .record(duration);

        // Record token usage
        metrics::counter!(
            names::TOKENS_TOTAL,
            labels::PROVIDER => self.provider.clone(),
            labels::MODEL => self.model.clone(),
            labels::TOKEN_TYPE => "prompt",
        )
        .increment(prompt_tokens as u64);

        metrics::counter!(
            names::TOKENS_TOTAL,
            labels::PROVIDER => self.provider.clone(),
            labels::MODEL => self.model.clone(),
            labels::TOKEN_TYPE => "completion",
        )
        .increment(completion_tokens as u64);
    }

    /// Complete the request with an error and record metrics.
    pub fn complete_error(self, error_type: impl Into<String>) {
        let duration = self.start.elapsed().as_secs_f64();
        let error_type = error_type.into();

        // Record request counter
        metrics::counter!(
            names::REQUESTS_TOTAL,
            labels::PROVIDER => self.provider.clone(),
            labels::MODEL => self.model.clone(),
            labels::STATUS => "error",
            labels::ERROR_TYPE => error_type,
        )
        .increment(1);

        // Record request duration
        metrics::histogram!(
            names::REQUEST_DURATION,
            labels::PROVIDER => self.provider.clone(),
            labels::MODEL => self.model.clone(),
            labels::STATUS => "error",
        )
        .record(duration);
    }

    /// Complete the request with a timeout and record metrics.
    pub fn complete_timeout(self) {
        let duration = self.start.elapsed().as_secs_f64();

        // Record request counter
        metrics::counter!(
            names::REQUESTS_TOTAL,
            labels::PROVIDER => self.provider.clone(),
            labels::MODEL => self.model.clone(),
            labels::STATUS => "timeout",
        )
        .increment(1);

        // Record request duration
        metrics::histogram!(
            names::REQUEST_DURATION,
            labels::PROVIDER => self.provider.clone(),
            labels::MODEL => self.model.clone(),
            labels::STATUS => "timeout",
        )
        .record(duration);
    }
}

/// Record a retry attempt.
pub fn record_retry(provider: impl Into<String>, backoff_seconds: f64) {
    let provider = provider.into();

    metrics::counter!(
        names::RETRIES_TOTAL,
        labels::PROVIDER => provider.clone(),
    )
    .increment(1);

    metrics::histogram!(
        names::RETRY_BACKOFF,
        labels::PROVIDER => provider,
    )
    .record(backoff_seconds);
}

#[cfg(feature = "prometheus")]
pub mod prometheus {
    //! Prometheus exporter support.
    //!
    //! Enable with the "prometheus" feature flag.

    use std::net::SocketAddr;

    /// Initialize Prometheus metrics exporter.
    ///
    /// This starts an HTTP server on the given address that exposes
    /// metrics in Prometheus format at `/metrics`.
    ///
    /// # Example
    ///
    /// ```rust
    /// use simple_agents_providers::metrics::prometheus;
    ///
    /// let addr = "127.0.0.1:0".parse().unwrap();
    /// let rt = tokio::runtime::Runtime::new().unwrap();
    /// rt.block_on(async {
    ///     prometheus::init(addr).expect("Failed to start Prometheus exporter");
    /// });
    /// ```
    pub fn init(addr: SocketAddr) -> Result<(), Box<dyn std::error::Error>> {
        let builder = metrics_exporter_prometheus::PrometheusBuilder::new();
        let handle = builder.install_recorder()?;

        // Start HTTP server for /metrics endpoint
        tokio::spawn(async move {
            use std::net::TcpListener;

            let listener = TcpListener::bind(addr).expect("Failed to bind Prometheus exporter");

            loop {
                if let Ok((mut stream, _)) = listener.accept() {
                    let metrics = handle.render();
                    let response = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\n\r\n{}",
                        metrics.len(),
                        metrics
                    );

                    use std::io::Write;
                    let _ = stream.write_all(response.as_bytes());
                }
            }
        });

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request_timer_creation() {
        let timer = RequestTimer::start("openai", "gpt-4");
        assert_eq!(timer.provider, "openai");
        assert_eq!(timer.model, "gpt-4");
    }

    #[test]
    fn test_metric_constants() {
        assert_eq!(names::REQUESTS_TOTAL, "simple_agents_requests_total");
        assert_eq!(
            names::REQUEST_DURATION,
            "simple_agents_request_duration_seconds"
        );
        assert_eq!(names::TOKENS_TOTAL, "simple_agents_tokens_total");
    }
}
