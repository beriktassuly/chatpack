//! Async parser support for chatpack.
//!
//! This module provides async/await-based parsers for use with tokio.
//!
//! # Example
//!
//! ```rust,no_run
//! use chatpack::async_parser::{AsyncParser, AsyncTelegramParser};
//!
//! # async fn example() -> Result<(), chatpack::ChatpackError> {
//! let parser = AsyncTelegramParser::new();
//! let messages = parser.parse("telegram_export.json").await?;
//!
//! for msg in messages {
//!     println!("{}: {}", msg.sender, msg.content);
//! }
//! # Ok(())
//! # }
//! ```
//!
//! # Features
//!
//! This module requires the `async` feature to be enabled:
//!
//! ```toml
//! [dependencies]
//! chatpack = { version = "0.7.0", features = ["async", "telegram"] }
//! ```

use std::path::Path;

use async_trait::async_trait;
use tokio::fs;

use crate::Message;
use crate::error::ChatpackError;

mod telegram;

#[cfg(feature = "telegram")]
pub use telegram::AsyncTelegramParser;

/// Trait for async parsers.
///
/// This is the async equivalent of the synchronous `Parser` trait.
/// It allows parsing files asynchronously using tokio.
///
/// # Example
///
/// ```rust,no_run
/// use chatpack::async_parser::{AsyncParser, AsyncTelegramParser};
///
/// # async fn example() -> Result<(), chatpack::ChatpackError> {
/// let parser = AsyncTelegramParser::new();
/// let messages = parser.parse("export.json").await?;
/// # Ok(())
/// # }
/// ```
#[async_trait]
pub trait AsyncParser: Send + Sync {
    /// Returns the name of the parser.
    fn name(&self) -> &'static str;

    /// Parses a file asynchronously.
    ///
    /// Reads the file using tokio's async I/O and parses its contents.
    async fn parse(&self, path: impl AsRef<Path> + Send) -> Result<Vec<Message>, ChatpackError>;

    /// Parses content from a string.
    ///
    /// This is useful when you already have the content in memory.
    fn parse_str(&self, content: &str) -> Result<Vec<Message>, ChatpackError>;
}

/// Helper function to read a file asynchronously.
pub(crate) async fn read_file_async(path: impl AsRef<Path>) -> Result<String, ChatpackError> {
    let content = fs::read_to_string(path).await?;
    Ok(content)
}
