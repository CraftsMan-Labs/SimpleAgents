use std::collections::HashSet;
use std::path::Path;

use super::models::{EvalDatasetRecord, EvalError, EvalSuite};

pub(crate) fn load_eval_suite(path: &Path) -> Result<EvalSuite, EvalError> {
    let content = std::fs::read_to_string(path).map_err(|source| EvalError::ReadSuite {
        path: path.display().to_string(),
        source,
    })?;
    serde_yaml::from_str(&content).map_err(|source| EvalError::ParseSuite {
        path: path.display().to_string(),
        source,
    })
}

pub(crate) fn load_dataset(path: &Path) -> Result<Vec<EvalDatasetRecord>, EvalError> {
    let content = std::fs::read_to_string(path).map_err(|source| EvalError::ReadDataset {
        path: path.display().to_string(),
        source,
    })?;

    let mut records = Vec::new();
    for (index, line) in content.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let record = serde_json::from_str::<EvalDatasetRecord>(line).map_err(|source| {
            EvalError::ParseDatasetLine {
                path: path.display().to_string(),
                line: index + 1,
                source,
            }
        })?;
        records.push(record);
    }

    validate_dataset(&records)?;
    Ok(records)
}

pub(crate) fn validate_suite(suite: &EvalSuite) -> Result<(), EvalError> {
    if suite.id.trim().is_empty() {
        return invalid_suite("id cannot be empty");
    }
    if suite.workflow_path.as_os_str().is_empty() {
        return invalid_suite("workflow_path cannot be empty");
    }
    if suite.dataset_path.as_os_str().is_empty() {
        return invalid_suite("dataset_path cannot be empty");
    }
    if matches!(suite.comparison.mode, super::EvalComparisonMode::Paths)
        && suite.comparison.paths.is_empty()
    {
        return invalid_suite("comparison.paths cannot be empty when mode is paths");
    }
    for path in &suite.comparison.paths {
        if !is_supported_path(path) {
            return invalid_suite(format!("unsupported comparison path '{}'", path));
        }
    }
    for custom_eval in &suite.custom_evals {
        if custom_eval.id.trim().is_empty() {
            return invalid_suite("custom_evals.id cannot be empty");
        }
        if custom_eval.handler.trim().is_empty() {
            return invalid_suite(format!(
                "custom_evals '{}' handler cannot be empty",
                custom_eval.id
            ));
        }
        if !is_supported_path(&custom_eval.actual_path) {
            return invalid_suite(format!(
                "custom_evals '{}' has unsupported actual_path '{}'",
                custom_eval.id, custom_eval.actual_path
            ));
        }
        if !is_supported_path(&custom_eval.expected_path) {
            return invalid_suite(format!(
                "custom_evals '{}' has unsupported expected_path '{}'",
                custom_eval.id, custom_eval.expected_path
            ));
        }
        if let Some(threshold) = custom_eval.threshold {
            if !(0.0..=1.0).contains(&threshold) {
                return invalid_suite(format!(
                    "custom_evals '{}' threshold must be between 0.0 and 1.0",
                    custom_eval.id
                ));
            }
        }
    }
    Ok(())
}

fn validate_dataset(records: &[EvalDatasetRecord]) -> Result<(), EvalError> {
    if records.is_empty() {
        return invalid_dataset("dataset must contain at least one record");
    }

    let mut ids = HashSet::new();
    for record in records {
        if record.id.trim().is_empty() {
            return invalid_dataset("record id cannot be empty");
        }
        if !ids.insert(record.id.as_str()) {
            return invalid_dataset(format!("duplicate record id '{}'", record.id));
        }
        if !record.input.is_object() {
            return invalid_dataset(format!("record '{}' input must be an object", record.id));
        }
        if !record.expected_output.is_object() {
            return invalid_dataset(format!(
                "record '{}' expected_output must be a workflow output object",
                record.id
            ));
        }
    }

    Ok(())
}

pub(crate) fn resolve_relative_path(base_dir: &Path, path: &Path) -> std::path::PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        base_dir.join(path)
    }
}

fn is_supported_path(path: &str) -> bool {
    path == "$" || path.starts_with("$.")
}

fn invalid_suite<T>(message: impl Into<String>) -> Result<T, EvalError> {
    Err(EvalError::InvalidSuite {
        message: message.into(),
    })
}

fn invalid_dataset<T>(message: impl Into<String>) -> Result<T, EvalError> {
    Err(EvalError::InvalidDataset {
        message: message.into(),
    })
}
