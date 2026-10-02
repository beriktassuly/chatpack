//! CSV output writer with semicolon delimiter.
//!
//! CSV format provides the best token efficiency for LLM context windows,
//! achieving up to 13x compression compared to raw chat exports.

use std::fs::File;

use crate::Message;
use crate::core::models::OutputConfig;
use crate::error::ChatpackError;

/// Writes messages to a CSV file.
///
/// Uses semicolon (`;`) as delimiter for Excel compatibility and to avoid
/// conflicts with commas in message content.
///
/// # Format
///
/// Columns depend on [`OutputConfig`]:
/// - Base: `Sender`, `Content`
/// - `with_timestamps()`: adds `Timestamp` column
/// - `with_ids()`: adds `ID` column
/// - `with_replies()`: adds `ReplyTo`, `ReplyToSender`, and `ReplyToTopic` columns
/// - `with_edited()`: adds `Edited` column
///
/// # Examples
///
/// ```no_run
/// # #[cfg(feature = "csv-output")]
/// # fn main() -> chatpack::Result<()> {
/// use chatpack::prelude::*;
///
/// let messages = vec![Message::new("Alice", "Hello!")];
/// write_csv(&messages, "output.csv", &OutputConfig::new())?;
/// # Ok(())
/// # }
/// # #[cfg(not(feature = "csv-output"))]
/// # fn main() {}
/// ```
///
/// # Errors
///
/// Returns [`ChatpackError::Io`] if the file cannot be created or written.
pub fn write_csv(
    messages: &[Message],
    output_path: &str,
    config: &OutputConfig,
) -> Result<(), ChatpackError> {
    let file = File::create(output_path)?;
    let mut writer = csv::WriterBuilder::new().delimiter(b';').from_writer(file);

    // Build header dynamically
    let header = build_header(config);
    writer.write_record(&header)?;

    // Write each message
    for msg in messages {
        let record = build_record(msg, config);
        writer.write_record(&record)?;
    }

    writer.flush()?;
    Ok(())
}

/// Converts messages to a CSV string.
///
/// Same format as [`write_csv`], but returns a [`String`] instead of writing
/// to a file. Useful for WASM environments or when you need the output in memory.
///
/// # Examples
///
/// ```
/// # #[cfg(feature = "csv-output")]
/// # fn main() -> chatpack::Result<()> {
/// use chatpack::prelude::*;
///
/// let messages = vec![
///     Message::new("Alice", "Hello"),
///     Message::new("Bob", "Hi"),
/// ];
///
/// let csv = to_csv(&messages, &OutputConfig::new())?;
/// assert!(csv.contains("Sender;Content"));
/// assert!(csv.contains("Alice;Hello"));
/// # Ok(())
/// # }
/// # #[cfg(not(feature = "csv-output"))]
/// # fn main() {}
/// ```
pub fn to_csv(messages: &[Message], config: &OutputConfig) -> Result<String, ChatpackError> {
    let mut writer = csv::WriterBuilder::new()
        .delimiter(b';')
        .from_writer(Vec::new());

    let header = build_header(config);
    writer.write_record(&header)?;

    for msg in messages {
        let record = build_record(msg, config);
        writer.write_record(&record)?;
    }

    let bytes = writer
        .into_inner()
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    Ok(String::from_utf8(bytes)?)
}

/// Build CSV header based on output configuration.
fn build_header(config: &OutputConfig) -> Vec<&'static str> {
    let mut header = Vec::new();

    if config.include_ids {
        header.push("ID");
    }
    if config.include_timestamps {
        header.push("Timestamp");
    }

    header.push("Sender");
    header.push("Content");

    if config.include_replies {
        header.push("ReplyTo");
        header.push("ReplyToSender");
        header.push("ReplyToTopic");
    }
    if config.include_edited {
        header.push("Edited");
    }

    header
}

/// Build CSV record for a single message.
fn build_record(msg: &Message, config: &OutputConfig) -> Vec<String> {
    let mut record = Vec::new();

    if config.include_ids {
        record.push(msg.id.map(|id| id.to_string()).unwrap_or_default());
    }
    if config.include_timestamps {
        record.push(
            msg.timestamp
                .map(|ts| ts.format("%Y-%m-%d %H:%M:%S").to_string())
                .unwrap_or_default(),
        );
    }

    record.push(msg.sender.clone());
    record.push(msg.content.clone());

    if config.include_replies {
        record.push(msg.reply_to.map(|id| id.to_string()).unwrap_or_default());
        record.push(msg.reply_to_sender.clone().unwrap_or_default());
        record.push(msg.reply_to_topic.clone().unwrap_or_default());
    }
    if config.include_edited {
        record.push(
            msg.edited
                .map(|ts| ts.format("%Y-%m-%d %H:%M:%S").to_string())
                .unwrap_or_default(),
        );
    }

    record
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_csv_basic() {
        let messages = vec![Message::new("Alice", "Hello")];
        let config = OutputConfig::default();

        let csv = to_csv(&messages, &config).unwrap();
        assert!(csv.contains("Sender;Content"));
        assert!(csv.contains("Alice;Hello"));
    }
}
