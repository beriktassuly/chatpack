//! Parse and convert chat exports from messaging platforms into LLM-friendly formats.
//!
//! # Overview
//!
//! Chatpack provides a unified API for parsing chat exports from popular messaging
//! platforms and converting them into formats optimized for Large Language Models.
//! It handles platform-specific quirks (encoding issues, date formats, message types)
//! and provides tools for filtering, merging, and exporting messages.
//!
//! **Supported platforms:**
//!
//! | Platform | Export Format | Special Handling |
//! |----------|---------------|------------------|
//! | Telegram | JSON | Service messages, forwarded messages |
//! | WhatsApp | TXT | Auto-detects 5 locale-specific date formats |
//! | Instagram | JSON | Fixes Mojibake encoding from Meta exports |
//! | Discord | JSON/TXT/CSV | Attachments, stickers, replies |
//!
//! # Quick Start
//!
//! ```no_run
//! use chatpack::prelude::*;
//!
//! # #[cfg(all(feature = "telegram", feature = "csv-output"))]
//! # fn main() -> chatpack::Result<()> {
//! // Parse Telegram export
//! let parser = create_parser(Platform::Telegram);
//! let messages = parser.parse("export.json".as_ref())?;
//!
//! // Filter, merge, and export
//! let filtered = apply_filters(messages, &FilterConfig::new().with_sender("Alice"));
//! let merged = merge_consecutive(filtered);
//! write_csv(&merged, "output.csv", &OutputConfig::default())?;
//! # Ok(())
//! # }
//! # #[cfg(not(all(feature = "telegram", feature = "csv-output")))]
//! # fn main() {}
//! ```
//!
//! # Core Concepts
//!
//! ## Message
//!
//! [`Message`] is the universal representation of a chat message across all platforms:
//!
//! ```
//! use chatpack::Message;
//!
//! let msg = Message::new("Alice", "Hello, world!");
//! assert_eq!(msg.sender, "Alice");
//! assert_eq!(msg.content, "Hello, world!");
//! ```
//!
//! ## Parser Trait
//!
//! All platform parsers implement the [`Parser`](parser::Parser) trait, providing
//! a consistent interface:
//!
//! ```no_run
//! # #[cfg(feature = "whatsapp")]
//! # fn main() -> chatpack::Result<()> {
//! use chatpack::parser::Parser;
//! use chatpack::parsers::WhatsAppParser;
//!
//! let parser = WhatsAppParser::new();
//!
//! // Parse from file
//! let messages = parser.parse("chat.txt".as_ref())?;
//!
//! // Or parse from string
//! let content = "[1/15/24, 10:30:45 AM] Alice: Hello";
//! let messages = parser.parse_str(content)?;
//! # Ok(())
//! # }
//! # #[cfg(not(feature = "whatsapp"))]
//! # fn main() {}
//! ```
//!
//! # Common Patterns
//!
//! ## Filter by Date Range
//!
//! ```
//! use chatpack::prelude::*;
//!
//! # fn main() -> chatpack::Result<()> {
//! let messages = vec![
//!     Message::new("Alice", "Old message"),
//!     Message::new("Bob", "Recent message"),
//! ];
//!
//! let filter = FilterConfig::new()
//!     .with_date_from("2024-01-01")?
//!     .with_date_to("2024-12-31")?;
//!
//! let filtered = apply_filters(messages, &filter);
//! # Ok(())
//! # }
//! ```
//!
//! ## Merge Consecutive Messages
//!
//! Combine messages from the same sender within a time window:
//!
//! ```
//! use chatpack::prelude::*;
//!
//! let messages = vec![
//!     Message::new("Alice", "Hello"),
//!     Message::new("Alice", "How are you?"),
//!     Message::new("Bob", "I'm fine!"),
//! ];
//!
//! let merged = merge_consecutive(messages);
//! assert_eq!(merged.len(), 2); // Alice's messages merged
//! assert!(merged[0].content.contains("Hello"));
//! assert!(merged[0].content.contains("How are you?"));
//! ```
//!
//! ## Stream Large Files
//!
//! Process files larger than available memory:
//!
//! ```no_run
//! # #[cfg(all(feature = "telegram", feature = "streaming"))]
//! # fn main() -> chatpack::Result<()> {
//! use chatpack::prelude::*;
//!
//! let parser = create_streaming_parser(Platform::Telegram);
//!
//! for result in parser.stream("huge_export.json".as_ref())? {
//!     let msg = result?;
//!     println!("{}: {}", msg.sender, msg.content);
//! }
//! # Ok(())
//! # }
//! # #[cfg(not(all(feature = "telegram", feature = "streaming")))]
//! # fn main() {}
//! ```
//!
//! ## Export to Multiple Formats
//!
//! ```no_run
//! # #[cfg(all(feature = "csv-output", feature = "json-output"))]
//! # fn main() -> chatpack::Result<()> {
//! use chatpack::prelude::*;
//!
//! let messages = vec![Message::new("Alice", "Hello!")];
//! let config = OutputConfig::new().with_timestamps();
//!
//! // CSV - best for LLM context (13x token compression)
//! write_csv(&messages, "output.csv", &config)?;
//!
//! // JSON - structured array for APIs
//! write_json(&messages, "output.json", &config)?;
//!
//! // JSONL - one object per line for RAG pipelines
//! write_jsonl(&messages, "output.jsonl", &config)?;
//! # Ok(())
//! # }
//! # #[cfg(not(all(feature = "csv-output", feature = "json-output")))]
//! # fn main() {}
//! ```
//!
//! # Module Structure
//!
//! | Module | Description |
//! |--------|-------------|
//! | [`parser`] | Unified parser API with [`Parser`](parser::Parser) trait and [`Platform`](parser::Platform) enum |
//! | [`parsers`] | Platform-specific implementations: [`TelegramParser`](parsers::TelegramParser), [`WhatsAppParser`](parsers::WhatsAppParser), etc. |
//! | [`config`] | Parser configurations: [`TelegramConfig`](config::TelegramConfig), [`WhatsAppConfig`](config::WhatsAppConfig), etc. |
//! | [`core`] | Core types: [`Message`], [`OutputConfig`](core::OutputConfig), [`FilterConfig`](core::FilterConfig) |
//! | [`streaming`] | Memory-efficient streaming parsers for large files |
//! | [`mod@format`] | Output formats: [`OutputFormat`](format::OutputFormat), [`write_to_format`](format::write_to_format) |
//! | [`error`] | Error types: [`ChatpackError`], [`Result`] |
//! | [`prelude`] | Convenient re-exports for common usage |
//!
//! # Feature Flags
//!
//! Enable only the features you need to minimize compile time and dependencies:
//!
//! | Feature | Description | Dependencies |
//! |---------|-------------|--------------|
//! | `telegram` | Telegram JSON parser | `serde_json` |
//! | `whatsapp` | WhatsApp TXT parser | `regex` |
//! | `instagram` | Instagram JSON parser | `serde_json` |
//! | `discord` | Discord multi-format parser | `serde_json`, `regex`, `csv` |
//! | `csv-output` | CSV output writer | `csv` |
//! | `json-output` | JSON/JSONL output writers | `serde_json` |
//! | `streaming` | Streaming parsers for large files | - |
//! | `async` | Async parser support | `tokio` |
//! | `full` | All features (default) | all above |
//!
//! ```toml
//! # Cargo.toml - minimal configuration
//! [dependencies]
//! chatpack = { version = "0.7.0", default-features = false, features = ["telegram", "csv-output"] }
//! ```
//!
//! # Serialization
//!
//! All public types implement [`serde::Serialize`] and [`serde::Deserialize`]:
//!
//! ```
//! use chatpack::Message;
//!
//! let msg = Message::new("Alice", "Hello!");
//! let json = serde_json::to_string(&msg).unwrap();
//! let parsed: Message = serde_json::from_str(&json).unwrap();
//! assert_eq!(msg.content, parsed.content);
//! ```

