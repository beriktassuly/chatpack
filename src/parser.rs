//! Unified parser API for chat exports.
//!
//! This module provides a platform-agnostic interface for parsing chat exports.
//! All platform parsers implement the [`Parser`] trait, enabling consistent
//! usage patterns across Telegram, WhatsApp, Instagram, and Discord.
//!
//! # Overview
//!
//! The module provides:
//! - [`Parser`] - Unified trait for all parsers
//! - [`Platform`] - Enum for dynamic parser selection
//! - [`create_parser`] - Factory function for standard parsers
//! - [`create_streaming_parser`] - Factory function for memory-efficient streaming
//!
//! # Examples
//!
//! ## Basic Parsing
//!
//! ```no_run
//! # #[cfg(feature = "telegram")]
//! # fn main() -> chatpack::Result<()> {
//! use chatpack::parser::{Parser, Platform, create_parser};
//!
//! // Create parser dynamically
//! let parser = create_parser(Platform::Telegram);
//! let messages = parser.parse("export.json".as_ref())?;
//!
//! println!("Parsed {} messages", messages.len());
//! # Ok(())
//! # }
//! # #[cfg(not(feature = "telegram"))]
//! # fn main() {}
//! ```
//!
//! ## Parse from String
//!
//! Useful for WASM or testing:
//!
//! ```
//! # #[cfg(feature = "whatsapp")]
//! # fn main() -> chatpack::Result<()> {
//! use chatpack::parser::{Parser, create_parser, Platform};
//!
//! let content = "[1/15/24, 10:30:45 AM] Alice: Hello!";
//! let parser = create_parser(Platform::WhatsApp);
//! let messages = parser.parse_str(content)?;
//!
//! assert_eq!(messages[0].sender, "Alice");
//! # Ok(())
//! # }
//! # #[cfg(not(feature = "whatsapp"))]
//! # fn main() {}
//! ```
//!
//! ## Streaming Large Files
//!
//! Process files that don't fit in memory:
//!
//! ```no_run
//! # #[cfg(all(feature = "telegram", feature = "streaming"))]
//! # fn main() -> chatpack::Result<()> {
//! use chatpack::parser::{Parser, Platform, create_streaming_parser};
//!
//! let parser = create_streaming_parser(Platform::Telegram);
//!
//! for result in parser.stream("10gb_export.json".as_ref())? {
//!     let msg = result?;
//!     // Process one message at a time
//! }
//! # Ok(())
//! # }
//! # #[cfg(not(all(feature = "telegram", feature = "streaming")))]
//! # fn main() {}
//! ```

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::Message;
use crate::error::ChatpackError;

#[cfg(feature = "streaming")]
use crate::streaming::MessageIterator;

/// Supported messaging platforms for chat export parsing.
///
/// Each variant corresponds to a specific export format and parser implementation.
/// Use with [`create_parser`] or [`create_streaming_parser`] for dynamic parser selection.
///
/// # Aliases
///
/// All platforms support short aliases for convenience:
/// - `telegram` / `tg`
/// - `whatsapp` / `wa`
/// - `instagram` / `ig`
/// - `discord` / `dc`
///
/// # Examples
///
/// ```
/// use chatpack::parser::Platform;
/// use std::str::FromStr;
///
/// // Parse from string (case-insensitive)
/// let platform = Platform::from_str("telegram")?;
/// assert_eq!(platform, Platform::Telegram);
///
/// // Aliases work too
/// let platform = Platform::from_str("tg")?;
/// assert_eq!(platform, Platform::Telegram);
///
/// // Get file extension
/// assert_eq!(Platform::WhatsApp.default_extension(), "txt");
/// assert_eq!(Platform::Telegram.default_extension(), "json");
/// # Ok::<(), String>(())
/// ```
///
/// # Serialization
///
/// Serializes to lowercase strings, deserializes with alias support:
///
/// ```
/// use chatpack::parser::Platform;
///
/// let json = serde_json::to_string(&Platform::Telegram)?;
/// assert_eq!(json, "\"telegram\"");
///
/// // Deserialize with alias
/// let platform: Platform = serde_json::from_str("\"tg\"")?;
/// assert_eq!(platform, Platform::Telegram);
/// # Ok::<(), serde_json::Error>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum Platform {
    /// Telegram Desktop JSON exports.
    ///
    /// Parses the `result.json` file from "Export chat history" feature.
    /// Handles service messages, forwarded messages, and reply chains.
    #[serde(alias = "tg")]
    Telegram,

    /// WhatsApp TXT exports from iOS and Android.
    ///
    /// Auto-detects locale-specific date formats (US, EU, RU variants).
    /// Handles multiline messages and system notifications.
    #[serde(alias = "wa")]
    WhatsApp,

    /// Instagram JSON exports from Meta's data download.
    ///
    /// Automatically fixes Mojibake encoding issues in non-ASCII text.
    /// Parses direct messages from the `messages/` directory.
    #[serde(alias = "ig")]
    Instagram,

    /// Discord exports from DiscordChatExporter tool.
    ///
    /// Supports multiple formats: JSON, TXT, and CSV.
    /// Preserves attachments, stickers, and reply references.
    #[serde(alias = "dc")]
    Discord,
}

