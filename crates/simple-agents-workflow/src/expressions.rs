use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard, OnceLock};

use serde_json::{Number, Value};
use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ExpressionError {
    #[error("expression is empty")]
    Empty,
    #[error("invalid expression '{expression}': {reason}")]
    Invalid { expression: String, reason: String },
    #[error("path '{path}' not found in scoped input")]
    MissingPath { path: String },
    #[error("expression complexity limit exceeded: {metric}={value}, max={max}")]
    ComplexityLimitExceeded {
        metric: &'static str,
        value: usize,
        max: usize,
    },
}

#[derive(Debug, Clone, Copy)]
pub struct ExpressionLimits {
    pub max_expression_chars: usize,
    pub max_operator_count: usize,
    pub max_depth: usize,
    pub max_path_segments: usize,
    pub max_cache_entries: usize,
}

/// Expression backend strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ExpressionBackend {
    /// Native Rust parser/evaluator.
    #[default]
    Native,
    /// CEL-compatible backend routed through the same abstraction.
    CelCompatible,
}

impl Default for ExpressionLimits {
    fn default() -> Self {
        Self {
            max_expression_chars: 2048,
            max_operator_count: 64,
            max_depth: 24,
            max_path_segments: 16,
            max_cache_entries: 512,
        }
    }
}

#[derive(Debug, Clone)]
enum ExpressionNode {
    Not(Box<ExpressionNode>),
    Eq(Operand, Operand),
    Ne(Operand, Operand),
    Or(Vec<ExpressionNode>),
    And(Vec<ExpressionNode>),
    Truthy(Operand),
}

#[derive(Debug, Clone)]
enum Operand {
    Bool(bool),
    Null,
    Number(Number),
    String(String),
    Path(String),
}

#[derive(Debug, Default)]
pub struct ExpressionEngine {
    cache: Mutex<HashMap<String, ExpressionNode>>,
    backend: ExpressionBackend,
    limits: ExpressionLimits,
}

impl ExpressionEngine {
    pub fn new() -> Self {
        Self::with_limits(ExpressionLimits::default())
    }

    pub fn with_limits(limits: ExpressionLimits) -> Self {
        Self::with_backend(ExpressionBackend::Native, limits)
    }

    pub fn with_backend(backend: ExpressionBackend, limits: ExpressionLimits) -> Self {
        Self {
            cache: Mutex::new(HashMap::new()),
            backend,
            limits,
        }
    }

    pub fn validate(&self, expression: &str) -> Result<(), ExpressionError> {
        let _ = self.compile(expression)?;
        Ok(())
    }

    pub fn evaluate_bool(
        &self,
        expression: &str,
        scoped_input: &Value,
    ) -> Result<bool, ExpressionError> {
        let node = self.compile(expression)?;
        eval_node(&node, scoped_input)
    }

    fn cache_lock(&self) -> MutexGuard<'_, HashMap<String, ExpressionNode>> {
        self.cache
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn compile(&self, expression: &str) -> Result<ExpressionNode, ExpressionError> {
        let normalized = expression.trim();
        if normalized.is_empty() {
            return Err(ExpressionError::Empty);
        }

        if normalized.len() > self.limits.max_expression_chars {
            return Err(ExpressionError::ComplexityLimitExceeded {
                metric: "chars",
                value: normalized.len(),
                max: self.limits.max_expression_chars,
            });
        }

        if let Some(cached) = self.cache_lock().get(normalized).cloned() {
            return Ok(cached);
        }

        let parsed = match self.backend {
            ExpressionBackend::Native | ExpressionBackend::CelCompatible => parse_expr(normalized)?,
        };
        let op_count = count_operators(&parsed);
        if op_count > self.limits.max_operator_count {
            return Err(ExpressionError::ComplexityLimitExceeded {
                metric: "operators",
                value: op_count,
                max: self.limits.max_operator_count,
            });
        }
        let depth = tree_depth(&parsed);
        if depth > self.limits.max_depth {
            return Err(ExpressionError::ComplexityLimitExceeded {
                metric: "depth",
                value: depth,
                max: self.limits.max_depth,
            });
        }
        validate_path_segments(&parsed, self.limits.max_path_segments)?;

        let mut cache = self.cache_lock();
        if cache.len() >= self.limits.max_cache_entries {
            if let Some(evicted) = cache.keys().next().cloned() {
                cache.remove(&evicted);
            }
        }
        cache.insert(normalized.to_string(), parsed.clone());
        Ok(parsed)
    }
}

