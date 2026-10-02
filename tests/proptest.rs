//! Property-based tests for chatpack using proptest.
//!
//! These tests generate random inputs to find edge cases and verify invariants.
//! Covers all major parsing functions, transformations, and output formats.

use proptest::prelude::*;
use serde_json::{Value, json};

use chatpack::core::output::{to_csv, to_json, to_jsonl};
use chatpack::core::{FilterConfig, Message, OutputConfig, apply_filters, merge_consecutive};
use chatpack::parsing::discord::{
    DiscordAttachment, DiscordAuthor, DiscordRawMessage, DiscordReference, DiscordSticker,
    parse_discord_message,
};
use chatpack::parsing::instagram::{
    InstagramRawMessage, InstagramShare, fix_mojibake_encoding, parse_instagram_message,
    parse_instagram_message_owned,
};
use chatpack::parsing::telegram::{
    TelegramRawMessage, extract_telegram_text, parse_telegram_message, parse_unix_timestamp,
};
use chatpack::parsing::whatsapp::{
    DateFormat, detect_whatsapp_format, is_whatsapp_system_message, parse_whatsapp_timestamp,
};

// =============================================================================
// STRATEGY DEFINITIONS
// =============================================================================

/// Generate a random Message using fast strategies (no regex!)
fn arb_message() -> impl Strategy<Value = Message> {
    (
        // Fast: select from predefined senders
        prop::sample::select(vec![
            "Alice".to_string(),
            "Bob".to_string(),
            "Charlie".to_string(),
            "User123".to_string(),
            "Иван".to_string(),
            "Test".to_string(),
            "日本語".to_string(),
            "مستخدم".to_string(),
        ]),
        // Fast: select from predefined contents
        prop::sample::select(vec![
            "Hello".to_string(),
            "Hi there!".to_string(),
            "How are you?".to_string(),
            "Good morning".to_string(),
            "Test message 123".to_string(),
            "Привет мир".to_string(),
            String::new(),
            "   ".to_string(),
            "Special;chars\"here\nnewline".to_string(),
            "🎉🔥💀 emoji".to_string(),
            "日本語テスト".to_string(),
            "مرحبا بالعالم".to_string(),
            "Mixed Тест 日本 🎉".to_string(),
            "\t\n\r special whitespace".to_string(),
            "a]b[c{d}e".to_string(),
        ]),
    )
        .prop_map(|(sender, content)| Message {
            sender,
            content,
            timestamp: None,
            id: None,
            reply_to: None,
            reply_to_sender: None,
            reply_to_topic: None,
            edited: None,
        })
}

/// Generate a Message with optional timestamp
fn arb_message_with_timestamp() -> impl Strategy<Value = Message> {
    (
        prop::sample::select(vec!["Alice", "Bob", "Charlie", "User"]),
        prop::sample::select(vec!["Hello", "Test", "Message", ""]),
        prop::option::of(1700000000i64..1800000000i64),
    )
        .prop_map(|(sender, content, ts_opt)| {
            let mut msg = Message::new(sender, content);
            if let Some(ts) = ts_opt {
                msg.timestamp = chrono::DateTime::from_timestamp(ts, 0);
            }
            msg
        })
}

/// Generate a vector of random messages
fn arb_messages(max_len: usize) -> impl Strategy<Value = Vec<Message>> {
    prop::collection::vec(arb_message(), 0..max_len)
}

/// Generate messages with timestamps for filter testing
fn arb_messages_with_ts(max_len: usize) -> impl Strategy<Value = Vec<Message>> {
    prop::collection::vec(arb_message_with_timestamp(), 0..max_len)
}

/// Generate arbitrary JSON values for Telegram text extraction
fn arb_telegram_text_value() -> impl Strategy<Value = Value> {
    prop_oneof![
        // Simple string
        prop::sample::select(vec![
            json!("Hello"),
            json!("Test message"),
            json!("Привет мир"),
            json!("🎉 emoji"),
            json!(""),
            json!("   "),
        ]),
        // Array with strings
        prop::sample::select(vec![
            json!(["Hello", " ", "World"]),
            json!(["Part1", "Part2", "Part3"]),
            json!([]),
        ]),
        // Array with objects (links, mentions, etc.)
        prop::sample::select(vec![
            json!(["Check: ", {"type": "link", "text": "https://example.com"}]),
            json!([{"type": "mention", "text": "@user"}, " said hello"]),
            json!(["Mixed ", {"type": "bold", "text": "bold"}, " text"]),
        ]),
        // Complex nested
        prop::sample::select(vec![
            json!(["A", {"type": "link", "text": "B"}, "C", {"type": "mention", "text": "D"}]),
            json!([{"type": "unknown"}, "text"]), // Object without text field
        ]),
        // Edge cases
        prop::sample::select(vec![
            json!(null),
            json!(123),
            json!(true),
            json!({"text": "object at root"}),
        ]),
    ]
}

/// Generate Unix timestamp strings for testing
fn arb_unix_timestamp_str() -> impl Strategy<Value = String> {
    prop_oneof![
        // Valid timestamps
        (1000000000i64..2000000000i64).prop_map(|ts| ts.to_string()),
        // Edge cases
        Just("0".to_string()),
        Just("1".to_string()),
        Just("-1".to_string()),
        Just("9999999999".to_string()),
        // Invalid
        Just(String::new()),
        Just("not_a_number".to_string()),
        Just("12.34".to_string()),
        Just("123abc".to_string()),
    ]
}

