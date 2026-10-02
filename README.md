# chatpack

**Rust library for converting chat exports into compact, LLM- and RAG-ready data.**

[![CI](https://github.com/berektassuly/chatpack/actions/workflows/ci.yml/badge.svg)](https://github.com/berektassuly/chatpack/actions/workflows/ci.yml)
[![codecov](https://codecov.io/gh/berektassuly/chatpack/branch/main/graph/badge.svg)](https://codecov.io/gh/berektassuly/chatpack)
[![Crates.io](https://img.shields.io/crates/v/chatpack.svg)](https://crates.io/crates/chatpack)
[![docs.rs](https://docs.rs/chatpack/badge.svg)](https://docs.rs/chatpack)
[![Downloads](https://img.shields.io/crates/d/chatpack.svg)](https://crates.io/crates/chatpack)

[API Docs](https://docs.rs/chatpack) |
[Export Guide](docs/EXPORT_GUIDE.md) |
[Benchmarks](docs/BENCHMARKS.md) |
[Website](https://chatpack.berektassuly.com)

## Overview

`chatpack` is the core Rust crate behind the Chatpack ecosystem. It parses chat exports from Telegram, WhatsApp, Instagram, and Discord, normalizes them into one `Message` type, and writes token-efficient CSV, JSON, or JSONL output for LLM analysis, RAG ingestion, archival, and analytics.

Raw messenger exports often spend most of their tokens on nested JSON, repeated field names, and metadata. In a real Telegram sample with 34,478 messages, CSV output reduced 11.2M raw-export tokens to about 850K tokens: **13.2x smaller**.

| Platform | Input | Notes |
|----------|-------|-------|
| Telegram | JSON | Parses Telegram Desktop `result.json`, formatted text, replies, edits, and service-message filtering |
| WhatsApp | TXT | Auto-detects US and European date formats, multiline messages, media placeholders, and common system messages |
| Instagram | JSON | Parses Meta `message_*.json` files, fixes common mojibake, and returns chronological messages |
| Discord | JSON, TXT, CSV | Supports DiscordChatExporter outputs, attachments, stickers, replies, and edited timestamps where available |

## Install

```bash
cargo add chatpack
```

Minimal builds can opt into only the parsers and writers they need:

```toml
[dependencies]
chatpack = { version = "0.7.0", default-features = false, features = ["telegram", "csv-output"] }
```

## Quick Start

```rust
use std::path::Path;

use chatpack::prelude::*;

fn main() -> Result<()> {
    let parser = create_parser(Platform::Telegram);
    let messages = parser.parse(Path::new("result.json"))?;

    let filtered = apply_filters(messages, &FilterConfig::new().with_sender("Alice"));
    let merged = merge_consecutive(filtered);

    write_to_format(
        &merged,
        "chat.jsonl",
        OutputFormat::Jsonl,
        &OutputConfig::new().with_timestamps(),
    )?;

    Ok(())
}
```

## Common Workflows

Parse from a string when the export is already in memory:

```rust
use chatpack::prelude::*;

fn main() -> Result<()> {
    let parser = create_parser(Platform::WhatsApp);
    let messages = parser.parse_str("[1/15/24, 10:30 AM] Alice: Hello")?;

    println!("Parsed {} message(s)", messages.len());
    Ok(())
}
```

Stream large files when loading the full export is not practical:

```rust
use std::path::Path;

use chatpack::prelude::*;

fn main() -> Result<()> {
    let parser = create_streaming_parser(Platform::Telegram);

    for result in parser.stream(Path::new("huge_result.json"))? {
        let message = result?;
        println!("{}: {}", message.sender, message.content);
    }

    Ok(())
}
```

Choose output based on the downstream task:

| Output | Best for | Why |
|--------|----------|-----|
| CSV | LLM context windows, spreadsheets | Most compact; sender/content only by default |
| JSONL | RAG, vector DB ingestion, streaming pipelines | One message per line |
| JSON | APIs, archival, structured post-processing | Full JSON array |

Optional metadata is controlled by `OutputConfig`:

```rust
let compact = OutputConfig::new();
let detailed = OutputConfig::all();
let timestamps_only = OutputConfig::new().with_timestamps();
```

For Telegram exports, `with_replies()` includes the referenced ID and, when
available, the referenced message's sender or the title of a referenced topic.
These appear as `ReplyTo`, `ReplyToSender`, and `ReplyToTopic` in CSV (and
`reply_to`, `reply_to_sender`, and `reply_to_topic` in JSON/JSONL). Topic
creation events are service records, so their IDs may appear as reply targets
even though they are not emitted as messages. The standard and async parsers
resolve the extra context from the complete export; streaming retains the ID.
When an AyuGram sender name is `-`, its `from_id` is used when present.

## Feature Flags

The default feature set is `full`, which enables every parser, CSV/JSON output, and streaming support.

| Feature | Description | Default |
|---------|-------------|---------|
| `full` | All parsers, outputs, and streaming | Yes |
| `telegram` | Telegram JSON parser | Yes |
| `whatsapp` | WhatsApp TXT parser | Yes |
| `instagram` | Instagram JSON parser | Yes |
| `discord` | Discord JSON/TXT/CSV parser | Yes |
| `csv-output` | CSV writer and string conversion | Yes |
| `json-output` | JSON and JSONL writers/string conversion | Yes |
| `streaming` | Native streaming parsers and progress tracking | Yes |
| `async` | Tokio-based async parser support, currently Telegram | No |

## Documentation

| Resource | Description |
|----------|-------------|
| [API Docs](https://docs.rs/chatpack) | Public Rust API, modules, traits, and examples |
| [Export Guide](docs/EXPORT_GUIDE.md) | How to prepare Telegram, WhatsApp, Instagram, and Discord files |
| [Benchmarks](docs/BENCHMARKS.md) | Compression data, current benchmark groups, and local benchmark commands |
| [examples/library_usage.rs](examples/library_usage.rs) | Basic library usage patterns |
| [examples/rag_integration.rs](examples/rag_integration.rs) | Example chunking flow for RAG systems |

## Related Tools

This repository is the Rust core library. Other Chatpack tools live separately:

| Repository | Purpose |
|------------|---------|
| [chatpack-cli](https://github.com/Berektassuly/chatpack-cli) | Command-line interface |
| [chatpack-web](https://github.com/Berektassuly/chatpack-web) | Browser/WASM interface |
| [chatpack-python](https://github.com/Berektassuly/chatpack-python) | Python bindings |

## Development

```bash
cargo fmt --all -- --check
cargo test --all-features
cargo clippy --all-targets -- -D warnings
cargo bench --bench parsing
```

The crate uses Rust 2024 edition, so Rust 1.85 or newer is required. CI currently builds and tests on stable Rust across Linux, macOS, and Windows.