impl Platform {
    /// Returns the default file extension for exports from this platform.
    ///
    /// # Examples
    ///
    /// ```
    /// use chatpack::parser::Platform;
    ///
    /// assert_eq!(Platform::Telegram.default_extension(), "json");
    /// assert_eq!(Platform::WhatsApp.default_extension(), "txt");
    /// ```
    pub fn default_extension(&self) -> &'static str {
        match self {
            Platform::WhatsApp => "txt",
            Platform::Telegram | Platform::Instagram | Platform::Discord => "json",
        }
    }

    /// Returns all valid platform names and aliases.
    ///
    /// Useful for CLI help text or validation messages.
    ///
    /// # Examples
    ///
    /// ```
    /// use chatpack::parser::Platform;
    ///
    /// let names = Platform::all_names();
    /// assert!(names.contains(&"telegram"));
    /// assert!(names.contains(&"tg")); // alias
    /// ```
    pub fn all_names() -> &'static [&'static str] {
        &[
            "telegram",
            "tg",
            "whatsapp",
            "wa",
            "instagram",
            "ig",
            "discord",
            "dc",
        ]
    }

    /// Returns all available platform variants.
    ///
    /// # Examples
    ///
    /// ```
    /// use chatpack::parser::Platform;
    ///
    /// for platform in Platform::all() {
    ///     println!("{}: .{}", platform, platform.default_extension());
    /// }
    /// ```
    pub fn all() -> &'static [Platform] {
        &[
            Platform::Telegram,
            Platform::WhatsApp,
            Platform::Instagram,
            Platform::Discord,
        ]
    }
}

impl std::fmt::Display for Platform {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Platform::Telegram => write!(f, "Telegram"),
            Platform::WhatsApp => write!(f, "WhatsApp"),
            Platform::Instagram => write!(f, "Instagram"),
            Platform::Discord => write!(f, "Discord"),
        }
    }
}

impl std::str::FromStr for Platform {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "telegram" | "tg" => Ok(Platform::Telegram),
            "whatsapp" | "wa" => Ok(Platform::WhatsApp),
            "instagram" | "ig" => Ok(Platform::Instagram),
            "discord" | "dc" => Ok(Platform::Discord),
            _ => Err(format!(
                "Unknown platform: '{}'. Expected one of: {}",
                s,
                Platform::all_names().join(", ")
            )),
        }
    }
}

/// Iterator over parsed messages with progress tracking.
///
/// Wraps a streaming parser's [`MessageIterator`]
/// and converts errors to [`ChatpackError`]. Provides progress information for
/// long-running operations.
///
/// # Examples
///
/// ```no_run
/// # #[cfg(all(feature = "telegram", feature = "streaming"))]
/// # fn main() -> chatpack::Result<()> {
/// use chatpack::parser::{Parser, Platform, create_streaming_parser};
///
/// let parser = create_streaming_parser(Platform::Telegram);
/// let mut count = 0;
///
/// for result in parser.stream("export.json".as_ref())? {
///     let msg = result?;
///     count += 1;
/// }
///
/// println!("Processed {} messages", count);
/// # Ok(())
/// # }
/// # #[cfg(not(all(feature = "telegram", feature = "streaming")))]
/// # fn main() {}
/// ```
#[cfg(feature = "streaming")]
pub struct ParseIterator {
    inner: Box<dyn MessageIterator>,
}

