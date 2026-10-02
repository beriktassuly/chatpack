//! Streaming parser for Telegram JSON exports.
//!
//! Telegram exports are structured as:
//! ```json
//! {
//!   "name": "Chat Name",
//!   "type": "personal_chat",
//!   "messages": [
//!     {"id": 1, "type": "message", ...},
//!     {"id": 2, "type": "message", ...}
//!   ]
//! }
//! ```
//!
//! This parser streams the messages array without loading the entire file.

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use crate::Message;
use crate::error::ChatpackError;
use crate::parsing::telegram::{TelegramRawMessage, parse_telegram_message};

#[cfg(test)]
use super::StreamingError;
use super::json_array::JsonArrayObjectReader;
use super::{MessageIterator, StreamingConfig, StreamingParser, StreamingResult};

/// Streaming parser for Telegram JSON exports.
///
/// This parser reads the file sequentially, parsing one message at a time.
/// Memory usage is O(1) relative to file size.
///
/// # Example
///
/// ```rust,no_run
/// use chatpack::streaming::TelegramStreamingParser;
/// use chatpack::streaming::StreamingParser;
///
/// let parser = TelegramStreamingParser::new();
///
/// for result in parser.stream("large_telegram_export.json").unwrap() {
///     if let Ok(msg) = result {
///         println!("{}: {}", msg.sender, msg.content);
///     }
/// }
/// ```
pub struct TelegramStreamingParser {
    config: StreamingConfig,
}

impl TelegramStreamingParser {
    /// Creates a new streaming parser with default configuration.
    pub fn new() -> Self {
        Self {
            config: StreamingConfig::default(),
        }
    }

    /// Creates a new streaming parser with custom configuration.
    pub fn with_config(config: StreamingConfig) -> Self {
        Self { config }
    }
}

impl Default for TelegramStreamingParser {
    fn default() -> Self {
        Self::new()
    }
}

impl StreamingParser for TelegramStreamingParser {
    fn name(&self) -> &'static str {
        "Telegram (Streaming)"
    }

    fn stream(&self, file_path: &str) -> Result<Box<dyn MessageIterator>, ChatpackError> {
        let path = Path::new(file_path);
        let file = File::open(path)?;
        let file_size = file.metadata()?.len();

        let reader = BufReader::with_capacity(self.config.buffer_size, file);
        let iterator = TelegramMessageIterator::new(reader, file_size, self.config)?;

        Ok(Box::new(iterator))
    }

    fn recommended_buffer_size(&self) -> usize {
        self.config.buffer_size
    }
}

/// Iterator over Telegram messages.
pub struct TelegramMessageIterator<R: BufRead> {
    objects: JsonArrayObjectReader<R>,
    file_size: u64,
    config: StreamingConfig,
}

impl<R: BufRead> TelegramMessageIterator<R> {
    /// Creates a new iterator, seeking to the messages array.
    fn new(reader: R, file_size: u64, config: StreamingConfig) -> StreamingResult<Self> {
        Ok(Self {
            objects: JsonArrayObjectReader::new(
                reader,
                "messages",
                config.buffer_size,
                config.max_message_size,
            )?,
            file_size,
            config,
        })
    }

    /// Parses a JSON string into a Message using shared parsing logic.
    fn parse_message_from_json(json_str: &str) -> StreamingResult<Option<Message>> {
        let msg: TelegramRawMessage = serde_json::from_str(json_str)?;
        Ok(parse_telegram_message(&msg))
    }
}

impl<R: BufRead + Send> MessageIterator for TelegramMessageIterator<R> {
    fn progress(&self) -> Option<f64> {
        if self.file_size == 0 {
            return None;
        }
        Some((self.objects.bytes_read() as f64 / self.file_size as f64) * 100.0)
    }

    fn bytes_processed(&self) -> u64 {
        self.objects.bytes_read()
    }

    fn total_bytes(&self) -> Option<u64> {
        Some(self.file_size)
    }
}

impl<R: BufRead + Send> Iterator for TelegramMessageIterator<R> {
    type Item = StreamingResult<Message>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.objects.next_object() {
                Ok(Some(json_str)) => {
                    match Self::parse_message_from_json(&json_str) {
                        Ok(Some(msg)) => return Some(Ok(msg)),
                        Ok(None) => {} // Skip non-messages, try next
                        Err(_) if self.config.skip_invalid => {} // Skip invalid
                        Err(e) => return Some(Err(e)),
                    }
                }
                Ok(None) => return None, // End of array
                Err(_) if self.config.skip_invalid => {}
                Err(e) => return Some(Err(e)),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parsing::telegram::extract_telegram_text;
    use std::io::Cursor;

    fn create_test_json() -> String {
        r#"{
  "name": "Test Chat",
  "type": "personal_chat",
  "messages": [
    {"id": 1, "type": "message", "date_unixtime": "1705314600", "from": "Alice", "text": "Hello!"},
    {"id": 2, "type": "message", "date_unixtime": "1705314660", "from": "Bob", "text": "Hi!"},
    {"id": 3, "type": "service", "action": "pin_message"},
    {"id": 4, "type": "message", "date_unixtime": "1705314720", "from": "Alice", "text": "Bye!"}
  ]
}"#
        .to_string()
    }

