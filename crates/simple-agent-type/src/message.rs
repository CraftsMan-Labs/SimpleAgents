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

/// Content of a message — either plain text or a list of multimodal parts.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum MessageContent {
    /// Plain text content.
    Text(String),
    /// Multimodal content parts (text, images, video, etc.).
    Parts(Vec<ContentPart>),
}

impl From<String> for MessageContent {
    fn from(s: String) -> Self {
        Self::Text(s)
    }
}

impl From<&str> for MessageContent {
    fn from(s: &str) -> Self {
        Self::Text(s.to_string())
    }
}

impl MessageContent {
    /// Returns the text content length for validation purposes.
    /// For Parts, sums the text lengths of all text parts.
    pub fn text_len(&self) -> usize {
        match self {
            Self::Text(s) => s.len(),
            Self::Parts(parts) => parts
                .iter()
                .map(|p| match p {
                    ContentPart::Text { text } => text.len(),
                    _ => 0,
                })
                .sum(),
        }
    }

    /// Returns true if the message has no usable content (empty string, empty parts,
    /// or every part is empty).
    pub fn is_empty_content(&self) -> bool {
        match self {
            Self::Text(s) => s.is_empty(),
            Self::Parts(parts) => {
                parts.is_empty()
                    || parts.iter().all(|p| match p {
                        ContentPart::Text { text } => text.is_empty(),
                        ContentPart::ImageUrl { image_url } => image_url.url.is_empty(),
                        ContentPart::Audio { input_audio } => input_audio.data.is_empty(),
                        ContentPart::Video { video } => video.data.is_empty(),
                    })
            }
        }
    }

    /// Returns true if the content contains a null byte.
    pub fn contains_null(&self) -> bool {
        match self {
            Self::Text(s) => s.contains('\0'),
            Self::Parts(parts) => parts.iter().any(|p| match p {
                ContentPart::Text { text } => text.contains('\0'),
                ContentPart::ImageUrl { image_url } => image_url.url.contains('\0'),
                ContentPart::Audio { input_audio } => {
                    input_audio.data.contains('\0') || input_audio.media_type.contains('\0')
                }
                ContentPart::Video { video } => {
                    video.data.contains('\0') || video.media_type.contains('\0')
                }
            }),
        }
    }
}

/// Well-known MIME type constants for multimodal content.
pub mod mime {
    /// PNG image.
    pub const IMAGE_PNG: &str = "image/png";
    /// JPEG image.
    pub const IMAGE_JPEG: &str = "image/jpeg";
    /// WebP image.
    pub const IMAGE_WEBP: &str = "image/webp";
    /// GIF image.
    pub const IMAGE_GIF: &str = "image/gif";
    /// MP3 audio.
    pub const AUDIO_MP3: &str = "audio/mpeg";
    /// WAV audio.
    pub const AUDIO_WAV: &str = "audio/wav";
    /// FLAC audio.
    pub const AUDIO_FLAC: &str = "audio/flac";
    /// OGG audio.
    pub const AUDIO_OGG: &str = "audio/ogg";
    /// MP4 video.
    pub const VIDEO_MP4: &str = "video/mp4";
    /// WebM video.
    pub const VIDEO_WEBM: &str = "video/webm";
    /// MOV video.
    pub const VIDEO_MOV: &str = "video/quicktime";
    /// MKV video.
    pub const VIDEO_MKV: &str = "video/x-matroska";
}

/// Base64-encoded media content with its MIME type.
///
/// Used by [`ContentPart::Audio`] and [`ContentPart::Video`] to carry inline
/// media data. The same struct works for both input (user uploads) and output
/// (model-generated audio/video).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MediaContent {
    /// MIME type (e.g. `"image/png"`, `"audio/wav"`). See [`mime`] for constants.
    pub media_type: String,
    /// Base64-encoded media bytes.
    pub data: String,
}