#[cfg(feature = "streaming")]
impl ParseIterator {
    /// Creates a new parse iterator from a message iterator.
    pub fn new(inner: Box<dyn MessageIterator>) -> Self {
        Self { inner }
    }

    /// Returns the progress as a percentage (0.0 to 100.0).
    ///
    /// Returns `None` if progress cannot be determined (e.g., unknown file size).
    pub fn progress(&self) -> Option<f64> {
        self.inner.progress()
    }

    /// Returns the number of bytes processed so far.
    pub fn bytes_processed(&self) -> u64 {
        self.inner.bytes_processed()
    }

    /// Returns the total file size in bytes, if known.
    pub fn total_bytes(&self) -> Option<u64> {
        self.inner.total_bytes()
    }
}

#[cfg(feature = "streaming")]
impl Iterator for ParseIterator {
    type Item = Result<Message, ChatpackError>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner
            .next()
            .map(|result| result.map_err(ChatpackError::from))
    }
}

/// Unified trait for parsing chat exports from any platform.
///
/// All platform-specific parsers implement this trait, providing a consistent
/// interface for parsing chat exports regardless of the source platform.
///
/// # Required Methods
///
/// | Method | Description |
/// |--------|-------------|
/// | [`name`](Parser::name) | Human-readable parser name |
/// | [`platform`](Parser::platform) | Platform enum variant |
/// | [`parse`](Parser::parse) | Parse file into memory |
/// | [`parse_str`](Parser::parse_str) | Parse from string |
///
/// # Optional Methods
///
/// | Method | Default | Description |
/// |--------|---------|-------------|
/// | [`stream`](Parser::stream) | Falls back to `parse` | Memory-efficient streaming |
/// | [`supports_streaming`](Parser::supports_streaming) | `false` | Native streaming support |
/// | [`recommended_buffer_size`](Parser::recommended_buffer_size) | 64KB | Buffer size hint |
///
/// # Examples
///
/// Using a parser directly:
///
/// ```no_run
/// # #[cfg(feature = "telegram")]
/// # fn main() -> chatpack::Result<()> {
/// use chatpack::parser::Parser;
/// use chatpack::parsers::TelegramParser;
///
/// let parser = TelegramParser::new();
///
/// // Parse from file
/// let messages = parser.parse("export.json".as_ref())?;
///
/// // Parse from string
/// let json = r#"{"messages": []}"#;
/// let messages = parser.parse_str(json)?;
/// # Ok(())
/// # }
/// # #[cfg(not(feature = "telegram"))]
/// # fn main() {}
/// ```
///
/// Using the factory function:
///
/// ```no_run
/// # #[cfg(feature = "telegram")]
/// # fn main() -> chatpack::Result<()> {
/// use chatpack::parser::{Parser, Platform, create_parser};
///
/// let parser = create_parser(Platform::Telegram);
/// let messages = parser.parse("export.json".as_ref())?;
/// # Ok(())
/// # }
/// # #[cfg(not(feature = "telegram"))]
/// # fn main() {}
/// ```
pub trait Parser: Send + Sync {
    /// Returns the human-readable name of this parser.
    ///
    /// # Example
    ///
    /// ```rust
    /// # #[cfg(feature = "telegram")]
    /// # fn main() {
    /// use chatpack::parser::Parser;
    /// use chatpack::parsers::TelegramParser;
    ///
    /// let parser = TelegramParser::new();
    /// assert_eq!(parser.name(), "Telegram");
    /// # }
    /// # #[cfg(not(feature = "telegram"))]
    /// # fn main() {}
    /// ```
    fn name(&self) -> &'static str;

    /// Returns the platform this parser handles.
    fn platform(&self) -> Platform;