// Core modules (always available)
pub mod config;
pub mod core;
pub mod error;
pub mod format;
pub mod message;
pub mod progress;

// Shared parsing utilities (DRY - used by both parsers and streaming)
#[cfg(any(
    feature = "telegram",
    feature = "whatsapp",
    feature = "instagram",
    feature = "discord"
))]
pub mod parsing;

// Parser modules - require at least one parser feature
#[cfg(any(
    feature = "telegram",
    feature = "whatsapp",
    feature = "instagram",
    feature = "discord"
))]
pub mod parser;

#[cfg(any(
    feature = "telegram",
    feature = "whatsapp",
    feature = "instagram",
    feature = "discord"
))]
pub mod parsers;

// Streaming module (requires streaming feature and at least one parser)
#[cfg(all(
    feature = "streaming",
    any(
        feature = "telegram",
        feature = "whatsapp",
        feature = "instagram",
        feature = "discord"
    )
))]
pub mod streaming;

// Async parser module (requires async feature and at least one parser)
#[cfg(all(feature = "async", feature = "telegram"))]
pub mod async_parser;

// Re-export the main types at the crate root for convenience
pub use error::{ChatpackError, Result};
pub use message::Message;

/// Convenient re-exports for common usage patterns.
///
/// This module provides a single import for the most commonly used types
/// and functions. It's designed to cover 90% of use cases with minimal imports.
///
/// # Example
///
/// ```
/// use chatpack::prelude::*;
///
/// // Now you have access to:
/// // - Message, ChatpackError, Result
/// // - Platform, Parser, create_parser, create_streaming_parser
/// // - FilterConfig, apply_filters
/// // - OutputConfig, merge_consecutive
/// // - write_csv, write_json, write_jsonl (with features)
/// // - All platform parsers (with features)
///
/// let msg = Message::new("Alice", "Hello!");
/// ```
pub mod prelude {
    // Core message type
    pub use crate::Message;

