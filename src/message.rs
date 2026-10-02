//! Universal message type for all chat platforms.
//!
//! This module provides [`Message`], the normalized representation of chat messages.
//! All platform parsers convert their native formats into this structure, enabling
//! uniform processing regardless of source.
//!
//! # Overview
//!
//! A message consists of:
//! - **Required**: `sender` and `content`
//! - **Optional**: `timestamp`, `id`, `reply_to`, `reply_to_sender`, `reply_to_topic`, `edited`
//!
//! # Examples
//!
//! ## Basic Usage
//!
//! ```
//! use chatpack::Message;
//!
//! let msg = Message::new("Alice", "Hello, world!");
//! assert_eq!(msg.sender(), "Alice");
//! assert_eq!(msg.content(), "Hello, world!");
//! ```
//!
//! ## Builder Pattern
//!
//! ```
//! use chatpack::Message;
//! use chrono::Utc;
//!
//! let msg = Message::new("Bob", "Check this out!")
//!     .with_id(12345)
//!     .with_timestamp(Utc::now())
//!     .with_reply_to(12344);
//!
//! assert!(msg.has_metadata());
//! ```
//!
//! ## Serialization
//!
//! ```
//! use chatpack::Message;
//!
//! let msg = Message::new("Alice", "Hello!");
//! let json = serde_json::to_string(&msg)?;
//! let parsed: Message = serde_json::from_str(&json)?;
//!
//! assert_eq!(msg, parsed);
//! # Ok::<(), serde_json::Error>(())
//! ```

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A normalized chat message from any supported platform.
///
/// This struct is the core data type in chatpack. All platform-specific parsers
/// convert their native message formats into this universal representation,
/// enabling uniform processing, filtering, and export.
///
/// # Fields
///
/// | Field | Type | Description |
/// |-------|------|-------------|
/// | `sender` | `String` | Display name or username of the message author |
/// | `content` | `String` | Text content of the message |
/// | `timestamp` | `Option<DateTime<Utc>>` | When the message was sent |
/// | `id` | `Option<u64>` | Platform-specific message identifier |
/// | `reply_to` | `Option<u64>` | ID of the referenced message or topic service event |
/// | `reply_to_sender` | `Option<String>` | Author of the referenced message, when available |
/// | `reply_to_topic` | `Option<String>` | Title of a referenced Telegram topic creation event |
/// | `edited` | `Option<DateTime<Utc>>` | When the message was last edited |
///
/// # Construction
///
/// Use [`Message::new`] for simple messages or the builder pattern for metadata:
///
/// ```
/// use chatpack::Message;
/// use chrono::Utc;
///
/// // Simple message
/// let msg = Message::new("Alice", "Hello!");
///
/// // With metadata
/// let msg = Message::new("Alice", "Hello!")
///     .with_timestamp(Utc::now())
///     .with_id(12345);
/// ```
///
/// # Serialization
///
/// Implements `Serialize` and `Deserialize` with these behaviors:
/// - Optional fields are omitted from JSON when `None`
/// - Timestamps use RFC 3339 format
/// - Suitable for storage, IPC, and RAG pipelines
///
/// ```
/// use chatpack::Message;
///
/// let msg = Message::new("Alice", "Hello!").with_id(123);
/// let json = serde_json::to_string(&msg)?;
///
/// // timestamp is omitted (None)
/// assert!(!json.contains("timestamp"));
/// assert!(json.contains("123"));
/// # Ok::<(), serde_json::Error>(())
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Message {
    /// Display name or username of the message author.
    pub sender: String,

    /// Text content of the message.
    ///
    /// May contain newlines for multiline messages. Platform-specific
    /// attachments (images, files) are typically represented as text
    /// placeholders like `[Attachment: image.png]`.
    pub content: String,

    /// When the message was originally sent.
    ///
    /// Available from most platforms except some WhatsApp export formats.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub timestamp: Option<DateTime<Utc>>,

    /// Platform-specific message identifier.
    ///
    /// - Telegram: message ID from the chat
    /// - Discord: snowflake ID
    /// - WhatsApp/Instagram: typically not available in exports
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub id: Option<u64>,

    /// ID of the message this is replying to.
    ///
    /// Enables reconstruction of reply chains and conversation threads.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub reply_to: Option<u64>,

    /// Author of the referenced message, when the export contains it.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub reply_to_sender: Option<String>,

    /// Title of the referenced topic, when the reply target creates a topic.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub reply_to_topic: Option<String>,

    /// When the message was last edited.
    ///
    /// Present when the platform tracks edit history (Telegram, Discord).
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub edited: Option<DateTime<Utc>>,
}

