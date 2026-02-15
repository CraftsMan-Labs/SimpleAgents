use std::collections::{BTreeMap, HashMap};

use serde_json::{Map, Value};
use thiserror::Error;

/// Opaque token used to gate scoped state reads/writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapabilityToken {
    capability: ScopeCapability,
}

impl CapabilityToken {
    pub fn llm_read() -> Self {
        Self {
            capability: ScopeCapability::LlmRead,
        }
    }

    pub fn tool_read() -> Self {
        Self {
            capability: ScopeCapability::ToolRead,
        }
    }

    pub fn condition_read() -> Self {
        Self {
            capability: ScopeCapability::ConditionRead,
        }
    }

    pub fn subgraph_read() -> Self {
        Self {
            capability: ScopeCapability::SubgraphRead,
        }
    }

    pub fn batch_read() -> Self {
        Self {
            capability: ScopeCapability::BatchRead,
        }
    }

    pub fn filter_read() -> Self {
        Self {
            capability: ScopeCapability::FilterRead,
        }
    }

    pub fn llm_write() -> Self {
        Self {
            capability: ScopeCapability::LlmWrite,
        }
    }

    pub fn tool_write() -> Self {
        Self {
            capability: ScopeCapability::ToolWrite,
        }
    }

    pub fn condition_write() -> Self {
        Self {
            capability: ScopeCapability::ConditionWrite,
        }
    }

    pub fn subgraph_write() -> Self {
        Self {
            capability: ScopeCapability::SubgraphWrite,
        }
    }

    pub fn batch_write() -> Self {
        Self {
            capability: ScopeCapability::BatchWrite,
        }
    }

    pub fn filter_write() -> Self {
        Self {
            capability: ScopeCapability::FilterWrite,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScopeCapability {
    LlmRead,
    ToolRead,
    ConditionRead,
    SubgraphRead,
    BatchRead,
    FilterRead,
    LlmWrite,
    ToolWrite,
    ConditionWrite,
    SubgraphWrite,
    BatchWrite,
    FilterWrite,
}

impl ScopeCapability {
    fn as_str(self) -> &'static str {
        match self {
            Self::LlmRead => "llm_read",
            Self::ToolRead => "tool_read",
            Self::ConditionRead => "condition_read",
            Self::SubgraphRead => "subgraph_read",
            Self::BatchRead => "batch_read",
            Self::FilterRead => "filter_read",
            Self::LlmWrite => "llm_write",
            Self::ToolWrite => "tool_write",
            Self::ConditionWrite => "condition_write",
            Self::SubgraphWrite => "subgraph_write",
            Self::BatchWrite => "batch_write",
            Self::FilterWrite => "filter_write",
        }
    }

    fn allows_read(self) -> bool {
        matches!(
            self,
            Self::LlmRead
                | Self::ToolRead
                | Self::ConditionRead
                | Self::SubgraphRead
                | Self::BatchRead
                | Self::FilterRead
        )
    }
}

/// Typed read/write boundary failures for scoped runtime state.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ScopeAccessError {
    /// Node attempted to read scope with an invalid capability.
    #[error("scope read denied for capability '{capability}'")]
    ReadDenied {
        /// Capability used for the read.
        capability: &'static str,
    },
    /// Node attempted to write scope with an invalid capability.
    #[error("scope write denied for capability '{capability}'")]
    WriteDenied {
        /// Capability used for the write.
        capability: &'static str,
    },
    /// Attempted to exit root scope.
    #[error("cannot exit root scope")]
    CannotExitRoot,
}

#[derive(Debug)]
struct ScopeFrame {
    parent: Option<usize>,
    node_outputs: BTreeMap<String, Value>,
    loop_iterations: HashMap<String, u32>,
    last_llm_output: Option<String>,
    last_tool_output: Option<Value>,
}

/// Hierarchical scoped runtime state with capability-checked access.
#[derive(Debug)]
pub struct ScopedState {
    workflow_input: Value,
    frames: Vec<ScopeFrame>,
    active_frame: usize,
}

impl ScopedState {
    pub fn new(workflow_input: Value) -> Self {
        Self {
            workflow_input,
            frames: vec![ScopeFrame {
                parent: None,
                node_outputs: BTreeMap::new(),
                loop_iterations: HashMap::new(),
                last_llm_output: None,
                last_tool_output: None,
            }],
            active_frame: 0,
        }
    }

