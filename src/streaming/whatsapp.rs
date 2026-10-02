//! Streaming parser for WhatsApp TXT exports.
//!
//! WhatsApp exports are text files with various date formats:
//! - US: `[1/15/24, 10:30:45 AM] Sender: Message`
//! - EU (dot): `[15.01.24, 10:30:45] Sender: Message`
//! - EU (no bracket): `26.10.2025, 20:40 - Sender: Message`
//! - EU (slash): `15/01/2024, 10:30 - Sender: Message`
//!
//! This parser streams line-by-line, handling multi-line messages.

use std::collections::VecDeque;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use chrono::{DateTime, Utc};
use regex::Regex;

use crate::Message;
use crate::error::ChatpackError;
use crate::parsing::whatsapp::{
    DateFormat, detect_whatsapp_format_owned, is_whatsapp_system_message, parse_whatsapp_timestamp,
};

use super::{MessageIterator, StreamingConfig, StreamingParser, StreamingResult};

/// Streaming parser for WhatsApp TXT exports.
pub struct WhatsAppStreamingParser {
    config: StreamingConfig,
}

impl WhatsAppStreamingParser {
    pub fn new() -> Self {
        Self {
            config: StreamingConfig::default(),
        }
    }

    pub fn with_config(config: StreamingConfig) -> Self {
        Self { config }
    }
}

impl Default for WhatsAppStreamingParser {
    fn default() -> Self {
        Self::new()
    }
}

impl StreamingParser for WhatsAppStreamingParser {
    fn name(&self) -> &'static str {
        "WhatsApp (Streaming)"
    }

    fn stream(&self, file_path: &str) -> Result<Box<dyn MessageIterator>, ChatpackError> {
        let path = Path::new(file_path);
        let file = File::open(path)?;
        let file_size = file.metadata()?.len();

        let reader = BufReader::with_capacity(self.config.buffer_size, file);
        let iterator = WhatsAppMessageIterator::new(reader, file_size, self.config)?;

        Ok(Box::new(iterator))
    }

    fn recommended_buffer_size(&self) -> usize {
        self.config.buffer_size
    }
}

#[derive(Debug, Default)]
struct PendingMessage {
    sender: String,
    content: String,
    timestamp: Option<DateTime<Utc>>,
}

impl PendingMessage {
    fn is_empty(&self) -> bool {
        self.sender.is_empty()
    }

    fn take(&mut self) -> Self {
        std::mem::take(self)
    }

    fn into_message(self) -> Option<Message> {
        if self.sender.is_empty() || self.content.trim().is_empty() {
            return None;
        }

        if is_whatsapp_system_message(&self.sender, &self.content) {
            return None;
        }

        Some(Message::with_metadata(
            self.sender,
            self.content.trim().to_string(),
            self.timestamp,
            None,
            None,
            None,
        ))
    }
}

/// Iterator over WhatsApp messages.
pub struct WhatsAppMessageIterator<R: BufRead> {
    reader: R,
    file_size: u64,
    bytes_read: u64,
    config: StreamingConfig,
    line_buffer: String,
    pending: PendingMessage,
    queued: VecDeque<Message>,
    finished: bool,
    detected_format: Option<DateFormat>,
    format_regex: Option<Regex>,
}

impl<R: BufRead> WhatsAppMessageIterator<R> {
    fn new(mut reader: R, file_size: u64, config: StreamingConfig) -> StreamingResult<Self> {
        // Read first few lines to detect format
        let mut sample_lines = Vec::new();
        let mut sample_bytes = 0u64;

        for _ in 0..20 {
            let mut line = String::new();
            let bytes = reader.read_line(&mut line)?;
            if bytes == 0 {
                break;
            }
            sample_bytes += bytes as u64;
            sample_lines.push(line);
        }

        let detected_format = detect_whatsapp_format_owned(&sample_lines);
        let format_regex = detected_format.map(|f| Regex::new(f.pattern()).unwrap());

        let mut iter = Self {
            reader,
            file_size,
            bytes_read: sample_bytes,
            config,
            line_buffer: String::with_capacity(4096),
            pending: PendingMessage::default(),
            queued: VecDeque::new(),
            finished: false,
            detected_format,
            format_regex,
        };

        // Process sample lines, queuing completed messages
        for line in sample_lines {
            iter.process_line_queuing(&line);
        }

        Ok(iter)
    }

