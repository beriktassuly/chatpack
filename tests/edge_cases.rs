//! Edge case tests for chatpack
//!
//! These tests cover various edge cases and boundary conditions
//! that might not be covered by regular unit and integration tests.

use chatpack::Message;
use chatpack::core::filter::FilterConfig;
use chatpack::core::models::OutputConfig;
use chatpack::core::processor::merge_consecutive;
use chrono::{TimeZone, Utc};

// =========================================================================
// Unicode and special character tests
// =========================================================================

#[test]
fn test_unicode_normalization() {
    // Test various Unicode scripts
    let cyrillic = Message::new("Иван", "Привет мир!");
    assert_eq!(cyrillic.sender, "Иван");
    assert_eq!(cyrillic.content, "Привет мир!");

    let japanese = Message::new("田中太郎", "こんにちは世界！");
    assert_eq!(japanese.sender, "田中太郎");
    assert_eq!(japanese.content, "こんにちは世界！");

    let arabic = Message::new("محمد", "مرحبا بالعالم");
    assert_eq!(arabic.sender, "محمد");
    assert_eq!(arabic.content, "مرحبا بالعالم");

    let emoji = Message::new("User 🎉", "Hello 👋 World 🌍");
    assert_eq!(emoji.sender, "User 🎉");
    assert_eq!(emoji.content, "Hello 👋 World 🌍");

    // Mixed scripts
    let mixed = Message::new("User123", "Hello 你好 Привет مرحبا 🌍");
    assert_eq!(mixed.content, "Hello 你好 Привет مرحبا 🌍");
}

#[test]
fn test_zero_width_characters() {
    // Zero-width joiner in names (used in emoji sequences)
    let zwj_emoji = Message::new("User👨‍👩‍👧", "Family emoji test");
    assert!(zwj_emoji.sender.contains("👨‍👩‍👧"));

    // Zero-width non-joiner
    let zwnj = Message::new("User\u{200C}Name", "ZWNJ test");
    assert!(zwnj.sender.contains("\u{200C}"));

    // Zero-width space
    let zws = Message::new("User\u{200B}Name", "ZWS test");
    assert!(zws.sender.contains("\u{200B}"));
}

#[test]
fn test_combining_diacritics() {
    // Test combining characters
    let combining = Message::new("Café", "Naïve résumé");
    assert!(combining.sender.contains("é"));

    // NFD vs NFC normalization
    let nfc = Message::new("é", "NFC form");
    let nfd_content = "e\u{0301}"; // e + combining acute accent
    let nfd = Message::new(nfd_content, "NFD form");

    // Both should work, even if different representations
    assert_ne!(nfc.sender, "");
    assert_ne!(nfd.sender, "");
}

// =========================================================================
// Very long message tests
// =========================================================================

#[test]
fn test_very_long_content() {
    // 10KB message
    let long_content = "x".repeat(10 * 1024);
    let msg = Message::new("Sender", &long_content);
    assert_eq!(msg.content.len(), 10 * 1024);

    // 100KB message
    let very_long_content = "y".repeat(100 * 1024);
    let msg2 = Message::new("Sender", &very_long_content);
    assert_eq!(msg2.content.len(), 100 * 1024);

    // 1MB message
    let huge_content = "z".repeat(1024 * 1024);
    let msg3 = Message::new("Sender", &huge_content);
    assert_eq!(msg3.content.len(), 1024 * 1024);
}

#[test]
fn test_very_long_sender_name() {
    let long_name = "A".repeat(10000);
    let msg = Message::new(&long_name, "Content");
    assert_eq!(msg.sender.len(), 10000);
}

// =========================================================================
// Special sender name tests
// =========================================================================

#[test]
fn test_empty_sender_name() {
    let msg = Message::new("", "Content");
    assert_eq!(msg.sender, "");
}

#[test]
fn test_whitespace_only_sender() {
    let msg = Message::new("   ", "Content");
    assert_eq!(msg.sender, "   ");
}