pub fn default_expression_engine() -> &'static ExpressionEngine {
    static ENGINE: OnceLock<ExpressionEngine> = OnceLock::new();
    ENGINE.get_or_init(ExpressionEngine::new)
}

pub fn evaluate_bool(expression: &str, scoped_input: &Value) -> Result<bool, ExpressionError> {
    default_expression_engine().evaluate_bool(expression, scoped_input)
}

fn parse_expr(expression: &str) -> Result<ExpressionNode, ExpressionError> {
    if let Some(parts) = split_top_level(expression, "||") {
        let mut nodes = Vec::with_capacity(parts.len());
        for part in parts {
            nodes.push(parse_expr(part)?);
        }
        return Ok(ExpressionNode::Or(nodes));
    }

    if let Some(parts) = split_top_level(expression, "&&") {
        let mut nodes = Vec::with_capacity(parts.len());
        for part in parts {
            nodes.push(parse_expr(part)?);
        }
        return Ok(ExpressionNode::And(nodes));
    }

    if let Some(inner) = expression.strip_prefix('!') {
        return Ok(ExpressionNode::Not(Box::new(parse_expr(inner.trim())?)));
    }

    if let Some((left, right)) = split_once_top_level(expression, "==") {
        return Ok(ExpressionNode::Eq(
            parse_operand(left)?,
            parse_operand(right)?,
        ));
    }

    if let Some((left, right)) = split_once_top_level(expression, "!=") {
        return Ok(ExpressionNode::Ne(
            parse_operand(left)?,
            parse_operand(right)?,
        ));
    }

    Ok(ExpressionNode::Truthy(parse_operand(expression)?))
}

fn parse_operand(token: &str) -> Result<Operand, ExpressionError> {
    let trimmed = token.trim();
    if trimmed.is_empty() {
        return Err(ExpressionError::Invalid {
            expression: token.to_string(),
            reason: "empty operand".to_string(),
        });
    }

    if trimmed.eq_ignore_ascii_case("true") {
        return Ok(Operand::Bool(true));
    }
    if trimmed.eq_ignore_ascii_case("false") {
        return Ok(Operand::Bool(false));
    }
    if trimmed.eq_ignore_ascii_case("null") {
        return Ok(Operand::Null);
    }

    if let Some(value) = trimmed.strip_prefix('"').and_then(|v| v.strip_suffix('"')) {
        return Ok(Operand::String(value.to_string()));
    }
    if let Some(value) = trimmed
        .strip_prefix('\'')
        .and_then(|v| v.strip_suffix('\''))
    {
        return Ok(Operand::String(value.to_string()));
    }

    if let Ok(value) = trimmed.parse::<i64>() {
        return Ok(Operand::Number(Number::from(value)));
    }

    if let Ok(value) = trimmed.parse::<f64>() {
        if let Some(number) = Number::from_f64(value) {
            return Ok(Operand::Number(number));
        }
    }

    let path = trimmed.strip_prefix("$.").unwrap_or(trimmed);
    if path.is_empty() {
        return Err(ExpressionError::Invalid {
            expression: token.to_string(),
            reason: "path cannot be '$.'".to_string(),
        });
    }

    Ok(Operand::Path(path.to_string()))
}

fn split_top_level<'a>(input: &'a str, delimiter: &'a str) -> Option<Vec<&'a str>> {
    let mut parts = Vec::new();
    let mut start = 0usize;
    let mut in_single = false;
    let mut in_double = false;
    let bytes = input.as_bytes();
    let delim = delimiter.as_bytes();
    let mut idx = 0usize;

    while idx < bytes.len() {
        match bytes[idx] {
            b'\'' if !in_double => in_single = !in_single,
            b'"' if !in_single => in_double = !in_double,
            _ => {}
        }

        if !in_single
            && !in_double
            && idx + delim.len() <= bytes.len()
            && &bytes[idx..idx + delim.len()] == delim
        {
            parts.push(input[start..idx].trim());
            start = idx + delim.len();
            idx = start;
            continue;
        }
        idx += 1;
    }

    if parts.is_empty() {
        return None;
    }
    parts.push(input[start..].trim());
    Some(parts)
}

