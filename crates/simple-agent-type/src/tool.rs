//! Tool calling types for function/tool integrations.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Tool type supported by the API.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolType {
    /// Function tool (OpenAI-compatible).
    Function,
}

/// Function tool definition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolFunction {
    /// Function name.
    pub name: String,
    /// Optional description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// JSON schema for parameters.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameters: Option<Value>,
}

/// Tool definition for requests.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolDefinition {
    /// Tool type.
    #[serde(rename = "type")]
    pub tool_type: ToolType,
    /// Function payload (required for function tools).
    pub function: ToolFunction,
}

/// Tool choice mode for requests.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolChoiceMode {
    /// Let the model decide.
    Auto,
    /// Disable tool calling.
    None,
}

/// Tool choice specifying a concrete function to call.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolChoiceTool {
    /// Tool type.
    #[serde(rename = "type")]
    pub tool_type: ToolType,
    /// Function name.
    pub function: ToolChoiceFunction,
}

/// Function selector for tool choice.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolChoiceFunction {
    /// Function name to call.
    pub name: String,
}

/// Tool choice for requests.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ToolChoice {
    /// "auto" or "none".
    Mode(ToolChoiceMode),
    /// Specific tool selection.
    Tool(ToolChoiceTool),
}

/// Tool call function payload in responses.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolCallFunction {
    /// Function name.
    pub name: String,
    /// JSON arguments as a string.
    pub arguments: String,
}

/// Tool call emitted by the model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolCall {
    /// Tool call identifier.
    pub id: String,
    /// Tool type.
    #[serde(rename = "type")]
    pub tool_type: ToolType,
    /// Function payload.
    pub function: ToolCallFunction,
}
