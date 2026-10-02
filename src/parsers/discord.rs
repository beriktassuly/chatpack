//! Discord export parser.
//!
//! Parses exports from the DiscordChatExporter tool in JSON, TXT, or CSV format.

use std::fs::{self, File};
use std::io::BufReader;
use std::path::Path;

use chrono::{DateTime, NaiveDateTime, Utc};
use regex::Regex;
use serde::Deserialize;

use crate::Message;
use crate::config::DiscordConfig;
use crate::error::ChatpackError;
use crate::parser::{Parser, Platform};

#[cfg(feature = "streaming")]
use crate::streaming::{DiscordStreamingParser, StreamingConfig, StreamingParser};

/// Parser for Discord channel exports.
///
/// Handles exports created by [DiscordChatExporter](https://github.com/Tyrrrz/DiscordChatExporter).
/// Auto-detects format based on file extension.
///
/// # Supported Formats
///
/// | Extension | Format | Notes |
/// |-----------|--------|-------|
/// | `.json` | JSON | Full metadata, recommended |
/// | `.txt` | Plain text | Basic, regex-parsed |
/// | `.csv` | CSV | Tabular format |
///
/// # Message Types
///
/// - Regular messages
/// - Replies (preserves reference)
/// - Attachments (as placeholders)
/// - Stickers
/// - Embeds (text only)
///
/// # Examples
///
/// ```no_run
/// use chatpack::parsers::DiscordParser;
/// use chatpack::parser::Parser;
///
/// # fn main() -> chatpack::Result<()> {
/// let parser = DiscordParser::new();
///
/// // Auto-detects format from extension
/// let messages = parser.parse("channel.json".as_ref())?;
///
/// for msg in &messages {
///     if let Some(id) = msg.id {
///         println!("[{}] {}: {}", id, msg.sender, msg.content);
///     }
/// }
/// # Ok(())
/// # }
/// ```
pub struct DiscordParser {
    config: DiscordConfig,
}

impl DiscordParser {
    /// Creates a new parser with default configuration.
    pub fn new() -> Self {
        Self {
            config: DiscordConfig::default(),
        }
    }

    /// Creates a parser with custom configuration.
    pub fn with_config(config: DiscordConfig) -> Self {
        Self { config }
    }

    /// Creates a parser optimized for streaming large files.
    pub fn with_streaming() -> Self {
        Self {
            config: DiscordConfig::streaming(),
        }
    }

    /// Returns the current configuration.
    pub fn config(&self) -> &DiscordConfig {
        &self.config
    }

    /// Detect format from file extension
    fn detect_format_from_ext(file_path: &str) -> Option<DiscordFormat> {
        let path = Path::new(file_path);
        path.extension().and_then(|ext| {
            if ext.eq_ignore_ascii_case("json") {
                Some(DiscordFormat::Json)
            } else if ext.eq_ignore_ascii_case("csv") {
                Some(DiscordFormat::Csv)
            } else if ext.eq_ignore_ascii_case("txt") {
                Some(DiscordFormat::Txt)
            } else {
                None
            }
        })
    }

    /// Detect format from content
    fn detect_format_from_content(content: &str) -> DiscordFormat {
        let trimmed = content.trim();
        if trimmed.starts_with('{') {
            DiscordFormat::Json
        } else if trimmed.starts_with("AuthorID,") || trimmed.contains("\",\"") {
            DiscordFormat::Csv
        } else {
            DiscordFormat::Txt
        }
    }

