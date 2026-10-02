//! Message merging and processing statistics.
//!
//! This module provides utilities for reducing message count while preserving
//! conversation structure, primarily for LLM context optimization.
//!
//! # Overview
//!
//! | Function | Description |
//! |----------|-------------|
//! | [`merge_consecutive`] | Combine consecutive messages from same sender |
//! | [`ProcessingStats`] | Track compression metrics |
//!
//! # Token Compression
//!
//! Merging consecutive messages typically achieves 30-50% reduction in message count,
//! which translates to significant token savings when feeding to LLMs.
//!
//! # Examples
//!
//! ```
//! use chatpack::core::processor::{merge_consecutive, ProcessingStats};
//! use chatpack::Message;
//!
//! let messages = vec![
//!     Message::new("Alice", "Hey"),
//!     Message::new("Alice", "Are you there?"),
//!     Message::new("Alice", "Hello???"),
//!     Message::new("Bob", "Sorry, was busy"),
//! ];
//!
//! let original_count = messages.len();
//! let merged = merge_consecutive(messages);
//!
//! // 4 messages -> 2 messages (50% reduction)
//! assert_eq!(merged.len(), 2);
//! assert!(merged[0].content.contains("Hey"));
//! assert!(merged[0].content.contains("Hello???"));
//!
//! let stats = ProcessingStats::new(original_count, merged.len());
//! println!("{}", stats); // "4 → 2 messages (50.0% reduction)"
//! ```

use crate::Message;

/// Merges consecutive messages from the same sender into single entries.
///
/// This significantly reduces token count when feeding to LLMs by combining
/// rapid-fire messages from the same person into single entries.
///
/// # Algorithm
///
/// Messages are merged when:
/// 1. They come from the same sender (exact string match)
/// 2. They are consecutive (no messages from others in between)
///
/// When merging:
/// - Contents are joined with newline (`\n`)
/// - First message's metadata (timestamp, id, `reply_to`, edited) is preserved
///
/// # Example
///
/// ```rust
/// use chatpack::core::processor::merge_consecutive;
/// use chatpack::Message;
///
/// let messages = vec![
///     Message::new("Alice", "Hi"),
///     Message::new("Alice", "How are you?"),
///     Message::new("Bob", "Good!"),
/// ];
///
/// let merged = merge_consecutive(messages);
///
/// assert_eq!(merged.len(), 2);
/// assert_eq!(merged[0].content, "Hi\nHow are you?");
/// assert_eq!(merged[1].content, "Good!");
/// ```
///
/// # Performance
///
/// This function:
/// - Consumes the input vector (no cloning of messages)
/// - Allocates a new output vector
/// - O(n) time complexity
pub fn merge_consecutive(messages: Vec<Message>) -> Vec<Message> {
    let mut merged: Vec<Message> = Vec::with_capacity(messages.len());

    for msg in messages {
        match merged.last_mut() {
            Some(last) if last.sender == msg.sender => {
                last.content.push('\n');
                last.content.push_str(&msg.content);
            }
            _ => {
                merged.push(msg);
            }
        }
    }

    merged.shrink_to_fit();
    merged
}

/// Statistics about the processing result.
///
/// Provides information about how many messages were processed
/// and the compression achieved through merging.
///
/// # Example
///
/// ```rust
/// use chatpack::core::processor::ProcessingStats;
///
/// let stats = ProcessingStats::new(100, 60);
/// println!("Compression: {:.1}%", stats.compression_ratio()); // 40.0%
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct ProcessingStats {
    /// Number of messages before processing
    pub original_count: usize,

    /// Number of messages after merging
    pub merged_count: usize,

    /// Number of messages after filtering (if filtering was applied)
    pub filtered_count: Option<usize>,
}

impl ProcessingStats {
    /// Creates new statistics with original and merged counts.
    pub fn new(original: usize, merged: usize) -> Self {
        Self {
            original_count: original,
            merged_count: merged,
            filtered_count: None,
        }
    }

    /// Sets the filtered count.
    #[must_use]
    pub fn with_filtered(mut self, filtered: usize) -> Self {
        self.filtered_count = Some(filtered);
        self
    }