fn split_once_top_level<'a>(input: &'a str, delimiter: &'a str) -> Option<(&'a str, &'a str)> {
    let mut in_single = false;
    let mut in_double = false;
    let bytes = input.as_bytes();
    let delim = delimiter.as_bytes();
    let mut idx = 0usize;

    while idx < bytes.len() {
        match bytes[idx] {
            b'\'' if !in_double => in_single = !in_single,
            b'"' if !in_single => in_double = !in_double,
            _ => {}
        }

        if !in_single
            && !in_double
            && idx + delim.len() <= bytes.len()
            && &bytes[idx..idx + delim.len()] == delim
        {
            let left = input[..idx].trim();
            let right = input[idx + delim.len()..].trim();
            if left.is_empty() || right.is_empty() {
                return None;
            }
            return Some((left, right));
        }
        idx += 1;
    }

    None
}

fn eval_node(node: &ExpressionNode, scoped_input: &Value) -> Result<bool, ExpressionError> {
    match node {
        ExpressionNode::Not(inner) => Ok(!eval_node(inner, scoped_input)?),
        ExpressionNode::Eq(left, right) => {
            Ok(eval_operand(left, scoped_input)? == eval_operand(right, scoped_input)?)
        }
        ExpressionNode::Ne(left, right) => {
            Ok(eval_operand(left, scoped_input)? != eval_operand(right, scoped_input)?)
        }
        ExpressionNode::Or(nodes) => {
            for node in nodes {
                if eval_node(node, scoped_input)? {
                    return Ok(true);
                }
            }
            Ok(false)
        }
        ExpressionNode::And(nodes) => {
            for node in nodes {
                if !eval_node(node, scoped_input)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        ExpressionNode::Truthy(operand) => Ok(is_truthy(&eval_operand(operand, scoped_input)?)),
    }
}

fn eval_operand(operand: &Operand, scoped_input: &Value) -> Result<Value, ExpressionError> {
    match operand {
        Operand::Bool(v) => Ok(Value::Bool(*v)),
        Operand::Null => Ok(Value::Null),
        Operand::Number(v) => Ok(Value::Number(v.clone())),
        Operand::String(v) => Ok(Value::String(v.clone())),
        Operand::Path(path) => resolve_path(scoped_input, path)
            .cloned()
            .ok_or_else(|| ExpressionError::MissingPath { path: path.clone() }),
    }
}

fn resolve_path<'a>(root: &'a Value, path: &str) -> Option<&'a Value> {
    if path.is_empty() {
        return Some(root);
    }

    path.split('.')
        .filter(|segment| !segment.is_empty())
        .try_fold(root, |current, segment| current.get(segment))
}

fn is_truthy(value: &Value) -> bool {
    match value {
        Value::Bool(value) => *value,
        Value::Null => false,
        Value::Number(number) => number.as_f64().is_some_and(|n| n != 0.0),
        Value::String(value) => !value.is_empty(),
        Value::Array(values) => !values.is_empty(),
        Value::Object(values) => !values.is_empty(),
    }
}

fn count_operators(node: &ExpressionNode) -> usize {
    match node {
        ExpressionNode::Not(inner) => 1 + count_operators(inner),
        ExpressionNode::Eq(..) | ExpressionNode::Ne(..) | ExpressionNode::Truthy(..) => 1,
        ExpressionNode::Or(nodes) | ExpressionNode::And(nodes) => {
            1 + nodes.iter().map(count_operators).sum::<usize>()
        }
    }
}

fn tree_depth(node: &ExpressionNode) -> usize {
    match node {
        ExpressionNode::Not(inner) => 1 + tree_depth(inner),
        ExpressionNode::Eq(..) | ExpressionNode::Ne(..) | ExpressionNode::Truthy(..) => 1,
        ExpressionNode::Or(nodes) | ExpressionNode::And(nodes) => {
            1 + nodes.iter().map(tree_depth).max().unwrap_or(0)
        }
    }
}

