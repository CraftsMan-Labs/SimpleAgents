//! Shared utilities for provider implementations.

use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use std::borrow::Cow;
use std::time::{Duration, SystemTime};

/// Default timeout for HTTP requests
#[allow(dead_code)]
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

/// Default maximum retries
#[allow(dead_code)]
pub const DEFAULT_MAX_RETRIES: u32 = 3;

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
#[allow(dead_code)]
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
}