    #[allow(clippy::unused_self)]
    fn parse_json(&self, content: &str) -> Result<Vec<Message>, ChatpackError> {
        let export: DiscordExport = serde_json::from_str(content)?;

        let messages = export
            .messages
            .iter()
            .filter_map(|msg| {
                // Skip empty messages without attachments/stickers
                if msg.content.trim().is_empty()
                    && msg.attachments.as_ref().is_none_or(|a| a.is_empty())
                    && msg.stickers.as_ref().is_none_or(|s| s.is_empty())
                {
                    return None;
                }

                // Build content: text + attachment/sticker info
                let mut content = msg.content.clone();

                // Append attachment filenames
                if let Some(attachments) = &msg.attachments {
                    for att in attachments {
                        if !content.is_empty() {
                            content.push('\n');
                        }
                        content.push_str(&format!("[Attachment: {}]", att.file_name));
                    }
                }

                // Append sticker names
                if let Some(stickers) = &msg.stickers {
                    for sticker in stickers {
                        if !content.is_empty() {
                            content.push('\n');
                        }
                        content.push_str(&format!("[Sticker: {}]", sticker.name));
                    }
                }

                // Use nickname if available, fallback to username
                let sender = msg
                    .author
                    .nickname
                    .as_ref()
                    .unwrap_or(&msg.author.name)
                    .clone();

                // Parse timestamp (ISO 8601)
                let timestamp = DateTime::parse_from_rfc3339(&msg.timestamp)
                    .ok()
                    .map(|dt| dt.to_utc());

                // Parse edited timestamp
                let edited = msg
                    .timestamp_edited
                    .as_ref()
                    .and_then(|ts| DateTime::parse_from_rfc3339(ts).ok())
                    .map(|dt| dt.to_utc());

                // Parse message ID (Discord snowflake)
                let id = msg.id.parse::<u64>().ok();

                // Parse reply reference
                let reply_to = msg
                    .reference
                    .as_ref()
                    .and_then(|r| r.message_id.as_ref())
                    .and_then(|id_str| id_str.parse::<u64>().ok());

                Some(Message::with_metadata(
                    sender, content, timestamp, id, reply_to, edited,
                ))
            })
            .collect();

        Ok(messages)
    }

    #[allow(clippy::unused_self)]
    fn parse_txt(&self, content: &str) -> Result<Vec<Message>, ChatpackError> {
        let mut messages = Vec::new();

        // Pattern: [M/D/YYYY H:MM AM] sender OR [M/D/YYYY H:MM:SS] sender
        let header_re = Regex::new(
            r"^\[(\d{1,2}/\d{1,2}/\d{4}\s+\d{1,2}:\d{2}(?::\d{2})?\s*(?:AM|PM)?)\]\s+(.+)$",
        )
        .map_err(|e| ChatpackError::invalid_format("Discord TXT", e.to_string()))?;

        let mut current_sender: Option<String> = None;
        let mut current_timestamp: Option<DateTime<Utc>> = None;
        let mut current_content = String::new();
        let mut in_attachments = false;
        let mut in_stickers = false;

        for line in content.lines() {
            // Check for message header
            if let Some(caps) = header_re.captures(line) {
                // Save previous message if exists
                if let Some(sender) = current_sender.take() {
                    if !current_content.trim().is_empty() {
                        messages.push(Message::with_metadata(
                            sender,
                            current_content.trim().to_string(),
                            current_timestamp,
                            None,
                            None,
                            None,
                        ));
                    }
                }

                // Parse new message header
                let timestamp_str = caps.get(1).unwrap().as_str();
                let sender = caps.get(2).unwrap().as_str().to_string();

                current_timestamp = Self::parse_txt_timestamp(timestamp_str);
                current_sender = Some(sender);
                current_content = String::new();
                in_attachments = false;
                in_stickers = false;
            } else if current_sender.is_some() {
                // Check for special sections
                if line == "{Attachments}" {
                    in_attachments = true;
                    in_stickers = false;
                    continue;
                }
                if line == "{Stickers}" {
                    in_stickers = true;
                    in_attachments = false;
                    continue;
                }

                // Handle content
                if in_attachments || in_stickers {
                    let trimmed = line.trim();
                    if !trimmed.is_empty() {
                        // Extract filename from URL or use as-is
                        let name = if trimmed.starts_with("http") {
                            trimmed.rsplit('/').next().unwrap_or(trimmed)
                        } else {
                            trimmed
                        };

                        if !current_content.is_empty() {
                            current_content.push('\n');
                        }
                        if in_attachments {
                            current_content.push_str(&format!("[Attachment: {}]", name));
                        } else {
                            current_content.push_str(&format!("[Sticker: {}]", name));
                        }
                    }
                } else {
                    // Regular message content
                    if !current_content.is_empty() {
                        current_content.push('\n');
                    }
                    current_content.push_str(line);
                }
            }
        }

        // Don't forget the last message
        if let Some(sender) = current_sender {
            if !current_content.trim().is_empty() {
                messages.push(Message::with_metadata(
                    sender,
                    current_content.trim().to_string(),
                    current_timestamp,
                    None,
                    None,
                    None,
                ));
            }
        }

        Ok(messages)
    }