fn validate_path_segments(
    node: &ExpressionNode,
    max_segments: usize,
) -> Result<(), ExpressionError> {
    match node {
        ExpressionNode::Not(inner) => validate_path_segments(inner, max_segments),
        ExpressionNode::Eq(left, right) | ExpressionNode::Ne(left, right) => {
            validate_operand_path(left, max_segments)?;
            validate_operand_path(right, max_segments)
        }
        ExpressionNode::Or(nodes) | ExpressionNode::And(nodes) => {
            for item in nodes {
                validate_path_segments(item, max_segments)?;
            }
            Ok(())
        }
        ExpressionNode::Truthy(operand) => validate_operand_path(operand, max_segments),
    }
}

fn validate_operand_path(operand: &Operand, max_segments: usize) -> Result<(), ExpressionError> {
    if let Operand::Path(path) = operand {
        let segments = path
            .split('.')
            .filter(|segment| !segment.is_empty())
            .count();
        if segments > max_segments {
            return Err(ExpressionError::ComplexityLimitExceeded {
                metric: "path_segments",
                value: segments,
                max: max_segments,
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{ExpressionBackend, ExpressionEngine, ExpressionError, ExpressionLimits};

    #[test]
    fn supports_truthy_path_and_equality() {
        let engine = ExpressionEngine::new();
        let input = json!({"input": {"approved": true}, "score": 5});

        assert!(engine
            .evaluate_bool("input.approved", &input)
            .expect("truthy check should pass"));
        assert!(engine
            .evaluate_bool("score == 5", &input)
            .expect("equality check should pass"));
        assert!(engine
            .evaluate_bool("score != 2", &input)
            .expect("inequality check should pass"));
    }

    #[test]
    fn supports_boolean_operators() {
        let engine = ExpressionEngine::new();
        let input = json!({"a": true, "b": false, "n": 1});

        assert!(engine
            .evaluate_bool("a && n == 1", &input)
            .expect("and expression should pass"));
        assert!(engine
            .evaluate_bool("b || n == 1", &input)
            .expect("or expression should pass"));
        assert!(engine
            .evaluate_bool("!b", &input)
            .expect("not expression should pass"));
    }

    #[test]
    fn reports_missing_path() {
        let engine = ExpressionEngine::new();
        let error = engine
            .evaluate_bool("missing.path", &json!({}))
            .expect_err("missing path should fail");
        assert!(matches!(error, ExpressionError::MissingPath { .. }));
    }

    #[test]
    fn validate_uses_parse_cache() {
        let engine = ExpressionEngine::new();
        engine
            .validate("input.ready == true")
            .expect("first parse should pass");
        engine
            .validate("input.ready == true")
            .expect("second parse should hit cache");
    }

    #[test]
    fn rejects_expression_when_depth_limit_exceeded() {
        let engine = ExpressionEngine::with_limits(ExpressionLimits {
            max_depth: 1,
            ..ExpressionLimits::default()
        });

        let error = engine
            .evaluate_bool("a && b && c", &json!({"a": true, "b": true, "c": true}))
            .expect_err("depth guard should reject expression");
        assert!(matches!(
            error,
            ExpressionError::ComplexityLimitExceeded {
                metric: "depth",
                ..
            }
        ));
    }

    #[test]
    fn rejects_expression_when_path_segments_limit_exceeded() {
        let engine = ExpressionEngine::with_limits(ExpressionLimits {
            max_path_segments: 2,
            ..ExpressionLimits::default()
        });

        let error = engine
            .evaluate_bool(
                "input.deep.value == true",
                &json!({"input": {"deep": {"value": true}}}),
            )
            .expect_err("path segment guard should reject expression");
        assert!(matches!(
            error,
            ExpressionError::ComplexityLimitExceeded {
                metric: "path_segments",
                ..
            }
        ));
    }

    #[test]
    fn supports_cel_compatible_backend_path() {
        let engine = ExpressionEngine::with_backend(
            ExpressionBackend::CelCompatible,
            ExpressionLimits::default(),
        );
        let result = engine
            .evaluate_bool("input.ready == true", &json!({"input": {"ready": true}}))
            .expect("cel-compatible backend should evaluate expression");
        assert!(result);
    }
}