    // =========================================================================
    // Constructor tests
    // =========================================================================

    #[test]
    fn test_parser_new() {
        let parser = TelegramStreamingParser::new();
        assert_eq!(parser.name(), "Telegram (Streaming)");
    }

    #[test]
    fn test_parser_default() {
        let parser = TelegramStreamingParser::default();
        assert_eq!(parser.name(), "Telegram (Streaming)");
    }

    #[test]
    fn test_parser_with_config() {
        let config = StreamingConfig::default()
            .with_buffer_size(512 * 1024)
            .with_max_message_size(2 * 1024 * 1024)
            .with_skip_invalid(true);
        let parser = TelegramStreamingParser::with_config(config);
        assert_eq!(parser.name(), "Telegram (Streaming)");
    }

    #[test]
    fn test_recommended_buffer_size() {
        let parser = TelegramStreamingParser::new();
        assert!(parser.recommended_buffer_size() > 0);
    }

    // =========================================================================
    // Basic parsing tests
    // =========================================================================

    #[test]
    fn test_streaming_parser_basic() {
        let json = create_test_json();
        let cursor = Cursor::new(json.as_bytes().to_vec());
        let reader = BufReader::new(cursor);

        let mut iterator =
            TelegramMessageIterator::new(reader, json.len() as u64, StreamingConfig::default())
                .unwrap();

        let messages: Vec<_> = iterator.by_ref().filter_map(Result::ok).collect();

        assert_eq!(messages.len(), 3); // Service message skipped
        assert_eq!(messages[0].sender, "Alice");
        assert_eq!(messages[0].content, "Hello!");
        assert_eq!(messages[1].sender, "Bob");
        assert_eq!(messages[2].content, "Bye!");
    }

    #[test]
    fn test_empty_messages_array() {
        let json = r#"{"name": "Chat", "messages": []}"#;
        let cursor = Cursor::new(json.as_bytes().to_vec());
        let reader = BufReader::new(cursor);

        let mut iterator =
            TelegramMessageIterator::new(reader, json.len() as u64, StreamingConfig::default())
                .unwrap();

        let messages: Vec<_> = iterator.by_ref().filter_map(Result::ok).collect();
        assert_eq!(messages, []);
    }