    fn parse_txt_timestamp(s: &str) -> Option<DateTime<Utc>> {
        // Try formats: "M/D/YYYY H:MM AM", "M/D/YYYY H:MM:SS"
        let formats = [
            "%m/%d/%Y %I:%M %p",
            "%m/%d/%Y %I:%M:%S %p",
            "%m/%d/%Y %H:%M",
            "%m/%d/%Y %H:%M:%S",
        ];

        for fmt in &formats {
            if let Ok(dt) = NaiveDateTime::parse_from_str(s.trim(), fmt) {
                return Some(dt.and_utc());
            }
        }
        None
    }

    #[allow(clippy::unused_self)]
    fn parse_csv_file(&self, file_path: &str) -> Result<Vec<Message>, ChatpackError> {
        let file = File::open(file_path)?;
        let reader = BufReader::new(file);
        self.parse_csv_reader(reader)
    }

    #[allow(clippy::unused_self)]
    fn parse_csv_str(&self, content: &str) -> Result<Vec<Message>, ChatpackError> {
        let reader = content.as_bytes();
        self.parse_csv_reader(reader)
    }

    #[allow(clippy::unused_self)]
    fn parse_csv_reader<R: std::io::Read>(&self, reader: R) -> Result<Vec<Message>, ChatpackError> {
        let mut csv_reader = csv::ReaderBuilder::new()
            .has_headers(true)
            .flexible(true)
            .from_reader(reader);

        let mut messages = Vec::new();

        for result in csv_reader.records() {
            let record = result?;

            // CSV columns: AuthorID, Author, Date, Content, Attachments, Reactions
            let sender = record.get(1).unwrap_or("").to_string();
            let timestamp_str = record.get(2).unwrap_or("");
            let mut content = record.get(3).unwrap_or("").to_string();
            let attachments = record.get(4).unwrap_or("");

            // Skip empty messages
            if content.trim().is_empty() && attachments.trim().is_empty() {
                continue;
            }

            // Parse attachments (comma-separated URLs)
            if !attachments.trim().is_empty() {
                for url in attachments.split(',') {
                    let url = url.trim();
                    if !url.is_empty() {
                        let filename = url.rsplit('/').next().unwrap_or(url);
                        if !content.is_empty() {
                            content.push('\n');
                        }
                        content.push_str(&format!("[Attachment: {}]", filename));
                    }
                }
            }

            // Parse timestamp
            let timestamp = DateTime::parse_from_rfc3339(timestamp_str)
                .ok()
                .map(|dt| dt.to_utc());

            messages.push(Message::with_metadata(
                sender, content, timestamp, None, None, None,
            ));
        }

        Ok(messages)
    }
}

impl Default for DiscordParser {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy)]
enum DiscordFormat {
    Json,
    Txt,
    Csv,
}

