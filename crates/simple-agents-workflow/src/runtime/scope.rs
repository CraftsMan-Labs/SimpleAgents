use std::collections::{BTreeMap, HashMap};

use serde_json::{Map, Value};

use crate::state::ScopeAccessError;

/// Scope access capabilities used by runtime nodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ScopeCapability {
    LlmRead,
    ToolRead,
    ConditionRead,
    LlmWrite,
    ToolWrite,
    ConditionWrite,
    MapRead,
    MapWrite,
    ReduceWrite,
}

impl ScopeCapability {
    fn as_str(self) -> &'static str {
        match self {
            Self::LlmRead => "llm_read",
            Self::ToolRead => "tool_read",
            Self::ConditionRead => "condition_read",
            Self::LlmWrite => "llm_write",
            Self::ToolWrite => "tool_write",
            Self::ConditionWrite => "condition_write",
            Self::MapRead => "map_read",
            Self::MapWrite => "map_write",
            Self::ReduceWrite => "reduce_write",
        }
    }
}

#[derive(Debug)]
pub(super) struct RuntimeScope {
    workflow_input: Value,
    node_outputs: Map<String, Value>,
    loop_iterations: HashMap<String, u32>,
    debounce_last_seen: HashMap<String, usize>,
    throttle_last_pass: HashMap<String, usize>,
    cache_entries: BTreeMap<String, Value>,
    last_llm_output: Option<String>,
    last_tool_output: Option<Value>,
}

impl RuntimeScope {
    pub(super) fn new(workflow_input: Value) -> Self {
        Self {
            workflow_input,
            node_outputs: Map::new(),
            loop_iterations: HashMap::new(),
            debounce_last_seen: HashMap::new(),
            throttle_last_pass: HashMap::new(),
            cache_entries: BTreeMap::new(),
            last_llm_output: None,
            last_tool_output: None,
        }
    }

    pub(super) fn scoped_input(
        &self,
        capability: ScopeCapability,
    ) -> Result<Value, ScopeAccessError> {
        if !matches!(
            capability,
            ScopeCapability::LlmRead
                | ScopeCapability::ToolRead
                | ScopeCapability::ConditionRead
                | ScopeCapability::MapRead
        ) {
            return Err(ScopeAccessError::ReadDenied {
                capability: capability.as_str(),
            });
        }

        let mut object = Map::new();
        object.insert("input".to_string(), self.workflow_input.clone());
        object.insert(
            "last_llm_output".to_string(),
            self.last_llm_output
                .as_ref()
                .map_or(Value::Null, |value| Value::String(value.clone())),
        );
        object.insert(
            "last_tool_output".to_string(),
            self.last_tool_output.clone().unwrap_or(Value::Null),
        );
        object.insert(
            "node_outputs".to_string(),
            Value::Object(self.node_outputs.clone()),
        );
        Ok(Value::Object(object))
    }

    pub(super) fn record_llm_output(
        &mut self,
        node_id: &str,
        output: String,
        capability: ScopeCapability,
    ) -> Result<(), ScopeAccessError> {
        if capability != ScopeCapability::LlmWrite {
            return Err(ScopeAccessError::WriteDenied {
                capability: capability.as_str(),
            });
        }

        self.last_llm_output = Some(output.clone());
        self.node_outputs
            .insert(node_id.to_string(), Value::String(output));
        Ok(())
    }

    pub(super) fn record_tool_output(
        &mut self,
        node_id: &str,
        output: Value,
        capability: ScopeCapability,
    ) -> Result<(), ScopeAccessError> {
        if capability != ScopeCapability::ToolWrite {
            return Err(ScopeAccessError::WriteDenied {
                capability: capability.as_str(),
            });
        }

        self.last_tool_output = Some(output.clone());
        self.node_outputs.insert(node_id.to_string(), output);
        Ok(())
    }

    pub(super) fn record_condition_output(
        &mut self,
        node_id: &str,
        evaluated: bool,
        capability: ScopeCapability,
    ) -> Result<(), ScopeAccessError> {
        if capability != ScopeCapability::ConditionWrite {
            return Err(ScopeAccessError::WriteDenied {
                capability: capability.as_str(),
            });
        }

        self.node_outputs
            .insert(node_id.to_string(), Value::Bool(evaluated));
        Ok(())
    }

    pub(super) fn record_node_output(
        &mut self,
        node_id: &str,
        output: Value,
        capability: ScopeCapability,
    ) -> Result<(), ScopeAccessError> {
        if !matches!(
            capability,
            ScopeCapability::MapWrite | ScopeCapability::ReduceWrite
        ) {
            return Err(ScopeAccessError::WriteDenied {
                capability: capability.as_str(),
            });
        }

        self.node_outputs.insert(node_id.to_string(), output);
        Ok(())
    }

    pub(super) fn node_output(&self, node_id: &str) -> Option<&Value> {
        self.node_outputs.get(node_id)
    }

    pub(super) fn loop_iteration(&self, node_id: &str) -> u32 {
        self.loop_iterations.get(node_id).copied().unwrap_or(0)
    }

    pub(super) fn set_loop_iteration(&mut self, node_id: &str, iteration: u32) {
        self.loop_iterations.insert(node_id.to_string(), iteration);
    }

    pub(super) fn clear_loop_iteration(&mut self, node_id: &str) {
        self.loop_iterations.remove(node_id);
    }

    pub(super) fn debounce(
        &mut self,
        node_id: &str,
        key: &str,
        step: usize,
        window_steps: u32,
    ) -> bool {
        let namespaced = format!("{node_id}:{key}");
        let window = window_steps as usize;
        let suppressed = self
            .debounce_last_seen
            .get(&namespaced)
            .is_some_and(|last| step.saturating_sub(*last) < window);
        self.debounce_last_seen.insert(namespaced, step);
        suppressed
    }

    pub(super) fn throttle(
        &mut self,
        node_id: &str,
        key: &str,
        step: usize,
        window_steps: u32,
    ) -> bool {
        let namespaced = format!("{node_id}:{key}");
        let window = window_steps as usize;
        let throttled = self
            .throttle_last_pass
            .get(&namespaced)
            .is_some_and(|last| step.saturating_sub(*last) < window);
        if !throttled {
            self.throttle_last_pass.insert(namespaced, step);
        }
        throttled
    }

    pub(super) fn put_cache(&mut self, key: &str, value: Value) {
        self.cache_entries.insert(key.to_string(), value);
    }

    pub(super) fn cache_value(&self, key: &str) -> Option<&Value> {
        self.cache_entries.get(key)
    }

    pub(super) fn into_node_outputs_btree(self) -> BTreeMap<String, Value> {
        self.node_outputs.into_iter().collect()
    }
}