    #[test]
    fn test_braces_inside_json_strings() {
        let json = r#"{"name":"Chat","messages":[{"id":1,"type":"message","date_unixtime":"1705314600","from":"Alice","text":"has { brace and escaped \"quote\""},{"id":2,"type":"message","date_unixtime":"1705314660","from":"Bob","text":"has } brace"}]}"#;
        let cursor = Cursor::new(json.as_bytes().to_vec());
        let reader = BufReader::new(cursor);

        let mut iterator =
            TelegramMessageIterator::new(reader, json.len() as u64, StreamingConfig::default())
                .unwrap();

        let messages: Vec<_> = iterator.by_ref().filter_map(Result::ok).collect();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].content, "has { brace and escaped \"quote\"");
        assert_eq!(messages[1].content, "has } brace");
    }

    // =========================================================================
    // Progress and iterator trait tests
    // =========================================================================

    #[test]
    fn test_progress_reporting() {
        let json = create_test_json();
        let cursor = Cursor::new(json.as_bytes().to_vec());
        let reader = BufReader::new(cursor);

        let mut iterator =
            TelegramMessageIterator::new(reader, json.len() as u64, StreamingConfig::default())
                .unwrap();

        // Consume all messages
        let _: Vec<_> = iterator.by_ref().collect();

        let progress = iterator.progress().unwrap();
        assert!(progress > 90.0); // Should be close to 100%
    }

    #[test]
    fn test_progress_with_zero_file_size() {
        let json = create_test_json();
        let cursor = Cursor::new(json.as_bytes().to_vec());
        let reader = BufReader::new(cursor);

        let iterator = TelegramMessageIterator::new(reader, 0, StreamingConfig::default()).unwrap();

        assert!(iterator.progress().is_none());
    }

    #[test]
    fn test_bytes_processed() {
        let json = create_test_json();
        let cursor = Cursor::new(json.as_bytes().to_vec());
        let reader = BufReader::new(cursor);

        let mut iterator =
            TelegramMessageIterator::new(reader, json.len() as u64, StreamingConfig::default())
                .unwrap();

        let initial_bytes = iterator.bytes_processed();
        assert!(initial_bytes > 0);

        let _ = iterator.next();
        let bytes_after = iterator.bytes_processed();
        assert!(bytes_after > initial_bytes);
    }

    #[test]
    fn test_total_bytes() {
        let json = create_test_json();
        let file_size = json.len() as u64;
        let cursor = Cursor::new(json.as_bytes().to_vec());
        let reader = BufReader::new(cursor);

        let iterator =
            TelegramMessageIterator::new(reader, file_size, StreamingConfig::default()).unwrap();

        assert_eq!(iterator.total_bytes(), Some(file_size));
    }

    // =========================================================================
    // Error handling tests
    // =========================================================================

    #[test]
    fn test_no_messages_array() {
        let json = r#"{"name": "Chat"}"#;
        let cursor = Cursor::new(json.as_bytes().to_vec());
        let reader = BufReader::new(cursor);

        let result =
            TelegramMessageIterator::new(reader, json.len() as u64, StreamingConfig::default());

        assert!(result.is_err());
        // Verify it's an InvalidFormat error
        if let Err(StreamingError::InvalidFormat(msg)) = result {
            assert!(msg.contains("messages"));
        } else {
            panic!("Expected InvalidFormat error");
        }
    }

    #[test]
    fn test_skip_invalid_messages() {
        let json = r#"{
  "name": "Chat",
  "messages": [
    {"invalid": "object"},
    {"id": 1, "type": "message", "date_unixtime": "1705314600", "from": "Alice", "text": "Valid!"}
  ]
}"#;
        let cursor = Cursor::new(json.as_bytes().to_vec());
        let reader = BufReader::new(cursor);

        let config = StreamingConfig::default().with_skip_invalid(true);
        let mut iterator = TelegramMessageIterator::new(reader, json.len() as u64, config).unwrap();

        let messages: Vec<_> = iterator.by_ref().filter_map(Result::ok).collect();

        // Should skip invalid and return valid message
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].content, "Valid!");
    }

    #[test]
    fn test_invalid_message_without_skip() {
        let json = r#"{
  "name": "Chat",
  "messages": [
    {"id": "invalid_id_type"}
  ]
}"#;
        let cursor = Cursor::new(json.as_bytes().to_vec());
        let reader = BufReader::new(cursor);

        let config = StreamingConfig::default().with_skip_invalid(false);
        let mut iterator = TelegramMessageIterator::new(reader, json.len() as u64, config).unwrap();

        // First message should be an error
        let first = iterator.next();
        assert!(first.is_some());
        assert!(first.unwrap().is_err());
    }

    // =========================================================================
    // Content type tests
    // =========================================================================

    #[test]
    fn test_extract_text_with_formatting() {
        let value = serde_json::json!([
            "Hello, ",
            {"type": "bold", "text": "world"},
            "!"
        ]);
        assert_eq!(extract_telegram_text(&value), "Hello, world!");
    }

    #[test]
    fn test_parser_name() {
        let parser = TelegramStreamingParser::new();
        assert_eq!(parser.name(), "Telegram (Streaming)");
    }

    #[test]
    fn test_multiline_message() {
        let json = r#"{
  "name": "Chat",
  "messages": [
    {
      "id": 1,
      "type": "message",
      "date_unixtime": "1705314600",
      "from": "Alice",
      "text": "Line1\nLine2\nLine3"
    }
  ]
}"#;
        let cursor = Cursor::new(json.as_bytes().to_vec());
        let reader = BufReader::new(cursor);

        let mut iterator =
            TelegramMessageIterator::new(reader, json.len() as u64, StreamingConfig::default())
                .unwrap();

        let messages: Vec<_> = iterator.by_ref().filter_map(Result::ok).collect();
        assert_eq!(messages.len(), 1);
    }

    #[test]
    fn test_iterator_finished_returns_none() {
        let json = r#"{"name": "Chat", "messages": [{"id": 1, "type": "message", "date_unixtime": "1000", "from": "A", "text": "Hi"}]}"#;
        let cursor = Cursor::new(json.as_bytes().to_vec());
        let reader = BufReader::new(cursor);

        let mut iterator =
            TelegramMessageIterator::new(reader, json.len() as u64, StreamingConfig::default())
                .unwrap();

        // Consume all messages
        let _: Vec<_> = iterator.by_ref().collect();

        // Additional calls should return None
        assert!(iterator.next().is_none());
        assert!(iterator.next().is_none());
    }

    #[test]
    fn test_service_messages_skipped() {
        let json = r#"{
  "name": "Chat",
  "messages": [
    {"id": 1, "type": "service", "action": "pin_message"},
    {"id": 2, "type": "service", "action": "create_group"}
  ]
}"#;
        let cursor = Cursor::new(json.as_bytes().to_vec());
        let reader = BufReader::new(cursor);

        let mut iterator =
            TelegramMessageIterator::new(reader, json.len() as u64, StreamingConfig::default())
                .unwrap();

        let messages: Vec<_> = iterator.by_ref().filter_map(Result::ok).collect();
        assert_eq!(messages, []);
    }

    #[test]
    fn test_formatted_text_array() {
        let json = r#"{
  "name": "Chat",
  "messages": [
    {"id": 1, "type": "message", "date_unixtime": "1705314600", "from": "Alice", "text": ["Hello ", {"type": "bold", "text": "World"}, "!"]}
  ]
}"#;
        let cursor = Cursor::new(json.as_bytes().to_vec());
        let reader = BufReader::new(cursor);

        let mut iterator =
            TelegramMessageIterator::new(reader, json.len() as u64, StreamingConfig::default())
                .unwrap();

        let messages: Vec<_> = iterator.by_ref().filter_map(Result::ok).collect();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].content, "Hello World!");
    }
}
