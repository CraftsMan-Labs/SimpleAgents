//! Message types for LLM interactions.
//!
//! Provides role-based messages compatible with OpenAI's message format.

use serde::{Deserialize, Serialize};

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
}

impl Message {
    /// Create a user message.
    ///
    /// # Example
    /// ```
    /// use simple_agents_types::message::{Message, Role};
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
        }
    }

    /// Create an assistant message.
    ///
    /// # Example
    /// ```
    /// use simple_agents_types::message::{Message, Role};
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
        }
    }

    /// Create a system message.
    ///
    /// # Example
    /// ```
    /// use simple_agents_types::message::{Message, Role};
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
        }
    }

    /// Create a tool message.
    ///
    /// # Example
    /// ```
    /// use simple_agents_types::message::{Message, Role};
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
        }
    }

    /// Set the name field (builder pattern).
    ///
    /// # Example
    /// ```
    /// use simple_agents_types::message::Message;
    ///
    /// let msg = Message::user("Hello").with_name("Alice");
    /// assert_eq!(msg.name, Some("Alice".to_string()));
    /// ```
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }
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
    }

    #[test]
    fn test_message_assistant() {
        let msg = Message::assistant("response");
        assert_eq!(msg.role, Role::Assistant);
        assert_eq!(msg.content, "response");
    }

    #[test]
    fn test_message_system() {
        let msg = Message::system("instruction");
        assert_eq!(msg.role, Role::System);
        assert_eq!(msg.content, "instruction");
    }

    #[test]
    fn test_message_tool() {
        let msg = Message::tool("result", "call_123");
        assert_eq!(msg.role, Role::Tool);
        assert_eq!(msg.content, "result");
        assert_eq!(msg.tool_call_id, Some("call_123".to_string()));
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
        assert!(!json.get("name").is_some());
        assert!(!json.get("tool_call_id").is_some());
    }

    #[test]
    fn test_message_with_name_serialized() {
        let msg = Message::user("test").with_name("Alice");
        let json = serde_json::to_value(&msg).unwrap();
        assert_eq!(json.get("name").and_then(|v| v.as_str()), Some("Alice"));
    }
}
