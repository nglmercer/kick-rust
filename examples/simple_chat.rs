//! Simple Kick.com chat example with raw message support
//!
//! This example demonstrates how to use both parsed and raw message handlers
//! for maximum flexibility in processing Kick.com chat data.

use kick_rust::{KickClient, RawMessage, parse_custom_message, extract_field};
use serde::{Deserialize, Serialize};

/// Custom message structure for parsing specific data
#[derive(Debug, Deserialize, Serialize)]
struct CustomUserData {
    id: u64,
    username: String,
    #[serde(rename = "displayname")]
    display_name: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create client
    let client = KickClient::new();

    // Option 1: Handle raw messages for custom parsing
    client.on_raw_message(|data: RawMessage| {
        println!("🔴 Raw Event: {}", data.event_type);

        // Extract specific fields using utility function
        if let Some(sender) = extract_field(&data, "data.sender.username") {
            println!("👤 Sender: {}", sender);
        }

        // Try to parse as custom data structure
        if data.event_type == "App\\Events\\ChatMessageEvent" {
            if let Ok(custom_data) = parse_custom_message::<CustomUserData>(&data.data) {
                println!("🎯 Custom Parsed: {:?}", custom_data);
            }
        }

        // Show raw data for debugging
        if data.event_type != "pusher:ping" && data.event_type != "pusher:pong" {
            println!("📄 Raw Data: {}", data.data);
        }
        println!("---");
    }).await;

    // Option 2: Handle parsed chat messages (simpler)
    client.on_message(|msg| {
        println!("💬 {}: {}", msg.username, msg.content);
    }).await;

    // Connect to channel with timeout
    println!("Connecting to daadota2 channel...");
    client.connect("daadota2").await?;

    // Keep running
    println!("✅ Connected! Press Ctrl+C to stop.");
    println!("📊 Client status: Connected = {}", client.is_connected().await);

    tokio::signal::ctrl_c().await?;

    // Clean disconnect
    client.disconnect().await?;
    println!("👋 Disconnected gracefully");

    Ok(())
}