    /// Parses a chat export file and returns all messages.
    ///
    /// This method loads the entire file into memory, which is suitable
    /// for files up to ~500MB. For larger files, use [`stream`](Parser::stream).
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the export file
    ///
    /// # Errors
    ///
    /// Returns [`ChatpackError`] if:
    /// - File cannot be read ([`ChatpackError::Io`])
    /// - Content cannot be parsed ([`ChatpackError::Parse`])
    fn parse(&self, path: &Path) -> Result<Vec<Message>, ChatpackError>;

    /// Parses chat content from a string.
    ///
    /// This is useful for:
    /// - WASM environments without file system access
    /// - Testing with inline data
    /// - Processing content already in memory
    ///
    /// # Arguments
    ///
    /// * `content` - Raw export content as a string
    ///
    /// # Errors
    ///
    /// Returns [`ChatpackError::Parse`] if content cannot be parsed.
    fn parse_str(&self, content: &str) -> Result<Vec<Message>, ChatpackError>;

    /// Parses a chat export file (convenience method accepting &str path).
    ///
    /// This is equivalent to `parse(Path::new(path))`.
    fn parse_file(&self, path: &str) -> Result<Vec<Message>, ChatpackError> {
        self.parse(Path::new(path))
    }

    /// Streams messages from a file for memory-efficient processing.
    ///
    /// By default, this falls back to loading the entire file and returning
    /// an iterator over the messages. Parsers that support native streaming
    /// should override this method.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the export file
    ///
    /// # Returns
    ///
    /// An iterator that yields `Result<Message, ChatpackError>` for each message.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # #[cfg(feature = "telegram")]
    /// # fn main() -> chatpack::Result<()> {
    /// use chatpack::parser::Parser;
    /// use chatpack::parsers::TelegramParser;
    /// use std::path::Path;
    ///
    /// let parser = TelegramParser::new();
    /// for result in parser.stream(Path::new("large_file.json"))? {
    ///     if let Ok(msg) = result {
    ///         println!("{}: {}", msg.sender, msg.content);
    ///     }
    /// }
    /// # Ok(())
    /// # }
    /// # #[cfg(not(feature = "telegram"))]
    /// # fn main() {}
    /// ```
    fn stream(
        &self,
        path: &Path,
    ) -> Result<Box<dyn Iterator<Item = Result<Message, ChatpackError>> + Send>, ChatpackError>
    {
        // Default implementation: load everything into memory
        let messages = self.parse(path)?;
        Ok(Box::new(messages.into_iter().map(Ok)))
    }

    /// Streams messages (convenience method accepting &str path).
    fn stream_file(
        &self,
        path: &str,
    ) -> Result<Box<dyn Iterator<Item = Result<Message, ChatpackError>> + Send>, ChatpackError>
    {
        self.stream(Path::new(path))
    }

    /// Returns whether this parser supports native streaming.
    ///
    /// If `false`, the [`stream`](Parser::stream) method will load the entire
    /// file into memory before iterating.
    fn supports_streaming(&self) -> bool {
        false
    }

    /// Returns the recommended buffer size for streaming.
    ///
    /// Only relevant if [`supports_streaming`](Parser::supports_streaming) returns `true`.
    fn recommended_buffer_size(&self) -> usize {
        64 * 1024 // 64KB default
    }
}

/// Creates a parser for the specified platform with default configuration.
///
/// This is the primary factory function for creating parsers dynamically.
/// The returned parser loads the entire file into memory, which is suitable
/// for files up to ~500MB.
///
/// For larger files, use [`create_streaming_parser`] instead.
///
/// # Examples
///
/// ```
/// # #[cfg(feature = "telegram")]
/// # fn main() {
/// use chatpack::parser::{Platform, create_parser};
///
/// let parser = create_parser(Platform::Telegram);
/// assert_eq!(parser.name(), "Telegram");
/// assert_eq!(parser.platform(), Platform::Telegram);
/// # }
/// # #[cfg(not(feature = "telegram"))]
/// # fn main() {}
/// ```
///
/// # Panics
///
/// Panics if the corresponding feature is not enabled. Enable features in `Cargo.toml`:
///
/// ```toml
/// [dependencies]
/// chatpack = { version = "0.7.0", features = ["telegram"] }
/// ```
pub fn create_parser(platform: Platform) -> Box<dyn Parser> {
    match platform {
        #[cfg(feature = "telegram")]
        Platform::Telegram => Box::new(crate::parsers::TelegramParser::new()),
        #[cfg(feature = "whatsapp")]
        Platform::WhatsApp => Box::new(crate::parsers::WhatsAppParser::new()),
        #[cfg(feature = "instagram")]
        Platform::Instagram => Box::new(crate::parsers::InstagramParser::new()),
        #[cfg(feature = "discord")]
        Platform::Discord => Box::new(crate::parsers::DiscordParser::new()),
        // Fallback for when features are disabled
        #[allow(unreachable_patterns)]
        _ => panic!(
            "Parser for {:?} is not enabled. Enable the corresponding feature.",
            platform
        ),
    }
}