/// Generate WhatsApp format test lines
fn arb_whatsapp_line() -> impl Strategy<Value = String> {
    prop_oneof![
        // US format: [M/D/YY, H:MM:SS AM/PM]
        Just("[1/15/24, 10:30:45 AM] Alice: Hello".to_string()),
        Just("[12/31/23, 11:59:59 PM] Bob: Bye".to_string()),
        // EU Dot Bracketed: [DD.MM.YY, HH:MM:SS]
        Just("[15.01.24, 10:30:45] Alice: Hello".to_string()),
        Just("[31.12.23, 23:59:59] Bob: Bye".to_string()),
        // EU Dot No Bracket: DD.MM.YYYY, HH:MM -
        Just("15.01.2024, 10:30 - Alice: Hello".to_string()),
        Just("26.10.2025, 20:40 - Bob: Test".to_string()),
        // EU Slash: DD/MM/YYYY, HH:MM -
        Just("15/01/2024, 10:30 - Alice: Hello".to_string()),
        Just("31/12/2023, 23:59 - Bob: Bye".to_string()),
        // EU Slash Bracketed: [DD/MM/YYYY, HH:MM:SS]
        Just("[15/01/2024, 10:30:45] Alice: Hello".to_string()),
        // Invalid/continuation lines
        Just("This is a continuation".to_string()),
        Just("No date format here".to_string()),
        Just(String::new()),
    ]
}

/// Generate sender names for system message testing
fn arb_sender() -> impl Strategy<Value = String> {
    prop::sample::select(vec![
        "Alice".to_string(),
        "Bob".to_string(),
        "WhatsApp".to_string(),
        "System".to_string(),
        String::new(),
        "   ".to_string(),
        "Иван".to_string(),
        "user123".to_string(),
    ])
}

/// Generate content for system message testing
fn arb_content_for_system_check() -> impl Strategy<Value = String> {
    prop::sample::select(vec![
        "created group \"Test\"".to_string(),
        "added Charlie to the group".to_string(),
        "removed Dave from the group".to_string(),
        "changed the subject to \"New Name\"".to_string(),
        "security code changed".to_string(),
        "You're now an admin".to_string(),
        "Сообщения и звонки защищены сквозным шифрованием".to_string(),
        "создал(а) группу".to_string(),
    ])
}

/// Generate strings for mojibake testing
fn arb_mojibake_input() -> impl Strategy<Value = String> {
    prop::sample::select(vec![
        // ASCII (should pass through)
        "Hello World".to_string(),
        "Test 123".to_string(),
        String::new(),
        // Non-ASCII that might be mojibake
        "Привет".to_string(),
        "日本語".to_string(),
        "🎉🔥".to_string(),
        "مرحبا".to_string(),
        // Mixed
        "Hello Привет 日本".to_string(),
    ])
}

/// Generate Discord messages for testing
fn arb_discord_raw_message() -> impl Strategy<Value = DiscordRawMessage> {
    (
        prop::sample::select(vec!["123", "456789", "999999999999999999"]),
        prop::sample::select(vec![
            "2024-01-15T10:30:00+00:00",
            "2024-12-31T23:59:59+00:00",
            "2020-01-01T00:00:00Z",
        ]),
        prop::option::of(prop::sample::select(vec![
            "2024-01-15T11:00:00+00:00",
            "2024-12-31T23:59:59+00:00",
        ])),
        prop::sample::select(vec![
            "Hello".to_string(),
            "Test message".to_string(),
            String::new(),
            "   ".to_string(),
            "Привет 🎉".to_string(),
        ]),
        prop::sample::select(vec!["alice", "bob123", "Тест"]),
        prop::option::of(prop::sample::select(vec!["Alice", "Bob Display", "Ник"])),
        prop::option::of(prop::sample::select(vec!["111", "222"])),
        prop::bool::ANY,
        prop::bool::ANY,
    )
        .prop_map(
            |(id, ts, ts_edited, content, name, nickname, ref_id, has_attach, has_sticker)| {
                DiscordRawMessage {
                    id: id.to_string(),
                    timestamp: ts.to_string(),
                    timestamp_edited: ts_edited.map(|s| s.to_string()),
                    content,
                    author: DiscordAuthor {
                        name: name.to_string(),
                        nickname: nickname.map(|s| s.to_string()),
                    },
                    reference: ref_id.map(|id| DiscordReference {
                        message_id: Some(id.to_string()),
                    }),
                    attachments: if has_attach {
                        Some(vec![DiscordAttachment {
                            file_name: "test.png".to_string(),
                        }])
                    } else {
                        None
                    },
                    stickers: if has_sticker {
                        Some(vec![DiscordSticker {
                            name: "TestSticker".to_string(),
                        }])
                    } else {
                        None
                    },
                }
            },
        )
}

/// Generate Instagram raw messages for testing
fn arb_instagram_raw_message() -> impl Strategy<Value = InstagramRawMessage> {
    (
        prop::sample::select(vec!["user_one", "user_two", "Тест", "日本"]),
        1700000000000i64..1800000000000i64,
        prop::option::of(prop::sample::select(vec![
            "Hello!".to_string(),
            "Привет".to_string(),
            String::new(),
            "   ".to_string(),
            "🎉 emoji".to_string(),
        ])),
        prop::bool::ANY,
    )
        .prop_map(|(sender, ts, content, has_share)| InstagramRawMessage {
            sender_name: sender.to_string(),
            timestamp_ms: ts,
            content,
            share: if has_share {
                Some(InstagramShare {
                    share_text: Some("Shared content".to_string()),
                    link: Some("https://example.com".to_string()),
                })
            } else {
                None
            },
            photos: None,
            videos: None,
            audio_files: None,
        })
}

