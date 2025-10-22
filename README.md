# kick-rust

A Rust library for interacting with Kick.com WebSocket chat and API. This library provides a simple and robust way to connect to Kick.com chat rooms, receive real-time messages, and interact with the Kick.com API.

## Features

- **WebSocket Chat Client**: Connect to Kick.com chat rooms and receive real-time messages
- **Event Handling**: Comprehensive event system for different types of chat events
- **Automatic Reconnection**: Built-in reconnection logic with configurable intervals
- **Message Parsing**: Automatic parsing of chat messages, emotes, and metadata
- **API Client**: HTTP client for interacting with Kick.com REST API
- **Async/Await**: Full async support using Tokio
- **Type Safety**: Strongly typed events and data structures

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
kick-rust = "0.1.0"
```

## Quick Start

### Basic WebSocket Chat Example

```rust
use kick_rust::KickClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = KickClient::new();

    // Handle chat messages
    client.on_chat_message(|data| {
        println!("{}: {}", data.sender.username, data.content);
    }).await;

    // Handle connection ready
    client.on_ready(|()| {
        println!("Connected to chat!");
    }).await;

    // Connect to a channel
    client.connect("channel_name").await?;

    // Keep the program running
    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    }
}
```

### Advanced Configuration

```rust
use kick_rust::{KickClient, KickWebSocketOptions};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = KickClient::with_options(KickWebSocketOptions {
        auto_reconnect: true,
        reconnect_interval: 5000,
        enable_buffer: true,
        buffer_size: 1000,
        debug: true,
        ..Default::default()
    });

    // Set up event handlers
    client.on_chat_message(|msg| {
        println!("Chat: {} - {}", msg.sender.username, msg.content);
    }).await;

    client.on_user_banned(|event| {
        println!("User banned: {}", event.username);
    }).await;

    client.on_subscription(|event| {
        println!("New subscription from: {}", event.username);
    }).await;

    // Connect with timeout
    client.connect_with_timeout("channel_name", std::time::Duration::from_secs(10)).await?;

    // Your application logic here
    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    }
}
```

### API Client Usage

```rust
use kick_rust::KickApiClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = KickApiClient::new();
    
    // Get channel information
    let channel = client.get_channel("channel_name").await?;
    println!("Channel: {} ({})", channel.user.username, channel.id);
    
    // Get chatroom information
    let chatroom = client.get_chatroom("channel_name").await?;
    println!("Chatroom ID: {}", chatroom.id);
    
    Ok(())
}
```

## Event Types

The library supports various event types:

- `ChatMessage`: Regular chat messages
- `MessageDeleted`: When a message is deleted
- `UserBanned`: When a user is banned
- `UserUnbanned`: When a user is unbanned
- `Subscription`: New subscriptions
- `GiftedSubscriptions`: Gifted subscriptions
- `PinnedMessageCreated`: When a message is pinned
- `StreamHost`: Stream hosting events
- `PollUpdate`: Poll updates
- `PollDelete`: Poll deletions

## Configuration Options

```rust
KickWebSocketOptions {
    debug: false,                    // Enable debug logging
    auto_reconnect: true,            // Auto-reconnect on disconnect
    reconnect_interval: 5000,        // Reconnection interval in ms
    enable_buffer: false,            // Enable message buffer
    buffer_size: 1000,              // Buffer size
    filtered_events: vec![],         // Events to filter out
    custom_user_agent: None,         // Custom user agent
    rotate_user_agent: true,         // Rotate user agent
}
```

## Requirements

- Rust 1.70 or higher
- Tokio runtime

## License

This project is licensed under either of:

- Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or
  https://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or
  https://opensource.org/licenses/MIT)

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## Support

If you encounter any issues or have questions, please file an issue on the [GitHub repository](https://github.com/nglmercer/kick-rust).
```

Now let me check if there are license files:
