//! Debug test to verify chatroom ID fix
//!
//! This test verifies that we're using the correct chatroom ID for WebSocket subscription
//! instead of the channel ID.

use kick_rust::fetch::client::KickApiClient;
use kick_rust::websocket_manager::WebSocketManager;
use tracing::{info, Level};
use tracing_subscriber;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .init();

    info!("🔍 Testing Chatroom ID Fix");

    // Test 1: Get channel info and verify IDs are different
    let client = KickApiClient::new()?;
    let channel_name = "daadota2";

    info!("📡 Fetching channel info for: {}", channel_name);
    let channel = client.get_channel(channel_name).await?;

    let channel_id = channel.id;
    let chatroom_id = channel.chatroom
        .ok_or("No chatroom info found")?
        .id;

    info!("🔢 Channel ID: {}", channel_id);
    info!("💬 Chatroom ID: {}", chatroom_id);

    if channel_id != chatroom_id {
        info!("✅ IDs are different - this is expected!");
    } else {
        info!("⚠️  IDs are the same - this might be unusual");
    }

    // Test 2: Verify WebSocket manager uses chatroom ID
    info!("🔌 Testing WebSocket manager chatroom ID resolution...");

    let ws_manager = WebSocketManager::new();
    let resolved_chatroom_id = ws_manager.get_chatroom_id_from_name(channel_name).await?;

    info!("🎯 WebSocket manager resolved chatroom ID: {}", resolved_chatroom_id);

    if resolved_chatroom_id == chatroom_id {
        info!("✅ WebSocket manager correctly uses chatroom ID!");
    } else {
        info!("❌ WebSocket manager is using wrong ID!");
        return Err("Chatroom ID mismatch".into());
    }

    // Test 3: Show what the WebSocket subscription message would look like
    info!("📨 WebSocket subscription message would be:");
    let subscription_msg = format!(
        r#"{{"event":"pusher:subscribe","data":{{"auth":"","channel":"chatrooms.{}.v2"}}}}"#,
        resolved_chatroom_id
    );
    info!("   {}", subscription_msg);

    info!("🎉 Chatroom ID fix verification completed successfully!");
    Ok(())
}