/// Generate date strings for filter testing
fn arb_date_string() -> impl Strategy<Value = String> {
    prop_oneof![
        // Valid YYYY-MM-DD
        Just("2024-01-15".to_string()),
        Just("2024-12-31".to_string()),
        Just("2000-01-01".to_string()),
        Just("2099-12-31".to_string()),
        // Leap year
        Just("2024-02-29".to_string()),
        // Invalid formats
        Just("01-15-2024".to_string()),
        Just("15/01/2024".to_string()),
        Just("2024/01/15".to_string()),
        Just("not-a-date".to_string()),
        Just(String::new()),
        // Invalid dates
        Just("2023-02-29".to_string()), // Not a leap year
        Just("2024-13-01".to_string()), // Invalid month
        Just("2024-01-32".to_string()), // Invalid day
    ]
}

// =============================================================================
// MERGE PROPERTIES
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    /// Merge never increases message count
    #[test]
    fn merge_never_increases_count(messages in arb_messages(30)) {
        let original_len = messages.len();
        let merged = merge_consecutive(messages);
        prop_assert!(merged.len() <= original_len);
    }

    /// Empty input produces empty output
    #[test]
    fn merge_empty_is_empty(_dummy in Just(())) {
        let result = merge_consecutive(vec![]);
        prop_assert!(result.is_empty());
    }

    /// Single message stays single
    #[test]
    fn merge_single_stays_single(msg in arb_message()) {
        let result = merge_consecutive(vec![msg]);
        prop_assert_eq!(result.len(), 1);
    }

    /// Merge with same sender produces exactly one message
    #[test]
    fn merge_same_sender_produces_one(n in 1usize..10) {
        let messages: Vec<Message> = (0..n)
            .map(|i| Message {
                sender: "Alice".to_string(),
                content: format!("Message {}", i),
                timestamp: None,
                id: None,
                reply_to: None,
                reply_to_sender: None,
                reply_to_topic: None,
                edited: None,
            })
            .collect();
        let merged = merge_consecutive(messages);
        prop_assert_eq!(merged.len(), 1);
    }

    /// Merge preserves total content
    #[test]
    fn merge_preserves_content_parts(n in 1usize..5) {
        let messages: Vec<Message> = (0..n)
            .map(|i| Message {
                sender: "Alice".to_string(),
                content: format!("part{}", i),
                timestamp: None,
                id: None,
                reply_to: None,
                reply_to_sender: None,
                reply_to_topic: None,
                edited: None,
            })
            .collect();
        let merged = merge_consecutive(messages);
        for i in 0..n {
            let expected = format!("part{}", i);
            prop_assert!(merged[0].content.contains(&expected), "Missing part{}", i);
        }
    }

    /// Merge is idempotent: merge(merge(x)) == merge(x)
    #[test]
    fn merge_is_idempotent(messages in arb_messages(20)) {
        let first_merge = merge_consecutive(messages.clone());
        let second_merge = merge_consecutive(first_merge.clone());

        prop_assert_eq!(first_merge.len(), second_merge.len());
        for (m1, m2) in first_merge.iter().zip(second_merge.iter()) {
            prop_assert_eq!(&m1.sender, &m2.sender);
            prop_assert_eq!(&m1.content, &m2.content);
        }
    }

    /// Alternating senders produce same count
    #[test]
    fn merge_alternating_preserves_count(n in 1usize..10) {
        let messages: Vec<Message> = (0..n)
            .map(|i| Message {
                sender: if i % 2 == 0 { "Alice" } else { "Bob" }.to_string(),
                content: format!("Msg {}", i),
                timestamp: None,
                id: None,
                reply_to: None,
                reply_to_sender: None,
                reply_to_topic: None,
                edited: None,
            })
            .collect();
        let merged = merge_consecutive(messages.clone());
        prop_assert_eq!(merged.len(), messages.len());
    }
}

// =============================================================================
// FILTER PROPERTIES
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    /// Filter never increases message count
    #[test]
    fn filter_never_increases_count(messages in arb_messages(30)) {
        let original_len = messages.len();
        let config = FilterConfig::new();
        let filtered = apply_filters(messages, &config);
        prop_assert!(filtered.len() <= original_len);
    }

    /// No filter means passthrough
    #[test]
    fn no_filter_is_passthrough(messages in arb_messages(30)) {
        let original_len = messages.len();
        let config = FilterConfig::new();
        let filtered = apply_filters(messages, &config);
        prop_assert_eq!(filtered.len(), original_len);
    }

    /// User filter only keeps matching users (case insensitive)
    #[test]
    fn user_filter_only_keeps_matching(messages in arb_messages(30)) {
        let config = FilterConfig::new().with_user("Alice".to_string());
        let filtered = apply_filters(messages, &config);

        for msg in &filtered {
            prop_assert!(
                msg.sender.eq_ignore_ascii_case("Alice"),
                "Found non-matching sender: {}", msg.sender
            );
        }
    }

    /// Filter results are always subset of input
    #[test]
    fn filter_is_subset(messages in arb_messages_with_ts(20)) {
        let config = FilterConfig::new().with_user("Alice".to_string());
        let original_senders: Vec<_> = messages.iter().map(|m| m.sender.clone()).collect();
        let filtered = apply_filters(messages, &config);

        for msg in &filtered {
            prop_assert!(original_senders.contains(&msg.sender));
        }
    }

    /// Filter with non-existent user returns empty
    #[test]
    fn filter_nonexistent_user_empty(messages in arb_messages(20)) {
        let config = FilterConfig::new().with_user("NonExistentUser12345".to_string());
        let filtered = apply_filters(messages, &config);
        prop_assert!(filtered.is_empty());
    }

    /// Date filter excludes messages without timestamps
    #[test]
    fn date_filter_excludes_no_timestamp(messages in arb_messages(20)) {
        if let Ok(config) = FilterConfig::new().after_date("2024-01-01") {
            let filtered = apply_filters(messages, &config);
            // All filtered messages should have timestamps (or be empty)
            for msg in &filtered {
                prop_assert!(msg.timestamp.is_some() || filtered.is_empty());
            }
        }
    }
}

