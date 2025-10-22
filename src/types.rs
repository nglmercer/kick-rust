//! Types and data structures for Kick WebSocket events

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Connection state of the WebSocket
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Reconnecting,
    Error,
}

/// Event types that can be received from Kick WebSocket
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KickEventType {
    ChatMessage,
    MessageDeleted,
    UserBanned,
    UserUnbanned,
    Subscription,
    GiftedSubscriptions,
    PinnedMessageCreated,
    StreamHost,
    PollUpdate,
    PollDelete,
}

/// Configuration options for the WebSocket connection
#[derive(Debug, Clone)]
pub struct KickWebSocketOptions {
    pub debug: bool,
    pub auto_reconnect: bool,
    pub reconnect_interval: u64,
    pub enable_buffer: bool,
    pub buffer_size: usize,
    pub filtered_events: Vec<KickEventType>,
    pub custom_user_agent: Option<String>,
    pub rotate_user_agent: bool,
}

impl Default for KickWebSocketOptions {
    fn default() -> Self {
        Self {
            debug: false,
            auto_reconnect: true,
            reconnect_interval: 5000,
            enable_buffer: false,
            buffer_size: 1000,
            filtered_events: vec![],
            custom_user_agent: None,
            rotate_user_agent: true,
        }
    }
}

/// Statistics for the message buffer
#[derive(Debug, Clone)]
pub struct BufferStats {
    pub total: usize,
    pub by_type: HashMap<String, usize>,
    pub oldest_timestamp: Option<chrono::DateTime<chrono::Utc>>,
    pub newest_timestamp: Option<chrono::DateTime<chrono::Utc>>,
}

/// Kick channel information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KickChannel {
    pub id: u64,
    pub slug: String,
    pub user: KickUser,
    pub chatroom: KickChatroom,
}

/// Kick user information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KickUser {
    pub id: u64,
    pub username: String,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
}

/// Chat message event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessageEvent {
    pub id: String,
    pub content: String,
    #[serde(rename = "type")]
    pub message_type: String,
    #[serde(rename = "created_at")]
    pub created_at: String,
    pub sender: KickUser,
    pub chatroom: KickChatroom,
}

/// Message deleted event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageDeletedEvent {
    #[serde(rename = "message_id")]
    pub message_id: String,
    #[serde(rename = "chatroom_id")]
    pub chatroom_id: u64,
    #[serde(rename = "type")]
    pub event_type: String,
}

/// User banned event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserBannedEvent {
    pub username: String,
    #[serde(rename = "type")]
    pub event_type: String,
}

/// User unbanned event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserUnbannedEvent {
    pub username: String,
    #[serde(rename = "type")]
    pub event_type: String,
}

/// Subscription event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriptionEvent {
    pub username: String,
    pub months: Option<u32>,
    #[serde(rename = "type")]
    pub event_type: String,
}

/// Gifted subscriptions event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GiftedSubscriptionsEvent {
    #[serde(rename = "gifted_by")]
    pub gifted_by: String,
    pub recipients: Vec<String>,
    #[serde(rename = "type")]
    pub event_type: String,
}

/// Pinned message created event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PinnedMessageCreatedEvent {
    pub message: ChatMessageEvent,
    #[serde(rename = "type")]
    pub event_type: String,
}

/// Stream host event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamHostEvent {
    pub hoster: String,
    #[serde(rename = "hosted_channel")]
    pub hosted_channel: String,
    #[serde(rename = "type")]
    pub event_type: String,
}

/// Poll update event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PollUpdateEvent {
    #[serde(rename = "poll_id")]
    pub poll_id: String,
    pub question: String,
    pub options: Vec<PollOption>,
    #[serde(rename = "type")]
    pub event_type: String,
}

/// Poll option
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PollOption {
    pub id: String,
    pub text: String,
    pub votes: u32,
}

/// Poll delete event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PollDeleteEvent {
    #[serde(rename = "poll_id")]
    pub poll_id: String,
    #[serde(rename = "type")]
    pub event_type: String,
}

/// Kick chatroom information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KickChatroom {
    pub id: u64,
    pub channel_id: u64,
    pub name: String,
}

/// Raw WebSocket message structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSocketMessage {
    pub event: String,
    pub data: serde_json::Value,
    pub channel: Option<String>,
}