    /// Process line, queuing any completed message before starting new one.
    fn process_line_queuing(&mut self, line: &str) {
        if line.trim().is_empty() {
            return;
        }

        if let (Some(format), Some(regex)) = (self.detected_format, &self.format_regex) {
            if let Some(caps) = regex.captures(line) {
                // New message - queue the pending one first
                if !self.pending.is_empty() {
                    if let Some(msg) = self.pending.take().into_message() {
                        self.queued.push_back(msg);
                    }
                }

                let date_str = caps.get(1).map_or("", |m| m.as_str());
                let time_str = caps.get(2).map_or("", |m| m.as_str());
                let sender = caps.get(3).map_or("", |m| m.as_str().trim());
                let content = caps.get(4).map_or("", |m| m.as_str());

                self.pending.sender = sender.to_string();
                self.pending.content = content.to_string();
                self.pending.timestamp = parse_whatsapp_timestamp(date_str, time_str, format);
                return;
            }
        }

        // Continuation line
        if !self.pending.is_empty() {
            self.pending.content.push('\n');
            self.pending.content.push_str(line.trim_end());
        }
    }

    fn read_line(&mut self) -> std::io::Result<Option<String>> {
        self.line_buffer.clear();
        let bytes = self.reader.read_line(&mut self.line_buffer)?;
        if bytes == 0 {
            return Ok(None);
        }
        self.bytes_read += bytes as u64;
        Ok(Some(self.line_buffer.clone()))
    }
}

impl<R: BufRead + Send> MessageIterator for WhatsAppMessageIterator<R> {
    fn progress(&self) -> Option<f64> {
        if self.file_size == 0 {
            return None;
        }
        Some((self.bytes_read as f64 / self.file_size as f64) * 100.0)
    }

    fn bytes_processed(&self) -> u64 {
        self.bytes_read
    }

    fn total_bytes(&self) -> Option<u64> {
        Some(self.file_size)
    }
}

impl<R: BufRead + Send> Iterator for WhatsAppMessageIterator<R> {
    type Item = StreamingResult<Message>;

