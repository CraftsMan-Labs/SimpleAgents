use std::collections::HashSet;
use std::path::Path;

use super::models::{EvalDatasetRecord, EvalError};

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

fn invalid_dataset<T>(message: impl Into<String>) -> Result<T, EvalError> {
    Err(EvalError::InvalidDataset {
        message: message.into(),
    })
}
