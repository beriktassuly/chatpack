//! Integration tests for parsers with real files

use chatpack::core::{FilterConfig, ProcessingStats};
use chatpack::parser::{Platform, create_parser};
use chatpack::prelude::*;
use std::fs;
use std::path::Path;
use std::sync::{Once, OnceLock};
use tempfile::TempDir;

static INIT: Once = Once::new();
static FIXTURES: OnceLock<TempDir> = OnceLock::new();

fn fixtures_dir() -> String {
    FIXTURES
        .get_or_init(|| tempfile::tempdir().expect("create temporary fixtures directory"))
        .path()
        .to_string_lossy()
        .into_owned()
}

fn ensure_fixtures() {
    INIT.call_once(|| {
        let dir = fixtures_dir();
        if !Path::new(&dir).exists() {
            fs::create_dir_all(&dir).unwrap();
        }

        // Telegram: Simple with explicit timestamps and unixtime for robustness
        let telegram_simple = r#"{
  "name": "Test Chat",
  "type": "personal_chat",
  "id": 123456789,
  "messages": [
    {"id": 1, "type": "message", "date": "2024-01-15T10:30:00", "date_unixtime": "1705314600", "from": "Alice", "text": "Hello!"},
    {"id": 2, "type": "message", "date": "2024-01-15T10:31:00", "date_unixtime": "1705314660", "from": "Bob", "text": "Hi Alice!"},
    {"id": 3, "type": "message", "date": "2024-01-15T10:31:30", "date_unixtime": "1705314690", "from": "Alice", "text": "How are you?"},
    {"id": 4, "type": "message", "date": "2024-01-15T10:32:00", "date_unixtime": "1705314720", "from": "Alice", "text": "I'm doing great!"}
  ]
}"#;
        fs::write(format!("{dir}/telegram_simple.json"), telegram_simple).unwrap();

        // Telegram: Complex
        let telegram_complex = r#"{
  "name": "Complex Chat",
  "type": "personal_chat",
  "id": 987654321,
  "messages": [
    {"id": 1, "type": "message", "date": "2024-02-20T15:00:00", "date_unixtime": "1708441200", "from": "Developer", "text": [
      "Check out this link: ",
      {"type": "link", "text": "https://example.com"},
      " - it's great!"
    ]},
    {"id": 2, "type": "message", "date": "2024-02-20T15:01:00", "date_unixtime": "1708441260", "from": "Developer", "text": [
      {"type": "bold", "text": "Important:"},
      " this is ",
      {"type": "italic", "text": "formatted"},
      " text"
    ], "reply_to_message_id": 1},
    {"id": 3, "type": "message", "date": "2024-02-20T15:02:00", "date_unixtime": "1708441320", "from": "Developer", "text": "Edited message", "edited": "2024-02-20T15:05:00", "edited_unixtime": "1708441500"},
    {"id": 4, "type": "service", "date": "2024-02-20T15:03:00", "date_unixtime": "1708441380", "action": "pin_message"},
    {"id": 5, "type": "message", "date": "2024-02-20T15:04:00", "date_unixtime": "1708441440", "from": "Developer", "text": ""}
  ]
}"#;
        fs::write(format!("{dir}/telegram_complex.json"), telegram_complex).unwrap();

        // WhatsApp: iOS Bracketed Format (Reliable detection)
        let whatsapp_us = "[1/15/24, 10:30:00 AM] Alice: Hello everyone!
[1/15/24, 10:31:00 AM] Bob: Hi Alice!
[1/15/24, 10:32:00 AM] Alice: How is everyone doing?
[1/15/24, 10:32:00 AM] Alice: I hope you're all well
[1/15/24, 10:33:00 AM] Charlie: Messages and calls are end-to-end encrypted. No one outside of this chat, not even WhatsApp, can read or listen to them.
[1/15/24, 10:34:00 AM] Bob: I'm doing great!
[1/15/24, 10:35:00 AM] Alice: <Media omitted>
[1/15/24, 10:36:00 AM] Alice: Check out this link https://example.com";
        fs::write(format!("{dir}/whatsapp_us.txt"), whatsapp_us).unwrap();

        // WhatsApp: EU Format
        let whatsapp_eu = "[15.01.24, 10:30:00] Alice: Привет всем!
[15.01.24, 10:31:00] Bob: Привет!
[15.01.24, 10:32:00] Alice: Как дела?
[15.01.24, 10:33:00] Alice: Надеюсь все хорошо
[15.01.24, 10:34:00] Charlie: Сообщения и звонки защищены сквозным шифрованием.
[15.01.24, 10:35:00] Bob: Все отлично!";
        fs::write(format!("{dir}/whatsapp_eu.txt"), whatsapp_eu).unwrap();

        // Instagram: Full structure with magic_words to ensure auto-detection
        let instagram = r#"{
  "participants": [
    {
      "name": "user_one"
    },
    {
      "name": "user_two"
    }
  ],
  "messages": [
    {
      "sender_name": "user_one",
      "timestamp_ms": 1705315800000,
      "content": "Hey! How are you?",
      "is_geoblocked_for_viewer": false
    },
    {
      "sender_name": "user_two",
      "timestamp_ms": 1705315860000,
      "content": "I'm good, thanks!",
      "is_geoblocked_for_viewer": false
    },
    {
      "sender_name": "user_one",
      "timestamp_ms": 1705315920000,
      "content": "Great to hear!",
      "is_geoblocked_for_viewer": false
    },
    {
      "sender_name": "user_one",
      "timestamp_ms": 1705315950000,
      "content": "What are you up to?",
      "is_geoblocked_for_viewer": false
    },
    {
      "sender_name": "user_two",
      "timestamp_ms": 1705316000000,
      "content": "Check this shared link: https://instagram.com/p/xyz",
      "share": {
        "link": "https://instagram.com/p/xyz",
        "share_text": "Check this out"
      },
      "is_geoblocked_for_viewer": false
    },
    {
      "sender_name": "user_one",
      "timestamp_ms": 1705316060000,
      "is_geoblocked_for_viewer": false
    }
  ],
  "title": "Test Instagram Chat",
  "is_still_participant": true,
  "thread_type": "Regular",
  "thread_path": "inbox/test_123",
  "magic_words": [],
  "joinable_mode": {
    "mode": 1,
    "link": ""
  }
}"#;
        fs::write(format!("{dir}/instagram.json"), instagram).unwrap();

        // Discord JSON
        let discord_json = r#"{
  "guild": {
    "id": "123456789",
    "name": "Test Server",
    "iconUrl": null
  },
  "channel": {
    "id": "987654321",
    "type": "GuildTextChat",
    "name": "general",
    "topic": null
  },
  "messages": [
    {
      "id": "1001",
      "type": "Default",
      "timestamp": "2024-01-15T10:30:00+00:00",
      "timestampEdited": null,
      "content": "Hello Discord!",
      "author": {
        "id": "111",
        "name": "alice",
        "nickname": "Alice"
      },
      "attachments": [],
      "stickers": [],
      "embeds": []
    },
    {
      "id": "1002",
      "type": "Default",
      "timestamp": "2024-01-15T10:31:00+00:00",
      "timestampEdited": "2024-01-15T10:32:00+00:00",
      "content": "Hi Alice!",
      "author": {
        "id": "222",
        "name": "bob",
        "nickname": null
      },
      "reference": {
        "messageId": "1001"
      },
      "attachments": [
        {"fileName": "image.png"}
      ],
      "stickers": [],
      "embeds": []
    },
    {
      "id": "1003",
      "type": "Default",
      "timestamp": "2024-01-15T10:33:00+00:00",
      "timestampEdited": null,
      "content": "",
      "author": {
        "id": "111",
        "name": "alice",
        "nickname": "Alice"
      },
      "attachments": [],
      "stickers": [
        {"name": "Wave"}
      ],
      "embeds": []
    }
  ],
  "messageCount": 3
}"#;
        fs::write(format!("{dir}/discord.json"), discord_json).unwrap();

        // Discord TXT
        let discord_txt = r"==============================================================