    fn next(&mut self) -> Option<Self::Item> {
        // First, drain queued messages from sample lines
        if let Some(msg) = self.queued.pop_front() {
            return Some(Ok(msg));
        }

        if self.finished && self.pending.is_empty() {
            return None;
        }

        if self.detected_format.is_none() {
            self.finished = true;
            return None;
        }

        loop {
            match self.read_line() {
                Ok(Some(line)) => {
                    if let Some(regex) = &self.format_regex {
                        if regex.is_match(&line) {
                            let to_yield = self.pending.take();
                            self.process_line_queuing(&line);

                            if let Some(msg) = to_yield.into_message() {
                                return Some(Ok(msg));
                            }
                            continue;
                        }
                    }
                    self.process_line_queuing(&line);
                }
                Ok(None) => {
                    self.finished = true;
                    let to_yield = self.pending.take();
                    if let Some(msg) = to_yield.into_message() {
                        return Some(Ok(msg));
                    }
                    return None;
                }
                Err(e) => {
                    if self.config.skip_invalid {
                        continue;
                    }
                    return Some(Err(e.into()));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn create_test_us_format() -> String {
        "[1/15/24, 10:30:00 AM] Alice: Hello everyone!
[1/15/24, 10:31:00 AM] Bob: Hi Alice!
[1/15/24, 10:32:00 AM] Alice: How is everyone doing?
This is a continuation line
[1/15/24, 10:33:00 AM] Charlie: Messages and calls are end-to-end encrypted.
[1/15/24, 10:34:00 AM] Bob: I'm doing great!"
            .to_string()
    }

    fn create_test_eu_format() -> String {
        "[15.01.24, 10:30:00] Alice: Привет всем!
[15.01.24, 10:31:00] Bob: Привет!
[15.01.24, 10:32:00] Alice: Как дела?"
            .to_string()
    }

    // =========================================================================
    // Constructor tests
    // =========================================================================

    #[test]
    fn test_parser_new() {
        let parser = WhatsAppStreamingParser::new();
        assert_eq!(parser.name(), "WhatsApp (Streaming)");
    }

    #[test]
    fn test_parser_default() {
        let parser = WhatsAppStreamingParser::default();
        assert_eq!(parser.name(), "WhatsApp (Streaming)");
    }

    #[test]
    fn test_parser_with_config() {
        let config = StreamingConfig::default()
            .with_buffer_size(512 * 1024)
            .with_max_message_size(2 * 1024 * 1024)
            .with_skip_invalid(true);
        let parser = WhatsAppStreamingParser::with_config(config);
        assert_eq!(parser.name(), "WhatsApp (Streaming)");
    }

    #[test]
    fn test_recommended_buffer_size() {
        let parser = WhatsAppStreamingParser::new();
        assert!(parser.recommended_buffer_size() > 0);
    }

    // =========================================================================
    // Basic parsing tests
    // =========================================================================

    #[test]
    fn test_streaming_parser_us_format() {
        let txt = create_test_us_format();
        let cursor = Cursor::new(txt.as_bytes().to_vec());
        let reader = BufReader::new(cursor);

        let iterator =
            WhatsAppMessageIterator::new(reader, txt.len() as u64, StreamingConfig::default())
                .unwrap();

        let messages: Vec<_> = iterator.filter_map(Result::ok).collect();

        assert_eq!(messages.len(), 4);
        assert_eq!(messages[0].sender, "Alice");
        assert_eq!(messages[0].content, "Hello everyone!");
        assert_eq!(messages[1].sender, "Bob");
        assert!(messages[2].content.contains("continuation"));
    }

    #[test]
    fn test_streaming_parser_eu_format() {
        let txt = create_test_eu_format();
        let cursor = Cursor::new(txt.as_bytes().to_vec());
        let reader = BufReader::new(cursor);

        let iterator =
            WhatsAppMessageIterator::new(reader, txt.len() as u64, StreamingConfig::default())
                .unwrap();

        let messages: Vec<_> = iterator.filter_map(Result::ok).collect();

        assert_eq!(messages.len(), 3);
        assert!(messages[0].content.contains("Привет"));
    }

    #[test]
    fn test_empty_file() {
        let txt = "";
        let cursor = Cursor::new(txt.as_bytes().to_vec());
        let reader = BufReader::new(cursor);

        let iterator =
            WhatsAppMessageIterator::new(reader, txt.len() as u64, StreamingConfig::default())
                .unwrap();

        let messages: Vec<_> = iterator.filter_map(Result::ok).collect();
        assert_eq!(messages, []);
    }

    // =========================================================================
    // Progress and iterator trait tests
    // =========================================================================

    #[test]
    fn test_progress_reporting() {
        let txt = create_test_us_format();
        let cursor = Cursor::new(txt.as_bytes().to_vec());
        let reader = BufReader::new(cursor);

        let mut iterator =
            WhatsAppMessageIterator::new(reader, txt.len() as u64, StreamingConfig::default())
                .unwrap();

        let _: Vec<_> = iterator.by_ref().collect();

        let progress = iterator.progress().unwrap();
        assert!(progress > 90.0);
    }

    #[test]
    fn test_progress_with_zero_file_size() {
        let txt = create_test_us_format();
        let cursor = Cursor::new(txt.as_bytes().to_vec());
        let reader = BufReader::new(cursor);

        let iterator = WhatsAppMessageIterator::new(reader, 0, StreamingConfig::default()).unwrap();

        assert!(iterator.progress().is_none());
    }

    #[test]
    fn test_bytes_processed() {
        let txt = create_test_us_format();
        let cursor = Cursor::new(txt.as_bytes().to_vec());
        let reader = BufReader::new(cursor);

        let mut iterator =
            WhatsAppMessageIterator::new(reader, txt.len() as u64, StreamingConfig::default())
                .unwrap();

        // Sample lines already read
        let initial_bytes = iterator.bytes_processed();
        assert!(initial_bytes > 0);

        // Consume all messages
        let _: Vec<_> = iterator.by_ref().collect();

        let final_bytes = iterator.bytes_processed();
        assert!(final_bytes >= initial_bytes);
    }

    #[test]
    fn test_total_bytes() {
        let txt = create_test_us_format();
        let file_size = txt.len() as u64;
        let cursor = Cursor::new(txt.as_bytes().to_vec());
        let reader = BufReader::new(cursor);

        let iterator =
            WhatsAppMessageIterator::new(reader, file_size, StreamingConfig::default()).unwrap();

        assert_eq!(iterator.total_bytes(), Some(file_size));
    }

    // =========================================================================
    // Content tests
    // =========================================================================

    #[test]
    fn test_parser_name() {
        let parser = WhatsAppStreamingParser::new();
        assert_eq!(parser.name(), "WhatsApp (Streaming)");
    }

    #[test]
    fn test_system_message_detection() {
        assert!(is_whatsapp_system_message(
            "Alice",
            "Messages and calls are end-to-end encrypted"
        ));
        assert!(!is_whatsapp_system_message("Alice", "Hello everyone!"));
        assert!(is_whatsapp_system_message(
            "Bob",
            "added Charlie to the group"
        ));
    }

    #[test]
    fn test_detect_format_us() {
        let lines = vec![
            "[1/15/24, 10:30:45 AM] Alice: Hello".to_string(),
            "[1/15/24, 10:31:00 AM] Bob: Hi there".to_string(),
        ];
        assert_eq!(detect_whatsapp_format_owned(&lines), Some(DateFormat::US));
    }

    #[test]
    fn test_detect_format_eu_dot() {
        let lines = vec![
            "[15.01.24, 10:30:45] Alice: Hello".to_string(),
            "[15.01.24, 10:31:00] Bob: Hi there".to_string(),
        ];
        assert_eq!(
            detect_whatsapp_format_owned(&lines),
            Some(DateFormat::EuDotBracketed)
        );
    }

    #[test]
    fn test_multiline_messages() {
        let txt = "[1/15/24, 10:30:00 AM] Alice: Line 1
Line 2
Line 3
[1/15/24, 10:31:00 AM] Bob: Reply";

        let cursor = Cursor::new(txt.as_bytes().to_vec());
        let reader = BufReader::new(cursor);

        let iterator =
            WhatsAppMessageIterator::new(reader, txt.len() as u64, StreamingConfig::default())
                .unwrap();

        let messages: Vec<_> = iterator.filter_map(Result::ok).collect();

        assert_eq!(messages.len(), 2);
        assert!(messages[0].content.contains("Line 1"));
        assert!(messages[0].content.contains("Line 2"));
        assert!(messages[0].content.contains("Line 3"));
    }

    #[test]
    fn test_empty_content_skipped() {
        let txt = "[1/15/24, 10:30:00 AM] Alice:
[1/15/24, 10:31:00 AM] Bob: Real message";

        let cursor = Cursor::new(txt.as_bytes().to_vec());
        let reader = BufReader::new(cursor);

        let iterator =
            WhatsAppMessageIterator::new(reader, txt.len() as u64, StreamingConfig::default())
                .unwrap();

        let messages: Vec<_> = iterator.filter_map(Result::ok).collect();

        // Empty content message should be skipped
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].sender, "Bob");
    }

    #[test]
    fn test_unrecognized_format_returns_empty() {
        let txt = "This is not a WhatsApp export format
Just random lines
With no pattern";

        let cursor = Cursor::new(txt.as_bytes().to_vec());
        let reader = BufReader::new(cursor);

        let iterator =
            WhatsAppMessageIterator::new(reader, txt.len() as u64, StreamingConfig::default())
                .unwrap();

        let messages: Vec<_> = iterator.filter_map(Result::ok).collect();
        assert_eq!(messages, []);
    }

    #[test]
    fn test_pending_message_helpers() {
        let pending = PendingMessage::default();
        assert!(pending.is_empty());

        let with_content = PendingMessage {
            sender: "Alice".to_string(),
            content: "Hello".to_string(),
            timestamp: None,
        };
        assert!(!with_content.is_empty());

        // Test into_message
        let msg = with_content.into_message();
        assert!(msg.is_some());
        let msg = msg.unwrap();
        assert_eq!(msg.sender, "Alice");
        assert_eq!(msg.content, "Hello");
    }

    #[test]
    fn test_pending_message_take() {
        let mut pending = PendingMessage {
            sender: "Alice".to_string(),
            content: "Hello".to_string(),
            timestamp: None,
        };

        let taken = pending.take();
        assert!(!taken.is_empty());
        assert!(pending.is_empty());
    }

    #[test]
    fn test_system_messages_filtered() {
        let txt = "[1/15/24, 10:30:00 AM] Alice: Hello
[1/15/24, 10:31:00 AM] System: created this group
[1/15/24, 10:32:00 AM] Bob: Hi there";

        let cursor = Cursor::new(txt.as_bytes().to_vec());
        let reader = BufReader::new(cursor);

        let iterator =
            WhatsAppMessageIterator::new(reader, txt.len() as u64, StreamingConfig::default())
                .unwrap();

        let messages: Vec<_> = iterator.filter_map(Result::ok).collect();

        // System message should be filtered
        assert!(messages.iter().all(|m| m.sender != "System"));
    }

    #[test]
    fn test_empty_lines_between_messages() {
        let txt = "[1/15/24, 10:30:00 AM] Alice: Hello

[1/15/24, 10:31:00 AM] Bob: Hi";

        let cursor = Cursor::new(txt.as_bytes().to_vec());
        let reader = BufReader::new(cursor);

        let iterator =
            WhatsAppMessageIterator::new(reader, txt.len() as u64, StreamingConfig::default())
                .unwrap();

        let messages: Vec<_> = iterator.filter_map(Result::ok).collect();

        assert_eq!(messages.len(), 2);
    }

    #[test]
    fn test_iterator_returns_none_when_finished() {
        let txt = "[1/15/24, 10:30:00 AM] Alice: Hello";

        let cursor = Cursor::new(txt.as_bytes().to_vec());
        let reader = BufReader::new(cursor);

        let mut iterator =
            WhatsAppMessageIterator::new(reader, txt.len() as u64, StreamingConfig::default())
                .unwrap();

        // Consume all messages
        let _: Vec<_> = iterator.by_ref().collect();

        // Additional calls should return None
        assert!(iterator.next().is_none());
        assert!(iterator.next().is_none());
    }
}
