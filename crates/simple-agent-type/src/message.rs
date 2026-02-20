//! Message types for LLM interactions.
//!
//! Provides role-based messages compatible with OpenAI's message format.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::str::FromStr;
use thiserror::Error;

use crate::tool::ToolCall;

/// Role of a message in a conversation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    /// User message
    User,
    /// Assistant (LLM) message
    Assistant,
    /// System instruction message
    System,
    /// Tool/function call result
    #[serde(rename = "tool")]
    Tool,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("invalid message role '{role}' (expected: system|user|assistant|tool)")]
/// Error returned when parsing an unknown message role string.
pub struct ParseRoleError {
    /// Original role string that failed to parse.
    pub role: String,
}

impl Role {
    /// Returns this role as its canonical lowercase string value.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::User => "user",
            Self::Assistant => "assistant",
            Self::Tool => "tool",
        }
    }
}

impl FromStr for Role {
    type Err = ParseRoleError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "system" => Ok(Self::System),
            "user" => Ok(Self::User),
            "assistant" => Ok(Self::Assistant),
            "tool" => Ok(Self::Tool),
            _ => Err(ParseRoleError {
                role: s.to_string(),
            }),
        }
    }
}

/// A message in a conversation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Message {
    /// Role of the message sender
    pub role: Role,
    /// Content of the message
    pub content: String,
    /// Optional name (for multi-user conversations or tool calls)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Tool call ID (for tool role messages)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    /// Tool calls emitted by the assistant.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
}

impl Message {
    /// Create a user message.
    ///
    /// # Example
    /// ```
    /// use simple_agent_type::message::{Message, Role};
    ///
    /// let msg = Message::user("Hello!");
    /// assert_eq!(msg.role, Role::User);
    /// assert_eq!(msg.content, "Hello!");
    /// ```
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: content.into(),
            name: None,
            tool_call_id: None,
            tool_calls: None,
        }
    }

    /// Create an assistant message.
    ///
    /// # Example
    /// ```
    /// use simple_agent_type::message::{Message, Role};
    ///
    /// let msg = Message::assistant("Hi there!");
    /// assert_eq!(msg.role, Role::Assistant);
    /// ```
    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: Role::Assistant,
            content: content.into(),
            name: None,
            tool_call_id: None,
            tool_calls: None,
        }
    }

    /// Create a system message.
    ///
    /// # Example
    /// ```
    /// use simple_agent_type::message::{Message, Role};
    ///
    /// let msg = Message::system("You are a helpful assistant.");
    /// assert_eq!(msg.role, Role::System);
    /// ```
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: Role::System,
            content: content.into(),
            name: None,
            tool_call_id: None,
            tool_calls: None,
        }
    }

    /// Create a tool message.
    ///
    /// # Example
    /// ```
    /// use simple_agent_type::message::{Message, Role};
    ///
    /// let msg = Message::tool("result", "call_123");
    /// assert_eq!(msg.role, Role::Tool);
    /// assert_eq!(msg.tool_call_id, Some("call_123".to_string()));
    /// ```
    pub fn tool(content: impl Into<String>, tool_call_id: impl Into<String>) -> Self {
        Self {
            role: Role::Tool,
            content: content.into(),
            name: None,
            tool_call_id: Some(tool_call_id.into()),
            tool_calls: None,
        }
    }

    /// Set the name field (builder pattern).
    ///
    /// # Example
    /// ```
    /// use simple_agent_type::message::Message;
    ///
    /// let msg = Message::user("Hello").with_name("Alice");
    /// assert_eq!(msg.name, Some("Alice".to_string()));
    /// ```
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Set tool calls for assistant messages.
    pub fn with_tool_calls(mut self, tool_calls: Vec<ToolCall>) -> Self {
        self.tool_calls = Some(tool_calls);
        self
    }
}

#[derive(Debug, Clone, Deserialize)]
struct MessageInputWire {
    role: Role,
    content: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default, alias = "toolCallId")]
    tool_call_id: Option<String>,
    #[serde(default)]
    tool_calls: Option<Vec<ToolCall>>,
}