#[test]
fn test_special_chars_in_sender() {
    let special = Message::new("User<>&\"'", "Content");
    assert_eq!(special.sender, "User<>&\"'");

    let newlines = Message::new("User\nName", "Content");
    assert!(newlines.sender.contains('\n'));

    let tabs = Message::new("User\tName", "Content");
    assert!(tabs.sender.contains('\t'));
}

// =========================================================================
// Timestamp edge cases
// =========================================================================

#[test]
fn test_timestamp_unix_epoch() {
    let epoch = Utc.timestamp_opt(0, 0).unwrap();
    let msg = Message::with_metadata("Sender", "1970-01-01", Some(epoch), None, None, None);
    assert_eq!(msg.timestamp, Some(epoch));
}

#[test]
fn test_timestamp_y2k() {
    let y2k = Utc.with_ymd_and_hms(2000, 1, 1, 0, 0, 0).unwrap();
    let msg = Message::with_metadata("Sender", "Y2K", Some(y2k), None, None, None);
    assert_eq!(msg.timestamp, Some(y2k));
}

#[test]
fn test_timestamp_near_2038() {
    // Y2038 problem for 32-bit systems (2^31 - 1 seconds from epoch)
    let near_2038 = Utc.with_ymd_and_hms(2038, 1, 18, 0, 0, 0).unwrap();
    let msg = Message::with_metadata("Sender", "Near 2038", Some(near_2038), None, None, None);
    assert_eq!(msg.timestamp, Some(near_2038));
}

#[test]
fn test_timestamp_far_future() {
    // Year 3000
    let far_future = Utc.with_ymd_and_hms(3000, 1, 1, 0, 0, 0).unwrap();
    let msg = Message::with_metadata("Sender", "Far future", Some(far_future), None, None, None);
    assert_eq!(msg.timestamp, Some(far_future));
}

// =========================================================================
// Filter edge cases
// =========================================================================

#[test]
fn test_filter_boundary_dates() {
    let msg1 = Message::with_metadata(
        "Alice",
        "Start of day",
        Some(Utc.with_ymd_and_hms(2024, 1, 15, 0, 0, 0).unwrap()),
        None,
        None,
        None,
    );
    let msg2 = Message::with_metadata(
        "Bob",
        "End of day",
        Some(Utc.with_ymd_and_hms(2024, 1, 15, 23, 59, 59).unwrap()),
        None,
        None,
        None,
    );
    let msg3 = Message::with_metadata(
        "Charlie",
        "Next day end",
        Some(Utc.with_ymd_and_hms(2024, 1, 17, 0, 0, 0).unwrap()),
        None,
        None,
        None,
    );

    let messages = vec![msg1.clone(), msg2.clone(), msg3.clone()];

    // Filter for messages before 2024-01-16 (includes up to 23:59:59 on that day)
    let filter = FilterConfig::new().before_date("2024-01-16").unwrap();
    let filtered = chatpack::core::filter::apply_filters(messages.clone(), &filter);
    // Should include messages on 2024-01-15 and 2024-01-16
    assert!(filtered.iter().any(|m| m.sender == "Alice"));
    assert!(filtered.iter().any(|m| m.sender == "Bob"));
    // Message on 2024-01-17 should be excluded
    assert!(!filtered.iter().any(|m| m.sender == "Charlie"));
}

#[test]
fn test_filter_empty_result() {
    let msg = Message::with_metadata(
        "Alice",
        "Hello",
        Some(Utc.with_ymd_and_hms(2024, 1, 15, 0, 0, 0).unwrap()),
        None,
        None,
        None,
    );

    let messages = vec![msg];
    let filter = FilterConfig::new().after_date("2025-01-01").unwrap();
    let filtered = chatpack::core::filter::apply_filters(messages, &filter);
    assert_eq!(filtered, []);
}

