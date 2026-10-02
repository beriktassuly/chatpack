//! JSON Lines (JSONL/NDJSON) output writer.
//!
//! JSONL format outputs one JSON object per line, making it ideal for:
//! - RAG (Retrieval-Augmented Generation) pipelines
//! - ML training data
//! - Streaming processing
//! - Large datasets that don't fit in memory

use std::fs::File;
use std::io::{BufWriter, Write};

use serde::Serialize;

use crate::Message;
use crate::core::models::OutputConfig;
use crate::error::ChatpackError;

/// Internal message representation for JSONL serialization.
///
/// Only includes fields enabled in [`OutputConfig`].
#[derive(Serialize)]
struct JsonlMessage {
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

impl JsonlMessage {
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

/// Writes messages to a JSONL (JSON Lines) file.
///
/// Each line is a complete, valid JSON object that can be parsed independently.
/// Also known as NDJSON (Newline Delimited JSON).
///
/// # Format
///
/// ```text
/// {"sender":"Alice","content":"Hello"}
/// {"sender":"Bob","content":"Hi"}
/// ```
///
/// # Examples
///
/// ```no_run
/// # #[cfg(feature = "json-output")]
/// # fn main() -> chatpack::Result<()> {
/// use chatpack::prelude::*;
///
/// let messages = vec![
///     Message::new("Alice", "Hello!"),
///     Message::new("Bob", "Hi there!"),
/// ];
/// write_jsonl(&messages, "output.jsonl", &OutputConfig::new())?;
/// # Ok(())
/// # }
/// # #[cfg(not(feature = "json-output"))]
/// # fn main() {}
/// ```
///
/// # Errors
///
/// Returns [`ChatpackError::Io`] if the file cannot be created or written.
pub fn write_jsonl(
    messages: &[Message],
    output_path: &str,
    config: &OutputConfig,
) -> Result<(), ChatpackError> {
    let file = File::create(output_path)?;
    let mut writer = BufWriter::new(file);

    for msg in messages {
        let json_msg = JsonlMessage::from_message(msg, config);
        let line = serde_json::to_string(&json_msg)?;
        writeln!(writer, "{line}")?;
    }

    writer.flush()?;
    Ok(())
}

/// Converts messages to a JSONL string.
///
/// Same format as [`write_jsonl`], but returns a [`String`] instead of writing
/// to a file. Useful for WASM environments or streaming to other destinations.
///
/// # Examples
///
/// ```
/// # #[cfg(feature = "json-output")]
/// # fn main() -> chatpack::Result<()> {
/// use chatpack::prelude::*;
///
/// let messages = vec![
///     Message::new("Alice", "Hello"),
///     Message::new("Bob", "Hi"),
/// ];
///
/// let jsonl = to_jsonl(&messages, &OutputConfig::new())?;
/// let lines: Vec<&str> = jsonl.lines().collect();
///
/// assert_eq!(lines.len(), 2);
/// assert!(lines[0].contains("Alice"));
/// # Ok(())
/// # }
/// # #[cfg(not(feature = "json-output"))]
/// # fn main() {}
/// ```
pub fn to_jsonl(messages: &[Message], config: &OutputConfig) -> Result<String, ChatpackError> {
    let mut output = String::new();

    for msg in messages {
        let json_msg = JsonlMessage::from_message(msg, config);
        let line = serde_json::to_string(&json_msg)?;
        output.push_str(&line);
        output.push('\n');
    }

    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader};
    use tempfile::NamedTempFile;

    #[test]
    fn test_to_jsonl_basic() {
        let messages = vec![Message::new("Alice", "Hello"), Message::new("Bob", "Hi")];
        let config = OutputConfig::new();

        let jsonl = to_jsonl(&messages, &config).unwrap();
        let lines: Vec<&str> = jsonl.lines().collect();

        assert_eq!(lines.len(), 2);

        let msg1: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
        assert_eq!(msg1["sender"], "Alice");
        assert_eq!(msg1["content"], "Hello");

        let msg2: serde_json::Value = serde_json::from_str(lines[1]).unwrap();
        assert_eq!(msg2["sender"], "Bob");
    }

    #[test]
    fn test_write_jsonl_basic() {
        let messages = vec![Message::new("Alice", "Hello"), Message::new("Bob", "Hi")];

        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path().to_str().unwrap();

        let config = OutputConfig::new();
        write_jsonl(&messages, path, &config).unwrap();

        // Read and verify each line is valid JSON
        let file = std::fs::File::open(path).unwrap();
        let reader = BufReader::new(file);
        let lines: Vec<String> = reader.lines().map(|l| l.unwrap()).collect();

        assert_eq!(lines.len(), 2);

        // Parse each line as JSON to verify validity
        let msg1: serde_json::Value = serde_json::from_str(&lines[0]).unwrap();
        assert_eq!(msg1["sender"], "Alice");
        assert_eq!(msg1["content"], "Hello");

        let msg2: serde_json::Value = serde_json::from_str(&lines[1]).unwrap();
        assert_eq!(msg2["sender"], "Bob");
        assert_eq!(msg2["content"], "Hi");
    }

    #[test]
    fn test_write_jsonl_with_metadata() {
        use chrono::TimeZone;

        let ts = chrono::Utc
            .with_ymd_and_hms(2024, 6, 15, 12, 30, 0)
            .unwrap();
        let edited = chrono::Utc.with_ymd_and_hms(2024, 6, 15, 13, 0, 0).unwrap();

        let msg = Message::new("Alice", "Hello")
            .with_timestamp(ts)
            .with_id(123)
            .with_edited(edited);

        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path().to_str().unwrap();

        let config = OutputConfig::new()
            .with_timestamps()
            .with_ids()
            .with_edited();
        write_jsonl(&[msg], path, &config).unwrap();

        let content = std::fs::read_to_string(path).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(content.trim()).unwrap();

        assert_eq!(parsed["timestamp"], "2024-06-15T12:30:00Z");
        assert_eq!(parsed["id"], 123);
        assert_eq!(parsed["edited"], "2024-06-15T13:00:00Z");
    }

    #[test]
    fn test_jsonl_no_trailing_comma() {
        let messages = vec![Message::new("Alice", "Hello")];

        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path().to_str().unwrap();

        write_jsonl(&messages, path, &OutputConfig::new()).unwrap();

        let content = std::fs::read_to_string(path).unwrap();
        // Should not contain array brackets or commas between objects
        assert!(!content.contains('['));
        assert!(!content.contains(']'));
    }
}