/// Parses a JSON value containing an array of message objects into typed messages.
pub fn parse_messages_value(value: &Value) -> Result<Vec<Message>, String> {
    let wire_messages: Vec<MessageInputWire> = serde_json::from_value(value.clone())
        .map_err(|e| format!("messages must be a list of message objects: {e}"))?;
    if wire_messages.is_empty() {
        return Err("messages cannot be empty".to_string());
    }

    wire_messages
        .into_iter()
        .enumerate()
        .map(|(idx, wire)| {
            if wire.content.is_empty() {
                return Err(format!("message[{idx}].content cannot be empty"));
            }

            let mut msg = match wire.role {
                Role::System => Message::system(wire.content),
                Role::User => Message::user(wire.content),
                Role::Assistant => {
                    let mut m = Message::assistant(wire.content);
                    if let Some(calls) = wire.tool_calls {
                        if !calls.is_empty() {
                            m = m.with_tool_calls(calls);
                        }
                    }
                    m
                }
                Role::Tool => {
                    let call_id = wire.tool_call_id.ok_or_else(|| {
                        format!("message[{idx}].tool_call_id is required for tool role")
                    })?;
                    Message::tool(wire.content, call_id)
                }
            };

            if let Some(name) = wire.name {
                if !name.is_empty() {
                    msg = msg.with_name(name);
                }
            }

            Ok(msg)
        })
        .collect()
}

/// Parses a JSON string containing an array of message objects.
pub fn parse_messages_json(messages_json: &str) -> Result<Vec<Message>, String> {
    let value: Value =
        serde_json::from_str(messages_json).map_err(|e| format!("invalid messages json: {e}"))?;
    parse_messages_value(&value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_user() {
        let msg = Message::user("test");
        assert_eq!(msg.role, Role::User);
        assert_eq!(msg.content, "test");
        assert_eq!(msg.name, None);
        assert_eq!(msg.tool_call_id, None);
        assert_eq!(msg.tool_calls, None);
    }

    #[test]
    fn test_message_assistant() {
        let msg = Message::assistant("response");
        assert_eq!(msg.role, Role::Assistant);
        assert_eq!(msg.content, "response");
        assert_eq!(msg.tool_calls, None);
    }

    #[test]
    fn test_message_system() {
        let msg = Message::system("instruction");
        assert_eq!(msg.role, Role::System);
        assert_eq!(msg.content, "instruction");
        assert_eq!(msg.tool_calls, None);
    }

    #[test]
    fn test_message_tool() {
        let msg = Message::tool("result", "call_123");
        assert_eq!(msg.role, Role::Tool);
        assert_eq!(msg.content, "result");
        assert_eq!(msg.tool_call_id, Some("call_123".to_string()));
        assert_eq!(msg.tool_calls, None);
    }

    #[test]
    fn test_message_with_name() {
        let msg = Message::user("test").with_name("Alice");
        assert_eq!(msg.name, Some("Alice".to_string()));
    }

    #[test]
    fn test_role_serialization() {
        let json = serde_json::to_string(&Role::User).unwrap();
        assert_eq!(json, "\"user\"");

        let json = serde_json::to_string(&Role::Assistant).unwrap();
        assert_eq!(json, "\"assistant\"");

        let json = serde_json::to_string(&Role::System).unwrap();
        assert_eq!(json, "\"system\"");

        let json = serde_json::to_string(&Role::Tool).unwrap();
        assert_eq!(json, "\"tool\"");
    }

    #[test]
    fn test_message_serialization() {
        let msg = Message::user("Hello");
        let json = serde_json::to_string(&msg).unwrap();
        let parsed: Message = serde_json::from_str(&json).unwrap();
        assert_eq!(msg, parsed);
    }

    #[test]
    fn test_message_optional_fields_not_serialized() {
        let msg = Message::user("test");
        let json = serde_json::to_value(&msg).unwrap();
        assert!(json.get("name").is_none());
        assert!(json.get("tool_call_id").is_none());
        assert!(json.get("tool_calls").is_none());
    }

    #[test]
    fn test_message_with_name_serialized() {
        let msg = Message::user("test").with_name("Alice");
        let json = serde_json::to_value(&msg).unwrap();
        assert_eq!(json.get("name").and_then(|v| v.as_str()), Some("Alice"));
    }
}
