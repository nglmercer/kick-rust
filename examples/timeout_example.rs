//! Advanced Kick.com chat example with timeout and error handling
//!
//! This example demonstrates advanced features including:
//! - Custom timeout configuration
//! - Error handling and reconnection logic
//! - Message filtering and statistics
//! - Graceful shutdown

use kick_rust::{KickClient, RawMessage, extract_field};
use std::time::Duration;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::collections::HashMap;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Statistics tracking
    let message_count = Arc::new(AtomicU64::new(0));
    let event_types = Arc::new(tokio::sync::Mutex::new(HashMap::new()));

    // Create client
    let client = KickClient::new();

    // Set up raw message handler with statistics
    let msg_count_clone = message_count.clone();
    let event_types_clone = event_types.clone();

    client.on_raw_message(move |data: RawMessage| {
        // Increment message counter
        msg_count_clone.fetch_add(1, Ordering::Relaxed);

        // Track event types
        let event_types = event_types_clone.clone();
        let event_type_clone = data.event_type.clone();
        tokio::spawn(async move {
            let mut types = event_types.lock().await;
            *types.entry(event_type_clone).or_insert(0) += 1;
        });

        // Filter out ping/pong messages for cleaner output
        match data.event_type.as_str() {
            "pusher:ping" | "pusher:pong" => {
                // Silent handling for keepalive messages
            }
            "App\\Events\\ChatMessageEvent" => {
                if let Some(username) = extract_field(&data, "data.sender.username") {
                    if let Some(content) = extract_field(&data, "data.content") {
                        println!("💬 {}: {}", username.as_str().unwrap_or("unknown"),
                                content.as_str().unwrap_or(""));
                    }
                }
            }
            _ => {
                println!("🔔 {}: {}", data.event_type,
                        data.data.chars().take(100).collect::<String>());
            }
        }
    }).await;

    // Try to connect with different timeout strategies
    println!("🚀 Attempting to connect with 10 second timeout...");

    match client.connect_with_timeout("daadota2", Duration::from_secs(10)).await {
        Ok(_) => {
            println!("✅ Connected successfully!");
        }
        Err(e) => {
            println!("❌ Connection failed: {}", e);
            println!("🔄 Trying with longer timeout...");

            // Retry with longer timeout
            client.connect_with_timeout("daadota2", Duration::from_secs(30)).await?;
            println!("✅ Connected on retry!");
        }
    }

    // Start statistics task
    let stats_count = message_count.clone();
    let stats_events = event_types.clone();
    let stats_handle = tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(10));

        loop {
            interval.tick().await;

            let count = stats_count.load(Ordering::Relaxed);
            let events = stats_events.lock().await;

            println!("\n📊 Statistics (last 10 seconds):");
            println!("   Total messages: {}", count);
            println!("   Event types: {:?}", *events);
            println!("   Connected: {}", !events.is_empty());
            println!();
        }
    });

    // Set up graceful shutdown
    let shutdown_signal = tokio::signal::ctrl_c();
    let client_clone = client.clone();

    tokio::select! {
        _ = shutdown_signal => {
            println!("\n🛑 Shutdown signal received...");

            // Cancel statistics task
            stats_handle.abort();

            // Graceful disconnect
            if let Err(e) = client_clone.disconnect().await {
                println!("⚠️  Error during disconnect: {}", e);
            } else {
                println!("✅ Disconnected gracefully");
            }

            // Final statistics
            let final_count = message_count.load(Ordering::Relaxed);
            println!("📈 Final statistics:");
            println!("   Total messages processed: {}", final_count);

            let final_events = event_types.lock().await;
            for (event_type, count) in final_events.iter() {
                println!("   {}: {} times", event_type, count);
            }
        }

        _ = tokio::time::sleep(Duration::from_secs(60)) => {
            println!("\n⏰ 60 second timeout reached, shutting down...");
            client.disconnect().await?;
        }
    }

    Ok(())
}

/// Example of custom error handling for connection issues
/*
async fn robust_connect(client: &KickClient, channel: &str, max_retries: u32) -> Result<(), Box<dyn std::error::Error>> {
    let mut retries = 0;
    while retries < max_retries {
        let timeout = match retries {
            0 => Duration::from_secs(5),
            1 => Duration::from_secs(10),
            _ => Duration::from_secs(30),
        };

        println!("🔄 Attempt {} (timeout: {:?})...", retries + 1, timeout);

        match client.connect_with_timeout(channel, timeout).await {
            Ok(_) => {
                println!("✅ Connected successfully on attempt {}", retries + 1);
                return Ok(());
            }
            Err(e) => {
                println!("❌ Attempt {} failed: {}", retries + 1, e);
                retries += 1;

                if retries < max_retries {
                    println!("⏳ Waiting before retry...");
                    tokio::time::sleep(Duration::from_secs(2)).await;
                }
            }
        }
    }

    Err(format!("Failed to connect after {} attempts", max_retries).into())
}
*/
/// Example of message filtering and processing

struct MessageFilter {
    keywords: Vec<String>,
    users: Vec<String>,
}

impl MessageFilter {
    fn new() -> Self {
        Self {
            keywords: vec![],
            users: vec![],
        }
    }

    fn add_keyword(&mut self, keyword: String) {
        self.keywords.push(keyword.to_lowercase());
    }

    fn add_user(&mut self, user: String) {
        self.users.push(user.to_lowercase());
    }

    fn should_process(&self, raw_msg: &RawMessage) -> bool {
        if raw_msg.event_type != "App\\Events\\ChatMessageEvent" {
            return false;
        }

        // Check if message contains keywords
        for keyword in &self.keywords {
            if raw_msg.data.to_lowercase().contains(keyword) {
                return true;
            }
        }

        // Check if message is from specific users
        if let Some(username) = extract_field(raw_msg, "data.sender.username") {
            if let Some(username_str) = username.as_str() {
                for user in &self.users {
                    if username_str.to_lowercase() == *user {
                        return true;
                    }
                }
            }
        }

        false
    }
}