#[test]
fn test_filter_user_case_insensitive() {
    let messages = vec![
        Message::new("Alice", "Hello"),
        Message::new("ALICE", "World"),
        Message::new("alice", "Test"),
        Message::new("Bob", "Hi"),
    ];

    let filter = FilterConfig::new().with_user("alice".to_string());
    let filtered = chatpack::core::filter::apply_filters(messages, &filter);

    // Should match all case variations of "alice"
    assert_eq!(filtered.len(), 3);
    assert!(filtered.iter().all(|m| m.sender.to_lowercase() == "alice"));
}

// =========================================================================
// Merge edge cases
// =========================================================================

#[test]
fn test_merge_empty_vector() {
    let empty: Vec<Message> = vec![];
    let result = merge_consecutive(empty);
    assert_eq!(result, []);
}

#[test]
fn test_merge_single_message() {
    let single = vec![Message::new("Alice", "Hello")];
    let result = merge_consecutive(single);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].sender, "Alice");
    assert_eq!(result[0].content, "Hello");
}

#[test]
fn test_merge_all_same_sender() {
    let messages = vec![
        Message::new("Alice", "Hello"),
        Message::new("Alice", "World"),
        Message::new("Alice", "!"),
    ];

    let result = merge_consecutive(messages);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].sender, "Alice");
    assert!(result[0].content.contains("Hello"));
    assert!(result[0].content.contains("World"));
    assert!(result[0].content.contains('!'));
}

#[test]
fn test_merge_all_different_senders() {
    let messages = vec![
        Message::new("Alice", "Hello"),
        Message::new("Bob", "World"),
        Message::new("Charlie", "!"),
    ];

    let result = merge_consecutive(messages);
    assert_eq!(result.len(), 3);
}

#[test]
fn test_merge_preserves_first_message_metadata() {
    let ts1 = Utc.with_ymd_and_hms(2024, 1, 15, 10, 0, 0).unwrap();
    let ts2 = Utc.with_ymd_and_hms(2024, 1, 15, 10, 1, 0).unwrap();

    let messages = vec![
        Message::with_metadata("Alice", "Hello", Some(ts1), Some(1), None, None),
        Message::with_metadata("Alice", "World", Some(ts2), Some(2), None, None),
    ];

    let result = merge_consecutive(messages);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].timestamp, Some(ts1));
    assert_eq!(result[0].id, Some(1));
}

#[test]
fn test_merge_with_empty_content() {
    let messages = vec![
        Message::new("Alice", "Hello"),
        Message::new("Alice", ""),
        Message::new("Alice", "World"),
    ];

    let result = merge_consecutive(messages);
    assert_eq!(result.len(), 1);
}

// =========================================================================
// Output config edge cases
// =========================================================================

#[test]
fn test_output_config_all_disabled() {
    let config = OutputConfig::new();
    assert!(!config.include_timestamps);
    assert!(!config.include_ids);
    assert!(!config.include_replies);
    assert!(!config.include_edited);
}

#[test]
fn test_output_config_all_enabled() {
    let config = OutputConfig::new()
        .with_timestamps()
        .with_ids()
        .with_replies()
        .with_edited();
    assert!(config.include_timestamps);
    assert!(config.include_ids);
    assert!(config.include_replies);
    assert!(config.include_edited);
}

#[test]
fn test_output_config_full() {
    let config = OutputConfig::all();
    assert!(config.include_timestamps);
    assert!(config.include_ids);
    assert!(config.include_replies);
    assert!(config.include_edited);
}

// =========================================================================
// CSV output edge cases
// =========================================================================

#[cfg(feature = "csv-output")]
#[test]
fn test_csv_escaping_special_chars() {
    use chatpack::core::output::to_csv;

    let messages = vec![
        Message::new("Alice", "Hello, World"),   // Comma
        Message::new("Bob", "Say \"Hi\""),       // Quotes
        Message::new("Charlie", "Line1\nLine2"), // Newline
        Message::new("David", "Semi;colon"),     // Semicolon (delimiter)
    ];

    let config = OutputConfig::new();
    let csv = to_csv(&messages, &config).expect("CSV generation failed");

    // Verify the CSV can be generated without errors
    assert_ne!(csv, "");
    assert!(csv.contains("Alice"));
    assert!(csv.contains("Bob"));
}