// =============================================================================
// ROBUSTNESS PROPERTIES
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(300))]

    /// Merge never panics on any input
    #[test]
    fn merge_never_panics(messages in arb_messages(50)) {
        let _ = merge_consecutive(messages);
    }

    /// Filter never panics on any input
    #[test]
    fn filter_never_panics(messages in arb_messages(50)) {
        let config = FilterConfig::new().with_user("Test".to_string());
        let _ = apply_filters(messages, &config);
    }

    /// Messages with special characters don't break merge
    #[test]
    fn special_chars_dont_break_merge(sender in prop::sample::select(vec!["A", "B", "C"])) {
        let msg = Message {
            sender: sender.to_string(),
            content: "test;with\"special\nchars\ttab\r\n\0null".to_string(),
            timestamp: None,
            id: None,
            reply_to: None,
            reply_to_sender: None,
            reply_to_topic: None,
            edited: None,
        };
        let _ = merge_consecutive(vec![msg.clone(), msg]);
    }

    /// Unicode content is handled correctly
    #[test]
    fn unicode_content_preserved(idx in 0usize..7) {
        let contents = [
            "Привет",
            "こんにちは",
            "مرحبا",
            "🎉🔥💀",
            "Mixed Тест 日本",
            "👨‍👩‍👧‍👦",  // ZWJ sequence
            "e\u{0301}", // Combining accent
        ];
        let content = contents[idx].to_string();
        let msg = Message {
            sender: "User".to_string(),
            content: content.clone(),
            timestamp: None,
            id: None,
            reply_to: None,
            reply_to_sender: None,
            reply_to_topic: None,
            edited: None,
        };
        let merged = merge_consecutive(vec![msg]);
        prop_assert_eq!(&merged[0].content, &content);
    }
}

// =============================================================================
// SERDE ROUNDTRIP PROPERTIES
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    /// Message serialization roundtrip
    #[test]
    fn message_serde_roundtrip(msg in arb_message()) {
        let json = serde_json::to_string(&msg).expect("serialize");
        let parsed: Message = serde_json::from_str(&json).expect("deserialize");
        prop_assert_eq!(msg.sender, parsed.sender);
        prop_assert_eq!(msg.content, parsed.content);
    }

    /// Message with all fields roundtrip
    #[test]
    fn message_full_serde_roundtrip(
        sender in prop::sample::select(vec!["Alice", "Bob"]),
        content in prop::sample::select(vec!["Hello", "Test"]),
        ts in 1700000000i64..1800000000i64,
        id in 1u64..1000u64,
        reply in prop::option::of(1u64..1000u64)
    ) {
        let msg = Message {
            sender: sender.to_string(),
            content: content.to_string(),
            timestamp: chrono::DateTime::from_timestamp(ts, 0),
            id: Some(id),
            reply_to: reply,
            reply_to_sender: None,
            reply_to_topic: None,
            edited: None,
        };

        let json = serde_json::to_string(&msg).expect("serialize");
        let parsed: Message = serde_json::from_str(&json).expect("deserialize");

        prop_assert_eq!(msg.sender, parsed.sender);
        prop_assert_eq!(msg.content, parsed.content);
        prop_assert_eq!(msg.id, parsed.id);
        prop_assert_eq!(msg.reply_to, parsed.reply_to);
    }
}

// =============================================================================
// TELEGRAM PARSING PROPERTIES
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    /// extract_telegram_text never panics
    #[test]
    fn telegram_extract_never_panics(value in arb_telegram_text_value()) {
        let _ = extract_telegram_text(&value);
    }

    /// extract_telegram_text returns valid UTF-8
    #[test]
    fn telegram_extract_valid_utf8(value in arb_telegram_text_value()) {
        let result = extract_telegram_text(&value);
        // Result is String, so it's always valid UTF-8
        prop_assert!(result.len() <= result.len()); // trivial, ensures no panic
    }

    /// Simple string extraction preserves content
    #[test]
    fn telegram_extract_string_identity(s in "[a-zA-Z0-9 ]{0,50}") {
        let value = json!(s);
        let result = extract_telegram_text(&value);
        prop_assert_eq!(result, s);
    }

    /// Array extraction concatenates all strings
    #[test]
    fn telegram_extract_array_concat(parts in prop::collection::vec("[a-zA-Z]{1,10}", 1..5)) {
        let value = Value::Array(parts.iter().map(|s| json!(s)).collect());
        let result = extract_telegram_text(&value);
        let expected: String = parts.concat();
        prop_assert_eq!(result, expected);
    }

    /// parse_unix_timestamp never panics
    #[test]
    fn telegram_parse_ts_never_panics(ts_str in arb_unix_timestamp_str()) {
        let _ = parse_unix_timestamp(&ts_str);
    }

    /// Valid timestamps parse successfully
    #[test]
    fn telegram_valid_ts_parses(ts in 1000000000i64..2000000000i64) {
        let ts_str = ts.to_string();
        let result = parse_unix_timestamp(&ts_str);
        prop_assert!(result.is_some());
        prop_assert_eq!(result.unwrap().timestamp(), ts);
    }

    /// parse_telegram_message never panics
    #[test]
    fn telegram_parse_message_never_panics(
        msg_type in prop::sample::select(vec!["message", "service", "other"]),
        sender in prop::option::of(prop::sample::select(vec!["Alice", "Bob"])),
        text_value in arb_telegram_text_value(),
        ts in prop::option::of(1700000000i64..1800000000i64)
    ) {
        let msg = TelegramRawMessage {
            id: Some(123),
            msg_type: msg_type.to_string(),
            date_unixtime: ts.map(|t| t.to_string()),
            from: sender.map(|s| s.to_string()),
            from_id: None,
            action: None,
            title: None,
            text: Some(text_value),
            reply_to_message_id: None,
            edited_unixtime: None,
        };
        let _ = parse_telegram_message(&msg);
    }

    /// Non-message types are skipped
    #[test]
    fn telegram_skip_service_messages(
        msg_type in prop::sample::select(vec!["service", "action", "unknown"])
    ) {
        let msg = TelegramRawMessage {
            id: Some(123),
            msg_type: msg_type.to_string(),
            date_unixtime: Some("1700000000".to_string()),
            from: Some("Alice".to_string()),
            from_id: None,
            action: None,
            title: None,
            text: Some(json!("Hello")),
            reply_to_message_id: None,
            edited_unixtime: None,
        };
        let result = parse_telegram_message(&msg);
        prop_assert!(result.is_none());
    }
}

