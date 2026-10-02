//! Telegram JSON export parser.
//!
//! Parses JSON exports from Telegram Desktop's "Export chat history" feature.

use std::fs;
use std::path::Path;

use crate::Message;
use crate::config::TelegramConfig;
use crate::error::ChatpackError;
use crate::parser::{Parser, Platform};
use crate::parsing::telegram::{TelegramExport, parse_telegram_export};

#[cfg(feature = "streaming")]
use crate::streaming::{StreamingConfig, StreamingParser, TelegramStreamingParser};

/// Parser for Telegram Desktop JSON exports.
///
/// Handles the `result.json` file produced by Telegram Desktop's
/// "Export chat history" feature (Settings > Advanced > Export).
///
/// # Supported Message Types
///
/// - Text messages (plain and with entities like links, mentions)
/// - Service messages (joins, leaves, pins)
/// - Forwarded messages
/// - Replies (preserves `reply_to` reference)
/// - Edited messages (preserves edit timestamp)
///
/// # JSON Structure
///
/// ```json
/// {
///   "name": "Chat Name",
///   "messages": [
///     {
///       "id": 12345,
///       "type": "message",
///       "date_unixtime": "1234567890",
///       "from": "Sender Name",
///       "text": "Hello"
///     }
///   ]
/// }
/// ```
///
/// # Examples
///
/// ```no_run
/// use chatpack::parsers::TelegramParser;
/// use chatpack::parser::Parser;
///
/// # fn main() -> chatpack::Result<()> {
/// let parser = TelegramParser::new();
/// let messages = parser.parse("result.json".as_ref())?;
///
/// for msg in &messages {
///     println!("{}: {}", msg.sender, msg.content);
/// }
/// # Ok(())
/// # }
/// ```
pub struct TelegramParser {
    config: TelegramConfig,
}

impl TelegramParser {
    /// Creates a new parser with default configuration.
    pub fn new() -> Self {
        Self {
            config: TelegramConfig::default(),
        }
    }

    /// Creates a parser with custom configuration.
    pub fn with_config(config: TelegramConfig) -> Self {
        Self { config }
    }

    /// Creates a parser optimized for streaming large files.
    pub fn with_streaming() -> Self {
        Self {
            config: TelegramConfig::streaming(),
        }
    }

    /// Returns the current configuration.
    pub fn config(&self) -> &TelegramConfig {
        &self.config
    }

    /// Parses content from a string (internal implementation).
    #[allow(clippy::unused_self)] // Keep &self for consistency with other parsers and future config use
    fn parse_content(&self, content: &str) -> Result<Vec<Message>, ChatpackError> {
        let export: TelegramExport = serde_json::from_str(content)?;

        Ok(parse_telegram_export(&export))
    }
}

impl Default for TelegramParser {
    fn default() -> Self {
        Self::new()
    }
}

