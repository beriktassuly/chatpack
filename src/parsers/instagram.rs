//! Instagram JSON export parser.
//!
//! Parses JSON exports from Meta's "Download Your Data" feature with
//! automatic Mojibake encoding fix.

use std::fs;
use std::path::Path;

use crate::Message;
use crate::config::InstagramConfig;
use crate::error::ChatpackError;
use crate::parser::{Parser, Platform};
use crate::parsing::instagram::{InstagramExport, parse_instagram_message_owned};

#[cfg(feature = "streaming")]
use crate::streaming::{InstagramStreamingParser, StreamingConfig, StreamingParser};

/// Parser for Instagram DM JSON exports.
///
/// Handles JSON files from Meta's "Download Your Data" feature (Settings >
/// Privacy > Download Your Information). Parses files from the `messages/`
/// directory.
///
/// # Mojibake Fix
///
/// Meta exports UTF-8 text encoded as ISO-8859-1, causing non-ASCII characters
/// (Cyrillic, emoji, etc.) to appear as garbage. This parser automatically
/// detects and fixes this encoding issue.
///
/// # JSON Structure
///
/// ```json
/// {
///   "participants": [{"name": "Alice"}, {"name": "Bob"}],
///   "messages": [
///     {
///       "sender_name": "Alice",
///       "timestamp_ms": 1234567890000,
///       "content": "Hello!"
///     }
///   ]
/// }
/// ```
///
/// # Examples
///
/// ```no_run
/// use chatpack::parsers::InstagramParser;
/// use chatpack::parser::Parser;
///
/// # fn main() -> chatpack::Result<()> {
/// let parser = InstagramParser::new();
/// let messages = parser.parse("message_1.json".as_ref())?;
///
/// // Non-ASCII text is automatically fixed
/// for msg in &messages {
///     println!("{}: {}", msg.sender, msg.content);
/// }
/// # Ok(())
/// # }
/// ```
pub struct InstagramParser {
    config: InstagramConfig,
}

impl InstagramParser {
    /// Creates a new parser with default configuration.
    pub fn new() -> Self {
        Self {
            config: InstagramConfig::default(),
        }
    }

    /// Creates a parser with custom configuration.
    pub fn with_config(config: InstagramConfig) -> Self {
        Self { config }
    }

    /// Creates a parser optimized for streaming large files.
    pub fn with_streaming() -> Self {
        Self {
            config: InstagramConfig::streaming(),
        }
    }

    /// Returns the current configuration.
    pub fn config(&self) -> &InstagramConfig {
        &self.config
    }

    /// Parses content from a string (internal implementation).
    fn parse_content(&self, content: &str) -> Result<Vec<Message>, ChatpackError> {
        let export: InstagramExport = serde_json::from_str(content)?;

        let fix = self.config.fix_encoding;
        // Use into_iter() with owned version to avoid allocations
        let mut messages: Vec<Message> = export
            .messages
            .into_iter()
            .filter_map(|msg| parse_instagram_message_owned(msg, fix))
            .collect();

        // Instagram stores messages newest-first, reverse for chronological order
        messages.reverse();

        Ok(messages)
    }
}

impl Default for InstagramParser {
    fn default() -> Self {
        Self::new()
    }
}

