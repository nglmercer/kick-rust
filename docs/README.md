# Kick Rust - Simple Kick.com WebSocket Chat

A minimal Rust library for connecting to Kick.com WebSocket chat and receiving real-time messages.

## Quick Start

Add this to your `Cargo.toml`:

```toml
[dependencies]
kick-rust = "0.1.0"
tokio = { version = "1.0", features = ["full"] }
```

## Example

```rust
use kick_rust::KickClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create client
    let client = KickClient::new();

    // Handle messages
    client.on_message(|msg| {
        println!("{}: {}", msg.username, msg.content);
    });

    // Connect to channel
    client.connect("channel_name").await?;

    // Keep running
    tokio::signal::ctrl_c().await?;

    Ok(())
}
```

## Run Example

```bash
cargo run --example simple_chat
```

## Features

- ✅ Simple WebSocket connection to Kick.com
- ✅ Real-time chat message handling
- ✅ Minimal dependencies
- ✅ Easy to use API

## Dependencies

- `tokio` - Async runtime
- `tokio-tungstenite` - WebSocket client
- `serde` & `serde_json` - JSON serialization
- `reqwest` - HTTP client for channel info
- `futures-util` - Async utilities

That's it! No complex configuration, no unnecessary features.