    // Error types
    pub use crate::error::{ChatpackError, Result};

    // New unified parser API (recommended)
    #[cfg(any(
        feature = "telegram",
        feature = "whatsapp",
        feature = "instagram",
        feature = "discord"
    ))]
    pub use crate::parser::{Parser, Platform, create_parser, create_streaming_parser};

    // Platform configs
    pub use crate::config::{DiscordConfig, InstagramConfig, TelegramConfig, WhatsAppConfig};

    // Models
    pub use crate::core::models::OutputConfig;

    // Filtering
    pub use crate::core::filter::{FilterConfig, apply_filters};

    // Processing
    pub use crate::core::processor::{ProcessingStats, merge_consecutive};

    // Output format
    pub use crate::format::OutputFormat;
    #[cfg(any(feature = "csv-output", feature = "json-output"))]
    pub use crate::format::{to_format_string, write_to_format};

    // Output (file writers and string converters)
    #[cfg(feature = "csv-output")]
    pub use crate::core::output::{to_csv, write_csv};

    #[cfg(feature = "json-output")]
    pub use crate::core::output::{to_json, to_jsonl, write_json, write_jsonl};

    // Progress reporting
    pub use crate::progress::{Progress, ProgressCallback, no_progress};

    // Parsers (implement Parser trait)
    #[cfg(feature = "telegram")]
    pub use crate::parsers::TelegramParser;

    #[cfg(feature = "whatsapp")]
    pub use crate::parsers::WhatsAppParser;

    #[cfg(feature = "instagram")]
    pub use crate::parsers::InstagramParser;

    #[cfg(feature = "discord")]
    pub use crate::parsers::DiscordParser;
}
