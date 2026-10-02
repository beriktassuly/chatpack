//! Async Telegram parser.

use std::path::Path;

use async_trait::async_trait;

use crate::Message;
use crate::config::TelegramConfig;
use crate::error::ChatpackError;
use crate::parsing::telegram::{TelegramExport, parse_telegram_export};

use super::{AsyncParser, read_file_async};

/// Async parser for Telegram JSON exports.
///
/// # Example
///
/// ```rust,no_run
/// use chatpack::async_parser::{AsyncParser, AsyncTelegramParser};
///
/// # async fn example() -> Result<(), chatpack::ChatpackError> {
/// let parser = AsyncTelegramParser::new();
/// let messages = parser.parse("telegram_export.json").await?;
///
/// for msg in messages {
///     println!("{}: {}", msg.sender, msg.content);
/// }
/// # Ok(())
/// # }
/// ```
pub struct AsyncTelegramParser {
    #[allow(dead_code)]
    config: TelegramConfig,
}

impl AsyncTelegramParser {
    /// Creates a new async parser with default configuration.
    pub fn new() -> Self {
        Self {
            config: TelegramConfig::default(),
        }
    }

    /// Creates a parser with custom configuration.
    pub fn with_config(config: TelegramConfig) -> Self {
        Self { config }
    }
}

impl Default for AsyncTelegramParser {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AsyncParser for AsyncTelegramParser {
    fn name(&self) -> &'static str {
        "Telegram (Async)"
    }

    async fn parse(&self, path: impl AsRef<Path> + Send) -> Result<Vec<Message>, ChatpackError> {
        let content = read_file_async(path).await?;
        self.parse_str(&content)
    }

    fn parse_str(&self, content: &str) -> Result<Vec<Message>, ChatpackError> {
        let export: TelegramExport = serde_json::from_str(content)?;

        Ok(parse_telegram_export(&export))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // AsyncTelegramParser construction tests
    // =========================================================================

    #[test]
    fn test_parser_new() {
        let parser = AsyncTelegramParser::new();
        assert_eq!(parser.name(), "Telegram (Async)");
    }

    #[test]
    fn test_parser_default() {
        let parser = AsyncTelegramParser::default();
        assert_eq!(parser.name(), "Telegram (Async)");
    }

    #[test]
    fn test_parser_with_config() {
        let config = TelegramConfig::new().with_streaming(true);
        let parser = AsyncTelegramParser::with_config(config);
        assert_eq!(parser.name(), "Telegram (Async)");
    }

    // =========================================================================
    // Async parse tests
    // =========================================================================

    #[tokio::test]
    async fn test_async_parser_name() {
        let parser = AsyncTelegramParser::new();
        assert_eq!(parser.name(), "Telegram (Async)");
    }

    #[tokio::test]
    async fn test_async_parse_file() {
        use tokio::io::AsyncWriteExt;
        let dir = tempfile::tempdir().expect("create temp dir");
        let file_path = dir.path().join("test.json");

        let json = r#"{
            "messages": [
                {
                    "id": 1,
                    "type": "message",
                    "date_unixtime": "1705314600",
                    "from": "Alice",
                    "text": "Hello async!"
                }
            ]
        }"#;

        let mut file = tokio::fs::File::create(&file_path)
            .await
            .expect("create file");
        file.write_all(json.as_bytes()).await.expect("write");
        file.flush().await.expect("flush");

        let parser = AsyncTelegramParser::new();
        let messages = parser.parse(&file_path).await.expect("parse failed");

        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].sender, "Alice");
        assert_eq!(messages[0].content, "Hello async!");
    }

    #[tokio::test]
    async fn test_async_parse_file_not_found() {
        let parser = AsyncTelegramParser::new();
        let result = parser.parse("/nonexistent/file.json").await;
        assert!(result.is_err());
    }

    // =========================================================================
    // parse_str tests
    // =========================================================================

    #[test]
    fn test_parse_str() {
        let json = r#"{
            "messages": [
                {
                    "id": 1,
                    "type": "message",
                    "date_unixtime": "1705314600",
                    "from": "Alice",
                    "text": "Hello!"
                }
            ]
        }"#;

        let parser = AsyncTelegramParser::new();
        let messages = parser.parse_str(json).unwrap();

        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].sender, "Alice");
        assert_eq!(messages[0].content, "Hello!");
    }

    #[test]
    fn test_parse_str_multiple_messages() {
        let json = r#"{
            "messages": [
                {"id": 1, "type": "message", "date_unixtime": "1705314600", "from": "Alice", "text": "Hello!"},
                {"id": 2, "type": "message", "date_unixtime": "1705314601", "from": "Bob", "text": "Hi!"}
            ]
        }"#;

        let parser = AsyncTelegramParser::new();
        let messages = parser.parse_str(json).unwrap();

        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].sender, "Alice");
        assert_eq!(messages[1].sender, "Bob");
    }

    #[test]
    fn test_parse_str_invalid_json() {
        let parser = AsyncTelegramParser::new();
        let result = parser.parse_str("invalid json");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_str_empty_messages() {
        let json = r#"{"messages": []}"#;
        let parser = AsyncTelegramParser::new();
        let messages = parser.parse_str(json).unwrap();
        assert_eq!(messages, []);
    }

    #[test]
    fn test_parse_str_with_formatted_text() {
        let json = r#"{
            "messages": [
                {
                    "id": 1,
                    "type": "message",
                    "date_unixtime": "1705314600",
                    "from": "Alice",
                    "text": ["Hello ", {"type": "bold", "text": "world"}]
                }
            ]
        }"#;

        let parser = AsyncTelegramParser::new();
        let messages = parser.parse_str(json).unwrap();

        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].content, "Hello world");
    }

    #[test]
    fn test_parse_str_filters_service_messages() {
        let json = r#"{
            "messages": [
                {"id": 1, "type": "message", "date_unixtime": "1705314600", "from": "Alice", "text": "Hello!"},
                {"id": 2, "type": "service", "date_unixtime": "1705314601", "from": "System", "text": "joined"},
                {"id": 3, "type": "message", "date_unixtime": "1705314602", "from": "Bob", "text": "Hi!"}
            ]
        }"#;

        let parser = AsyncTelegramParser::new();
        let messages = parser.parse_str(json).unwrap();

        assert_eq!(messages.len(), 2);
    }
}