impl Message {
    /// Creates a new message with only sender and content.
    ///
    /// All metadata fields (timestamp, id, `reply_to`, edited) are set to `None`.
    ///
    /// # Example
    ///
    /// ```rust
    /// use chatpack::Message;
    ///
    /// let msg = Message::new("Alice", "Hello!");
    /// assert_eq!(msg.sender(), "Alice");
    /// assert_eq!(msg.content(), "Hello!");
    /// assert!(msg.timestamp().is_none());
    /// ```
    pub fn new(sender: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            sender: sender.into(),
            content: content.into(),
            timestamp: None,
            id: None,
            reply_to: None,
            reply_to_sender: None,
            reply_to_topic: None,
            edited: None,
        }
    }

    /// Creates a new message with all fields specified.
    ///
    /// Use this when you have all metadata available upfront.
    /// For incremental construction, prefer [`new`](Self::new) with builder methods.
    pub fn with_metadata(
        sender: impl Into<String>,
        content: impl Into<String>,
        timestamp: Option<DateTime<Utc>>,
        id: Option<u64>,
        reply_to: Option<u64>,
        edited: Option<DateTime<Utc>>,
    ) -> Self {
        Self {
            sender: sender.into(),
            content: content.into(),
            timestamp,
            id,
            reply_to,
            reply_to_sender: None,
            reply_to_topic: None,
            edited,
        }
    }

    // =========================================================================
    // Builder methods
    // =========================================================================

    /// Builder method to set the timestamp.
    ///
    /// # Example
    ///
    /// ```rust
    /// use chatpack::Message;
    /// use chrono::Utc;
    ///
    /// let msg = Message::new("Alice", "Hello")
    ///     .with_timestamp(Utc::now());
    /// assert!(msg.timestamp().is_some());
    /// ```
    #[must_use]
    pub fn with_timestamp(mut self, ts: DateTime<Utc>) -> Self {
        self.timestamp = Some(ts);
        self
    }

    /// Builder method to set the message ID.
    ///
    /// # Example
    ///
    /// ```rust
    /// use chatpack::Message;
    ///
    /// let msg = Message::new("Alice", "Hello")
    ///     .with_id(12345);
    /// assert_eq!(msg.id(), Some(12345));
    /// ```
    #[must_use]
    pub fn with_id(mut self, id: u64) -> Self {
        self.id = Some(id);
        self
    }

    /// Builder method to set the reply reference.
    ///
    /// # Example
    ///
    /// ```rust
    /// use chatpack::Message;
    ///
    /// let msg = Message::new("Bob", "I agree!")
    ///     .with_reply_to(12344);
    /// assert_eq!(msg.reply_to(), Some(12344));
    /// ```
    #[must_use]
    pub fn with_reply_to(mut self, reply_id: u64) -> Self {
        self.reply_to = Some(reply_id);
        self
    }

    /// Builder method to set the edited timestamp.
    ///
    /// # Example
    ///
    /// ```rust
    /// use chatpack::Message;
    /// use chrono::Utc;
    ///
    /// let msg = Message::new("Alice", "Updated message")
    ///     .with_edited(Utc::now());
    /// assert!(msg.edited().is_some());
    /// ```
    #[must_use]
    pub fn with_edited(mut self, ts: DateTime<Utc>) -> Self {
        self.edited = Some(ts);
        self
    }

    // =========================================================================
    // Accessor methods
    // =========================================================================

    /// Returns the sender name.
    pub fn sender(&self) -> &str {
        &self.sender
    }

    /// Returns the message content.
    pub fn content(&self) -> &str {
        &self.content
    }

    /// Returns the timestamp, if available.
    pub fn timestamp(&self) -> Option<DateTime<Utc>> {
        self.timestamp
    }

    /// Returns the message ID, if available.
    pub fn id(&self) -> Option<u64> {
        self.id
    }

    /// Returns the reply-to ID, if available.
    pub fn reply_to(&self) -> Option<u64> {
        self.reply_to
    }

    /// Returns the edited timestamp, if available.
    pub fn edited(&self) -> Option<DateTime<Utc>> {
        self.edited
    }

    // =========================================================================
    // Utility methods
    // =========================================================================

    /// Returns `true` if this message has any metadata (timestamp, id, `reply_to`, or edited).
    pub fn has_metadata(&self) -> bool {
        self.timestamp.is_some()
            || self.id.is_some()
            || self.reply_to.is_some()
            || self.edited.is_some()
    }

    /// Returns `true` if this message's content is empty or whitespace-only.
    pub fn is_empty(&self) -> bool {
        self.content.trim().is_empty()
    }
}