Guild: Test Server
Channel: general
==============================================================

[1/15/2024 10:30 AM] Alice
Hello Discord!


[1/15/2024 10:31 AM] bob
Hi Alice!


{Attachments}
https://cdn.discordapp.com/attachments/123/456/image.png


[1/15/2024 10:33 AM] Alice


{Stickers}
https://cdn.discordapp.com/stickers/789.json


==============================================================
Exported 3 message(s)
==============================================================";
        fs::write(format!("{dir}/discord.txt"), discord_txt).unwrap();

        // Discord CSV
        let discord_csv = r#"AuthorID,Author,Date,Content,Attachments,Reactions
"111","Alice","2024-01-15T10:30:00+00:00","Hello Discord!","",""
"222","bob","2024-01-15T10:31:00+00:00","Hi Alice!","https://cdn.discordapp.com/attachments/123/456/image.png",""
"111","Alice","2024-01-15T10:33:00+00:00","How are you?","","""#;
        fs::write(format!("{dir}/discord.csv"), discord_csv).unwrap();
    });
}

// ============================================================================
// Discord Parser Tests
// ============================================================================

mod discord_tests {
    use super::*;

    #[test]
    fn test_parse_json() {
        ensure_fixtures();
        let parser = create_parser(Platform::Discord);
        let messages = parser
            .parse_file(&format!("{}/discord.json", fixtures_dir()))
            .unwrap();

        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0].sender, "Alice");
        assert_eq!(messages[0].content, "Hello Discord!");
        assert!(messages[0].timestamp.is_some());
        assert_eq!(messages[0].id, Some(1001));
    }

    #[test]
    fn test_parse_json_with_metadata() {
        ensure_fixtures();
        let parser = create_parser(Platform::Discord);
        let messages = parser
            .parse_file(&format!("{}/discord.json", fixtures_dir()))
            .unwrap();

        // Check reply reference
        assert_eq!(messages[1].reply_to, Some(1001));

        // Check edited timestamp
        assert!(messages[1].edited.is_some());

        // Check attachment in content
        assert!(messages[1].content.contains("[Attachment: image.png]"));

        // Check sticker in content
        assert!(messages[2].content.contains("[Sticker: Wave]"));
    }

    #[test]
    fn test_parse_json_nickname_fallback() {
        ensure_fixtures();
        let parser = create_parser(Platform::Discord);
        let messages = parser
            .parse_file(&format!("{}/discord.json", fixtures_dir()))
            .unwrap();

        // Alice has nickname
        assert_eq!(messages[0].sender, "Alice");
        // bob has no nickname, uses username
        assert_eq!(messages[1].sender, "bob");
    }

    #[test]
    fn test_parse_txt() {
        ensure_fixtures();
        let parser = create_parser(Platform::Discord);
        let messages = parser
            .parse_file(&format!("{}/discord.txt", fixtures_dir()))
            .unwrap();

        assert_ne!(messages, []);
        assert_eq!(messages[0].sender, "Alice");
        assert!(messages[0].content.contains("Hello Discord!"));
    }

    #[test]
    fn test_parse_txt_attachments() {
        ensure_fixtures();
        let parser = create_parser(Platform::Discord);
        let messages = parser
            .parse_file(&format!("{}/discord.txt", fixtures_dir()))
            .unwrap();

        let has_attachment = messages.iter().any(|m| m.content.contains("[Attachment:"));
        assert!(has_attachment);
    }

    #[test]
    fn test_parse_csv() {
        ensure_fixtures();
        let parser = create_parser(Platform::Discord);
        let messages = parser
            .parse_file(&format!("{}/discord.csv", fixtures_dir()))
            .unwrap();

        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0].sender, "Alice");
        assert_eq!(messages[0].content, "Hello Discord!");
    }

    #[test]
    fn test_parse_csv_attachments() {
        ensure_fixtures();
        let parser = create_parser(Platform::Discord);
        let messages = parser
            .parse_file(&format!("{}/discord.csv", fixtures_dir()))
            .unwrap();

        let has_attachment = messages.iter().any(|m| m.content.contains("[Attachment:"));
        assert!(has_attachment);
    }

    #[test]
    fn test_parser_name() {
        let parser = create_parser(Platform::Discord);
        assert_eq!(parser.name(), "Discord");
    }

    #[test]
    fn test_consecutive_merge() {
        ensure_fixtures();
        let parser = create_parser(Platform::Discord);
        let messages = parser
            .parse_file(&format!("{}/discord.json", fixtures_dir()))
            .unwrap();

        let original = messages.len();
        let merged = merge_consecutive(messages);

        assert!(merged.len() <= original);
    }
}

// ============================================================================
// Telegram Parser Tests
// ============================================================================

mod telegram_tests {
    use super::*;

    #[test]
    fn test_parse_simple_chat() {
        ensure_fixtures();
        let parser = create_parser(Platform::Telegram);
        let messages = parser
            .parse_file(&format!("{}/telegram_simple.json", fixtures_dir()))
            .unwrap();

        assert_eq!(messages.len(), 4);
        assert_eq!(messages[0].sender, "Alice");
        assert_eq!(messages[0].content, "Hello!");
        assert_eq!(messages[1].sender, "Bob");
        assert!(messages[0].timestamp.is_some());
    }

    #[test]
    fn test_parse_complex_chat() {
        ensure_fixtures();
        let parser = create_parser(Platform::Telegram);
        let messages = parser
            .parse_file(&format!("{}/telegram_complex.json", fixtures_dir()))
            .unwrap();

        assert!(messages.len() >= 2);
        assert!(messages[0].content.contains("https://example.com"));

        let has_reply = messages.iter().any(|m| m.reply_to.is_some());
        assert!(has_reply);

        let has_edited = messages.iter().any(|m| m.edited.is_some());
        assert!(has_edited);
    }

    #[test]
    fn test_merge_consecutive() {
        ensure_fixtures();
        let parser = create_parser(Platform::Telegram);
        let messages = parser
            .parse_file(&format!("{}/telegram_simple.json", fixtures_dir()))
            .unwrap();

        let original_count = messages.len();
        let merged = merge_consecutive(messages);

        assert!(merged.len() <= original_count);
    }

    #[test]
    fn test_parser_name() {
        let parser = create_parser(Platform::Telegram);
        assert_eq!(parser.name(), "Telegram");
    }
}

// ============================================================================
// WhatsApp Parser Tests
// ============================================================================

mod whatsapp_tests {
    use super::*;

    #[test]
    fn test_parse_us_format() {
        ensure_fixtures();
        let parser = create_parser(Platform::WhatsApp);
        let messages = parser
            .parse_file(&format!("{}/whatsapp_us.txt", fixtures_dir()))
            .unwrap();

        assert_ne!(messages, []);

        let senders: Vec<&str> = messages.iter().map(|m| m.sender.as_str()).collect();
        assert!(senders.contains(&"Alice"));
        assert!(senders.contains(&"Bob"));
    }

    #[test]
    fn test_parse_eu_format() {
        ensure_fixtures();
        let parser = create_parser(Platform::WhatsApp);
        let messages = parser
            .parse_file(&format!("{}/whatsapp_eu.txt", fixtures_dir()))
            .unwrap();

        assert_ne!(messages, []);
        let has_cyrillic = messages.iter().any(|m| m.content.contains("Привет"));
        assert!(has_cyrillic);
    }

    #[test]
    fn test_system_messages_filtered() {
        ensure_fixtures();
        let parser = create_parser(Platform::WhatsApp);
        let messages = parser
            .parse_file(&format!("{}/whatsapp_us.txt", fixtures_dir()))
            .unwrap();

        let has_system = messages.iter().any(|m| {
            m.content.contains("end-to-end encrypted") || m.content.contains("сквозным шифрованием")
        });
        assert!(!has_system);
    }

    #[test]
    fn test_media_preserved() {
        ensure_fixtures();
        let parser = create_parser(Platform::WhatsApp);
        let messages = parser
            .parse_file(&format!("{}/whatsapp_us.txt", fixtures_dir()))
            .unwrap();

        let has_media = messages.iter().any(|m| m.content.contains("Media omitted"));
        assert!(has_media);
    }

    #[test]
    fn test_parser_name() {
        let parser = create_parser(Platform::WhatsApp);
        assert_eq!(parser.name(), "WhatsApp");
    }

    #[test]
    fn test_consecutive_merge() {
        ensure_fixtures();
        let parser = create_parser(Platform::WhatsApp);
        let messages = parser
            .parse_file(&format!("{}/whatsapp_us.txt", fixtures_dir()))
            .unwrap();

        let original = messages.len();
        let merged = merge_consecutive(messages);

        assert!(merged.len() <= original);
    }
}

// ============================================================================
// Instagram Parser Tests
// ============================================================================

mod instagram_tests {
    use super::*;

    #[test]
    fn test_parse_instagram() {
        ensure_fixtures();
        let parser = create_parser(Platform::Instagram);
        let messages = parser
            .parse_file(&format!("{}/instagram.json", fixtures_dir()))
            .unwrap();

        assert_ne!(messages, []);

        let senders: Vec<&str> = messages.iter().map(|m| m.sender.as_str()).collect();
        assert!(senders.contains(&"user_one"));
        assert!(senders.contains(&"user_two"));
    }

    #[test]
    fn test_shared_links() {
        ensure_fixtures();
        let parser = create_parser(Platform::Instagram);
        let messages = parser
            .parse_file(&format!("{}/instagram.json", fixtures_dir()))
            .unwrap();

        let has_shared = messages.iter().any(|m| {
            m.content.contains("Shared") || m.content.contains("https://instagram.com/p/xyz")
        });
        assert!(has_shared, "Should contain 'Shared' or the actual link");
    }

    #[test]
    fn test_empty_content_filtered() {
        ensure_fixtures();
        let parser = create_parser(Platform::Instagram);
        let messages = parser
            .parse_file(&format!("{}/instagram.json", fixtures_dir()))
            .unwrap();

        let has_empty = messages.iter().any(|m| m.content.is_empty());
        assert!(!has_empty);
    }

    #[test]
    fn test_parser_name() {
        let parser = create_parser(Platform::Instagram);
        assert_eq!(parser.name(), "Instagram");
    }

    #[test]
    fn test_timestamps() {
        ensure_fixtures();
        let parser = create_parser(Platform::Instagram);
        let messages = parser
            .parse_file(&format!("{}/instagram.json", fixtures_dir()))
            .unwrap();

        assert!(messages.iter().all(|m| m.timestamp.is_some()));
    }
}

// ============================================================================
// Filter Tests with Real Data
// ============================================================================

mod filter_integration_tests {
    use super::*;

    #[test]
    fn test_filter_by_sender() {
        ensure_fixtures();
        let parser = create_parser(Platform::Telegram);
        let messages = parser
            .parse_file(&format!("{}/telegram_simple.json", fixtures_dir()))
            .unwrap();

        let config = FilterConfig::new().with_user("Alice".to_string());
        let filtered = apply_filters(messages, &config);

        assert!(filtered.iter().all(|m| m.sender == "Alice"));
    }

    #[test]
    fn test_filter_by_date() {
        ensure_fixtures();
        let parser = create_parser(Platform::Telegram);
        let messages = parser
            .parse_file(&format!("{}/telegram_simple.json", fixtures_dir()))
            .unwrap();

        let config = FilterConfig::new()
            .after_date("2024-01-14")
            .unwrap()
            .before_date("2024-01-16")
            .unwrap();

        let filtered = apply_filters(messages.clone(), &config);
        assert_eq!(filtered.len(), messages.len());
    }

    #[test]
    fn test_filter_excludes_outside_range() {
        ensure_fixtures();
        let parser = create_parser(Platform::Telegram);
        let messages = parser
            .parse_file(&format!("{}/telegram_simple.json", fixtures_dir()))
            .unwrap();

        let config = FilterConfig::new().before_date("2024-01-01").unwrap();

        let filtered = apply_filters(messages, &config);
        assert_eq!(filtered, []);
    }

    #[test]
    fn test_combined_filters() {
        ensure_fixtures();
        let parser = create_parser(Platform::WhatsApp);
        let messages = parser
            .parse_file(&format!("{}/whatsapp_us.txt", fixtures_dir()))
            .unwrap();

        let config = FilterConfig::new().with_user("Alice".to_string());
        let filtered = apply_filters(messages, &config);

        assert!(
            filtered
                .iter()
                .all(|m| m.sender.eq_ignore_ascii_case("Alice"))
        );
    }
}

// ============================================================================
// Processing Stats Tests
// ============================================================================

mod stats_tests {
    use super::*;

    #[test]
    fn test_stats_with_real_data() {
        ensure_fixtures();
        let parser = create_parser(Platform::Telegram);
        let messages = parser
            .parse_file(&format!("{}/telegram_simple.json", fixtures_dir()))
            .unwrap();

        let original = messages.len();
        let merged = merge_consecutive(messages);

        let stats = ProcessingStats::new(original, merged.len());

        assert_eq!(stats.original_count, original);
        assert!(stats.compression_ratio() >= 0.0);
        assert!(stats.compression_ratio() <= 100.0);
    }

    #[test]
    fn test_stats_display() {
        let stats = ProcessingStats::new(100, 80);
        let display = format!("{stats}");

        assert!(display.contains("100"));
        assert!(display.contains("80"));
    }

    #[test]
    fn test_stats_with_filtered() {
        let stats = ProcessingStats::new(100, 50).with_filtered(75);

        assert_eq!(stats.filtered_count, Some(75));
        assert_eq!(stats.messages_saved(), 25);
    }
}

// ============================================================================
// Output Config Tests
// ============================================================================

mod output_config_tests {
    use super::*;

    #[test]
    fn test_output_config_default() {
        let config = OutputConfig::new();
        assert!(!config.include_timestamps);
        assert!(!config.include_ids);
    }

    #[test]
    fn test_output_config_all() {
        let config = OutputConfig::all();
        assert!(config.include_timestamps);
        assert!(config.include_ids);
        assert!(config.include_replies);
        assert!(config.include_edited);
    }

    #[test]
    fn test_output_config_builder() {
        let config = OutputConfig::new().with_ids().with_replies();

        assert!(config.include_ids);
        assert!(config.include_replies);
    }

    #[test]
    fn test_output_config_has_any() {
        let config = OutputConfig::new().with_ids();
        assert!(config.has_any());

        let empty = OutputConfig {
            include_timestamps: false,
            include_ids: false,
            include_replies: false,
            include_edited: false,
        };
        assert!(!empty.has_any());
    }
}

// ============================================================================
// Message Tests
// ============================================================================

mod message_tests {
    use super::*;
    use chrono::{TimeZone, Utc};

    #[test]
    fn test_message_builder_chain() {
        let ts = Utc.with_ymd_and_hms(2024, 1, 15, 10, 30, 0).unwrap();
        let edit_ts = Utc.with_ymd_and_hms(2024, 1, 15, 11, 0, 0).unwrap();

        let msg = Message::new("Alice", "Hello")
            .with_id(123)
            .with_timestamp(ts)
            .with_reply_to(122)
            .with_edited(edit_ts);

        assert_eq!(msg.sender, "Alice");
        assert_eq!(msg.content, "Hello");
        assert_eq!(msg.id, Some(123));
        assert_eq!(msg.timestamp, Some(ts));
        assert_eq!(msg.reply_to, Some(122));
        assert_eq!(msg.edited, Some(edit_ts));
    }

    #[test]
    fn test_message_has_metadata() {
        let simple = Message::new("Bob", "Hi");
        assert!(!simple.has_metadata());

        let with_id = Message::new("Bob", "Hi").with_id(1);
        assert!(with_id.has_metadata());
    }

    #[test]
    fn test_message_is_empty() {
        let empty = Message::new("Alice", "");
        assert!(empty.is_empty());

        let not_empty = Message::new("Alice", "Hello");
        assert!(!not_empty.is_empty());
    }

    #[test]
    fn test_message_clone() {
        let msg = Message::new("Alice", "Hello").with_id(1);
        let cloned = msg.clone();

        assert_eq!(msg.sender, cloned.sender);
        assert_eq!(msg.content, cloned.content);
        assert_eq!(msg.id, cloned.id);
    }

    #[test]
    fn test_message_debug() {
        let msg = Message::new("Alice", "Hello");
        let debug = format!("{msg:?}");

        assert!(debug.contains("Alice"));
        assert!(debug.contains("Hello"));
    }

    #[test]
    fn test_message_partial_eq() {
        let msg1 = Message::new("Alice", "Hello");
        let msg2 = Message::new("Alice", "Hello");
        let msg3 = Message::new("Bob", "Hello");

        assert_eq!(msg1, msg2);
        assert_ne!(msg1, msg3);
    }
}

// ============================================================================
// Platform and Format Types Tests
// ============================================================================

mod platform_tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_platform_from_str_aliases() {
        assert_eq!(Platform::from_str("tg").unwrap(), Platform::Telegram);
        assert_eq!(Platform::from_str("wa").unwrap(), Platform::WhatsApp);
        assert_eq!(Platform::from_str("ig").unwrap(), Platform::Instagram);
        assert_eq!(Platform::from_str("dc").unwrap(), Platform::Discord);
        assert_eq!(Platform::from_str("telegram").unwrap(), Platform::Telegram);
        assert_eq!(Platform::from_str("whatsapp").unwrap(), Platform::WhatsApp);
        assert_eq!(
            Platform::from_str("instagram").unwrap(),
            Platform::Instagram
        );
        assert_eq!(Platform::from_str("discord").unwrap(), Platform::Discord);
    }

    #[test]
    fn test_platform_from_str_case_insensitive() {
        assert_eq!(Platform::from_str("TELEGRAM").unwrap(), Platform::Telegram);
        assert_eq!(Platform::from_str("WhatsApp").unwrap(), Platform::WhatsApp);
        assert_eq!(
            Platform::from_str("INSTAGRAM").unwrap(),
            Platform::Instagram
        );
        assert_eq!(Platform::from_str("DISCORD").unwrap(), Platform::Discord);
    }

    #[test]
    fn test_platform_from_str_error() {
        assert!(Platform::from_str("unknown").is_err());
        assert!(Platform::from_str("").is_err());
    }

    #[test]
    fn test_output_format_from_str() {
        assert_eq!(OutputFormat::from_str("json").unwrap(), OutputFormat::Json);
        assert_eq!(
            OutputFormat::from_str("jsonl").unwrap(),
            OutputFormat::Jsonl
        );
        assert_eq!(OutputFormat::from_str("csv").unwrap(), OutputFormat::Csv);
    }

    #[test]
    fn test_output_format_from_str_error() {
        assert!(OutputFormat::from_str("xml").is_err());
    }

    #[test]
    fn test_platform_default_extension() {
        assert_eq!(Platform::Telegram.default_extension(), "json");
        assert_eq!(Platform::WhatsApp.default_extension(), "txt");
        assert_eq!(Platform::Instagram.default_extension(), "json");
        assert_eq!(Platform::Discord.default_extension(), "json");
    }

    #[test]
    fn test_output_format_extension() {
        assert_eq!(OutputFormat::Json.extension(), "json");
        assert_eq!(OutputFormat::Jsonl.extension(), "jsonl");
        assert_eq!(OutputFormat::Csv.extension(), "csv");
    }

    #[test]
    fn test_output_format_mime_type() {
        assert_eq!(OutputFormat::Json.mime_type(), "application/json");
        assert_eq!(OutputFormat::Jsonl.mime_type(), "application/x-ndjson");
        assert_eq!(OutputFormat::Csv.mime_type(), "text/csv");
    }

    #[test]
    fn test_platform_all_names() {
        let names = Platform::all_names();
        assert!(names.contains(&"telegram"));
        assert!(names.contains(&"whatsapp"));
        assert!(names.contains(&"instagram"));
        assert!(names.contains(&"discord"));
    }

    #[test]
    fn test_output_format_all_names() {
        let names = OutputFormat::all_names();
        assert!(names.contains(&"json"));
        assert!(names.contains(&"jsonl"));
        assert!(names.contains(&"csv"));
    }

    #[test]
    fn test_platform_display() {
        assert_eq!(format!("{}", Platform::Telegram), "Telegram");
        assert_eq!(format!("{}", Platform::WhatsApp), "WhatsApp");
        assert_eq!(format!("{}", Platform::Instagram), "Instagram");
        assert_eq!(format!("{}", Platform::Discord), "Discord");
    }
}

// ============================================================================
// Error Handling Tests
// ============================================================================

mod error_tests {
    use super::*;

    #[test]
    fn test_parse_nonexistent_file() {
        let parser = create_parser(Platform::Telegram);
        let result = parser.parse_file("nonexistent.json");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_invalid_json() {
        ensure_fixtures();
        let dir = fixtures_dir();
        fs::write(format!("{dir}/invalid.json"), "not valid json").unwrap();

        let parser = create_parser(Platform::Telegram);
        let result = parser.parse_file(&format!("{dir}/invalid.json"));
        assert!(result.is_err());
    }

    #[test]
    fn test_filter_invalid_date() {
        let result = FilterConfig::new().after_date("not-a-date");
        assert!(result.is_err());
    }

    #[test]
    fn test_filter_error_display() {
        let err = FilterConfig::new().after_date("invalid").unwrap_err();
        let display = format!("{err}");
        // Error message format: "Invalid date 'invalid'. Expected format: YYYY-MM-DD"
        assert!(display.contains("Invalid date"));
        assert!(display.contains("invalid"));
    }
}

// ============================================================================
// Serde Tests
// ============================================================================

mod serde_tests {
    use super::*;
    use chrono::{TimeZone, Utc};

    #[test]
    fn test_message_serialize_deserialize() {
        let ts = Utc.with_ymd_and_hms(2024, 1, 15, 10, 30, 0).unwrap();
        let msg = Message::new("Alice", "Hello").with_id(1).with_timestamp(ts);

        let json = serde_json::to_string(&msg).unwrap();
        let deserialized: Message = serde_json::from_str(&json).unwrap();

        assert_eq!(msg, deserialized);
    }

    #[test]
    fn test_platform_serde() {
        let platform = Platform::Telegram;
        let json = serde_json::to_string(&platform).unwrap();
        let deserialized: Platform = serde_json::from_str(&json).unwrap();

        assert_eq!(platform, deserialized);
    }

    #[test]
    fn test_output_format_serde() {
        let format = OutputFormat::Json;
        let json = serde_json::to_string(&format).unwrap();
        let deserialized: OutputFormat = serde_json::from_str(&json).unwrap();

        assert_eq!(format, deserialized);
    }

    #[test]
    fn test_output_config_serde() {
        let config = OutputConfig::all();
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: OutputConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(config.include_timestamps, deserialized.include_timestamps);
        assert_eq!(config.include_ids, deserialized.include_ids);
    }
}
