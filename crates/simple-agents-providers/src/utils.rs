//! Shared utilities for provider implementations.

use crate::SimpleAgentsError;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use simple_agent_type::ProviderError;
use std::borrow::Cow;
use std::time::{Duration, SystemTime};

/// Captured data for a non-success provider HTTP response.
#[derive(Debug)]
pub struct ErrorResponseContext {
    pub status: reqwest::StatusCode,
    pub retry_after: Option<Duration>,
    pub body: String,
    pub headers_debug: Vec<(String, String)>,
}

/// Default timeout for HTTP requests
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

/// Build HTTP headers from key-value pairs (now optimized with Cow)
pub fn build_headers(
    pairs: Vec<(Cow<'static, str>, Cow<'static, str>)>,
) -> Result<HeaderMap, Box<dyn std::error::Error>> {
    let mut headers = HeaderMap::new();

    for (key, value) in pairs {
        let header_name = HeaderName::from_bytes(key.as_bytes())?;
        let header_value = HeaderValue::from_str(&value)?;
        headers.insert(header_name, header_value);
    }

    Ok(headers)
}

/// Parse retry-after header (seconds or HTTP date)
pub fn parse_retry_after(header_value: &str) -> Option<Duration> {
    // Try parsing as integer seconds first
    if let Ok(seconds) = header_value.parse::<u64>() {
        return Some(Duration::from_secs(seconds));
    }

    // Parse RFC 7231 HTTP-date
    if let Ok(retry_at) = httpdate::parse_http_date(header_value) {
        let now = SystemTime::now();
        return Some(
            retry_at
                .duration_since(now)
                .unwrap_or_else(|_| Duration::from_secs(0)),
        );
    }

    None
}

/// Extract and parse retry-after header from response headers.
pub fn retry_after_from_headers(headers: &HeaderMap) -> Option<Duration> {
    headers
        .get(reqwest::header::RETRY_AFTER)
        .and_then(|value| value.to_str().ok())
        .and_then(parse_retry_after)
}

/// Send a JSON HTTP POST request.
pub async fn send_json_request(
    client: &reqwest::Client,
    url: &str,
    headers: HeaderMap,
    body: &serde_json::Value,
) -> std::result::Result<reqwest::Response, reqwest::Error> {
    client.post(url).headers(headers).json(body).send().await
}

/// Read an error response body, falling back to status + read error.
pub async fn read_error_body(response: reqwest::Response) -> String {
    let status = response.status();
    response.text().await.unwrap_or_else(|error| {
        format!("HTTP {} - Could not read response body: {}", status, error)
    })
}

/// Parse a JSON response body into `serde_json::Value`.
pub async fn parse_json_body(
    response: reqwest::Response,
) -> std::result::Result<serde_json::Value, ProviderError> {
    response.json::<serde_json::Value>().await.map_err(|error| {
        ProviderError::InvalidResponse(format!("Failed to parse JSON response: {error}"))
    })
}

/// Return the response on success or capture status/body details on failure.
pub async fn ensure_success_response(
    response: reqwest::Response,
) -> std::result::Result<reqwest::Response, ErrorResponseContext> {
    if response.status().is_success() {
        return Ok(response);
    }

    let status = response.status();
    let retry_after = retry_after_from_headers(response.headers());
    let headers_debug: Vec<(String, String)> = response
        .headers()
        .iter()
        .map(|(key, value)| {
            (
                key.as_str().to_string(),
                value.to_str().unwrap_or("<binary>").to_string(),
            )
        })
        .collect();
    let body = read_error_body(response).await;

    Err(ErrorResponseContext {
        status,
        retry_after,
        body,
        headers_debug,
    })
}

/// Map reqwest transport errors to unified provider errors.
pub fn map_transport_error(error: reqwest::Error) -> SimpleAgentsError {
    if error.is_timeout() {
        SimpleAgentsError::Provider(ProviderError::Timeout(DEFAULT_TIMEOUT))
    } else {
        SimpleAgentsError::Network(format!("Network error: {}", error))
    }
}

/// Map reqwest transport errors and record request timer metrics.
pub fn map_transport_error_with_timer(
    error: reqwest::Error,
    timer: crate::metrics::RequestTimer,
) -> SimpleAgentsError {
    if error.is_timeout() {
        timer.complete_timeout();
    } else {
        timer.complete_error("network");
    }

    map_transport_error(error)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_headers() {
        let headers = build_headers(vec![
            (
                Cow::Borrowed("Authorization"),
                Cow::Borrowed("Bearer sk-test"),
            ),
            (
                Cow::Borrowed("Content-Type"),
                Cow::Borrowed("application/json"),
            ),
        ])
        .unwrap();

        assert_eq!(headers.len(), 2);
        assert_eq!(headers.get("Authorization").unwrap(), "Bearer sk-test");
    }

    #[test]
    fn test_parse_retry_after_seconds() {
        let duration = parse_retry_after("60").unwrap();
        assert_eq!(duration, Duration::from_secs(60));
    }

    #[test]
    fn test_parse_retry_after_invalid() {
        let duration = parse_retry_after("invalid");
        assert!(duration.is_none());
    }

    #[test]
    fn test_parse_retry_after_http_date() {
        let retry_at = SystemTime::now() + Duration::from_secs(90);
        let header = httpdate::fmt_http_date(retry_at);
        let duration = parse_retry_after(&header).unwrap();
        assert!(duration.as_secs() <= 90);
        assert!(duration.as_secs() >= 80);
    }

    #[test]
    fn test_parse_retry_after_negative_like_value_returns_none() {
        assert!(parse_retry_after("-1").is_none());
    }

    #[test]
    fn test_parse_retry_after_malformed_http_date_returns_none() {
        assert!(parse_retry_after("Wed, 99 Foo 9999 99:99:99 GMT").is_none());
    }

    #[test]
    fn test_parse_retry_after_past_http_date_clamps_to_zero() {
        let past = SystemTime::now() - Duration::from_secs(30);
        let header = httpdate::fmt_http_date(past);
        let duration = parse_retry_after(&header).expect("past date should parse");
        assert_eq!(duration, Duration::from_secs(0));
    }

    #[test]
    fn test_parse_retry_after_large_numeric_value() {
        let duration = parse_retry_after("18446744073709551615").expect("u64 max should parse");
        assert_eq!(duration.as_secs(), u64::MAX);
    }

    #[test]
    fn test_retry_after_from_headers() {
        let mut headers = HeaderMap::new();
        headers.insert(reqwest::header::RETRY_AFTER, HeaderValue::from_static("5"));
        assert_eq!(
            retry_after_from_headers(&headers),
            Some(Duration::from_secs(5))
        );
    }

    #[test]
    fn test_build_headers_invalid_name_returns_error() {
        let result = build_headers(vec![(
            Cow::Borrowed("Invalid Header"),
            Cow::Borrowed("value"),
        )]);
        assert!(result.is_err());
    }

    #[test]
    fn test_build_headers_invalid_value_returns_error() {
        let result = build_headers(vec![(
            Cow::Borrowed("x-test"),
            Cow::Borrowed("value\nwith-newline"),
        )]);
        assert!(result.is_err());
    }
}