impl Default for Message {
    fn default() -> Self {
        Self::new("", "")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn test_message_new() {
        let msg = Message::new("Alice", "Hello");
        assert_eq!(msg.sender(), "Alice");
        assert_eq!(msg.content(), "Hello");
        assert!(msg.timestamp().is_none());
        assert!(msg.id().is_none());
    }

    #[test]
    fn test_message_builder() {
        let ts = Utc.with_ymd_and_hms(2024, 6, 15, 12, 0, 0).unwrap();
        let msg = Message::new("Alice", "Hello")
            .with_timestamp(ts)
            .with_id(123)
            .with_reply_to(122)
            .with_edited(ts);

        assert_eq!(msg.timestamp(), Some(ts));
        assert_eq!(msg.id(), Some(123));
        assert_eq!(msg.reply_to(), Some(122));
        assert_eq!(msg.edited(), Some(ts));
    }

    #[test]
    fn test_message_has_metadata() {
        let msg1 = Message::new("Alice", "Hello");
        assert!(!msg1.has_metadata());

        let msg2 = Message::new("Alice", "Hello").with_id(123);
        assert!(msg2.has_metadata());
    }

    #[test]
    fn test_message_is_empty() {
        assert!(Message::new("Alice", "").is_empty());
        assert!(Message::new("Alice", "   ").is_empty());
        assert!(!Message::new("Alice", "Hello").is_empty());
    }

    #[test]
    fn test_message_serialization() {
        let msg = Message::new("Alice", "Hello").with_id(123);
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("Alice"));
        assert!(json.contains("123"));
        // timestamp should be skipped (None)
        assert!(!json.contains("timestamp"));
    }

    #[test]
    fn test_message_deserialization() {
        let json = r#"{"sender":"Bob","content":"Hi","id":456}"#;
        let msg: Message = serde_json::from_str(json).unwrap();
        assert_eq!(msg.sender(), "Bob");
        assert_eq!(msg.content(), "Hi");
        assert_eq!(msg.id(), Some(456));
        assert!(msg.timestamp().is_none());
    }

    #[test]
    fn test_message_accessors() {
        let ts = Utc.with_ymd_and_hms(2024, 6, 15, 12, 0, 0).unwrap();
        let msg = Message::new("Alice", "Hello")
            .with_timestamp(ts)
            .with_id(123)
            .with_reply_to(122)
            .with_edited(ts);

        assert_eq!(msg.sender(), "Alice");
        assert_eq!(msg.content(), "Hello");
        assert_eq!(msg.timestamp(), Some(ts));
        assert_eq!(msg.id(), Some(123));
        assert_eq!(msg.reply_to(), Some(122));
        assert_eq!(msg.edited(), Some(ts));
    }
}
