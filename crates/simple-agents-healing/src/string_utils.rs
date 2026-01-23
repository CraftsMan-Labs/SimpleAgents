//! String utility functions for case conversion and fuzzy matching.
//!
//! Provides helpers for field name normalization and similarity scoring.

/// Convert a string from camelCase or PascalCase to snake_case.
///
/// # Examples
///
/// ```
/// use simple_agents_healing::string_utils::to_snake_case;
///
/// assert_eq!(to_snake_case("firstName"), "first_name");
/// assert_eq!(to_snake_case("userID"), "user_id");
/// assert_eq!(to_snake_case("HTTPResponse"), "http_response");
/// ```
pub fn to_snake_case(s: &str) -> String {
    let mut result = String::with_capacity(s.len() + 5);
    let mut prev_is_lowercase = false;
    let mut prev_is_uppercase = false;

    for (i, ch) in s.chars().enumerate() {
        if ch.is_uppercase() {
            // Insert underscore before uppercase if:
            // 1. Not at start
            // 2. Previous char was lowercase (camelCase boundary)
            // 3. Next char is lowercase (HTTPResponse -> http_response)
            if i > 0 && (prev_is_lowercase || (prev_is_uppercase && next_is_lowercase(s, i))) {
                result.push('_');
            }
            result.push(ch.to_ascii_lowercase());
            prev_is_uppercase = true;
            prev_is_lowercase = false;
        } else {
            result.push(ch);
            prev_is_lowercase = ch.is_lowercase();
            prev_is_uppercase = false;
        }
    }

    result
}

/// Convert a string from snake_case to camelCase.
///
/// # Examples
///
/// ```
/// use simple_agents_healing::string_utils::to_camel_case;
///
/// assert_eq!(to_camel_case("first_name"), "firstName");
/// assert_eq!(to_camel_case("user_id"), "userId");
/// assert_eq!(to_camel_case("http_response"), "httpResponse");
/// ```
pub fn to_camel_case(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut capitalize_next = false;

    for ch in s.chars() {
        if ch == '_' {
            capitalize_next = true;
        } else if capitalize_next {
            result.push(ch.to_ascii_uppercase());
            capitalize_next = false;
        } else {
            result.push(ch);
        }
    }

    result
}

/// Check if the next character in the string is lowercase.
fn next_is_lowercase(s: &str, current_idx: usize) -> bool {
    s.chars()
        .nth(current_idx + 1)
        .map(|ch| ch.is_lowercase())
        .unwrap_or(false)
}

/// Calculate Jaro-Winkler similarity between two strings.
///
/// Returns a value between 0.0 (no similarity) and 1.0 (identical).
/// The Jaro-Winkler metric gives more weight to strings with matching prefixes.
///
/// # Algorithm
///
/// 1. Calculate Jaro similarity (based on matching characters and transpositions)
/// 2. Apply Winkler modification (boost for common prefix)
///
/// # Examples
///
/// ```
/// use simple_agents_healing::string_utils::jaro_winkler;
///
/// assert!(jaro_winkler("hello", "hello") > 0.99);
/// assert!(jaro_winkler("hello", "hallo") > 0.8);
/// assert!(jaro_winkler("hello", "world") < 0.5);
/// ```
pub fn jaro_winkler(s1: &str, s2: &str) -> f64 {
    if s1 == s2 {
        return 1.0;
    }
    if s1.is_empty() || s2.is_empty() {
        return 0.0;
    }

    // Calculate Jaro similarity first
    let jaro = jaro_similarity(s1, s2);

    // Calculate common prefix length (up to 4 characters)
    let prefix_len = s1
        .chars()
        .zip(s2.chars())
        .take(4)
        .take_while(|(c1, c2)| c1 == c2)
        .count();

    // Apply Winkler modification
    // jaro_winkler = jaro + (prefix_length * p * (1 - jaro))
    // where p = 0.1 (standard scaling factor)
    const P: f64 = 0.1;
    jaro + (prefix_len as f64 * P * (1.0 - jaro))
}

