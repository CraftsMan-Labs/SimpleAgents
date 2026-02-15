use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

/// Serializable checkpoint used to resume workflow execution.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkflowCheckpoint {
    /// Unique run id for this checkpoint.
    pub run_id: String,
    /// Workflow name associated with the run.
    pub workflow_name: String,
    /// Step index reached in this run.
    pub step: usize,
    /// Node id to execute next when resuming.
    pub next_node_id: String,
    /// Scoped input snapshot used to rebuild execution state.
    pub scope_snapshot: Value,
}

/// Errors produced by checkpoint persistence.
#[derive(Debug, Error)]
pub enum CheckpointError {
    /// IO operation failed.
    #[error("checkpoint io failure: {0}")]
    Io(#[from] std::io::Error),
    /// Serialization failed.
    #[error("checkpoint serialization failure: {0}")]
    Serde(#[from] serde_json::Error),
}

/// Storage abstraction for workflow checkpoints.
pub trait CheckpointStore: Send + Sync {
    /// Persists checkpoint content for the run id.
    fn save(&self, checkpoint: &WorkflowCheckpoint) -> Result<(), CheckpointError>;
    /// Loads checkpoint for the provided run id.
    fn load(&self, run_id: &str) -> Result<Option<WorkflowCheckpoint>, CheckpointError>;
    /// Deletes checkpoint for the provided run id.
    fn delete(&self, run_id: &str) -> Result<(), CheckpointError>;
}

/// Filesystem-backed checkpoint storage.
#[derive(Debug, Clone)]
pub struct FilesystemCheckpointStore {
    root: PathBuf,
}

impl FilesystemCheckpointStore {
    /// Creates a backend rooted at `root`.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    fn path_for(&self, run_id: &str) -> PathBuf {
        self.root.join(format!("{}.json", run_id))
    }

    fn ensure_root(root: &Path) -> Result<(), std::io::Error> {
        if !root.exists() {
            fs::create_dir_all(root)?;
        }
        Ok(())
    }
}

impl CheckpointStore for FilesystemCheckpointStore {
    fn save(&self, checkpoint: &WorkflowCheckpoint) -> Result<(), CheckpointError> {
        Self::ensure_root(&self.root)?;
        let path = self.path_for(&checkpoint.run_id);
        let bytes = serde_json::to_vec_pretty(checkpoint)?;
        fs::write(path, bytes)?;
        Ok(())
    }

    fn load(&self, run_id: &str) -> Result<Option<WorkflowCheckpoint>, CheckpointError> {
        let path = self.path_for(run_id);
        if !path.exists() {
            return Ok(None);
        }
        let bytes = fs::read(path)?;
        let checkpoint = serde_json::from_slice::<WorkflowCheckpoint>(&bytes)?;
        Ok(Some(checkpoint))
    }

    fn delete(&self, run_id: &str) -> Result<(), CheckpointError> {
        let path = self.path_for(run_id);
        if path.exists() {
            fs::remove_file(path)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use serde_json::json;

    use super::{CheckpointStore, FilesystemCheckpointStore, WorkflowCheckpoint};

    fn temp_dir() -> std::path::PathBuf {
        let millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should be monotonic")
            .as_millis();
        std::env::temp_dir().join(format!("simple-agents-workflow-checkpoints-{millis}"))
    }

    #[test]
    fn round_trips_checkpoint_on_filesystem() {
        let root = temp_dir();
        let store = FilesystemCheckpointStore::new(&root);
        let checkpoint = WorkflowCheckpoint {
            run_id: "run-1".to_string(),
            workflow_name: "wf".to_string(),
            step: 3,
            next_node_id: "tool".to_string(),
            scope_snapshot: json!({"input": {"k": "v"}}),
        };

        store.save(&checkpoint).expect("save should succeed");
        let loaded = store
            .load("run-1")
            .expect("load should succeed")
            .expect("checkpoint should exist");
        assert_eq!(loaded, checkpoint);

        store.delete("run-1").expect("delete should succeed");
        let missing = store.load("run-1").expect("load should succeed");
        assert!(missing.is_none());

        let _ = std::fs::remove_dir_all(root);
    }
}
