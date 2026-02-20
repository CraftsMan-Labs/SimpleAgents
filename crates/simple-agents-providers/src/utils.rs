//! Shared utilities for provider implementations.

use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use std::borrow::Cow;
use std::time::{Duration, SystemTime};

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
