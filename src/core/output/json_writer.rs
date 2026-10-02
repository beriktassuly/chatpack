//! JSON array output writer.
//!
//! Writes messages as a JSON array, suitable for APIs and structured data processing.

use std::fs::File;
use std::io::Write;

use serde::Serialize;

use crate::Message;
use crate::core::models::OutputConfig;
use crate::error::ChatpackError;

/// Internal message representation for JSON serialization.
///
/// Only includes fields enabled in [`OutputConfig`].
#[derive(Serialize)]
struct JsonMessage {
    sender: String,
    content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    timestamp: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reply_to: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reply_to_sender: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reply_to_topic: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    edited: Option<String>,
}

impl JsonMessage {
    fn from_message(msg: &Message, config: &OutputConfig) -> Self {
        Self {
            sender: msg.sender.clone(),
            content: msg.content.clone(),
            timestamp: if config.include_timestamps {
                msg.timestamp
                    .map(|ts| ts.format("%Y-%m-%dT%H:%M:%SZ").to_string())
            } else {
                None
            },
            id: if config.include_ids { msg.id } else { None },
            reply_to: if config.include_replies {
                msg.reply_to
            } else {
                None
            },
            reply_to_sender: if config.include_replies {
                msg.reply_to_sender.clone()
            } else {
                None
            },
            reply_to_topic: if config.include_replies {
                msg.reply_to_topic.clone()
            } else {
                None
            },
            edited: if config.include_edited {
                msg.edited
                    .map(|ts| ts.format("%Y-%m-%dT%H:%M:%SZ").to_string())
            } else {
                None
            },
        }
    }
}

/// Writes messages to a JSON file as an array.
///
/// Produces a pretty-printed JSON array suitable for APIs and structured
/// data processing.
///
/// # Format
///
/// ```json
/// [
///   {"sender": "Alice", "content": "Hello"},
///   {"sender": "Bob", "content": "Hi"}
/// ]
/// ```
///
/// # Examples
///
/// ```no_run
/// # #[cfg(feature = "json-output")]
/// # fn main() -> chatpack::Result<()> {
/// use chatpack::prelude::*;
///
/// let messages = vec![Message::new("Alice", "Hello!")];
/// write_json(&messages, "output.json", &OutputConfig::new())?;
/// # Ok(())
/// # }
/// # #[cfg(not(feature = "json-output"))]
/// # fn main() {}
/// ```
///
/// # Errors
///
/// Returns [`ChatpackError::Io`] if the file cannot be created or written.
pub fn write_json(
    messages: &[Message],
    output_path: &str,
    config: &OutputConfig,
) -> Result<(), ChatpackError> {
    let json = to_json(messages, config)?;
    let mut file = File::create(output_path)?;
    file.write_all(json.as_bytes())?;
    Ok(())
}

/// Converts messages to a JSON array string.
///
/// Same format as [`write_json`], but returns a [`String`] instead of writing
/// to a file. Useful for WASM environments or API responses.
///
/// # Examples
///
/// ```
/// # #[cfg(feature = "json-output")]
/// # fn main() -> chatpack::Result<()> {
/// use chatpack::prelude::*;
///
/// let messages = vec![Message::new("Alice", "Hello!")];
/// let json = to_json(&messages, &OutputConfig::new())?;
///
/// assert!(json.contains(r#""sender": "Alice""#));
/// # Ok(())
/// # }
/// # #[cfg(not(feature = "json-output"))]
/// # fn main() {}
/// ```
pub fn to_json(messages: &[Message], config: &OutputConfig) -> Result<String, ChatpackError> {
    let json_messages: Vec<JsonMessage> = messages
        .iter()
        .map(|m| JsonMessage::from_message(m, config))
        .collect();

    Ok(serde_json::to_string_pretty(&json_messages)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;
    use tempfile::NamedTempFile;

    #[test]
    fn test_to_json_basic() {
        let messages = vec![Message::new("Alice", "Hello"), Message::new("Bob", "Hi")];
        let config = OutputConfig::new();

        let json = to_json(&messages, &config).unwrap();

        assert!(json.contains(r#""sender": "Alice""#));
        assert!(json.contains(r#""content": "Hello""#));
        assert!(!json.contains("timestamp"));
    }

    #[test]
    fn test_write_json_basic() {
        let messages = vec![Message::new("Alice", "Hello"), Message::new("Bob", "Hi")];

        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path().to_str().unwrap();

        let config = OutputConfig::new();
        write_json(&messages, path, &config).unwrap();

        let mut content = String::new();
        std::fs::File::open(path)
            .unwrap()
            .read_to_string(&mut content)
            .unwrap();

        assert!(content.contains(r#""sender": "Alice""#));
        assert!(content.contains(r#""content": "Hello""#));
        // Should NOT contain timestamp when not enabled
        assert!(!content.contains("timestamp"));
    }

    #[test]
    fn test_write_json_with_metadata() {
        use chrono::TimeZone;

        let ts = chrono::Utc
            .with_ymd_and_hms(2024, 6, 15, 12, 30, 0)
            .unwrap();
        let msg = Message::new("Alice", "Hello")
            .with_timestamp(ts)
            .with_id(123);

        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path().to_str().unwrap();

        let config = OutputConfig::new().with_timestamps().with_ids();
        write_json(&[msg], path, &config).unwrap();

        let mut content = String::new();
        std::fs::File::open(path)
            .unwrap()
            .read_to_string(&mut content)
            .unwrap();

        assert!(content.contains(r#""timestamp": "2024-06-15T12:30:00Z""#));
        assert!(content.contains(r#""id": 123"#));
    }
}