/// A single content part in a multimodal message.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum ContentPart {
    /// Text content.
    #[serde(rename = "text")]
    Text {
        /// The text string.
        text: String,
    },
    /// Image content (base64 data URI for OpenAI wire compatibility).
    #[serde(rename = "image_url")]
    ImageUrl {
        /// The image URL and optional detail level.
        image_url: ImageUrlContent,
    },
    /// Audio content (inline base64).
    #[serde(rename = "input_audio")]
    Audio {
        /// The audio media payload.
        input_audio: MediaContent,
    },
    /// Video content (inline base64).
    #[serde(rename = "video")]
    Video {
        /// The video media payload.
        video: MediaContent,
    },
}

impl ContentPart {
    /// Create a text content part.
    pub fn text(text: impl Into<String>) -> Self {
        Self::Text { text: text.into() }
    }

    /// Create an image content part from base64 data.
    ///
    /// Wraps the data as a `data:` URI in [`ImageUrlContent`] for OpenAI wire
    /// compatibility.
    pub fn image(media_type: impl Into<String>, data: impl Into<String>) -> Self {
        let mt = media_type.into();
        let d = data.into();
        Self::ImageUrl {
            image_url: ImageUrlContent {
                url: format!("data:{mt};base64,{d}"),
                detail: None,
            },
        }
    }

    /// Create an audio content part from base64 data.
    pub fn audio(media_type: impl Into<String>, data: impl Into<String>) -> Self {
        Self::Audio {
            input_audio: MediaContent {
                media_type: media_type.into(),
                data: data.into(),
            },
        }
    }

    /// Create a video content part from base64 data.
    pub fn video(media_type: impl Into<String>, data: impl Into<String>) -> Self {
        Self::Video {
            video: MediaContent {
                media_type: media_type.into(),
                data: data.into(),
            },
        }
    }

    /// Create an image URL content part (legacy).
    ///
    /// Prefer [`ContentPart::image`] for inline base64 data.
    pub fn image_url(url: impl Into<String>) -> Self {
        Self::ImageUrl {
            image_url: ImageUrlContent {
                url: url.into(),
                detail: None,
            },
        }
    }
}

/// Image URL content with optional detail level.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ImageUrlContent {
    /// The image URL (or `data:` URI for inline base64 images).
    pub url: String,
    /// Optional detail level (e.g. "low", "high", "auto").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