    /// Calculate compression ratio as percentage.
    ///
    /// Returns the percentage of messages reduced by merging.
    /// Uses `filtered_count` as base if available, otherwise `original_count`.
    ///
    /// # Example
    ///
    /// ```rust
    /// use chatpack::core::processor::ProcessingStats;
    ///
    /// // 100 messages -> 60 after merge = 40% reduction
    /// let stats = ProcessingStats::new(100, 60);
    /// assert!((stats.compression_ratio() - 40.0).abs() < 0.1);
    /// ```
    pub fn compression_ratio(&self) -> f64 {
        let base = self.filtered_count.unwrap_or(self.original_count);
        if base == 0 {
            return 0.0;
        }
        (1.0 - (self.merged_count as f64 / base as f64)) * 100.0
    }

    /// Returns the number of messages removed by merging.
    pub fn messages_saved(&self) -> usize {
        let base = self.filtered_count.unwrap_or(self.original_count);
        base.saturating_sub(self.merged_count)
    }

    /// Returns the merge ratio (merged / original).
    pub fn merge_ratio(&self) -> f64 {
        let base = self.filtered_count.unwrap_or(self.original_count);
        if base == 0 {
            return 1.0;
        }
        self.merged_count as f64 / base as f64
    }
}

impl Default for ProcessingStats {
    fn default() -> Self {
        Self::new(0, 0)
    }
}

impl std::fmt::Display for ProcessingStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} → {} messages ({:.1}% reduction)",
            self.filtered_count.unwrap_or(self.original_count),
            self.merged_count,
            self.compression_ratio()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_merge_consecutive() {
        let messages = vec![
            Message::new("Alice", "Hi"),
            Message::new("Alice", "How are you?"),
            Message::new("Bob", "Fine"),
            Message::new("Bob", "Thanks"),
            Message::new("Alice", "Great!"),
        ];

        let merged = merge_consecutive(messages);

        assert_eq!(merged.len(), 3);
        assert_eq!(merged[0].sender, "Alice");
        assert_eq!(merged[0].content, "Hi\nHow are you?");
        assert_eq!(merged[1].sender, "Bob");
        assert_eq!(merged[1].content, "Fine\nThanks");
        assert_eq!(merged[2].sender, "Alice");
        assert_eq!(merged[2].content, "Great!");
    }

    #[test]
    fn test_merge_empty() {
        let messages: Vec<Message> = vec![];
        let merged = merge_consecutive(messages);
        assert_eq!(merged, []);
    }

    #[test]
    fn test_merge_single() {
        let messages = vec![Message::new("Alice", "Hi")];
        let merged = merge_consecutive(messages);
        assert_eq!(merged.len(), 1);
    }

    #[test]
    fn test_merge_preserves_metadata() {
        use chrono::{TimeZone, Utc};

        let ts = Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap();
        let messages = vec![
            Message::new("Alice", "First").with_timestamp(ts).with_id(1),
            Message::new("Alice", "Second").with_id(2),
        ];

        let merged = merge_consecutive(messages);

        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].timestamp, Some(ts));
        assert_eq!(merged[0].id, Some(1)); // First message's ID preserved
    }

    #[test]
    fn test_compression_ratio() {
        let stats = ProcessingStats::new(100, 50);
        assert!((stats.compression_ratio() - 50.0).abs() < 0.01);
    }

    #[test]
    fn test_compression_ratio_zero() {
        let stats = ProcessingStats::new(0, 0);
        assert!((stats.compression_ratio() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_compression_ratio_with_filtered() {
        let stats = ProcessingStats::new(100, 25).with_filtered(50);
        // 50 filtered -> 25 merged = 50% reduction
        assert!((stats.compression_ratio() - 50.0).abs() < 0.01);
    }

    #[test]
    fn test_messages_saved() {
        let stats = ProcessingStats::new(100, 60);
        assert_eq!(stats.messages_saved(), 40);
    }

    #[test]
    fn test_stats_display() {
        let stats = ProcessingStats::new(100, 60);
        let display = stats.to_string();
        assert!(display.contains("100"));
        assert!(display.contains("60"));
        assert!(display.contains("40.0%"));
    }
}