// Implement the new unified Parser trait
impl Parser for InstagramParser {
    fn name(&self) -> &'static str {
        "Instagram"
    }

    fn platform(&self) -> Platform {
        Platform::Instagram
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

            let streaming_parser = InstagramStreamingParser::with_config(streaming_config);
            let iterator =
                StreamingParser::stream(&streaming_parser, path.to_str().unwrap_or_default())?;

            // Instagram exports newest-first. The native streaming iterator yields file order,
            // so the high-level Parser API buffers here to match parse()/parse_str().
            let mut messages = iterator
                .map(|result| result.map_err(ChatpackError::from))
                .collect::<Result<Vec<_>, _>>()?;
            messages.reverse();

            Ok(Box::new(messages.into_iter().map(Ok)))
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
    // InstagramParser construction tests
    // =========================================================================

    #[test]
    fn test_parser_new() {
        let parser = InstagramParser::new();
        assert!(!parser.config().streaming);
        assert!(parser.config().fix_encoding);
    }

    #[test]
    fn test_parser_default() {
        let parser = InstagramParser::default();
        assert!(!parser.config().streaming);
        assert!(parser.config().fix_encoding);
    }

    #[test]
    fn test_parser_with_config() {
        let config = InstagramConfig::new()
            .with_streaming(true)
            .with_fix_encoding(false);
        let parser = InstagramParser::with_config(config);
        assert!(parser.config().streaming);
        assert!(!parser.config().fix_encoding);
    }

    #[test]
    fn test_parser_with_streaming() {
        let parser = InstagramParser::with_streaming();
        assert!(parser.config().streaming);
    }

    #[test]
    fn test_parser_name() {
        let parser = InstagramParser::new();
        assert_eq!(Parser::name(&parser), "Instagram");
    }

    #[test]
    fn test_parser_platform() {
        let parser = InstagramParser::new();
        assert_eq!(parser.platform(), Platform::Instagram);
    }

    // =========================================================================
    // parse_str tests
    // =========================================================================

    #[test]
    fn test_parse_str_simple() {
        let parser = InstagramParser::new();
        let json = r#"{"messages": [{"sender_name": "Alice", "content": "Hello", "timestamp_ms": 1234567890000}]}"#;
        let messages = parser.parse_str(json).expect("parse failed");
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].sender, "Alice");
        assert_eq!(messages[0].content, "Hello");
    }

    #[test]
    fn test_parse_str_filters_empty_content() {
        let parser = InstagramParser::new();
        let json = r#"{"messages": [
            {"sender_name": "Alice", "content": "Hello", "timestamp_ms": 1234567890000},
            {"sender_name": "Bob", "content": "", "timestamp_ms": 1234567891000},
            {"sender_name": "Charlie", "content": "Hi", "timestamp_ms": 1234567892000}
        ]}"#;
        let messages = parser.parse_str(json).expect("parse failed");
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].sender, "Charlie"); // Reversed order
        assert_eq!(messages[1].sender, "Alice");
    }

    #[test]
    fn test_parse_str_reverses_order() {
        let parser = InstagramParser::new();
        let json = r#"{"messages": [
            {"sender_name": "First", "content": "1", "timestamp_ms": 1234567890000},
            {"sender_name": "Second", "content": "2", "timestamp_ms": 1234567891000},
            {"sender_name": "Third", "content": "3", "timestamp_ms": 1234567892000}
        ]}"#;
        let messages = parser.parse_str(json).expect("parse failed");
        assert_eq!(messages.len(), 3);
        // Instagram stores newest first, so should be reversed
        assert_eq!(messages[0].sender, "Third");
        assert_eq!(messages[1].sender, "Second");
        assert_eq!(messages[2].sender, "First");
    }

    #[test]
    fn test_parse_str_with_shared_link() {
        let parser = InstagramParser::new();
        // When content is present, it's used (share link is separate metadata)
        let json = r#"{"messages": [{"sender_name": "Alice", "content": "Check this", "share": {"link": "https://example.com"}, "timestamp_ms": 1234567890000}]}"#;
        let messages = parser.parse_str(json).expect("parse failed");
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].content, "Check this");
    }

    #[test]
    fn test_parse_str_with_share_text_only() {
        let parser = InstagramParser::new();
        // When no content but share_text exists, use share_text
        let json = r#"{"messages": [{"sender_name": "Alice", "share": {"share_text": "Shared content", "link": "https://example.com"}, "timestamp_ms": 1234567890000}]}"#;
        let messages = parser.parse_str(json).expect("parse failed");
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].content, "Shared content");
    }

    #[test]
    fn test_parse_str_empty_messages() {
        let parser = InstagramParser::new();
        let json = r#"{"messages": []}"#;
        let messages = parser.parse_str(json).expect("parse failed");
        assert_eq!(messages, []);
    }

    #[test]
    fn test_parse_str_invalid_json() {
        let parser = InstagramParser::new();
        let result = parser.parse_str("not json");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_str_missing_messages() {
        let parser = InstagramParser::new();
        let result = parser.parse_str(r#"{"participants": []}"#);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_str_timestamp_parsing() {
        let parser = InstagramParser::new();
        let json = r#"{"messages": [{"sender_name": "Alice", "content": "Hello", "timestamp_ms": 1609459200000}]}"#;
        let messages = parser.parse_str(json).expect("parse failed");
        assert!(messages[0].timestamp.is_some());
    }

    // =========================================================================
    // Encoding fix tests
    // =========================================================================

    #[test]
    fn test_parse_str_without_fix_encoding() {
        let config = InstagramConfig::new().with_fix_encoding(false);
        let parser = InstagramParser::with_config(config);
        let json = r#"{"messages": [{"sender_name": "Test", "content": "Hello", "timestamp_ms": 1234567890000}]}"#;
        let messages = parser.parse_str(json).expect("parse failed");
        assert_eq!(messages.len(), 1);
    }

    // =========================================================================
    // Streaming support tests
    // =========================================================================

    #[cfg(feature = "streaming")]
    #[test]
    fn test_supports_streaming_false_by_default() {
        let parser = InstagramParser::new();
        assert!(!parser.supports_streaming());
    }

    #[cfg(feature = "streaming")]
    #[test]
    fn test_supports_streaming_true_when_enabled() {
        let parser = InstagramParser::with_streaming();
        assert!(parser.supports_streaming());
    }

    #[cfg(feature = "streaming")]
    #[test]
    fn test_recommended_buffer_size() {
        let parser = InstagramParser::new();
        assert_eq!(parser.recommended_buffer_size(), 64 * 1024);

        let streaming_parser = InstagramParser::with_streaming();
        assert_eq!(streaming_parser.recommended_buffer_size(), 256 * 1024);
    }

    #[cfg(feature = "streaming")]
    #[test]
    fn test_stream_matches_parse_order() {
        use std::io::Write;

        let json = r#"{"messages": [
            {"sender_name": "First", "content": "1", "timestamp_ms": 1234567890000},
            {"sender_name": "Second", "content": "2", "timestamp_ms": 1234567891000},
            {"sender_name": "Third", "content": "3", "timestamp_ms": 1234567892000}
        ]}"#;
        let mut file = tempfile::NamedTempFile::new().expect("create temp file");
        file.write_all(json.as_bytes()).expect("write fixture");

        let parser = InstagramParser::with_streaming();
        let streamed = parser
            .stream(file.path())
            .expect("stream fixture")
            .collect::<Result<Vec<_>, _>>()
            .expect("collect streamed messages");
        let parsed = parser.parse_str(json).expect("parse fixture");

        assert_eq!(streamed, parsed);
        assert_eq!(streamed[0].sender, "Third");
    }
}