/// Calculate Jaro similarity between two strings.
///
/// This is the base algorithm used by Jaro-Winkler.
/// Returns a value between 0.0 and 1.0.
fn jaro_similarity(s1: &str, s2: &str) -> f64 {
    let s1_chars: Vec<char> = s1.chars().collect();
    let s2_chars: Vec<char> = s2.chars().collect();

    let s1_len = s1_chars.len();
    let s2_len = s2_chars.len();

    if s1_len == 0 || s2_len == 0 {
        return 0.0;
    }

    // Calculate match window (max distance for characters to be considered matching)
    let match_distance = (s1_len.max(s2_len) / 2).saturating_sub(1);

    let mut s1_matches = vec![false; s1_len];
    let mut s2_matches = vec![false; s2_len];

    let mut matches = 0;
    let mut transpositions = 0;

    // Find matching characters
    for i in 0..s1_len {
        let start = i.saturating_sub(match_distance);
        let end = (i + match_distance + 1).min(s2_len);

        for j in start..end {
            if s2_matches[j] || s1_chars[i] != s2_chars[j] {
                continue;
            }
            s1_matches[i] = true;
            s2_matches[j] = true;
            matches += 1;
            break;
        }
    }

    if matches == 0 {
        return 0.0;
    }

    // Count transpositions
    let mut k = 0;
    for i in 0..s1_len {
        if !s1_matches[i] {
            continue;
        }
        while !s2_matches[k] {
            k += 1;
        }
        if s1_chars[i] != s2_chars[k] {
            transpositions += 1;
        }
        k += 1;
    }

    // Calculate Jaro similarity
    let m = matches as f64;
    (m / s1_len as f64 + m / s2_len as f64 + (m - transpositions as f64 / 2.0) / m) / 3.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_snake_case() {
        assert_eq!(to_snake_case("firstName"), "first_name");
        assert_eq!(to_snake_case("FirstName"), "first_name");
        assert_eq!(to_snake_case("userID"), "user_id");
        assert_eq!(to_snake_case("HTTPResponse"), "http_response");
        assert_eq!(to_snake_case("XMLHttpRequest"), "xml_http_request");
        assert_eq!(to_snake_case("already_snake"), "already_snake");
        assert_eq!(to_snake_case("IOError"), "io_error");
    }

    #[test]
    fn test_to_camel_case() {
        assert_eq!(to_camel_case("first_name"), "firstName");
        assert_eq!(to_camel_case("user_id"), "userId");
        assert_eq!(to_camel_case("http_response"), "httpResponse");
        assert_eq!(to_camel_case("already_camel"), "alreadyCamel");
    }

    #[test]
    fn test_case_conversion_roundtrip() {
        let snake = "user_first_name";
        let camel = to_camel_case(snake);
        assert_eq!(camel, "userFirstName");
        assert_eq!(to_snake_case(&camel), snake);
    }

    #[test]
    fn test_jaro_winkler_identical() {
        assert!((jaro_winkler("hello", "hello") - 1.0).abs() < 0.001);
        assert!((jaro_winkler("test", "test") - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_jaro_winkler_empty() {
        assert_eq!(jaro_winkler("", ""), 1.0);
        assert_eq!(jaro_winkler("hello", ""), 0.0);
        assert_eq!(jaro_winkler("", "world"), 0.0);
    }

    #[test]
    fn test_jaro_winkler_similar() {
        // Common prefix boost
        assert!(jaro_winkler("hello", "hallo") > 0.8);
        assert!(jaro_winkler("martha", "marhta") > 0.9);

        // Test cases from literature
        assert!(jaro_winkler("dixon", "dicksonx") > 0.8);
        assert!(jaro_winkler("william", "williams") > 0.9);
    }

    #[test]
    fn test_jaro_winkler_different() {
        assert!(jaro_winkler("hello", "world") < 0.6);
        assert!(jaro_winkler("abc", "xyz") < 0.3);
    }

    #[test]
    fn test_jaro_winkler_field_matching() {
        // Realistic field name matching scenarios
        assert!(jaro_winkler("userName", "username") > 0.9);
        assert!(jaro_winkler("firstName", "first_name") > 0.7);
        assert!(jaro_winkler("userId", "user_id") > 0.8);

        // Typos
        assert!(jaro_winkler("usrName", "userName") > 0.8);
        assert!(jaro_winkler("emailAdress", "emailAddress") > 0.95);
    }

    #[test]
    fn test_jaro_similarity() {
        assert!((jaro_similarity("martha", "marhta") - 0.944).abs() < 0.01);
        assert!((jaro_similarity("dixon", "dicksonx")).abs() < 0.8);
    }
}