/// Raw chat message data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawChatMessageData {
    pub id: String,
    pub content: String,
    #[serde(rename = "type")]
    pub message_type: String,
    #[serde(rename = "created_at")]
    pub created_at: String,
    pub sender: KickUser,
    pub chatroom: KickChatroom,
}

/// Raw message deleted data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawMessageDeletedData {
    #[serde(rename = "message_id")]
    pub message_id: String,
    #[serde(rename = "chatroom_id")]
    pub chatroom_id: u64,
}

/// Raw WebSocket message structure
#[derive(Debug, Clone)]
pub struct RawMessage {
    pub event_type: String,
    pub data: String,
    pub raw_json: String,
}

/// Simple message structure for easy chat message handling
#[derive(Debug, Clone)]
pub struct SimpleMessage {
    pub id: String,
    pub username: String,
    pub content: String,
    pub created_at: String,
}

impl From<&ChatMessageEvent> for SimpleMessage {
    fn from(chat_msg: &ChatMessageEvent) -> Self {
        Self {
            id: chat_msg.id.clone(),
            username: chat_msg.sender.username.clone(),
            content: chat_msg.content.clone(),
            created_at: chat_msg.created_at.clone(),
        }
    }
}

/// Raw user banned data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawUserBannedData {
    pub username: Option<String>,
    #[serde(rename = "banned_username")]
    pub banned_username: Option<String>,
}

/// Raw user unbanned data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawUserUnbannedData {
    pub username: Option<String>,
}

/// Raw subscription data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawSubscriptionData {
    pub username: String,
    pub months: Option<u32>,
}

/// Raw gifted subscriptions data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawGiftedSubscriptionsData {
    #[serde(rename = "gifted_by")]
    pub gifted_by: Option<String>,
    pub gifter: Option<serde_json::Value>,
    pub recipients: Vec<serde_json::Value>,
}

/// Raw pinned message created data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawPinnedMessageCreatedData {
    pub message: RawChatMessageData,
}

/// Raw stream host data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawStreamHostData {
    pub hoster: serde_json::Value,
    #[serde(rename = "hosted_channel")]
    pub hosted_channel: serde_json::Value,
}

/// Raw poll update data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawPollUpdateData {
    pub id: String,
    pub question: String,
    pub options: Vec<RawPollOption>,
}

/// Raw poll option
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawPollOption {
    pub id: String,
    pub text: String,
    pub votes: Option<u32>,
}

/// Raw poll delete data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawPollDeleteData {
    pub id: String,
}

/// Event data enum for all possible event types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum KickEventData {
    ChatMessage(ChatMessageEvent),
    MessageDeleted(MessageDeletedEvent),
    UserBanned(UserBannedEvent),
    UserUnbanned(UserUnbannedEvent),
    Subscription(SubscriptionEvent),
    GiftedSubscriptions(GiftedSubscriptionsEvent),
    PinnedMessageCreated(PinnedMessageCreatedEvent),
    StreamHost(StreamHostEvent),
    PollUpdate(PollUpdateEvent),
    PollDelete(PollDeleteEvent),
}

/// Parsed message with event type and data
#[derive(Debug, Clone)]
pub struct ParsedMessage {
    pub r#type: KickEventType,
    pub data: KickEventData,
}



/// Error types for the WebSocket client
#[derive(Debug, thiserror::Error)]
pub enum KickError {
    #[error("WebSocket error: {0}")]
    WebSocket(#[from] tokio_tungstenite::tungstenite::Error),

    #[error("JSON serialization/deserialization error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("HTTP request error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Fetch error: {0}")]
    Fetch(#[from] crate::fetch::types::FetchError),

    #[error("Invalid message format: {0}")]
    InvalidMessage(String),

    #[error("Channel not found: {0}")]
    ChannelNotFound(String),

    #[error("Connection error: {0}")]
    Connection(String),

    #[error("Unknown event type: {0}")]
    UnknownEventType(String),

    #[error("URL parse error: {0}")]
    UrlParse(#[from] url::ParseError),

    #[error("Network error: {0}")]
    NetworkError(String),
}

pub type Result<T> = std::result::Result<T, KickError>;