// =============================================================================
// WHATSAPP PARSING PROPERTIES
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    /// detect_whatsapp_format never panics
    #[test]
    fn whatsapp_detect_never_panics(lines in prop::collection::vec(arb_whatsapp_line(), 0..20)) {
        let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
        let _ = detect_whatsapp_format(&refs);
    }

    /// Empty input returns None
    #[test]
    fn whatsapp_detect_empty_is_none(_dummy in Just(())) {
        let result = detect_whatsapp_format(&[]);
        prop_assert!(result.is_none());
    }

    /// Detection is deterministic
    #[test]
    fn whatsapp_detect_deterministic(lines in prop::collection::vec(arb_whatsapp_line(), 1..10)) {
        let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
        let result1 = detect_whatsapp_format(&refs);
        let result2 = detect_whatsapp_format(&refs);
        prop_assert_eq!(result1, result2);
    }

    /// US format lines detected correctly
    #[test]
    fn whatsapp_detect_us_format(_dummy in Just(())) {
        let lines = vec![
            "[1/15/24, 10:30:45 AM] Alice: Hello",
            "[12/31/23, 11:59:59 PM] Bob: Bye",
        ];
        let result = detect_whatsapp_format(&lines);
        prop_assert_eq!(result, Some(DateFormat::US));
    }

    /// is_whatsapp_system_message never panics
    #[test]
    fn whatsapp_system_check_never_panics(
        sender in arb_sender(),
        content in arb_content_for_system_check()
    ) {
        let _ = is_whatsapp_system_message(&sender, &content);
    }

    /// Empty/whitespace sender is system
    #[test]
    fn whatsapp_empty_sender_is_system(content in arb_content_for_system_check()) {
        prop_assert!(is_whatsapp_system_message("", &content));
        prop_assert!(is_whatsapp_system_message("   ", &content));
    }

    /// WhatsApp/System sender is system
    #[test]
    fn whatsapp_system_sender_is_system(content in prop::sample::select(vec!["Hello", "Test"])) {
        prop_assert!(is_whatsapp_system_message("WhatsApp", content));
        prop_assert!(is_whatsapp_system_message("whatsapp", content));
        prop_assert!(is_whatsapp_system_message("System", content));
        prop_assert!(is_whatsapp_system_message("SYSTEM", content));
    }

    /// parse_whatsapp_timestamp never panics
    #[test]
    fn whatsapp_parse_ts_never_panics(
        date in prop::sample::select(vec!["1/15/24", "15.01.24", "15/01/2024", "invalid"]),
        time in prop::sample::select(vec!["10:30:45 AM", "23:59:59", "10:30", "invalid"]),
        format in prop::sample::select(vec![
            DateFormat::US,
            DateFormat::EuDotBracketed,
            DateFormat::EuSlash,
        ])
    ) {
        let _ = parse_whatsapp_timestamp(date, time, format);
    }

    /// Valid US format parses
    #[test]
    fn whatsapp_valid_us_ts_parses(
        month in 1u32..=12,
        day in 1u32..=28,
        hour in 1u32..=12
    ) {
        let date = format!("{}/{}/24", month, day);
        let time = format!("{}:30:00 AM", hour);
        let result = parse_whatsapp_timestamp(&date, &time, DateFormat::US);
        prop_assert!(result.is_some());
    }
}