/// A message in a conversation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Message {
    /// Role of the message sender
    pub role: Role,
    /// Content of the message
    pub content: MessageContent,
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
    /// assert_eq!(msg.content_text(), "Hello!");
    /// ```
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: MessageContent::Text(content.into()),
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
            content: MessageContent::Text(content.into()),
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
            content: MessageContent::Text(content.into()),
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
            content: MessageContent::Text(content.into()),
            name: None,
            tool_call_id: Some(tool_call_id.into()),
            tool_calls: None,
        }
    }

    /// Create a user message with multimodal content parts.
    pub fn user_parts(parts: Vec<ContentPart>) -> Self {
        Self {
            role: Role::User,
            content: MessageContent::Parts(parts),
            name: None,
            tool_call_id: None,
            tool_calls: None,
        }
    }

    /// Extract the first text string from the message content.
    ///
    /// For `MessageContent::Text`, returns the string directly.
    /// For `MessageContent::Parts`, returns the text of the first `Text` part.
    /// Returns `""` if no text is found.
    pub fn content_text(&self) -> &str {
        match &self.content {
            MessageContent::Text(s) => s.as_str(),
            MessageContent::Parts(parts) => parts
                .iter()
                .find_map(|p| match p {
                    ContentPart::Text { text } => Some(text.as_str()),
                    _ => None,
                })
                .unwrap_or(""),
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
    content: MessageContent,
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
            if wire.content.is_empty_content() {
                return Err(format!("message[{idx}].content cannot be empty"));
            }

            let content = wire.content;

            let mut msg = match wire.role {
                Role::System => Message {
                    role: Role::System,
                    content,
                    name: None,
                    tool_call_id: None,
                    tool_calls: None,
                },
                Role::User => Message {
                    role: Role::User,
                    content,
                    name: None,
                    tool_call_id: None,
                    tool_calls: None,
                },
                Role::Assistant => {
                    let mut m = Message {
                        role: Role::Assistant,
                        content,
                        name: None,
                        tool_call_id: None,
                        tool_calls: None,
                    };
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
                    Message {
                        role: Role::Tool,
                        content,
                        name: None,
                        tool_call_id: Some(call_id),
                        tool_calls: None,
                    }
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
        assert_eq!(msg.content, MessageContent::Text("test".to_string()));
        assert_eq!(msg.content_text(), "test");
        assert_eq!(msg.name, None);
        assert_eq!(msg.tool_call_id, None);
        assert_eq!(msg.tool_calls, None);
    }

    #[test]
    fn test_message_assistant() {
        let msg = Message::assistant("response");
        assert_eq!(msg.role, Role::Assistant);
        assert_eq!(msg.content_text(), "response");
        assert_eq!(msg.tool_calls, None);
    }

    #[test]
    fn test_message_system() {
        let msg = Message::system("instruction");
        assert_eq!(msg.role, Role::System);
        assert_eq!(msg.content_text(), "instruction");
        assert_eq!(msg.tool_calls, None);
    }

    #[test]
    fn test_message_tool() {
        let msg = Message::tool("result", "call_123");
        assert_eq!(msg.role, Role::Tool);
        assert_eq!(msg.content_text(), "result");
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

    #[test]
    fn test_message_user_text() {
        let msg = Message::user("hello");
        assert_eq!(msg.role, Role::User);
        assert_eq!(msg.content_text(), "hello");
    }

    #[test]
    fn test_message_multimodal() {
        let msg = Message::user_parts(vec![
            ContentPart::text("what is this?"),
            ContentPart::image_url("https://example.com/img.jpg"),
        ]);
        assert_eq!(msg.content_text(), "what is this?");
    }

    #[test]
    fn test_content_part_image_inline_serde() {
        let part = ContentPart::image(mime::IMAGE_PNG, "abc");
        let v = serde_json::to_value(&part).unwrap();
        assert_eq!(v["type"], "image_url");
        assert!(v["image_url"]["url"]
            .as_str()
            .unwrap()
            .starts_with("data:image/png;base64,"));
        let parsed: ContentPart = serde_json::from_value(v).unwrap();
        assert_eq!(parsed, part);
    }

    #[test]
    fn test_content_part_audio_video_serde() {
        let audio = ContentPart::audio(mime::AUDIO_WAV, "dGVzdA==");
        let json = serde_json::to_string(&audio).unwrap();
        let parsed: ContentPart = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, audio);

        let video = ContentPart::video(mime::VIDEO_MP4, "dGVzdA==");
        let json = serde_json::to_string(&video).unwrap();
        let parsed: ContentPart = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, video);
    }

    #[test]
    fn test_message_parts_image_only_not_empty() {
        let msg = Message::user_parts(vec![ContentPart::image(mime::IMAGE_JPEG, "e30=")]);
        assert!(!msg.content.is_empty_content());
    }

    #[test]
    fn test_message_content_serialization() {
        let msg = Message::user("hello");
        let json = serde_json::to_value(&msg).unwrap();
        assert_eq!(json["content"], "hello");
        let msg2 = Message::user_parts(vec![ContentPart::text("hi")]);
        let json2 = serde_json::to_value(&msg2).unwrap();
        assert!(json2["content"].is_array());
    }

    #[test]
    fn test_message_content_from_string() {
        let content: MessageContent = "hello".into();
        assert_eq!(content, MessageContent::Text("hello".to_string()));

        let content: MessageContent = String::from("world").into();
        assert_eq!(content, MessageContent::Text("world".to_string()));
    }
}
