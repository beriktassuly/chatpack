//! Unified error types for chatpack.
//!
//! This module provides a single [`ChatpackError`] enum that covers all error
//! cases in the library. This design follows the pattern used by popular crates
//! like `reqwest`, `serde_json`, and `csv`.
//!
//! # Error Handling Philosophy
//!
//! - **Library users** get typed errors they can match on
//! - **Application users** get clear, actionable error messages
//! - **Developers** get source error chains for debugging

use std::io;
use std::path::PathBuf;

use thiserror::Error;

/// A specialized [`Result`] type for chatpack operations.
///
/// This type is broadly used across the library for any operation that
/// may produce an error.
///
/// # Example
///
/// ```rust
/// use chatpack::error::Result;
/// use chatpack::Message;
///
/// fn my_function() -> Result<Vec<Message>> {
///     // ... operations that may fail
///     Ok(vec![])
/// }
/// ```
pub type Result<T> = std::result::Result<T, ChatpackError>;

/// The error type for all chatpack operations.
///
/// This enum represents all possible errors that can occur when using chatpack.
/// Each variant contains context about what went wrong and, where applicable,
/// the underlying source error.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ChatpackError {
    /// An I/O error occurred.
    ///
    /// This typically happens when:
    /// - The input file doesn't exist
    /// - Permission denied
    /// - Disk is full (when writing output)
    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    /// Failed to parse the input file.
    ///
    /// Contains the format being parsed, the underlying parse error,
    /// and optionally the file path.
    #[error("Failed to parse {format} export{}: {source}", path.as_ref().map(|p| format!(" (file: {})", p.display())).unwrap_or_default())]
    Parse {
        /// The format being parsed (e.g., "Telegram JSON", "WhatsApp TXT")
        format: &'static str,
        /// The underlying parse error
        #[source]
        source: ParseErrorKind,
        /// The file path, if available
        path: Option<PathBuf>,
    },

    /// The file format doesn't match the expected structure.
    ///
    /// This occurs when:
    /// - Telegram JSON is missing the "messages" array
    /// - WhatsApp TXT doesn't match any known date format
    /// - Discord export is in an unrecognized format
    #[error("Invalid {format} format: {message}")]
    InvalidFormat {
        /// The format that was expected
        format: &'static str,
        /// Description of what's wrong
        message: String,
    },

    /// Invalid date format in filter configuration.
    ///
    /// Date filters expect YYYY-MM-DD format.
    #[error("Invalid date '{input}'. Expected format: {expected}")]
    InvalidDate {
        /// The invalid date string that was provided
        input: String,
        /// Expected format description
        expected: &'static str,
    },

    /// CSV writing error.
    ///
    /// This can occur when writing output to CSV format.
    #[cfg(any(feature = "csv-output", feature = "discord"))]
    #[error("CSV error: {0}")]
    Csv(#[from] csv::Error),

    /// JSON parsing/serialization error.
    ///
    /// This can occur when parsing or writing JSON.
    #[cfg(any(
        feature = "telegram",
        feature = "instagram",
        feature = "discord",
        feature = "json-output"
    ))]
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// Streaming parser error.
    ///
    /// Errors specific to streaming parsers for large files.
    #[error("Streaming error: {0}")]
    Streaming(#[source] StreamingErrorKind),

    /// UTF-8 encoding error.
    ///
    /// Occurs when file content is not valid UTF-8.
    #[error("UTF-8 encoding error in {context}: {source}")]
    Utf8 {
        /// Description of where the error occurred
        context: String,
        /// The underlying UTF-8 error
        #[source]
        source: std::string::FromUtf8Error,
    },

    /// Buffer overflow in streaming parser.
    ///
    /// A single message exceeded the maximum allowed size.
    #[error("Message too large: {actual_size} bytes (maximum: {max_size} bytes)")]
    BufferOverflow {
        /// Maximum allowed size in bytes
        max_size: usize,
        /// Actual size encountered
        actual_size: usize,
    },

    /// Unexpected end of file.
    ///
    /// The file ended before parsing was complete.
    #[error("Unexpected end of file while {context}")]
    UnexpectedEof {
        /// Context about what was being parsed
        context: String,
    },
}

/// Kinds of parse errors that can occur.
#[derive(Debug, Error)]
pub enum ParseErrorKind {
    /// JSON parsing error
    #[cfg(any(
        feature = "telegram",
        feature = "instagram",
        feature = "discord",
        feature = "json-output"
    ))]
    #[error("{0}")]
    Json(#[from] serde_json::Error),
    /// Regex/pattern matching error
    #[error("{0}")]
    Pattern(String),
    /// Generic parsing error
    #[error("{0}")]
    Other(String),
}