// Implement the new unified Parser trait
impl Parser for TelegramParser {
    fn name(&self) -> &'static str {
        "Telegram"
    }

    fn platform(&self) -> Platform {
        Platform::Telegram
    }

    fn parse(&self, path: &Path) -> Result<Vec<Message>, ChatpackError> {
        let content = fs::read_to_string(path)?;
        self.parse_content(&content)
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

            let streaming_parser = TelegramStreamingParser::with_config(streaming_config);
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
    use crate::parsing::telegram::extract_telegram_text;
    use serde_json::json;

    // =========================================================================
    // extract_telegram_text tests
    // =========================================================================

    #[test]
    fn test_extract_text_string() {
        let value = json!("Hello world");
        assert_eq!(extract_telegram_text(&value), "Hello world");
    }

    #[test]
    fn test_extract_text_array_with_link() {
        let value = json!([
            "Check this: ",
            {"type": "link", "text": "https://example.com"},
            " cool!"
        ]);
        assert_eq!(
            extract_telegram_text(&value),
            "Check this: https://example.com cool!"
        );
    }

    #[test]
    fn test_extract_text_array_with_bold_italic() {
        let value = json!([
            "Normal ",
            {"type": "bold", "text": "bold"},
            " and ",
            {"type": "italic", "text": "italic"}
        ]);
        assert_eq!(extract_telegram_text(&value), "Normal bold and italic");
    }

    #[test]
    fn test_extract_text_empty() {
        let value = json!(null);
        assert_eq!(extract_telegram_text(&value), "");
    }

    #[test]
    fn test_extract_text_empty_string() {
        let value = json!("");
        assert_eq!(extract_telegram_text(&value), "");
    }

    #[test]
    fn test_extract_text_empty_array() {
        let value = json!([]);
        assert_eq!(extract_telegram_text(&value), "");
    }

    // =========================================================================
    // TelegramParser tests
    // =========================================================================

    #[test]
    fn test_parser_name() {
        let parser = TelegramParser::new();
        assert_eq!(Parser::name(&parser), "Telegram");
    }

    #[test]
    fn test_parser_platform() {
        let parser = TelegramParser::new();
        assert_eq!(parser.platform(), Platform::Telegram);
    }

    #[test]
    fn test_parser_default() {
        let parser = TelegramParser::default();
        assert!(!parser.config().streaming);
    }

    #[test]
    fn test_parser_with_config() {
        let config = TelegramConfig::new().with_streaming(true);
        let parser = TelegramParser::with_config(config);
        assert!(parser.config().streaming);
    }

    #[test]
    fn test_parser_with_streaming() {
        let parser = TelegramParser::with_streaming();
        assert!(parser.config().streaming);
    }

    #[test]
    fn test_parser_config_accessor() {
        let parser = TelegramParser::new();
        let config = parser.config();
        assert_eq!(config.buffer_size, 64 * 1024);
    }

    // =========================================================================
    // parse_str tests
    // =========================================================================

    #[test]
    fn test_parse_str_simple() {
        let parser = TelegramParser::new();
        let json = r#"{"messages": [{"id": 1, "type": "message", "date_unixtime": "1234567890", "from": "Alice", "text": "Hello"}]}"#;
        let messages = parser.parse_str(json).expect("parse failed");
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].sender, "Alice");
        assert_eq!(messages[0].content, "Hello");
    }

    #[test]
    fn test_parse_str_with_formatted_text() {
        let parser = TelegramParser::new();
        let json = r#"{"messages": [{"id": 1, "type": "message", "date_unixtime": "1234567890", "from": "Bob", "text": ["Hello ", {"type": "bold", "text": "world"}]}]}"#;
        let messages = parser.parse_str(json).expect("parse failed");
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].content, "Hello world");
    }

    #[test]
    fn test_parse_str_with_reply() {
        let parser = TelegramParser::new();
        let json = r#"{"messages": [{"id": 2, "type": "message", "date_unixtime": "1234567890", "from": "Alice", "text": "Reply", "reply_to_message_id": 1}]}"#;
        let messages = parser.parse_str(json).expect("parse failed");
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].reply_to, Some(1));
    }

    #[test]
    fn test_parse_str_with_edited() {
        let parser = TelegramParser::new();
        let json = r#"{"messages": [{"id": 1, "type": "message", "date_unixtime": "1234567890", "from": "Alice", "text": "Edited", "edited_unixtime": "1234567899"}]}"#;
        let messages = parser.parse_str(json).expect("parse failed");
        assert_eq!(messages.len(), 1);
        assert!(messages[0].edited.is_some());
    }

    #[test]
    fn test_parse_str_filters_service_messages() {
        let parser = TelegramParser::new();
        let json = r#"{"messages": [
            {"id": 1, "type": "message", "date_unixtime": "1234567890", "from": "Alice", "text": "Hello"},
            {"id": 2, "type": "service", "date_unixtime": "1234567890", "from": "System", "text": "joined"},
            {"id": 3, "type": "message", "date_unixtime": "1234567890", "from": "Bob", "text": "Hi"}
        ]}"#;
        let messages = parser.parse_str(json).expect("parse failed");
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].sender, "Alice");
        assert_eq!(messages[1].sender, "Bob");
    }

    #[test]
    fn test_parse_str_filters_empty_content() {
        let parser = TelegramParser::new();
        let json = r#"{"messages": [
            {"id": 1, "type": "message", "date_unixtime": "1234567890", "from": "Alice", "text": "Hello"},
            {"id": 2, "type": "message", "date_unixtime": "1234567890", "from": "Bob", "text": ""},
            {"id": 3, "type": "message", "date_unixtime": "1234567890", "from": "Charlie", "text": "Hi"}
        ]}"#;
        let messages = parser.parse_str(json).expect("parse failed");
        assert_eq!(messages.len(), 2);
    }

    #[test]
    fn test_parse_str_empty_messages() {
        let parser = TelegramParser::new();
        let json = r#"{"messages": []}"#;
        let messages = parser.parse_str(json).expect("parse failed");
        assert_eq!(messages, []);
    }

    #[test]
    fn test_parse_str_invalid_json() {
        let parser = TelegramParser::new();
        let result = parser.parse_str("invalid json");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_str_missing_messages() {
        let parser = TelegramParser::new();
        let result = parser.parse_str(r#"{"name": "Test"}"#);
        // Should fail because messages is missing
        assert!(result.is_err());
    }

    // =========================================================================
    // Streaming support tests
    // =========================================================================

    #[cfg(feature = "streaming")]
    #[test]
    fn test_supports_streaming_false_by_default() {
        let parser = TelegramParser::new();
        assert!(!parser.supports_streaming());
    }

    #[cfg(feature = "streaming")]
    #[test]
    fn test_supports_streaming_true_when_enabled() {
        let parser = TelegramParser::with_streaming();
        assert!(parser.supports_streaming());
    }

    #[cfg(feature = "streaming")]
    #[test]
    fn test_recommended_buffer_size() {
        let parser = TelegramParser::new();
        assert_eq!(parser.recommended_buffer_size(), 64 * 1024);

        let streaming_parser = TelegramParser::with_streaming();
        assert_eq!(streaming_parser.recommended_buffer_size(), 256 * 1024);
    }
}