// Internal structures for deserializing Discord JSON

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DiscordExport {
    messages: Vec<DiscordMessage>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DiscordMessage {
    id: String,
    timestamp: String,
    timestamp_edited: Option<String>,
    content: String,
    author: DiscordAuthor,
    reference: Option<DiscordReference>,
    attachments: Option<Vec<DiscordAttachment>>,
    stickers: Option<Vec<DiscordSticker>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DiscordAuthor {
    name: String,
    nickname: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(clippy::struct_field_names)]
#[serde(rename_all = "camelCase")]
struct DiscordReference {
    message_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DiscordAttachment {
    file_name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DiscordSticker {
    name: String,
}

impl DiscordParser {
    /// Parses content from file path (internal implementation).
    fn parse_file_internal(&self, file_path: &str) -> Result<Vec<Message>, ChatpackError> {
        // Try to detect format from extension first
        if let Some(format) = Self::detect_format_from_ext(file_path) {
            return match format {
                DiscordFormat::Csv => self.parse_csv_file(file_path),
                DiscordFormat::Json => {
                    let content = fs::read_to_string(file_path)?;
                    self.parse_json(&content)
                }
                DiscordFormat::Txt => {
                    let content = fs::read_to_string(file_path)?;
                    self.parse_txt(&content)
                }
            };
        }

        // Fallback: read content and detect from it
        let content = fs::read_to_string(file_path)?;
        self.parse_content(&content)
    }

    /// Parses content from a string (internal implementation).
    fn parse_content(&self, content: &str) -> Result<Vec<Message>, ChatpackError> {
        let format = Self::detect_format_from_content(content);

        match format {
            DiscordFormat::Json => self.parse_json(content),
            DiscordFormat::Txt => self.parse_txt(content),
            DiscordFormat::Csv => self.parse_csv_str(content),
        }
    }
}

// Implement the new unified Parser trait
impl Parser for DiscordParser {
    fn name(&self) -> &'static str {
        "Discord"
    }

    fn platform(&self) -> Platform {
        Platform::Discord
    }

    fn parse(&self, path: &Path) -> Result<Vec<Message>, ChatpackError> {
        self.parse_file_internal(path.to_str().unwrap_or_default())
    }

    fn parse_str(&self, content: &str) -> Result<Vec<Message>, ChatpackError> {
        self.parse_content(content)
    }

    #[cfg(feature = "streaming")]
    fn stream(
        &self,
        path: &Path,
    ) -> Result<Box<dyn Iterator<Item = Result<Message, ChatpackError>> + Send>, ChatpackError>
    {
        if self.config.streaming {
            // Use native streaming parser
            let streaming_config = StreamingConfig::new()
                .with_buffer_size(self.config.buffer_size)
                .with_max_message_size(self.config.max_message_size)
                .with_skip_invalid(self.config.skip_invalid);

            let streaming_parser = DiscordStreamingParser::with_config(streaming_config);
            let iterator =
                StreamingParser::stream(&streaming_parser, path.to_str().unwrap_or_default())?;

            Ok(Box::new(
                iterator.map(|result| result.map_err(ChatpackError::from)),
            ))
        } else {
            // Fallback: load everything into memory
            let messages = Parser::parse(self, path)?;
            Ok(Box::new(messages.into_iter().map(Ok)))
        }
    }

    #[cfg(feature = "streaming")]
    fn supports_streaming(&self) -> bool {
        self.config.streaming
    }

    #[cfg(feature = "streaming")]
    fn recommended_buffer_size(&self) -> usize {
        self.config.buffer_size
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // DiscordParser construction tests
    // =========================================================================

    #[test]
    fn test_parser_name() {
        let parser = DiscordParser::new();
        assert_eq!(Parser::name(&parser), "Discord");
    }

    #[test]
    fn test_parser_platform() {
        let parser = DiscordParser::new();
        assert_eq!(parser.platform(), Platform::Discord);
    }

    #[test]
    fn test_parser_default() {
        let parser = DiscordParser::default();
        assert_eq!(Parser::name(&parser), "Discord");
        assert!(parser.config().prefer_nickname);
        assert!(parser.config().include_attachments);
    }

    #[test]
    fn test_parser_with_config() {
        let config = DiscordConfig::new()
            .with_streaming(true)
            .with_prefer_nickname(false);
        let parser = DiscordParser::with_config(config);
        assert!(parser.config().streaming);
        assert!(!parser.config().prefer_nickname);
    }

    #[test]
    fn test_parser_with_streaming() {
        let parser = DiscordParser::with_streaming();
        assert!(parser.config().streaming);
    }

    // =========================================================================
    // Format detection tests
    // =========================================================================

    #[test]
    fn test_format_detection_from_ext() {
        assert!(matches!(
            DiscordParser::detect_format_from_ext("test.json"),
            Some(DiscordFormat::Json)
        ));
        assert!(matches!(
            DiscordParser::detect_format_from_ext("test.csv"),
            Some(DiscordFormat::Csv)
        ));
        assert!(matches!(
            DiscordParser::detect_format_from_ext("test.txt"),
            Some(DiscordFormat::Txt)
        ));

        // Case insensitive
        assert!(matches!(
            DiscordParser::detect_format_from_ext("test.JSON"),
            Some(DiscordFormat::Json)
        ));
        assert!(matches!(
            DiscordParser::detect_format_from_ext("test.CSV"),
            Some(DiscordFormat::Csv)
        ));
        assert!(matches!(
            DiscordParser::detect_format_from_ext("test.TXT"),
            Some(DiscordFormat::Txt)
        ));

        // No extension
        assert!(DiscordParser::detect_format_from_ext("test").is_none());
        assert!(DiscordParser::detect_format_from_ext("test.unknown").is_none());
    }

    #[test]
    fn test_format_detection_from_content() {
        assert!(matches!(
            DiscordParser::detect_format_from_content(r#"{"messages":[]}"#),
            DiscordFormat::Json
        ));
        assert!(matches!(
            DiscordParser::detect_format_from_content("AuthorID,Author,Date"),
            DiscordFormat::Csv
        ));
        assert!(matches!(
            DiscordParser::detect_format_from_content(r#""a","b","c""#),
            DiscordFormat::Csv
        ));
        assert!(matches!(
            DiscordParser::detect_format_from_content("[1/15/2024 10:30 AM] alice"),
            DiscordFormat::Txt
        ));
        assert!(matches!(
            DiscordParser::detect_format_from_content("plain text"),
            DiscordFormat::Txt
        ));
    }

    // =========================================================================
    // JSON parsing tests
    // =========================================================================

    #[test]
    fn test_parse_json_basic() {
        let parser = DiscordParser::new();
        let json = r#"{
            "messages": [
                {
                    "id": "123",
                    "timestamp": "2024-01-15T10:30:00+00:00",
                    "content": "Hello world",
                    "author": {"name": "alice", "nickname": null}
                }
            ]
        }"#;

        let messages = parser.parse_json(json).unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].sender, "alice");
        assert_eq!(messages[0].content, "Hello world");
        assert!(messages[0].timestamp.is_some());
        assert_eq!(messages[0].id, Some(123));
    }

    #[test]
    fn test_parse_json_with_nickname() {
        let parser = DiscordParser::new();
        let json = r#"{
            "messages": [
                {
                    "id": "123",
                    "timestamp": "2024-01-15T10:30:00+00:00",
                    "content": "Hi",
                    "author": {"name": "alice123", "nickname": "Alice"}
                }
            ]
        }"#;

        let messages = parser.parse_json(json).unwrap();
        assert_eq!(messages[0].sender, "Alice");
    }

    #[test]
    fn test_parse_json_with_attachments() {
        let parser = DiscordParser::new();
        let json = r#"{
            "messages": [
                {
                    "id": "123",
                    "timestamp": "2024-01-15T10:30:00+00:00",
                    "content": "Check this out",
                    "author": {"name": "alice"},
                    "attachments": [{"fileName": "image.png"}, {"fileName": "doc.pdf"}]
                }
            ]
        }"#;

        let messages = parser.parse_json(json).unwrap();
        assert_eq!(messages.len(), 1);
        assert!(messages[0].content.contains("Check this out"));
        assert!(messages[0].content.contains("[Attachment: image.png]"));
        assert!(messages[0].content.contains("[Attachment: doc.pdf]"));
    }

    #[test]
    fn test_parse_json_with_stickers() {
        let parser = DiscordParser::new();
        let json = r#"{
            "messages": [
                {
                    "id": "123",
                    "timestamp": "2024-01-15T10:30:00+00:00",
                    "content": "",
                    "author": {"name": "alice"},
                    "stickers": [{"name": "cool_sticker"}]
                }
            ]
        }"#;

        let messages = parser.parse_json(json).unwrap();
        assert_eq!(messages.len(), 1);
        assert!(messages[0].content.contains("[Sticker: cool_sticker]"));
    }

    #[test]
    fn test_parse_json_with_reply() {
        let parser = DiscordParser::new();
        let json = r#"{
            "messages": [
                {
                    "id": "124",
                    "timestamp": "2024-01-15T10:30:00+00:00",
                    "content": "Reply!",
                    "author": {"name": "bob"},
                    "reference": {"messageId": "123"}
                }
            ]
        }"#;

        let messages = parser.parse_json(json).unwrap();
        assert_eq!(messages[0].reply_to, Some(123));
    }

    #[test]
    fn test_parse_json_with_edited() {
        let parser = DiscordParser::new();
        let json = r#"{
            "messages": [
                {
                    "id": "123",
                    "timestamp": "2024-01-15T10:30:00+00:00",
                    "timestampEdited": "2024-01-15T10:35:00+00:00",
                    "content": "Edited message",
                    "author": {"name": "alice"}
                }
            ]
        }"#;

        let messages = parser.parse_json(json).unwrap();
        assert!(messages[0].edited.is_some());
    }

    #[test]
    fn test_parse_json_skips_empty() {
        let parser = DiscordParser::new();
        let json = r#"{
            "messages": [
                {
                    "id": "123",
                    "timestamp": "2024-01-15T10:30:00+00:00",
                    "content": "Hello",
                    "author": {"name": "alice"}
                },
                {
                    "id": "124",
                    "timestamp": "2024-01-15T10:31:00+00:00",
                    "content": "",
                    "author": {"name": "bob"}
                }
            ]
        }"#;

        let messages = parser.parse_json(json).unwrap();
        assert_eq!(messages.len(), 1);
    }

    #[test]
    fn test_parse_json_empty_messages() {
        let parser = DiscordParser::new();
        let json = r#"{"messages": []}"#;
        let messages = parser.parse_json(json).unwrap();
        assert_eq!(messages, []);
    }

    // =========================================================================
    // TXT parsing tests
    // =========================================================================

    #[test]
    fn test_txt_timestamp_parsing() {
        let ts = DiscordParser::parse_txt_timestamp("1/15/2024 10:30 AM");
        assert!(ts.is_some());

        let ts = DiscordParser::parse_txt_timestamp("12/31/2024 11:59 PM");
        assert!(ts.is_some());

        let ts = DiscordParser::parse_txt_timestamp("1/1/2024 1:00 AM");
        assert!(ts.is_some());

        // 24-hour format
        let ts = DiscordParser::parse_txt_timestamp("1/15/2024 14:30");
        assert!(ts.is_some());
    }

    #[test]
    fn test_parse_txt_basic() {
        let parser = DiscordParser::new();
        let txt = "[1/15/2024 10:30 AM] alice\nHello world\n[1/15/2024 10:31 AM] bob\nHi there";

        let messages = parser.parse_txt(txt).unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].sender, "alice");
        assert_eq!(messages[0].content, "Hello world");
        assert_eq!(messages[1].sender, "bob");
        assert_eq!(messages[1].content, "Hi there");
    }

    #[test]
    fn test_parse_txt_multiline() {
        let parser = DiscordParser::new();
        let txt = "[1/15/2024 10:30 AM] alice\nLine 1\nLine 2\nLine 3";

        let messages = parser.parse_txt(txt).unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].content, "Line 1\nLine 2\nLine 3");
    }

    #[test]
    fn test_parse_txt_with_attachments() {
        let parser = DiscordParser::new();
        let txt =
            "[1/15/2024 10:30 AM] alice\nMessage\n{Attachments}\nhttps://cdn.discord.com/image.png";

        let messages = parser.parse_txt(txt).unwrap();
        assert_eq!(messages.len(), 1);
        assert!(messages[0].content.contains("Message"));
        assert!(messages[0].content.contains("[Attachment: image.png]"));
    }

    #[test]
    fn test_parse_txt_with_stickers() {
        let parser = DiscordParser::new();
        let txt = "[1/15/2024 10:30 AM] alice\n{Stickers}\ncool_sticker";

        let messages = parser.parse_txt(txt).unwrap();
        assert_eq!(messages.len(), 1);
        assert!(messages[0].content.contains("[Sticker: cool_sticker]"));
    }

    #[test]
    fn test_parse_txt_empty() {
        let parser = DiscordParser::new();
        let txt = "";
        let messages = parser.parse_txt(txt).unwrap();
        assert_eq!(messages, []);
    }

    // =========================================================================
    // CSV parsing tests
    // =========================================================================

    #[test]
    fn test_parse_csv_basic() {
        let parser = DiscordParser::new();
        let csv = "AuthorID,Author,Date,Content,Attachments,Reactions\n123,alice,2024-01-15T10:30:00+00:00,Hello world,,";

        let messages = parser.parse_csv_str(csv).unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].sender, "alice");
        assert_eq!(messages[0].content, "Hello world");
    }

    #[test]
    fn test_parse_csv_with_attachments() {
        let parser = DiscordParser::new();
        let csv = "AuthorID,Author,Date,Content,Attachments,Reactions\n123,alice,2024-01-15T10:30:00+00:00,Check this,https://cdn.discord.com/image.png,";

        let messages = parser.parse_csv_str(csv).unwrap();
        assert_eq!(messages.len(), 1);
        assert!(messages[0].content.contains("Check this"));
        assert!(messages[0].content.contains("[Attachment: image.png]"));
    }

    #[test]
    fn test_parse_csv_skips_empty() {
        let parser = DiscordParser::new();
        let csv = "AuthorID,Author,Date,Content,Attachments,Reactions\n123,alice,2024-01-15T10:30:00+00:00,Hello,,\n124,bob,2024-01-15T10:31:00+00:00,,,";

        let messages = parser.parse_csv_str(csv).unwrap();
        assert_eq!(messages.len(), 1);
    }

    // =========================================================================
    // parse_str auto-detection tests
    // =========================================================================

    #[test]
    fn test_parse_str_json() {
        let parser = DiscordParser::new();
        let json = r#"{"messages": [{"id": "1", "timestamp": "2024-01-15T10:30:00+00:00", "content": "Test", "author": {"name": "bob"}}]}"#;

        let messages = Parser::parse_str(&parser, json).unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].sender, "bob");
    }

    #[test]
    fn test_parse_str_csv() {
        let parser = DiscordParser::new();
        let csv =
            "AuthorID,Author,Date,Content,Attachments\n123,alice,2024-01-15T10:30:00+00:00,Hello,";

        let messages = Parser::parse_str(&parser, csv).unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].sender, "alice");
    }

    #[test]
    fn test_parse_str_txt() {
        let parser = DiscordParser::new();
        let txt = "[1/15/2024 10:30 AM] alice\nHello world";

        let messages = Parser::parse_str(&parser, txt).unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].sender, "alice");
    }

    // =========================================================================
    // Streaming support tests
    // =========================================================================

    #[cfg(feature = "streaming")]
    #[test]
    fn test_supports_streaming_false_by_default() {
        let parser = DiscordParser::new();
        assert!(!parser.supports_streaming());
    }

    #[cfg(feature = "streaming")]
    #[test]
    fn test_supports_streaming_true_when_enabled() {
        let parser = DiscordParser::with_streaming();
        assert!(parser.supports_streaming());
    }

    #[cfg(feature = "streaming")]
    #[test]
    fn test_recommended_buffer_size() {
        let parser = DiscordParser::new();
        assert_eq!(parser.recommended_buffer_size(), 64 * 1024);

        let streaming_parser = DiscordParser::with_streaming();
        assert_eq!(streaming_parser.recommended_buffer_size(), 256 * 1024);
    }
}
