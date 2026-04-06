use std::path::{Path, PathBuf};

use super::contracts::{YamlWorkflow, YamlWorkflowRunError};

const MAX_WORKFLOW_YAML_BYTES: u64 = 1024 * 1024;
const MAX_WORKFLOW_YAML_DEPTH: usize = 64;

pub(crate) fn yaml_value_depth(value: &serde_yaml::Value, depth: usize) -> usize {
    match value {
        serde_yaml::Value::Sequence(items) => items
            .iter()
            .map(|item| yaml_value_depth(item, depth + 1))
            .max()
            .unwrap_or(depth),
        serde_yaml::Value::Mapping(map) => map
            .values()
            .map(|item| yaml_value_depth(item, depth + 1))
            .max()
            .unwrap_or(depth),
        _ => depth,
    }
}

pub(crate) fn is_yaml_file(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|ext| ext.to_str()),
        Some("yaml") | Some("yml")
    )
}

pub(crate) fn load_workflow_yaml_file(
    workflow_path: &Path,
) -> Result<(PathBuf, YamlWorkflow), YamlWorkflowRunError> {
    let canonical_path =
        std::fs::canonicalize(workflow_path).map_err(|source| YamlWorkflowRunError::Read {
            path: workflow_path.display().to_string(),
            source,
        })?;

    let metadata =
        std::fs::metadata(&canonical_path).map_err(|source| YamlWorkflowRunError::Read {
            path: canonical_path.display().to_string(),
            source,
        })?;

    if !metadata.is_file() {
        return Err(YamlWorkflowRunError::FileRejected {
            path: canonical_path.display().to_string(),
            reason: "path must reference a regular file".to_string(),
        });
    }

    if !is_yaml_file(&canonical_path) {
        return Err(YamlWorkflowRunError::FileRejected {
            path: canonical_path.display().to_string(),
            reason: "workflow file extension must be .yaml or .yml".to_string(),
        });
    }

    if metadata.len() > MAX_WORKFLOW_YAML_BYTES {
        return Err(YamlWorkflowRunError::FileRejected {
            path: canonical_path.display().to_string(),
            reason: format!(
                "workflow yaml is too large ({} bytes > {} bytes)",
                metadata.len(),
                MAX_WORKFLOW_YAML_BYTES
            ),
        });
    }

    let contents =
        std::fs::read_to_string(&canonical_path).map_err(|source| YamlWorkflowRunError::Read {
            path: canonical_path.display().to_string(),
            source,
        })?;

    let yaml_value: serde_yaml::Value =
        serde_yaml::from_str(&contents).map_err(|source| YamlWorkflowRunError::Parse {
            path: canonical_path.display().to_string(),
            source,
        })?;

    let depth = yaml_value_depth(&yaml_value, 1);
    if depth > MAX_WORKFLOW_YAML_DEPTH {
        return Err(YamlWorkflowRunError::FileRejected {
            path: canonical_path.display().to_string(),
            reason: format!(
                "workflow yaml nesting depth {} exceeds limit {}",
                depth, MAX_WORKFLOW_YAML_DEPTH
            ),
        });
    }

    let workflow: YamlWorkflow =
        serde_yaml::from_value(yaml_value).map_err(|source| YamlWorkflowRunError::Parse {
            path: canonical_path.display().to_string(),
            source,
        })?;

    Ok((canonical_path, workflow))
}