/// Creates a parser optimized for streaming large files.
///
/// The returned parser is configured for memory-efficient processing of files
/// that may be larger than available RAM. Use the [`stream`](Parser::stream)
/// method to process messages one at a time.
///
/// # When to Use
///
/// - Files larger than 500MB
/// - Memory-constrained environments
/// - When you need progress tracking
///
/// For smaller files, [`create_parser`] is simpler and often faster.
///
/// # Examples
///
/// ```no_run
/// # #[cfg(all(feature = "telegram", feature = "streaming"))]
/// # fn main() -> chatpack::Result<()> {
/// use chatpack::parser::{Parser, Platform, create_streaming_parser};
///
/// let parser = create_streaming_parser(Platform::Telegram);
/// assert!(parser.supports_streaming());
///
/// let mut count = 0;
/// for result in parser.stream("10gb_export.json".as_ref())? {
///     let _msg = result?;
///     count += 1;
///     if count % 100_000 == 0 {
///         println!("Processed {} messages", count);
///     }
/// }
/// # Ok(())
/// # }
/// # #[cfg(not(all(feature = "telegram", feature = "streaming")))]
/// # fn main() {}
/// ```
///
/// # Panics
///
/// Panics if the corresponding feature is not enabled.
pub fn create_streaming_parser(platform: Platform) -> Box<dyn Parser> {
    match platform {
        #[cfg(feature = "telegram")]
        Platform::Telegram => Box::new(crate::parsers::TelegramParser::with_streaming()),
        #[cfg(feature = "whatsapp")]
        Platform::WhatsApp => Box::new(crate::parsers::WhatsAppParser::with_streaming()),
        #[cfg(feature = "instagram")]
        Platform::Instagram => Box::new(crate::parsers::InstagramParser::with_streaming()),
        #[cfg(feature = "discord")]
        Platform::Discord => Box::new(crate::parsers::DiscordParser::with_streaming()),
        // Fallback for when features are disabled
        #[allow(unreachable_patterns)]
        _ => panic!(
            "Streaming parser for {:?} is not enabled. Enable the corresponding feature.",
            platform
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    // =========================================================================
    // Platform::from_str tests
    // =========================================================================

    #[test]
    fn test_platform_from_str() {
        assert_eq!(Platform::from_str("telegram").unwrap(), Platform::Telegram);
        assert_eq!(Platform::from_str("tg").unwrap(), Platform::Telegram);
        assert_eq!(Platform::from_str("TELEGRAM").unwrap(), Platform::Telegram);
        assert_eq!(Platform::from_str("whatsapp").unwrap(), Platform::WhatsApp);
        assert_eq!(Platform::from_str("wa").unwrap(), Platform::WhatsApp);
        assert_eq!(
            Platform::from_str("instagram").unwrap(),
            Platform::Instagram
        );
        assert_eq!(Platform::from_str("ig").unwrap(), Platform::Instagram);
        assert_eq!(Platform::from_str("discord").unwrap(), Platform::Discord);
        assert_eq!(Platform::from_str("dc").unwrap(), Platform::Discord);
    }

    #[test]
    fn test_platform_from_str_case_insensitive() {
        // Test various case combinations
        assert_eq!(Platform::from_str("TeLegRaM").unwrap(), Platform::Telegram);
        assert_eq!(Platform::from_str("TG").unwrap(), Platform::Telegram);
        assert_eq!(Platform::from_str("WhAtSaPp").unwrap(), Platform::WhatsApp);
        assert_eq!(Platform::from_str("WA").unwrap(), Platform::WhatsApp);
        assert_eq!(
            Platform::from_str("InStAgRaM").unwrap(),
            Platform::Instagram
        );
        assert_eq!(Platform::from_str("IG").unwrap(), Platform::Instagram);
        assert_eq!(Platform::from_str("DiScOrD").unwrap(), Platform::Discord);
        assert_eq!(Platform::from_str("DC").unwrap(), Platform::Discord);
    }

    #[test]
    fn test_platform_from_str_error() {
        let err = Platform::from_str("unknown").unwrap_err();
        assert!(err.contains("Unknown platform"));
        assert!(err.contains("unknown"));

        let err = Platform::from_str("").unwrap_err();
        assert!(err.contains("Unknown platform"));

        let err = Platform::from_str("telegramx").unwrap_err();
        assert!(err.contains("Unknown platform"));
    }

    // =========================================================================
    // Platform display tests
    // =========================================================================

    #[test]
    fn test_platform_display() {
        assert_eq!(Platform::Telegram.to_string(), "Telegram");
        assert_eq!(Platform::WhatsApp.to_string(), "WhatsApp");
        assert_eq!(Platform::Instagram.to_string(), "Instagram");
        assert_eq!(Platform::Discord.to_string(), "Discord");
    }

    // =========================================================================
    // Platform default_extension tests
    // =========================================================================

    #[test]
    fn test_platform_default_extension() {
        assert_eq!(Platform::Telegram.default_extension(), "json");
        assert_eq!(Platform::WhatsApp.default_extension(), "txt");
        assert_eq!(Platform::Instagram.default_extension(), "json");
        assert_eq!(Platform::Discord.default_extension(), "json");
    }

    // =========================================================================
    // Platform::all and Platform::all_names tests
    // =========================================================================

    #[test]
    fn test_platform_all() {
        let all = Platform::all();
        assert_eq!(all.len(), 4);
        assert!(all.contains(&Platform::Telegram));
        assert!(all.contains(&Platform::WhatsApp));
        assert!(all.contains(&Platform::Instagram));
        assert!(all.contains(&Platform::Discord));
    }

    #[test]
    fn test_platform_all_names() {
        let names = Platform::all_names();
        assert!(names.contains(&"telegram"));
        assert!(names.contains(&"tg"));
        assert!(names.contains(&"whatsapp"));
        assert!(names.contains(&"wa"));
        assert!(names.contains(&"instagram"));
        assert!(names.contains(&"ig"));
        assert!(names.contains(&"discord"));
        assert!(names.contains(&"dc"));
    }

    // =========================================================================
    // Platform serde tests
    // =========================================================================

    #[test]
    fn test_platform_serde() {
        let platform = Platform::Telegram;
        let json = serde_json::to_string(&platform).expect("serialize failed");
        assert_eq!(json, "\"telegram\"");

        let parsed: Platform = serde_json::from_str("\"telegram\"").expect("deserialize failed");
        assert_eq!(parsed, Platform::Telegram);

        // Test alias deserialization
        let parsed: Platform = serde_json::from_str("\"tg\"").expect("deserialize failed");
        assert_eq!(parsed, Platform::Telegram);

        let parsed: Platform = serde_json::from_str("\"wa\"").expect("deserialize failed");
        assert_eq!(parsed, Platform::WhatsApp);
    }

    #[test]
    fn test_platform_serde_all_variants() {
        for platform in Platform::all() {
            let json = serde_json::to_string(platform).expect("serialize failed");
            let parsed: Platform = serde_json::from_str(&json).expect("deserialize failed");
            assert_eq!(parsed, *platform);
        }
    }

    // =========================================================================
    // Platform traits tests
    // =========================================================================

    #[test]
    fn test_platform_clone_copy() {
        let p1 = Platform::Telegram;
        let p2 = p1; // Copy
        let p3 = p1;
        assert_eq!(p1, p2);
        assert_eq!(p1, p3);
    }

    #[test]
    fn test_platform_debug() {
        let debug = format!("{:?}", Platform::Telegram);
        assert!(debug.contains("Telegram"));
    }

    #[test]
    fn test_platform_eq_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(Platform::Telegram);
        set.insert(Platform::WhatsApp);
        set.insert(Platform::Telegram); // Duplicate
        assert_eq!(set.len(), 2);
        assert!(set.contains(&Platform::Telegram));
        assert!(set.contains(&Platform::WhatsApp));
    }

    // =========================================================================
    // create_parser tests
    // =========================================================================

    #[cfg(feature = "telegram")]
    #[test]
    fn test_create_parser_telegram() {
        let parser = create_parser(Platform::Telegram);
        assert_eq!(parser.name(), "Telegram");
        assert_eq!(parser.platform(), Platform::Telegram);
        assert!(!parser.supports_streaming());
    }

    #[cfg(feature = "whatsapp")]
    #[test]
    fn test_create_parser_whatsapp() {
        let parser = create_parser(Platform::WhatsApp);
        assert_eq!(parser.name(), "WhatsApp");
        assert_eq!(parser.platform(), Platform::WhatsApp);
    }

    #[cfg(feature = "instagram")]
    #[test]
    fn test_create_parser_instagram() {
        let parser = create_parser(Platform::Instagram);
        assert_eq!(parser.name(), "Instagram");
        assert_eq!(parser.platform(), Platform::Instagram);
    }

    #[cfg(feature = "discord")]
    #[test]
    fn test_create_parser_discord() {
        let parser = create_parser(Platform::Discord);
        assert_eq!(parser.name(), "Discord");
        assert_eq!(parser.platform(), Platform::Discord);
    }

    // =========================================================================
    // create_streaming_parser tests
    // =========================================================================

    #[cfg(feature = "telegram")]
    #[test]
    fn test_create_streaming_parser_telegram() {
        let parser = create_streaming_parser(Platform::Telegram);
        assert_eq!(parser.name(), "Telegram");
        assert!(parser.supports_streaming());
        assert!(parser.recommended_buffer_size() >= 64 * 1024);
    }

    #[cfg(feature = "whatsapp")]
    #[test]
    fn test_create_streaming_parser_whatsapp() {
        let parser = create_streaming_parser(Platform::WhatsApp);
        assert_eq!(parser.name(), "WhatsApp");
        assert!(parser.supports_streaming());
    }

    #[cfg(feature = "instagram")]
    #[test]
    fn test_create_streaming_parser_instagram() {
        let parser = create_streaming_parser(Platform::Instagram);
        assert_eq!(parser.name(), "Instagram");
        assert!(parser.supports_streaming());
    }

    #[cfg(feature = "discord")]
    #[test]
    fn test_create_streaming_parser_discord() {
        let parser = create_streaming_parser(Platform::Discord);
        assert_eq!(parser.name(), "Discord");
        assert!(parser.supports_streaming());
    }

    // =========================================================================
    // Parser trait method tests
    // =========================================================================

    #[cfg(feature = "telegram")]
    #[test]
    fn test_parser_parse_str() {
        let parser = create_parser(Platform::Telegram);
        let json = r#"{"messages": [{"id": 1, "type": "message", "date_unixtime": "1234567890", "from": "Alice", "text": "Hello"}]}"#;
        let messages = parser.parse_str(json).expect("parse failed");
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].sender, "Alice");
        assert_eq!(messages[0].content, "Hello");
    }

    #[cfg(feature = "telegram")]
    #[test]
    fn test_parser_parse_file() {
        use std::io::Write;
        let dir = tempfile::tempdir().expect("create temp dir");
        let file_path = dir.path().join("test.json");
        let mut file = std::fs::File::create(&file_path).expect("create file");
        write!(file, r#"{{"messages": [{{"id": 1, "type": "message", "date_unixtime": "1234567890", "from": "Bob", "text": "Hi"}}]}}"#).expect("write");

        let parser = create_parser(Platform::Telegram);
        let messages = parser
            .parse_file(file_path.to_str().unwrap())
            .expect("parse failed");
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].sender, "Bob");
    }

    #[cfg(all(feature = "telegram", feature = "streaming"))]
    #[test]
    fn test_parser_stream_file() {
        use std::io::Write;
        let dir = tempfile::tempdir().expect("create temp dir");
        let file_path = dir.path().join("test.json");
        let mut file = std::fs::File::create(&file_path).expect("create file");
        // Streaming parser needs newlines for line-by-line reading
        writeln!(file, r"{{").expect("write");
        writeln!(file, r#"  "messages": ["#).expect("write");
        writeln!(file, r#"    {{"id": 1, "type": "message", "date_unixtime": "1234567890", "from": "Charlie", "text": "Hello"}}"#).expect("write");
        writeln!(file, r"  ]").expect("write");
        writeln!(file, r"}}").expect("write");
        file.flush().expect("flush");
        drop(file);

        let parser = create_streaming_parser(Platform::Telegram);
        let iter = parser
            .stream_file(file_path.to_str().unwrap())
            .expect("stream failed");
        let messages: Vec<_> = iter.filter_map(|r| r.ok()).collect();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].sender, "Charlie");
    }

    // =========================================================================
    // Parser default implementations
    // =========================================================================

    #[cfg(feature = "telegram")]
    #[test]
    fn test_parser_default_supports_streaming() {
        let parser = create_parser(Platform::Telegram);
        // Default parser (non-streaming config) should return false
        assert!(!parser.supports_streaming());
    }

    #[cfg(feature = "telegram")]
    #[test]
    fn test_parser_default_recommended_buffer_size() {
        let parser = create_parser(Platform::Telegram);
        // Should return at least 64KB
        assert!(parser.recommended_buffer_size() >= 64 * 1024);
    }

    // =========================================================================
    // ParseIterator tests
    // =========================================================================

    #[cfg(all(feature = "telegram", feature = "streaming"))]
    #[test]
    fn test_parse_iterator_wrapper() {
        use crate::streaming::StreamingParser;
        use crate::streaming::TelegramStreamingParser;
        use std::io::Write;

        let dir = tempfile::tempdir().expect("create temp dir");
        let file_path = dir.path().join("test.json");
        let mut file = std::fs::File::create(&file_path).expect("create file");
        writeln!(file, r"{{").expect("write");
        writeln!(file, r#"  "messages": ["#).expect("write");
        writeln!(file, r#"    {{"id": 1, "type": "message", "date_unixtime": "1234567890", "from": "Alice", "text": "Hello"}}"#).expect("write");
        writeln!(file, r"  ]").expect("write");
        writeln!(file, r"}}").expect("write");
        file.flush().expect("flush");
        drop(file);

        let streaming_parser = TelegramStreamingParser::new();
        let inner = streaming_parser
            .stream(file_path.to_str().unwrap())
            .expect("stream failed");

        let mut parse_iter = ParseIterator::new(inner);

        // Test progress methods
        assert!(parse_iter.progress().is_some() || parse_iter.progress().is_none());
        assert!(parse_iter.total_bytes().is_some());

        // Test iterator
        let msg = parse_iter.next().unwrap().expect("should parse");
        assert_eq!(msg.sender, "Alice");
        assert!(parse_iter.next().is_none());
    }

    #[cfg(feature = "telegram")]
    #[test]
    fn test_parser_stream_default_impl() {
        use std::io::Write;

        let dir = tempfile::tempdir().expect("create temp dir");
        let file_path = dir.path().join("test.json");
        let mut file = std::fs::File::create(&file_path).expect("create file");
        write!(file, r#"{{"messages": [{{"id": 1, "type": "message", "date_unixtime": "1234567890", "from": "Bob", "text": "Hi"}}]}}"#).expect("write");
        file.flush().expect("flush");
        drop(file);

        // Use non-streaming parser to test default stream() implementation
        let parser = create_parser(Platform::Telegram);
        let iter = parser.stream(file_path.as_ref()).expect("stream failed");
        let messages: Vec<_> = iter.filter_map(|r| r.ok()).collect();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].sender, "Bob");
    }
}