    pub fn enter_child_scope(&mut self) {
        let parent = self.active_frame;
        self.frames.push(ScopeFrame {
            parent: Some(parent),
            node_outputs: BTreeMap::new(),
            loop_iterations: HashMap::new(),
            last_llm_output: None,
            last_tool_output: None,
        });
        self.active_frame = self.frames.len().saturating_sub(1);
    }

    pub fn exit_to_parent_scope(&mut self) -> Result<(), ScopeAccessError> {
        let parent = self.frames[self.active_frame]
            .parent
            .ok_or(ScopeAccessError::CannotExitRoot)?;
        self.active_frame = parent;
        Ok(())
    }

    pub fn scoped_input(&self, token: &CapabilityToken) -> Result<Value, ScopeAccessError> {
        if !token.capability.allows_read() {
            return Err(ScopeAccessError::ReadDenied {
                capability: token.capability.as_str(),
            });
        }

        let mut object = Map::new();
        object.insert("input".to_string(), self.workflow_input.clone());
        object.insert(
            "last_llm_output".to_string(),
            self.visible_last_llm_output()
                .map_or(Value::Null, Value::String),
        );
        object.insert(
            "last_tool_output".to_string(),
            self.visible_last_tool_output().unwrap_or(Value::Null),
        );
        object.insert(
            "node_outputs".to_string(),
            Value::Object(
                self.visible_node_outputs()
                    .into_iter()
                    .collect::<Map<String, Value>>(),
            ),
        );
        Ok(Value::Object(object))
    }

    pub fn record_llm_output(
        &mut self,
        node_id: &str,
        output: String,
        token: &CapabilityToken,
    ) -> Result<(), ScopeAccessError> {
        self.ensure_write(token, ScopeCapability::LlmWrite)?;
        let frame = &mut self.frames[self.active_frame];
        frame.last_llm_output = Some(output.clone());
        frame
            .node_outputs
            .insert(node_id.to_string(), Value::String(output));
        Ok(())
    }

    pub fn record_tool_output(
        &mut self,
        node_id: &str,
        output: Value,
        token: &CapabilityToken,
    ) -> Result<(), ScopeAccessError> {
        self.ensure_write(token, ScopeCapability::ToolWrite)?;
        let frame = &mut self.frames[self.active_frame];
        frame.last_tool_output = Some(output.clone());
        frame.node_outputs.insert(node_id.to_string(), output);
        Ok(())
    }

    pub fn record_condition_output(
        &mut self,
        node_id: &str,
        evaluated: bool,
        token: &CapabilityToken,
    ) -> Result<(), ScopeAccessError> {
        self.ensure_write(token, ScopeCapability::ConditionWrite)?;
        self.frames[self.active_frame]
            .node_outputs
            .insert(node_id.to_string(), Value::Bool(evaluated));
        Ok(())
    }

    pub fn record_subgraph_output(
        &mut self,
        node_id: &str,
        output: Value,
        token: &CapabilityToken,
    ) -> Result<(), ScopeAccessError> {
        self.ensure_write(token, ScopeCapability::SubgraphWrite)?;
        self.frames[self.active_frame]
            .node_outputs
            .insert(node_id.to_string(), output);
        Ok(())
    }

    pub fn record_batch_output(
        &mut self,
        node_id: &str,
        output: Value,
        token: &CapabilityToken,
    ) -> Result<(), ScopeAccessError> {
        self.ensure_write(token, ScopeCapability::BatchWrite)?;
        self.frames[self.active_frame]
            .node_outputs
            .insert(node_id.to_string(), output);
        Ok(())
    }