// =========================================================================
// JSON output edge cases
// =========================================================================

#[cfg(feature = "json-output")]
#[test]
fn test_json_escaping_special_chars() {
    use chatpack::core::output::to_json;

    let messages = vec![
        Message::new("User", "Quote: \"test\""),
        Message::new("User", "Backslash: \\"),
        Message::new("User", "Tab: \t"),
        Message::new("User", "Newline: \n"),
        Message::new("User", "Control: \x01\x02"),
    ];

    let config = OutputConfig::new();
    let json = to_json(&messages, &config).expect("JSON generation failed");

    // Verify valid JSON
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("Invalid JSON");
    assert!(parsed.is_array());
}

#[cfg(feature = "json-output")]
#[test]
fn test_jsonl_each_line_valid() {
    use chatpack::core::output::to_jsonl;

    let messages = vec![Message::new("Alice", "Hello"), Message::new("Bob", "World")];

    let config = OutputConfig::new();
    let jsonl = to_jsonl(&messages, &config).expect("JSONL generation failed");

    // Each line should be valid JSON
    for line in jsonl.lines() {
        if !line.is_empty() {
            let _: serde_json::Value = serde_json::from_str(line).expect("Invalid JSON line");
        }
    }
}

// =========================================================================
// Message builder pattern tests
// =========================================================================

#[test]
fn test_message_builder_with_all_metadata() {
    let ts = Utc.with_ymd_and_hms(2024, 1, 15, 10, 0, 0).unwrap();
    let edited = Utc.with_ymd_and_hms(2024, 1, 15, 10, 5, 0).unwrap();

    let msg = Message::new("Alice", "Hello")
        .with_timestamp(ts)
        .with_id(123)
        .with_reply_to(100)
        .with_edited(edited);

    assert_eq!(msg.sender, "Alice");
    assert_eq!(msg.content, "Hello");
    assert_eq!(msg.timestamp, Some(ts));
    assert_eq!(msg.id, Some(123));
    assert_eq!(msg.reply_to, Some(100));
    assert_eq!(msg.edited, Some(edited));
}

// =========================================================================
// Serde roundtrip tests
// =========================================================================

#[test]
fn test_message_serde_roundtrip_basic() {
    let msg = Message::new("Alice", "Hello");
    let json = serde_json::to_string(&msg).expect("serialize");
    let parsed: Message = serde_json::from_str(&json).expect("deserialize");

    assert_eq!(msg.sender, parsed.sender);
    assert_eq!(msg.content, parsed.content);
}

#[test]
fn test_message_serde_roundtrip_with_metadata() {
    let ts = Utc.with_ymd_and_hms(2024, 1, 15, 10, 0, 0).unwrap();
    let msg = Message::with_metadata("Alice", "Hello", Some(ts), Some(123), Some(100), None);

    let json = serde_json::to_string(&msg).expect("serialize");
    let parsed: Message = serde_json::from_str(&json).expect("deserialize");

    assert_eq!(msg.sender, parsed.sender);
    assert_eq!(msg.content, parsed.content);
    assert_eq!(msg.timestamp, parsed.timestamp);
    assert_eq!(msg.id, parsed.id);
    assert_eq!(msg.reply_to, parsed.reply_to);
}

#[test]
fn test_message_serde_ignores_unknown_fields() {
    let json = r#"{"sender": "Alice", "content": "Hello", "unknown_field": 123}"#;
    let parsed: Message = serde_json::from_str(json).expect("deserialize");

    assert_eq!(parsed.sender, "Alice");
    assert_eq!(parsed.content, "Hello");
}

// =========================================================================
// Message default tests
// =========================================================================

#[test]
fn test_message_default() {
    let msg = Message::default();
    assert_eq!(msg.sender, "");
    assert_eq!(msg.content, "");
    assert!(msg.timestamp.is_none());
    assert!(msg.id.is_none());
    assert!(msg.reply_to.is_none());
    assert!(msg.edited.is_none());
}