/// Kinds of streaming errors.
#[derive(Debug, Error)]
pub enum StreamingErrorKind {
    /// IO error during streaming
    #[error("IO error: {0}")]
    Io(#[from] io::Error),
    /// JSON parsing error during streaming
    #[cfg(any(
        feature = "telegram",
        feature = "instagram",
        feature = "discord",
        feature = "json-output"
    ))]
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    /// Invalid format encountered
    #[error("Invalid format: {0}")]
    InvalidFormat(String),
    /// Buffer overflow
    #[error("Buffer overflow: {actual_size} bytes (max: {max_size})")]
    BufferOverflow { max_size: usize, actual_size: usize },
    /// Unexpected EOF
    #[error("Unexpected end of file")]
    UnexpectedEof,
}

impl From<std::string::FromUtf8Error> for ChatpackError {
    fn from(err: std::string::FromUtf8Error) -> Self {
        ChatpackError::Utf8 {
            context: "output conversion".to_string(),
            source: err,
        }
    }
}

// ============================================================================
// Convenience constructors
// ============================================================================

impl ChatpackError {
    /// Creates a parse error for Telegram format.
    #[cfg(feature = "telegram")]
    pub fn telegram_parse(source: serde_json::Error, path: Option<PathBuf>) -> Self {
        ChatpackError::Parse {
            format: "Telegram JSON",
            source: ParseErrorKind::Json(source),
            path,
        }
    }

    /// Creates a parse error for WhatsApp format.
    pub fn whatsapp_parse(message: impl Into<String>, path: Option<PathBuf>) -> Self {
        ChatpackError::Parse {
            format: "WhatsApp TXT",
            source: ParseErrorKind::Pattern(message.into()),
            path,
        }
    }

    /// Creates a parse error for Instagram format.
    #[cfg(feature = "instagram")]
    pub fn instagram_parse(source: serde_json::Error, path: Option<PathBuf>) -> Self {
        ChatpackError::Parse {
            format: "Instagram JSON",
            source: ParseErrorKind::Json(source),
            path,
        }
    }

    /// Creates a parse error for Discord format.
    #[cfg(feature = "discord")]
    pub fn discord_parse(source: serde_json::Error, path: Option<PathBuf>) -> Self {
        ChatpackError::Parse {
            format: "Discord",
            source: ParseErrorKind::Json(source),
            path,
        }
    }

    /// Creates an invalid format error.
    pub fn invalid_format(format: &'static str, message: impl Into<String>) -> Self {
        ChatpackError::InvalidFormat {
            format,
            message: message.into(),
        }
    }

    /// Creates an invalid date error.
    pub fn invalid_date(input: impl Into<String>) -> Self {
        ChatpackError::InvalidDate {
            input: input.into(),
            expected: "YYYY-MM-DD",
        }
    }

    /// Creates a streaming error from components.
    pub fn streaming(kind: StreamingErrorKind) -> Self {
        ChatpackError::Streaming(kind)
    }

    /// Creates a buffer overflow error.
    pub fn buffer_overflow(max_size: usize, actual_size: usize) -> Self {
        ChatpackError::BufferOverflow {
            max_size,
            actual_size,
        }
    }

    /// Creates an unexpected EOF error.
    pub fn unexpected_eof(context: impl Into<String>) -> Self {
        ChatpackError::UnexpectedEof {
            context: context.into(),
        }
    }

    /// Returns `true` if this is an IO error.
    pub fn is_io(&self) -> bool {
        matches!(self, ChatpackError::Io(_))
    }

    /// Returns `true` if this is a parse error.
    pub fn is_parse(&self) -> bool {
        matches!(self, ChatpackError::Parse { .. })
    }

    /// Returns `true` if this is an invalid format error.
    pub fn is_invalid_format(&self) -> bool {
        matches!(self, ChatpackError::InvalidFormat { .. })
    }

    /// Returns `true` if this is a date-related error.
    pub fn is_invalid_date(&self) -> bool {
        matches!(self, ChatpackError::InvalidDate { .. })
    }
}

// ============================================================================
// Integration with streaming module
// ============================================================================