// =============================================================================
// INSTAGRAM PARSING PROPERTIES
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    /// fix_mojibake_encoding never panics
    #[test]
    fn instagram_mojibake_never_panics(input in arb_mojibake_input()) {
        let _ = fix_mojibake_encoding(&input);
    }

    /// ASCII passes through unchanged
    #[test]
    fn instagram_ascii_passthrough(s in "[a-zA-Z0-9 !?.,]{0,100}") {
        let result = fix_mojibake_encoding(&s);
        prop_assert_eq!(result, s);
    }

    /// Empty string returns empty
    #[test]
    fn instagram_empty_passthrough(_dummy in Just(())) {
        let result = fix_mojibake_encoding("");
        prop_assert_eq!(result, "");
    }

    /// Output is always valid UTF-8 (String guarantees this)
    #[test]
    fn instagram_output_valid_utf8(input in arb_mojibake_input()) {
        let result = fix_mojibake_encoding(&input);
        // String is always valid UTF-8, this just ensures no panic
        // We check that the result is non-negative length (always true for String)
        let _ = result.len();
    }

    /// parse_instagram_message never panics
    #[test]
    fn instagram_parse_never_panics(msg in arb_instagram_raw_message()) {
        let _ = parse_instagram_message(&msg, false);
        let _ = parse_instagram_message(&msg, true);
    }

    /// parse_instagram_message_owned never panics
    #[test]
    fn instagram_parse_owned_never_panics(msg in arb_instagram_raw_message()) {
        let _ = parse_instagram_message_owned(msg, false);
    }

    /// Messages without content return None
    #[test]
    fn instagram_no_content_is_none(sender in prop::sample::select(vec!["user", "test"])) {
        let msg = InstagramRawMessage {
            sender_name: sender.to_string(),
            timestamp_ms: 1700000000000,
            content: None,
            share: None,
            photos: None,
            videos: None,
            audio_files: None,
        };
        let result = parse_instagram_message(&msg, false);
        prop_assert!(result.is_none());
    }

    /// Share content is used when no direct content
    #[test]
    fn instagram_share_fallback(sender in prop::sample::select(vec!["user", "test"])) {
        let msg = InstagramRawMessage {
            sender_name: sender.to_string(),
            timestamp_ms: 1700000000000,
            content: None,
            share: Some(InstagramShare {
                share_text: Some("Shared!".to_string()),
                link: None,
            }),
            photos: None,
            videos: None,
            audio_files: None,
        };
        let result = parse_instagram_message(&msg, false);
        prop_assert!(result.is_some());
        prop_assert_eq!(result.unwrap().content, "Shared!");
    }
}

// =============================================================================
// DISCORD PARSING PROPERTIES
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    /// parse_discord_message never panics
    #[test]
    fn discord_parse_never_panics(msg in arb_discord_raw_message()) {
        let _ = parse_discord_message(&msg);
    }

    /// Empty content without attachments returns None
    #[test]
    fn discord_empty_is_none(
        id in prop::sample::select(vec!["123", "456"]),
        name in prop::sample::select(vec!["alice", "bob"])
    ) {
        let msg = DiscordRawMessage {
            id: id.to_string(),
            timestamp: "2024-01-15T10:30:00+00:00".to_string(),
            timestamp_edited: None,
            content: String::new(),
            author: DiscordAuthor {
                name: name.to_string(),
                nickname: None,
            },
            reference: None,
            attachments: None,
            stickers: None,
        };
        let result = parse_discord_message(&msg);
        prop_assert!(result.is_none());
    }

    /// Nickname takes priority over name
    #[test]
    fn discord_nickname_priority(
        name in prop::sample::select(vec!["user123", "user456"]),
        nickname in prop::sample::select(vec!["Display Name", "Nick"])
    ) {
        let msg = DiscordRawMessage {
            id: "123".to_string(),
            timestamp: "2024-01-15T10:30:00+00:00".to_string(),
            timestamp_edited: None,
            content: "Test".to_string(),
            author: DiscordAuthor {
                name: name.to_string(),
                nickname: Some(nickname.to_string()),
            },
            reference: None,
            attachments: None,
            stickers: None,
        };
        let result = parse_discord_message(&msg);
        prop_assert!(result.is_some());
        prop_assert_eq!(result.unwrap().sender, nickname);
    }

    /// Attachments are appended to content
    #[test]
    fn discord_attachments_appended(filename in prop::sample::select(vec!["test.png", "file.pdf"])) {
        let msg = DiscordRawMessage {
            id: "123".to_string(),
            timestamp: "2024-01-15T10:30:00+00:00".to_string(),
            timestamp_edited: None,
            content: "Check this".to_string(),
            author: DiscordAuthor {
                name: "alice".to_string(),
                nickname: None,
            },
            reference: None,
            attachments: Some(vec![DiscordAttachment {
                file_name: filename.to_string(),
            }]),
            stickers: None,
        };
        let result = parse_discord_message(&msg);
        prop_assert!(result.is_some());
        let content = result.unwrap().content;
        let expected = format!("[Attachment: {}]", filename);
        prop_assert!(content.contains(&expected), "Missing attachment marker in content");
    }

    /// Stickers are appended to content
    #[test]
    fn discord_stickers_appended(sticker_name in prop::sample::select(vec!["Cool", "Nice"])) {
        let msg = DiscordRawMessage {
            id: "123".to_string(),
            timestamp: "2024-01-15T10:30:00+00:00".to_string(),
            timestamp_edited: None,
            content: "Look".to_string(),
            author: DiscordAuthor {
                name: "bob".to_string(),
                nickname: None,
            },
            reference: None,
            attachments: None,
            stickers: Some(vec![DiscordSticker {
                name: sticker_name.to_string(),
            }]),
        };
        let result = parse_discord_message(&msg);
        prop_assert!(result.is_some());
        let content = result.unwrap().content;
        let expected = format!("[Sticker: {}]", sticker_name);
        prop_assert!(content.contains(&expected), "Missing sticker marker in content");
    }

    /// Attachment-only message is kept
    #[test]
    fn discord_attachment_only_kept(filename in prop::sample::select(vec!["a.jpg", "b.mp4"])) {
        let msg = DiscordRawMessage {
            id: "123".to_string(),
            timestamp: "2024-01-15T10:30:00+00:00".to_string(),
            timestamp_edited: None,
            content: String::new(),
            author: DiscordAuthor {
                name: "user".to_string(),
                nickname: None,
            },
            reference: None,
            attachments: Some(vec![DiscordAttachment {
                file_name: filename.to_string(),
            }]),
            stickers: None,
        };
        let result = parse_discord_message(&msg);
        prop_assert!(result.is_some());
    }
}

