//! Kick.com WebSocket library for Rust
//!
//! This library provides a simple way to connect to Kick.com chat rooms
//! and receive real-time messages via WebSocket connections.
//!
//! # Example
//!
//! ```rust,ignore
//! use kick_rust::KickClient;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let client = KickClient::new();
//!
//!     client.on_message(|msg| {
//!         println!("{}: {}", msg.username, msg.content);
//!     });
//!
//!     client.connect("channel_name").await?;
//!
//!     // Keep the program running
//!     loop {
//!         tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
//!     }
//! }
//! ```

pub mod fetch;
pub mod message_parser;
// pub mod robust_websocket; // Commented out - file doesn't exist
pub mod types;
pub mod websocket_manager;

// Re-export commonly used types
pub use websocket_manager::WebSocketManager as KickClient;
pub use types::*;
pub use message_parser::{MessageParser, parse_custom_message, extract_field};
pub use fetch::client::{KickApiClient, KickApiClientBuilder};

// Re-export fetch module types
pub use fetch::types::{FetchResult, FetchError, ClientConfig, StrategyConfig};