    pub fn record_filter_output(
        &mut self,
        node_id: &str,
        output: Value,
        token: &CapabilityToken,
    ) -> Result<(), ScopeAccessError> {
        self.ensure_write(token, ScopeCapability::FilterWrite)?;
        self.frames[self.active_frame]
            .node_outputs
            .insert(node_id.to_string(), output);
        Ok(())
    }

    pub fn loop_iteration(&self, node_id: &str) -> u32 {
        self.frames[self.active_frame]
            .loop_iterations
            .get(node_id)
            .copied()
            .unwrap_or(0)
    }

    pub fn set_loop_iteration(&mut self, node_id: &str, iteration: u32) {
        self.frames[self.active_frame]
            .loop_iterations
            .insert(node_id.to_string(), iteration);
    }

    pub fn clear_loop_iteration(&mut self, node_id: &str) {
        self.frames[self.active_frame]
            .loop_iterations
            .remove(node_id);
    }

    pub fn current_scope_node_outputs(&self) -> Value {
        Value::Object(
            self.frames[self.active_frame]
                .node_outputs
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
        )
    }

    pub fn visible_node_outputs(&self) -> BTreeMap<String, Value> {
        let mut chain = Vec::new();
        let mut cursor = Some(self.active_frame);
        while let Some(index) = cursor {
            chain.push(index);
            cursor = self.frames[index].parent;
        }
        chain.reverse();

        let mut merged = BTreeMap::new();
        for index in chain {
            for (key, value) in &self.frames[index].node_outputs {
                merged.insert(key.clone(), value.clone());
            }
        }
        merged
    }

    fn visible_last_llm_output(&self) -> Option<String> {
        let mut cursor = Some(self.active_frame);
        while let Some(index) = cursor {
            if let Some(value) = &self.frames[index].last_llm_output {
                return Some(value.clone());
            }
            cursor = self.frames[index].parent;
        }
        None
    }

    fn visible_last_tool_output(&self) -> Option<Value> {
        let mut cursor = Some(self.active_frame);
        while let Some(index) = cursor {
            if let Some(value) = &self.frames[index].last_tool_output {
                return Some(value.clone());
            }
            cursor = self.frames[index].parent;
        }
        None
    }

    fn ensure_write(
        &self,
        token: &CapabilityToken,
        required: ScopeCapability,
    ) -> Result<(), ScopeAccessError> {
        if token.capability != required {
            return Err(ScopeAccessError::WriteDenied {
                capability: token.capability.as_str(),
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{CapabilityToken, ScopeAccessError, ScopedState};

    #[test]
    fn enforces_capabilities() {
        let mut state = ScopedState::new(json!({"input": true}));

        let read_err = state
            .scoped_input(&CapabilityToken::llm_write())
            .expect_err("write token cannot read");
        assert!(matches!(read_err, ScopeAccessError::ReadDenied { .. }));

        let write_err = state
            .record_tool_output("tool", json!({"ok": true}), &CapabilityToken::llm_write())
            .expect_err("wrong write token should be rejected");
        assert!(matches!(write_err, ScopeAccessError::WriteDenied { .. }));
    }

    #[test]
    fn supports_parent_child_visibility() {
        let mut state = ScopedState::new(json!({"req": 1}));
        state
            .record_tool_output("root_tool", json!({"x": 1}), &CapabilityToken::tool_write())
            .expect("root write should succeed");

        state.enter_child_scope();
        state
            .record_llm_output("child_llm", "ok".to_string(), &CapabilityToken::llm_write())
            .expect("child write should succeed");

        let scoped = state
            .scoped_input(&CapabilityToken::condition_read())
            .expect("read should include parent and child outputs");
        assert!(scoped["node_outputs"]["root_tool"].is_object());
        assert_eq!(scoped["node_outputs"]["child_llm"], json!("ok"));

        state
            .exit_to_parent_scope()
            .expect("child scope should exit to parent");
        let scoped_parent = state
            .scoped_input(&CapabilityToken::condition_read())
            .expect("parent read should work");
        assert!(scoped_parent["node_outputs"].get("child_llm").is_none());
    }
}