// =============================================================================
// OUTPUT FORMAT PROPERTIES
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// to_csv never panics
    #[test]
    fn csv_output_never_panics(messages in arb_messages(20)) {
        let config = OutputConfig::default();
        let _ = to_csv(&messages, &config);
    }

    /// CSV output contains header
    #[test]
    fn csv_has_header(messages in arb_messages(10)) {
        let config = OutputConfig::default();
        if let Ok(csv) = to_csv(&messages, &config) {
            prop_assert!(csv.contains("Sender") && csv.contains("Content"));
        }
    }

    /// to_json never panics
    #[test]
    fn json_output_never_panics(messages in arb_messages(20)) {
        let config = OutputConfig::default();
        let _ = to_json(&messages, &config);
    }

    /// JSON output is valid JSON
    #[test]
    fn json_output_valid(messages in arb_messages(10)) {
        let config = OutputConfig::default();
        if let Ok(json_str) = to_json(&messages, &config) {
            let parsed: Result<Value, _> = serde_json::from_str(&json_str);
            prop_assert!(parsed.is_ok(), "Invalid JSON: {}", json_str);
        }
    }

    /// to_jsonl never panics
    #[test]
    fn jsonl_output_never_panics(messages in arb_messages(20)) {
        let config = OutputConfig::default();
        let _ = to_jsonl(&messages, &config);
    }

    /// JSONL output has one valid JSON per line
    #[test]
    fn jsonl_valid_per_line(messages in arb_messages(5)) {
        let config = OutputConfig::default();
        if let Ok(jsonl) = to_jsonl(&messages, &config) {
            for line in jsonl.lines() {
                if !line.is_empty() {
                    let parsed: Result<Value, _> = serde_json::from_str(line);
                    prop_assert!(parsed.is_ok(), "Invalid JSONL line: {}", line);
                }
            }
        }
    }

    /// CSV handles special characters without panic
    #[test]
    fn csv_special_chars_safe(content in prop::sample::select(vec![
        "Normal".to_string(),
        "With;semicolon".to_string(),
        "With\"quote".to_string(),
        "With\nnewline".to_string(),
        "All;\"chars\nhere".to_string(),
        "Tab\there".to_string(),
    ])) {
        let msg = Message::new("User", content);
        let config = OutputConfig::default();
        let result = to_csv(&[msg], &config);
        prop_assert!(result.is_ok());
    }
}

// =============================================================================
// DATE PARSING PROPERTIES
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// FilterConfig::after_date never panics
    #[test]
    fn filter_after_date_never_panics(date_str in arb_date_string()) {
        let _ = FilterConfig::new().after_date(&date_str);
    }

    /// FilterConfig::before_date never panics
    #[test]
    fn filter_before_date_never_panics(date_str in arb_date_string()) {
        let _ = FilterConfig::new().before_date(&date_str);
    }

    /// Valid YYYY-MM-DD parses successfully
    #[test]
    fn filter_valid_date_parses(
        year in 2000i32..2100,
        month in 1u32..=12,
        day in 1u32..=28
    ) {
        let date_str = format!("{:04}-{:02}-{:02}", year, month, day);
        let result = FilterConfig::new().after_date(&date_str);
        prop_assert!(result.is_ok(), "Failed to parse: {}", date_str);
    }

    /// Invalid format returns error
    #[test]
    fn filter_invalid_format_errors(
        format in prop::sample::select(vec![
            "01-15-2024",
            "15/01/2024",
            "2024/01/15",
            "not-a-date",
            "",
        ])
    ) {
let result = FilterConfig::new().after_date(format);
        prop_assert!(result.is_err());
    }

    /// Leap year dates handled correctly
    #[test]
    fn filter_leap_year_handled(_dummy in Just(())) {
        // 2024 is a leap year
        let valid = FilterConfig::new().after_date("2024-02-29");
        prop_assert!(valid.is_ok());

        // 2023 is not a leap year
        let invalid = FilterConfig::new().after_date("2023-02-29");
        prop_assert!(invalid.is_err());
    }
}

// =============================================================================
// EDGE CASE TESTS (NON-PROPTEST)
// =============================================================================

#[cfg(test)]
mod edge_cases {
    use super::*;

    #[test]
    fn merge_consecutive_same_sender() {
        let messages = vec![
            Message::new("Alice", "Hello"),
            Message::new("Alice", "World"),
        ];
        let merged = merge_consecutive(messages);
        assert_eq!(merged.len(), 1);
        assert!(merged[0].content.contains("Hello"));
        assert!(merged[0].content.contains("World"));
    }

    #[test]
    fn merge_alternating_senders() {
        let messages = vec![
            Message::new("Alice", "Hi"),
            Message::new("Bob", "Hey"),
            Message::new("Alice", "Bye"),
        ];
        let merged = merge_consecutive(messages);
        assert_eq!(merged.len(), 3);
    }

    #[test]
    fn filter_empty_messages() {
        let messages: Vec<Message> = vec![];
        let config = FilterConfig::new().with_user("Anyone".into());
        let filtered = apply_filters(messages, &config);
        assert_eq!(filtered, []);
    }

    #[test]
    fn merge_with_empty_content() {
        let messages = vec![
            Message::new("Alice", ""),
            Message::new("Alice", "Real message"),
        ];
        let merged = merge_consecutive(messages);
        assert_eq!(merged.len(), 1);
    }

    #[test]
    fn telegram_extract_null_value() {
        let value = json!(null);
        let result = extract_telegram_text(&value);
        assert_eq!(result, "");
    }

    #[test]
    fn telegram_extract_number_value() {
        let value = json!(12345);
        let result = extract_telegram_text(&value);
        assert_eq!(result, "");
    }

