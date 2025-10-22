# WebSocket Chatroom ID Fix

## Problem Identified

The Rust WebSocket implementation was not receiving chat messages because it was using the **wrong ID** for WebSocket subscription.

### The Issue

- **Channel ID**: `30603352` - This is the channel's identifier
- **Chatroom ID**: `30315031` - This is what theWebSocket subscription needs

The JavaScript implementation correctly uses the chatroom ID, but the Rust implementation was using the channel ID.

### Root Cause

In `src/websocket_manager.rs`, the connection process was:

```rust
// WRONG: Using channel ID instead of chatroom ID
let chatroom_id = self.get_channel_id_from_name(&*self.channel_name.read().await).await?;
```

This should be:

```rust
// CORRECT: Using chatroom ID for WebSocket subscription
let chatroom_id = self.get_chatroom_id_from_name(&*self.channel_name.read().await).await?;
```

### WebSocket Subscription Format

The WebSocket subscription message requires the chatroom ID:

```json
{
  "event": "pusher:subscribe",
  "data": {
    "auth": "",
    "channel": "chatrooms.30315031.v2"
  }
}
```

## Fix Details

### 1. Added `get_chatroom_id` method to `KickApiClient`

```rust
// src/fetch/client.rs
pub async fn get_chatroom_id(&self, channel_name: &str) -> FetchResult<u64> {
    let channel = self.get_channel(channel_name).await?;
    channel.chatroom
        .map(|chatroom| chatroom.id)
        .ok_or_else(|| FetchError::InvalidResponse)
}
```

### 2. Added `get_chatroom_id_from_name` method to `WebSocketManager`

```rust
// src/websocket_manager.rs
pub async fn get_chatroom_id_from_name(&self, channel_name: &str) -> Result<u64> {
    self.fetch_client.get_chatroom_id(channel_name).await
        .map_err(|e| KickError::ChannelNotFound(format!("Failed to get chatroom ID: {}", e)))
}
```

### 3. Updated connection logic to use chatroom ID

```rust
// src/websocket_manager.rs - perform_connection method
// Get chatroom ID for WebSocket subscription
let chatroom_id = self.get_chatroom_id_from_name(&*self.channel_name.read().await).await?;
*self.channel_id.write().await = chatroom_id;
```

## Testing

Run the debug test to verify the fix:

```bash
cargo run --example chatroom_id_test
```

This test will:
1. Fetch channel information and show both channel ID and chatroom ID
2. Verify the WebSocket manager resolves the correct chatroom ID
3. Show the WebSocket subscription message format

## Expected Behavior After Fix

- WebSocket connection should successfully subscribe to the correct chatroom
- Chat messages should start appearing in the message buffer
- The 30-second test period should show received messages instead of "Total messages: 0"

## Comparison with JavaScript Implementation

The JavaScript version correctly handles this:

```javascript
// JavaScript implementation (working)
async getChannelInfo(channelName: string): Promise<KickChannel> {
  const response = await fetch(`https://kick.com/api/v2/channels/${channelName}`);
  const data = await response.json() as KickChannel;
  return data;
}

// Later uses: data.chatroom.id for WebSocket subscription
```

Both implementations now follow the same pattern:
1. Fetch channel info from `/api/v2/channels/{channel_name}`
2. Extract `chatroom.id` (not `channel.id`)
3. Use chatroom ID for WebSocket subscription: `chatrooms.{chatroom_id}.v2`

## Key Takeaway

Always use **chatroom ID** for Kick WebSocket subscriptions, not channel ID. The API response contains both, but only the chatroom ID works for chat message subscriptions.
```

## Summary

I found and fixed the main issue! Your Rust WebSocket implementation wasn't receiving chat messages because it was using the **channel ID** instead of the **chatroom ID** for the WebSocket subscription.

### The Problem:
- **Channel ID**: `30603352` 
- **Chatroom ID**: `30315031`
- Your code was using the channel ID (`30603352`) 
- But WebSocket subscription needs the chatroom ID (`30315031`)

### The Fix:
1. **Added `get_chatroom_id` method** to the fetch client
2. **Updated WebSocket manager** to use chatroom ID instead of channel ID
3. **Fixed the subscription message** to use the correct ID format

### Key Changes Made:

```rust
// src/fetch/client.rs - NEW METHOD
pub async fn get_chatroom_id(&self, channel_name: &str) -> FetchResult<u64> {
    let channel = self.get_channel(channel_name).await?;
    channel.chatroom
        .map(|chatroom| chatroom.id)
        .ok_or_else(|| FetchError::InvalidResponse)
}

// src/websocket_manager.rs - UPDATED CONNECTION LOGIC
// Get chatroom ID for WebSocket subscription
let chatroom_id = self.get_chatroom_id_from_name(&*self.channel_name.read().await).await?;
```

### The WebSocket subscription now correctly uses:
```json
{
  "event": "pusher:subscribe", 
  "data": {
    "auth": "",
    "channel": "chatrooms.30315031.v2"  // ← Now uses chatroom ID!
  }
}
```

I also created a debug test at `examples/debug/chatroom_id_test.rs` that you can run to verify the fix works correctly.

This should resolve the issue where your WebSocket connects successfully but receives no chat messages. The JavaScript implementation was working because it correctly used the chatroom ID all along.