#[cfg(all(
    feature = "streaming",
    any(
        feature = "telegram",
        feature = "whatsapp",
        feature = "instagram",
        feature = "discord"
    )
))]
impl From<crate::streaming::StreamingError> for ChatpackError {
    #[allow(unreachable_patterns)]
    fn from(err: crate::streaming::StreamingError) -> Self {
        match err {
            crate::streaming::StreamingError::Io(e) => {
                ChatpackError::Streaming(StreamingErrorKind::Io(e))
            }
            #[cfg(any(feature = "telegram", feature = "instagram", feature = "discord"))]
            crate::streaming::StreamingError::Json(e) => {
                ChatpackError::Streaming(StreamingErrorKind::Json(e))
            }
            crate::streaming::StreamingError::InvalidFormat(s) => {
                ChatpackError::Streaming(StreamingErrorKind::InvalidFormat(s))
            }
            crate::streaming::StreamingError::BufferOverflow {
                max_size,
                actual_size,
            } => ChatpackError::Streaming(StreamingErrorKind::BufferOverflow {
                max_size,
                actual_size,
            }),
            crate::streaming::StreamingError::UnexpectedEof => {
                ChatpackError::Streaming(StreamingErrorKind::UnexpectedEof)
            }
            // Catch-all for when Json variant is not available
            _ => ChatpackError::Streaming(StreamingErrorKind::InvalidFormat(
                "Unknown streaming error".to_string(),
            )),
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // Display tests for all error variants
    // =========================================================================

    #[test]
    fn test_io_error_display() {
        let io_err = io::Error::new(io::ErrorKind::NotFound, "file not found");
        let err = ChatpackError::from(io_err);
        let display = err.to_string();
        assert!(display.contains("IO error"));
        assert!(display.contains("file not found"));
    }

    #[cfg(any(
        feature = "telegram",
        feature = "instagram",
        feature = "discord",
        feature = "json-output"
    ))]
    #[test]
    fn test_parse_error_with_path() {
        let json_err = serde_json::from_str::<serde_json::Value>("invalid").unwrap_err();
        let err = ChatpackError::Parse {
            format: "Telegram JSON",
            source: ParseErrorKind::Json(json_err),
            path: Some(PathBuf::from("/path/to/file.json")),
        };
        let display = err.to_string();
        assert!(display.contains("Telegram JSON"));
        assert!(display.contains("/path/to/file.json"));
    }

    #[test]
    fn test_parse_error_without_path() {
        let err = ChatpackError::Parse {
            format: "WhatsApp TXT",
            source: ParseErrorKind::Pattern("invalid pattern".into()),
            path: None,
        };
        let display = err.to_string();
        assert!(display.contains("WhatsApp TXT"));
        assert!(!display.contains("file:"));
    }

    #[test]
    fn test_parse_error_other_kind() {
        let err = ChatpackError::Parse {
            format: "Test",
            source: ParseErrorKind::Other("custom error".into()),
            path: None,
        };
        let display = err.to_string();
        assert!(display.contains("custom error"));
    }

    #[test]
    fn test_invalid_format_display() {
        let err = ChatpackError::InvalidFormat {
            format: "Discord",
            message: "unrecognized export format".into(),
        };
        let display = err.to_string();
        assert!(display.contains("Discord"));
        assert!(display.contains("unrecognized export format"));
    }

    #[test]
    fn test_invalid_date_display() {
        let err = ChatpackError::invalid_date("not-a-date");
        let display = err.to_string();
        assert!(display.contains("not-a-date"));
        assert!(display.contains("YYYY-MM-DD"));
    }

    #[test]
    fn test_buffer_overflow_display() {
        let err = ChatpackError::buffer_overflow(1024, 2048);
        let display = err.to_string();
        assert!(display.contains("2048"));
        assert!(display.contains("1024"));
    }

    #[test]
    fn test_unexpected_eof_display() {
        let err = ChatpackError::unexpected_eof("parsing JSON array");
        let display = err.to_string();
        assert!(display.contains("Unexpected end of file"));
        assert!(display.contains("parsing JSON array"));
    }

    #[test]
    fn test_streaming_error_display() {
        let err =
            ChatpackError::Streaming(StreamingErrorKind::InvalidFormat("missing header".into()));
        let display = err.to_string();
        assert!(display.contains("Streaming error"));
        assert!(display.contains("missing header"));
    }

    #[test]
    fn test_utf8_error_display() {
        let invalid_bytes = vec![0xff, 0xfe];
        let utf8_err = String::from_utf8(invalid_bytes).unwrap_err();
        let err = ChatpackError::Utf8 {
            context: "reading file".into(),
            source: utf8_err,
        };
        let display = err.to_string();
        assert!(display.contains("UTF-8"));
        assert!(display.contains("reading file"));
    }

    // =========================================================================
    // Error source chain tests
    // =========================================================================

    #[test]
    fn test_error_source_chain() {
        use std::error::Error;
        let io_err = io::Error::new(io::ErrorKind::PermissionDenied, "access denied");
        let err = ChatpackError::from(io_err);
        assert!(err.source().is_some());
    }

    #[test]
    fn test_streaming_error_source() {
        use std::error::Error;
        let io_err = io::Error::new(io::ErrorKind::NotFound, "not found");
        let streaming_err = StreamingErrorKind::Io(io_err);
        let err = ChatpackError::Streaming(streaming_err);
        assert!(err.source().is_some());
    }

    // =========================================================================
    // is_* methods tests
    // =========================================================================

    #[test]
    fn test_is_methods() {
        let io_err = ChatpackError::Io(io::Error::new(io::ErrorKind::NotFound, ""));
        assert!(io_err.is_io());
        assert!(!io_err.is_parse());
        assert!(!io_err.is_invalid_format());
        assert!(!io_err.is_invalid_date());

        let date_err = ChatpackError::invalid_date("bad");
        assert!(date_err.is_invalid_date());
        assert!(!date_err.is_io());
        assert!(!date_err.is_parse());
        assert!(!date_err.is_invalid_format());
    }

    #[test]
    fn test_is_parse() {
        let err = ChatpackError::Parse {
            format: "Test",
            source: ParseErrorKind::Other("test".into()),
            path: None,
        };
        assert!(err.is_parse());
        assert!(!err.is_io());
    }

    #[test]
    fn test_is_invalid_format() {
        let err = ChatpackError::invalid_format("Test", "bad format");
        assert!(err.is_invalid_format());
        assert!(!err.is_parse());
    }

    // =========================================================================
    // Convenience constructors tests
    // =========================================================================

    #[test]
    fn test_convenience_constructors() {
        let err = ChatpackError::invalid_format("WhatsApp", "could not detect date format");
        assert!(err.is_invalid_format());
        assert!(err.to_string().contains("WhatsApp"));

        let err = ChatpackError::unexpected_eof("reading message array");
        let display = err.to_string();
        assert!(display.contains("reading message array"));
    }

    #[test]
    fn test_whatsapp_parse_constructor() {
        let err = ChatpackError::whatsapp_parse("invalid format", None);
        assert!(err.is_parse());
        assert!(err.to_string().contains("WhatsApp TXT"));

        let err_with_path =
            ChatpackError::whatsapp_parse("invalid format", Some(PathBuf::from("/test.txt")));
        assert!(err_with_path.to_string().contains("/test.txt"));
    }

    #[cfg(feature = "telegram")]
    #[test]
    fn test_telegram_parse_constructor() {
        let json_err = serde_json::from_str::<serde_json::Value>("invalid").unwrap_err();
        let err = ChatpackError::telegram_parse(json_err, None);
        assert!(err.is_parse());
        assert!(err.to_string().contains("Telegram JSON"));
    }

    #[cfg(feature = "instagram")]
    #[test]
    fn test_instagram_parse_constructor() {
        let json_err = serde_json::from_str::<serde_json::Value>("invalid").unwrap_err();
        let err = ChatpackError::instagram_parse(json_err, None);
        assert!(err.is_parse());
        assert!(err.to_string().contains("Instagram JSON"));
    }

    #[cfg(feature = "discord")]
    #[test]
    fn test_discord_parse_constructor() {
        let json_err = serde_json::from_str::<serde_json::Value>("invalid").unwrap_err();
        let err = ChatpackError::discord_parse(json_err, None);
        assert!(err.is_parse());
        assert!(err.to_string().contains("Discord"));
    }

    #[test]
    fn test_streaming_constructor() {
        let kind = StreamingErrorKind::UnexpectedEof;
        let err = ChatpackError::streaming(kind);
        assert!(err.to_string().contains("Streaming error"));
    }

    // =========================================================================
    // From conversions tests
    // =========================================================================

    #[test]
    fn test_from_io_error() {
        let io_err = io::Error::new(io::ErrorKind::NotFound, "file not found");
        let err: ChatpackError = io_err.into();
        assert!(err.is_io());
    }

    #[cfg(any(feature = "csv-output", feature = "discord"))]
    #[test]
    fn test_from_csv_error() {
        // Create a CSV error by using a writer and forcing an error
        use std::io::Cursor;
        let mut wtr = csv::Writer::from_writer(Cursor::new(Vec::new()));
        // Write some data first
        wtr.write_record(["a", "b"]).expect("write");
        // Create error via deserialization of invalid data
        let data = "field1,field2\n\"unclosed";
        let mut rdr = csv::ReaderBuilder::new().from_reader(data.as_bytes());
        for result in rdr.records() {
            if let Err(csv_err) = result {
                let err: ChatpackError = csv_err.into();
                assert!(err.to_string().contains("CSV error"));
                return;
            }
        }
        // If we reach here, force an error in a different way
        // The Csv variant just needs to be tested for From conversion
        let io_err = std::io::Error::other("test");
        let csv_err = csv::Error::from(io_err);
        let err: ChatpackError = csv_err.into();
        assert!(err.to_string().contains("CSV error"));
    }

    #[cfg(any(
        feature = "telegram",
        feature = "instagram",
        feature = "discord",
        feature = "json-output"
    ))]
    #[test]
    fn test_from_json_error() {
        let json_err = serde_json::from_str::<serde_json::Value>("invalid").unwrap_err();
        let err: ChatpackError = json_err.into();
        assert!(err.to_string().contains("JSON error"));
    }

    #[test]
    fn test_from_utf8_error() {
        let invalid_bytes = vec![0xff, 0xfe];
        let utf8_err = String::from_utf8(invalid_bytes).unwrap_err();
        let err: ChatpackError = utf8_err.into();
        assert!(err.to_string().contains("UTF-8"));
    }

    // =========================================================================
    // StreamingErrorKind tests
    // =========================================================================

    #[test]
    fn test_streaming_error_kind_io() {
        let io_err = io::Error::new(io::ErrorKind::NotFound, "not found");
        let kind = StreamingErrorKind::Io(io_err);
        assert!(kind.to_string().contains("IO error"));
    }

    #[cfg(any(
        feature = "telegram",
        feature = "instagram",
        feature = "discord",
        feature = "json-output"
    ))]
    #[test]
    fn test_streaming_error_kind_json() {
        let json_err = serde_json::from_str::<serde_json::Value>("invalid").unwrap_err();
        let kind = StreamingErrorKind::Json(json_err);
        assert!(kind.to_string().contains("JSON error"));
    }

    #[test]
    fn test_streaming_error_kind_invalid_format() {
        let kind = StreamingErrorKind::InvalidFormat("missing messages array".into());
        assert!(kind.to_string().contains("Invalid format"));
        assert!(kind.to_string().contains("missing messages array"));
    }

    #[test]
    fn test_streaming_error_kind_buffer_overflow() {
        let kind = StreamingErrorKind::BufferOverflow {
            max_size: 1024,
            actual_size: 2048,
        };
        let display = kind.to_string();
        assert!(display.contains("Buffer overflow"));
        assert!(display.contains("1024"));
        assert!(display.contains("2048"));
    }

    #[test]
    fn test_streaming_error_kind_unexpected_eof() {
        let kind = StreamingErrorKind::UnexpectedEof;
        assert!(kind.to_string().contains("Unexpected end of file"));
    }

    // =========================================================================
    // ParseErrorKind tests
    // =========================================================================

    #[test]
    fn test_parse_error_kind_pattern() {
        let kind = ParseErrorKind::Pattern("invalid regex".into());
        assert!(kind.to_string().contains("invalid regex"));
    }

    #[test]
    fn test_parse_error_kind_other() {
        let kind = ParseErrorKind::Other("unknown error".into());
        assert!(kind.to_string().contains("unknown error"));
    }

    #[cfg(any(
        feature = "telegram",
        feature = "instagram",
        feature = "discord",
        feature = "json-output"
    ))]
    #[test]
    fn test_parse_error_kind_json() {
        let json_err = serde_json::from_str::<serde_json::Value>("invalid").unwrap_err();
        let kind = ParseErrorKind::Json(json_err);
        assert_ne!(kind.to_string(), "");
    }

    // =========================================================================
    // Result type alias test
    // =========================================================================

    #[test]
    fn test_result_type_alias() {
        fn returns_result() -> i32 {
            42
        }

        fn returns_error() -> Result<i32> {
            Err(ChatpackError::invalid_date("bad"))
        }

        fn returns_ok() -> i32 {
            42
        }

        assert_eq!(returns_result(), 42);
        assert!(returns_error().is_err());
        assert_eq!(returns_ok(), 42);
        assert_eq!(returns_ok(), 42);
    }

    // =========================================================================
    // Debug trait test
    // =========================================================================

    #[test]
    fn test_error_debug() {
        let err = ChatpackError::invalid_date("bad");
        let debug = format!("{:?}", err);
        assert!(debug.contains("InvalidDate"));
    }
}