    #[test]
    fn telegram_extract_nested_array() {
        let value = json!([
            "Start ",
            {"type": "link", "text": "http://example.com"},
            " middle ",
            {"type": "bold", "text": "bold text"},
            " end"
        ]);
        let result = extract_telegram_text(&value);
        assert_eq!(result, "Start http://example.com middle bold text end");
    }

    #[test]
    fn whatsapp_all_formats_detected() {
        // US - month/day/year with AM/PM (the AM/PM is key to identifying US format)
        let us_lines = vec!["[1/15/24, 10:30:45 AM] Alice: Hello"];
        assert_eq!(detect_whatsapp_format(&us_lines), Some(DateFormat::US));

        // EU Dot Bracketed - day.month.year with dots (unique separator)
        let eu_dot = vec!["[25.01.24, 10:30:45] Alice: Hello"];
        assert_eq!(
            detect_whatsapp_format(&eu_dot),
            Some(DateFormat::EuDotBracketed)
        );

        // EU Dot No Bracket - day.month.year with " - " separator (unique format)
        let eu_dot_no = vec!["25.01.2024, 10:30 - Alice: Hello"];
        assert_eq!(
            detect_whatsapp_format(&eu_dot_no),
            Some(DateFormat::EuDotNoBracket)
        );

        // EU Slash - day/month/year with " - " separator (no brackets is key)
        let eu_slash = vec!["25/01/2024, 10:30 - Alice: Hello"];
        assert_eq!(detect_whatsapp_format(&eu_slash), Some(DateFormat::EuSlash));

        // Note: EU Slash Bracketed overlaps with US format regex, so we skip
        // testing it with a single line. In practice, detection uses voting
        // across multiple lines.
    }

    #[test]
    fn instagram_mojibake_ascii_only() {
        assert_eq!(
            fix_mojibake_encoding("Hello World 123!"),
            "Hello World 123!"
        );
    }

    #[test]
    fn discord_multiple_attachments() {
        let msg = DiscordRawMessage {
            id: "123".to_string(),
            timestamp: "2024-01-15T10:30:00+00:00".to_string(),
            timestamp_edited: None,
            content: "Files:".to_string(),
            author: DiscordAuthor {
                name: "user".to_string(),
                nickname: None,
            },
            reference: None,
            attachments: Some(vec![
                DiscordAttachment {
                    file_name: "a.png".to_string(),
                },
                DiscordAttachment {
                    file_name: "b.jpg".to_string(),
                },
            ]),
            stickers: None,
        };
        let result = parse_discord_message(&msg).unwrap();
        assert!(result.content.contains("[Attachment: a.png]"));
        assert!(result.content.contains("[Attachment: b.jpg]"));
    }

    #[test]
    fn csv_output_with_all_options() {
        let msg = Message {
            sender: "Alice".to_string(),
            content: "Hello".to_string(),
            timestamp: chrono::DateTime::from_timestamp(1700000000, 0),
            id: Some(123),
            reply_to: Some(100),
            reply_to_sender: None,
            reply_to_topic: None,
            edited: chrono::DateTime::from_timestamp(1700000100, 0),
        };

        let config = OutputConfig {
            include_timestamps: true,
            include_ids: true,
            include_replies: true,
            include_edited: true,
        };

        let csv = to_csv(&[msg], &config).unwrap();
        assert!(csv.contains("ID"));
        assert!(csv.contains("Timestamp"));
        assert!(csv.contains("Sender"));
        assert!(csv.contains("Content"));
        assert!(csv.contains("ReplyTo"));
        assert!(csv.contains("Edited"));
    }

    #[test]
    fn very_long_message() {
        let long_content = "x".repeat(100_000);
        let msg = Message::new("User", &long_content);
        let merged = merge_consecutive(vec![msg.clone()]);
        assert_eq!(merged[0].content.len(), 100_000);

        // Also test serialization
        let json = serde_json::to_string(&msg).unwrap();
        let parsed: Message = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.content.len(), 100_000);
    }

    #[test]
    fn unicode_normalization_edge_cases() {
        // Combining characters
        let combining = "e\u{0301}"; // é as e + combining acute
        let msg = Message::new("User", combining);
        let merged = merge_consecutive(vec![msg]);
        assert_eq!(merged[0].content, combining);

        // ZWJ sequences (family emoji)
        let family = "👨‍👩‍👧‍👦";
        let msg2 = Message::new("User", family);
        let merged2 = merge_consecutive(vec![msg2]);
        assert_eq!(merged2[0].content, family);
    }

    #[test]
    fn filter_date_boundaries() {
        use chrono::TimeZone;

        let messages = vec![
            Message {
                sender: "Alice".to_string(),
                content: "At start".to_string(),
                timestamp: Some(chrono::Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap()),
                id: None,
                reply_to: None,
                reply_to_sender: None,
                reply_to_topic: None,
                edited: None,
            },
            Message {
                sender: "Alice".to_string(),
                content: "At end".to_string(),
                timestamp: Some(
                    chrono::Utc
                        .with_ymd_and_hms(2024, 1, 1, 23, 59, 59)
                        .unwrap(),
                ),
                id: None,
                reply_to: None,
                reply_to_sender: None,
                reply_to_topic: None,
                edited: None,
            },
        ];

        // Filter for exactly 2024-01-01 should include both
        let config = FilterConfig::new()
            .after_date("2024-01-01")
            .unwrap()
            .before_date("2024-01-01")
            .unwrap();

        let filtered = apply_filters(messages, &config);
        assert_eq!(filtered.len(), 2);
    }
}
