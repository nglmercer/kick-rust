//! WebSocket manager for handling Kick.com WebSocket connections
use crate::message_parser::MessageParser;
use crate::types::{BufferStats, *};
use crate::fetch::client::KickApiClient;
use crate::fetch::useragent::{generate_browser_fingerprint, get_rotating_user_agent};
use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{Mutex, RwLock};
use tokio_tungstenite::{connect_async, tungstenite::Message, WebSocketStream};
use tracing::{debug, error, info, warn};
use url::Url;
use tokio_tungstenite::tungstenite::http::Request;
// use tokio_tungstenite::tungstenite::client::IntoClientRequest; // No longer needed

/// Event handler callback type
pub type EventHandler<T> = Arc<dyn Fn(T) + Send + Sync>;

/// WebSocket manager for Kick.com
#[derive(Clone)]
pub struct WebSocketManager {
    ws: Arc<Mutex<Option<WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>>>>,
    channel_name: Arc<RwLock<String>>,
    channel_id: Arc<RwLock<u64>>,
    connection_state: Arc<RwLock<ConnectionState>>,
    options: Arc<RwLock<KickWebSocketOptions>>,
    message_buffer: Arc<RwLock<std::collections::VecDeque<String>>>,
    event_handlers: Arc<RwLock<EventHandlers>>,
    is_manual_disconnect: Arc<RwLock<bool>>,
    reconnect_timer: Arc<RwLock<Option<tokio::task::JoinHandle<()>>>>,
    custom_websocket_url: Arc<RwLock<Option<String>>>,
    custom_websocket_params: Arc<RwLock<Option<HashMap<String, String>>>>,
    fetch_client: Arc<KickApiClient>,
}

/// Container for event handlers
#[derive(Default)]
struct EventHandlers {
    chat_message: Vec<EventHandler<ChatMessageEvent>>,
    message_deleted: Vec<EventHandler<MessageDeletedEvent>>,
    user_banned: Vec<EventHandler<UserBannedEvent>>,
    user_unbanned: Vec<EventHandler<UserUnbannedEvent>>,
    subscription: Vec<EventHandler<SubscriptionEvent>>,
    gifted_subscriptions: Vec<EventHandler<GiftedSubscriptionsEvent>>,
    pinned_message_created: Vec<EventHandler<PinnedMessageCreatedEvent>>,
    stream_host: Vec<EventHandler<StreamHostEvent>>,
    poll_update: Vec<EventHandler<PollUpdateEvent>>,
    poll_delete: Vec<EventHandler<PollDeleteEvent>>,
    raw_message: Vec<EventHandler<RawMessage>>,
    error: Vec<EventHandler<KickError>>,
    ready: Vec<EventHandler<()>>,
    disconnected: Vec<EventHandler<()>>,
}

impl WebSocketManager {
    /// Create a new WebSocket manager with default options
    pub fn new() -> Self {
        Self::with_options(KickWebSocketOptions::default())
    }

    /// Create a new WebSocket manager with custom options
    pub fn with_options(options: KickWebSocketOptions) -> Self {
        let fetch_client = Arc::new(KickApiClient::new().expect("Failed to create KickApiClient with robust User-Agent"));
        Self {
            ws: Arc::new(Mutex::new(None)),
            channel_name: Arc::new(RwLock::new(String::new())),
            channel_id: Arc::new(RwLock::new(0)),
            connection_state: Arc::new(RwLock::new(ConnectionState::Disconnected)),
            options: Arc::new(RwLock::new(options)),
            message_buffer: Arc::new(RwLock::new(std::collections::VecDeque::new())),
            event_handlers: Arc::new(RwLock::new(EventHandlers::default())),
            is_manual_disconnect: Arc::new(RwLock::new(false)),
            reconnect_timer: Arc::new(RwLock::new(None)),
            custom_websocket_url: Arc::new(RwLock::new(None)),
            custom_websocket_params: Arc::new(RwLock::new(None)),
            fetch_client,
        }
    }

    /// Connect to a specific channel's WebSocket
    pub async fn connect(&self, channel_name: &str) -> Result<()> {
        info!("Connecting to channel: {}", channel_name);
        let current_state = *self.connection_state.read().await;
        if current_state == ConnectionState::Connected || current_state == ConnectionState::Connecting {
            warn!("Already connected or connecting");
            return Ok(());
        }

        *self.channel_name.write().await = channel_name.to_string();
        *self.is_manual_disconnect.write().await = false;

        if let Err(e) = self.perform_connection().await {
            self.handle_connection_error(&e).await;
            return Err(e);
        }

        Ok(())
    }

    /// Perform the actual WebSocket connection
    async fn perform_connection(&self) -> Result<()> {
        self.set_connection_state(ConnectionState::Connecting).await;
        info!("Connecting to channel: {}", *self.channel_name.read().await);

        // Get chatroom ID for WebSocket subscription
        let chatroom_id = self.get_chatroom_id_from_name(&*self.channel_name.read().await).await?;
        *self.channel_id.write().await = chatroom_id;

        // Build WebSocket URL
        let ws_url = self.build_websocket_url().await?;

        // Create WebSocket connection with realistic headers
        let options = self.options.read().await;
        let request = self.create_websocket_request(&ws_url, &*options).await?;
        drop(options);

        match connect_async(request).await {
            Ok((ws_stream, response)) => {
                info!("WebSocket connected! Status: {}", response.status());
                *self.ws.lock().await = Some(ws_stream);

                // Setup WebSocket handlers
                self.setup_websocket_handlers().await?;
            }
            Err(e) => {
                error!("WebSocket connection failed: {}", e);
                return Err(KickError::WebSocket(e));
            }
        }

        Ok(())
    }



    /// Setup WebSocket message handlers
    async fn setup_websocket_handlers(&self) -> Result<()> {
        let ws = self.ws.clone();
        let channel_id = *self.channel_id.read().await;
        let connection_state = self.connection_state.clone();
        let message_buffer = self.message_buffer.clone();
        let options = self.options.clone();
        let event_handlers = self.event_handlers.clone();

        tokio::spawn(async move {
            // Estado para controlar la suscripción
            let mut connection_established = false;
            let mut subscription_succeeded = false;

            // Handle messages
            loop {
                let msg = {
                    let mut ws_lock = ws.lock().await;
                    if let Some(ws_stream) = ws_lock.as_mut() {
                        ws_stream.next().await
                    } else {
                        break;
                    }
                };

                match msg {
                    Some(Ok(Message::Text(text))) => {
                        debug!("Received message: {}", text);

                        // Parsear el mensaje
                        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&text) {
                            if let Some(event) = parsed.get("event").and_then(|e| e.as_str()) {
                                // 1. Detectar pusher:connection_established
                                if event == "pusher:connection_established" && !connection_established {
                                    info!("✓ Pusher connection established!");
                                    connection_established = true;

                                    // ENVIAR SUSCRIPCIÓN INMEDIATAMENTE
                                    let subscribe_msg = json!({
                                        "event": "pusher:subscribe",
                                        "data": {
                                            "auth": "",
                                            "channel": format!("chatrooms.{}.v2", channel_id)
                                        }
                                    });

                                    info!("→ Sending subscription: {}", subscribe_msg);

                                    let mut ws_lock = ws.lock().await;
                                    if let Some(ws_stream) = ws_lock.as_mut() {
                                        if let Err(e) = ws_stream.send(Message::Text(subscribe_msg.to_string())).await {
                                            error!("Failed to subscribe: {}", e);
                                            break;
                                        }
                                    }
                                    drop(ws_lock); // Liberar el lock explícitamente
                                    continue;
                                }

                                // 2. Detectar pusher_internal:subscription_succeeded
                                if event == "pusher_internal:subscription_succeeded" && !subscription_succeeded {
                                    info!("✓ Subscription succeeded!");
                                    subscription_succeeded = true;
                                    *connection_state.write().await = ConnectionState::Connected;

                                    // Emitir evento ready
                                    let handlers = event_handlers.read().await;
                                    for handler in &handlers.ready {
                                        handler(());
                                    }
                                    continue;
                                }

                                // Emitir raw message
                                {
                                    let handlers = event_handlers.read().await;
                                    let raw_msg = RawMessage {
                                        event_type: event.to_string(),
                                        data: parsed.get("data").unwrap_or(&serde_json::Value::Null).to_string(),
                                        raw_json: text.clone(),
                                    };
                                    for handler in &handlers.raw_message {
                                        handler(raw_msg.clone());
                                    }
                                }
                            }
                        }

                        // Agregar al buffer si está habilitado
                        if options.read().await.enable_buffer {
                            let mut buffer = message_buffer.write().await;
                            if buffer.len() >= options.read().await.buffer_size {
                                buffer.pop_front();
                            }
                            buffer.push_back(text.clone());
                        }

                        // Parsear y manejar eventos de Kick
                        match MessageParser::parse_message(&text) {
                            Ok(Some(parsed_message)) => {
                                let handlers = event_handlers.read().await;
                                if !options.read().await.filtered_events.contains(&parsed_message.r#type) {
                                    match parsed_message.data {
                                        KickEventData::ChatMessage(data) => {
                                            for handler in &handlers.chat_message {
                                                handler(data.clone());
                                            }
                                        }
                                        KickEventData::MessageDeleted(data) => {
                                            for handler in &handlers.message_deleted {
                                                handler(data.clone());
                                            }
                                        }
                                        KickEventData::UserBanned(data) => {
                                            for handler in &handlers.user_banned {
                                                handler(data.clone());
                                            }
                                        }
                                        KickEventData::UserUnbanned(data) => {
                                            for handler in &handlers.user_unbanned {
                                                handler(data.clone());
                                            }
                                        }
                                        KickEventData::Subscription(data) => {
                                            for handler in &handlers.subscription {
                                                handler(data.clone());
                                            }
                                        }
                                        KickEventData::GiftedSubscriptions(data) => {
                                            for handler in &handlers.gifted_subscriptions {
                                                handler(data.clone());
                                            }
                                        }
                                        KickEventData::PinnedMessageCreated(data) => {
                                            for handler in &handlers.pinned_message_created {
                                                handler(data.clone());
                                            }
                                        }
                                        KickEventData::StreamHost(data) => {
                                            for handler in &handlers.stream_host {
                                                handler(data.clone());
                                            }
                                        }
                                        KickEventData::PollUpdate(data) => {
                                            for handler in &handlers.poll_update {
                                                handler(data.clone());
                                            }
                                        }
                                        KickEventData::PollDelete(data) => {
                                            for handler in &handlers.poll_delete {
                                                handler(data.clone());
                                            }
                                        }
                                    }
                                }
                            }
                            Ok(None) => {
                                // Evento de sistema de Pusher, ignorar
                            }
                            Err(e) => {
                                debug!("Parse error: {}", e);
                            }
                        }
                    }
                    Some(Ok(Message::Close(_))) => {
                        info!("WebSocket closed");
                        break;
                    }
                    Some(Ok(Message::Ping(data))) => {
                        let mut ws_lock = ws.lock().await;
                        if let Some(ws_stream) = ws_lock.as_mut() {
                            if let Err(e) = ws_stream.send(Message::Pong(data)).await {
                                error!("Failed to send pong: {}", e);
                            }
                        }
                    }
                    Some(Err(e)) => {
                        error!("WebSocket error: {}", e);
                        let handlers = event_handlers.read().await;
                        for handler in &handlers.error {
                            handler(KickError::Connection(format!("{}", e)));
                        }
                        break;
                    }
                    None => break,
                    _ => {}
                }
            }

            // Manejar desconexión
            *connection_state.write().await = ConnectionState::Disconnected;
            let handlers = event_handlers.read().await;
            for handler in &handlers.disconnected {
                handler(());
            }
        });

        Ok(())
    }

    /// Disconnect from the WebSocket
    pub async fn disconnect(&self) -> Result<()> {
        *self.is_manual_disconnect.write().await = true;

        if let Some(timer) = self.reconnect_timer.write().await.take() {
            timer.abort();
        }

        let mut ws_lock = self.ws.lock().await;
        if let Some(ws_stream) = ws_lock.as_mut() {
            let _ = ws_stream.close(None).await;
        }
        *ws_lock = None;

        // Clear all event handlers
        let mut handlers = self.event_handlers.write().await;
        handlers.chat_message.clear();
        handlers.message_deleted.clear();
        handlers.user_banned.clear();
        handlers.user_unbanned.clear();
        handlers.subscription.clear();
        handlers.gifted_subscriptions.clear();
        handlers.pinned_message_created.clear();
        handlers.stream_host.clear();
        handlers.poll_update.clear();
        handlers.poll_delete.clear();
        handlers.raw_message.clear();
        handlers.error.clear();
        handlers.ready.clear();
        handlers.disconnected.clear();

        self.set_connection_state(ConnectionState::Disconnected).await;
        info!("Disconnected");
        Ok(())
    }

    async fn set_connection_state(&self, state: ConnectionState) {
        *self.connection_state.write().await = state;
    }

    async fn handle_connection_error(&self, error: &KickError) {
        self.set_connection_state(ConnectionState::Error).await;
        error!("Connection error: {}", error);
    }

    // Event handler registration methods
    pub async fn on_chat_message<F>(&self, handler: F)
    where
        F: Fn(ChatMessageEvent) + Send + Sync + 'static,
    {
        self.event_handlers.write().await.chat_message.push(Arc::new(handler));
    }

    pub async fn on_message<F>(&self, handler: F)
    where
        F: Fn(SimpleMessage) + Send + Sync + 'static,
    {
        let simple_handler = move |chat_msg: ChatMessageEvent| {
            let simple_msg = SimpleMessage::from(&chat_msg);
            handler(simple_msg);
        };
        self.event_handlers.write().await.chat_message.push(Arc::new(simple_handler));
    }

    pub async fn on_ready<F>(&self, handler: F)
    where
        F: Fn(()) + Send + Sync + 'static,
    {
        self.event_handlers.write().await.ready.push(Arc::new(handler));
    }

    pub async fn on_disconnected<F>(&self, handler: F)
    where
        F: Fn(()) + Send + Sync + 'static,
    {
        self.event_handlers.write().await.disconnected.push(Arc::new(handler));
    }

    pub async fn on_raw_message<F>(&self, handler: F)
    where
        F: Fn(RawMessage) + Send + Sync + 'static,
    {
        self.event_handlers.write().await.raw_message.push(Arc::new(handler));
    }

    pub async fn on_error<F>(&self, handler: F)
    where
        F: Fn(KickError) + Send + Sync + 'static,
    {
        self.event_handlers.write().await.error.push(Arc::new(handler));
    }

    pub async fn on_message_deleted<F>(&self, handler: F)
    where
        F: Fn(MessageDeletedEvent) + Send + Sync + 'static,
    {
        self.event_handlers.write().await.message_deleted.push(Arc::new(handler));
    }

    pub async fn on_user_banned<F>(&self, handler: F)
    where
        F: Fn(UserBannedEvent) + Send + Sync + 'static,
    {
        self.event_handlers.write().await.user_banned.push(Arc::new(handler));
    }

    pub async fn on_user_unbanned<F>(&self, handler: F)
    where
        F: Fn(UserUnbannedEvent) + Send + Sync + 'static,
    {
        self.event_handlers.write().await.user_unbanned.push(Arc::new(handler));
    }

    pub async fn on_subscription<F>(&self, handler: F)
    where
        F: Fn(SubscriptionEvent) + Send + Sync + 'static,
    {
        self.event_handlers.write().await.subscription.push(Arc::new(handler));
    }

    pub async fn on_gifted_subscriptions<F>(&self, handler: F)
    where
        F: Fn(GiftedSubscriptionsEvent) + Send + Sync + 'static,
    {
        self.event_handlers.write().await.gifted_subscriptions.push(Arc::new(handler));
    }

    pub async fn on_pinned_message_created<F>(&self, handler: F)
    where
        F: Fn(PinnedMessageCreatedEvent) + Send + Sync + 'static,
    {
        self.event_handlers.write().await.pinned_message_created.push(Arc::new(handler));
    }

    pub async fn on_stream_host<F>(&self, handler: F)
    where
        F: Fn(StreamHostEvent) + Send + Sync + 'static,
    {
        self.event_handlers.write().await.stream_host.push(Arc::new(handler));
    }

    pub async fn on_poll_update<F>(&self, handler: F)
    where
        F: Fn(PollUpdateEvent) + Send + Sync + 'static,
    {
        self.event_handlers.write().await.poll_update.push(Arc::new(handler));
    }

    pub async fn on_poll_delete<F>(&self, handler: F)
    where
        F: Fn(PollDeleteEvent) + Send + Sync + 'static,
    {
        self.event_handlers.write().await.poll_delete.push(Arc::new(handler));
    }

    // Getters
    pub async fn get_connection_state(&self) -> ConnectionState {
        *self.connection_state.read().await
    }

    pub async fn get_channel_name(&self) -> String {
        self.channel_name.read().await.clone()
    }

    pub async fn get_channel_id(&self) -> u64 {
        *self.channel_id.read().await
    }

    /// Build the WebSocket URL with parameters (public method for testing)
    pub async fn build_websocket_url(&self) -> Result<Url> {
        let base_url = if let Some(custom_url) = self.custom_websocket_url.read().await.clone() {
            custom_url
        } else {
            "wss://ws-us2.pusher.com/app/32cbd69e4b950bf97679".to_string()
        };

        let mut url = Url::parse(&base_url)?;

        let mut params: Vec<(String, String)> = vec![
            ("protocol".to_string(), "7".to_string()),
            ("client".to_string(), "js".to_string()),
            ("version".to_string(), "8.4.0".to_string()),
            ("flash".to_string(), "false".to_string()),
        ];

        if let Some(custom_params) = self.custom_websocket_params.read().await.clone() {
            for (key, value) in custom_params {
                params.push((key, value));
            }
        }

        for (key, value) in params {
            url.query_pairs_mut().append_pair(&key, &value);
        }

        Ok(url)
    }

    /// Set a custom WebSocket URL
    pub async fn set_websocket_url(&self, url: String) {
        *self.custom_websocket_url.write().await = Some(url);
        info!("Custom WebSocket URL set");
    }

    /// Set custom WebSocket parameters
    pub async fn set_websocket_params(&self, params: HashMap<String, String>) {
        *self.custom_websocket_params.write().await = Some(params);
        info!("Custom WebSocket params set");
    }

    /// Reset WebSocket configuration to defaults
    pub async fn reset_websocket_config(&self) {
        *self.custom_websocket_url.write().await = None;
        *self.custom_websocket_params.write().await = None;
        info!("WebSocket configuration reset to defaults");
    }

    /// Get channel ID from channel name (public method for testing)
    pub async fn get_channel_id_from_name(&self, channel_name: &str) -> Result<u64> {
        self.fetch_client.get_channel_id(channel_name).await.map_err(Into::into)
    }

    /// Get chatroom ID for WebSocket subscription
    pub async fn get_chatroom_id_from_name(&self, channel_name: &str) -> Result<u64> {
        self.fetch_client.get_chatroom_id(channel_name).await.map_err(|e| KickError::ChannelNotFound(format!("Failed to get chatroom ID: {}", e)))
    }

    /// Clear the channel ID cache





    pub async fn export_raw_messages(&self) -> Vec<String> {
        self.message_buffer.read().await.iter().cloned().collect()
    }

    pub async fn get_buffer_stats(&self) -> BufferStats {
        let buffer = self.message_buffer.read().await;
        BufferStats {
            total: buffer.len(),
            by_type: HashMap::new(),
            oldest_timestamp: None,
            newest_timestamp: None,
        }
    }

    /// Export raw messages by event type
    pub async fn export_raw_messages_by_event_type(&self, event_type: KickEventType) -> Vec<String> {
        let buffer = self.message_buffer.read().await;
        buffer
            .iter()
            .filter(|msg| {
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(msg) {
                    if let Some(event) = parsed.get("event").and_then(|e| e.as_str()) {
                        let mapped_type = MessageParser::map_kick_event_to_standard(event);
                        mapped_type == Some(event_type)
                    } else {
                        false
                    }
                } else {
                    false
                }
            })
            .cloned()
            .collect()
    }

    /// Export raw messages in range
    pub async fn export_raw_messages_in_range(&self, start_index: usize, end_index: Option<usize>) -> Vec<String> {
        let buffer = self.message_buffer.read().await;
        let end = end_index.unwrap_or(buffer.len());
        buffer
            .iter()
            .skip(start_index)
            .take(end - start_index)
            .cloned()
            .collect()
    }

    /// Clear raw messages by event type
    pub async fn clear_raw_messages_by_event_type(&self, event_type: KickEventType) -> Result<()> {
        let mut buffer = self.message_buffer.write().await;
        buffer.retain(|msg| {
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(msg) {
                if let Some(event) = parsed.get("event").and_then(|e| e.as_str()) {
                    let should_remove = match (event, event_type) {
                        ("ChatMessage", KickEventType::ChatMessage) => true,
                        ("MessageDeleted", KickEventType::MessageDeleted) => true,
                        ("UserBanned", KickEventType::UserBanned) => true,
                        ("UserUnbanned", KickEventType::UserUnbanned) => true,
                        ("Subscription", KickEventType::Subscription) => true,
                        ("GiftedSubscriptions", KickEventType::GiftedSubscriptions) => true,
                        ("PinnedMessageCreated", KickEventType::PinnedMessageCreated) => true,
                        ("StreamHost", KickEventType::StreamHost) => true,
                        ("PollUpdate", KickEventType::PollUpdate) => true,
                        ("PollDelete", KickEventType::PollDelete) => true,
                        _ => false,
                    };
                    !should_remove // Keep if not matching
                } else {
                    true // Keep if no event field
                }
            } else {
                true // Keep if not valid JSON
            }
        });
        Ok(())
    }

    /// Create a WebSocket request with realistic browser headers
    async fn create_websocket_request(&self, ws_url: &Url, options: &KickWebSocketOptions) -> Result<Request<()>> {
        // Generate realistic browser fingerprint
        let fingerprint = generate_browser_fingerprint();

        // Use custom User-Agent if provided, otherwise use rotating one
        let user_agent = if let Some(custom_ua) = &options.custom_user_agent {
            custom_ua.clone()
        } else if options.rotate_user_agent {
            get_rotating_user_agent().to_string()
        } else {
            fingerprint.user_agent.clone()
        };

        // Extract host from URL
        let host = format!("{}:{}", ws_url.host_str().unwrap_or("kick.com"),
                          ws_url.port().unwrap_or(443));

        // Create request with headers
        let request = Request::builder()
            .uri(ws_url.as_str())
            .header("Host", &host)
            .header("User-Agent", user_agent)
            .header("Accept", "*/*")
            .header("Accept-Language", "en-US,en;q=0.9")
            .header("Cache-Control", "no-cache")
            .header("Pragma", "no-cache")
            .header("Sec-Ch-Ua", fingerprint.sec_ch_ua)
            .header("Sec-Ch-Ua-Mobile", fingerprint.sec_ch_ua_mobile)
            .header("Sec-Ch-Ua-Platform", fingerprint.sec_ch_ua_platform)
            .header("Sec-Fetch-Dest", "websocket")
            .header("Sec-Fetch-Mode", "websocket")
            .header("Sec-Fetch-Site", "cross-site")
            .header("Origin", "https://kick.com")
            .header("Referer", "https://kick.com/")
            .header("Connection", "Upgrade")
            .header("Upgrade", "websocket")
            .header("Sec-WebSocket-Key", tokio_tungstenite::tungstenite::handshake::client::generate_key())
            .header("Sec-WebSocket-Version", "13")
            .body(())
                .map_err(|_e| KickError::WebSocket(tokio_tungstenite::tungstenite::Error::Http(
                    tokio_tungstenite::tungstenite::http::Response::builder()
                        .status(400)
                        .body(None)
                        .unwrap()
                )))?;

        Ok(request)
    }

    /// Set a custom User-Agent for WebSocket connections
    pub async fn set_custom_user_agent(&self, user_agent: String) {
        let mut options = self.options.write().await;
        options.custom_user_agent = Some(user_agent);
    }

    /// Enable or disable User-Agent rotation for WebSocket connections
    pub async fn set_user_agent_rotation(&self, enabled: bool) {
        let mut options = self.options.write().await;
        options.rotate_user_agent = enabled;
    }

    /// Get current User-Agent configuration
    pub async fn get_user_agent_config(&self) -> (Option<String>, bool) {
        let options = self.options.read().await;
        (options.custom_user_agent.clone(), options.rotate_user_agent)
    }

    /// Reset User-Agent to default behavior (rotation enabled)
    pub async fn reset_user_agent(&self) {
        let mut options = self.options.write().await;
        options.custom_user_agent = None;
        options.rotate_user_agent = true;
    }

    /// Check if the client is connected to WebSocket
    pub async fn is_connected(&self) -> bool {
        matches!(*self.connection_state.read().await, ConnectionState::Connected)
    }

    /// Check if any event handlers are registered
    pub async fn has_handlers(&self) -> bool {
        let handlers = self.event_handlers.read().await;
        !handlers.chat_message.is_empty() ||
        !handlers.message_deleted.is_empty() ||
        !handlers.user_banned.is_empty() ||
        !handlers.user_unbanned.is_empty() ||
        !handlers.subscription.is_empty() ||
        !handlers.gifted_subscriptions.is_empty() ||
        !handlers.pinned_message_created.is_empty() ||
        !handlers.stream_host.is_empty() ||
        !handlers.poll_update.is_empty() ||
        !handlers.poll_delete.is_empty() ||
        !handlers.raw_message.is_empty() ||
        !handlers.error.is_empty() ||
        !handlers.ready.is_empty() ||
        !handlers.disconnected.is_empty()
    }

    /// Connect to a channel with timeout
    pub async fn connect_with_timeout(&self, channel_name: &str, timeout: Duration) -> Result<()> {
        let connect_future = self.connect(channel_name);
        match tokio::time::timeout(timeout, connect_future).await {
            Ok(result) => result,
            Err(_) => Err(KickError::Connection(format!("Connection timeout after {:?}", timeout))),
        }
    }
}

impl Default for WebSocketManager {
    fn default() -> Self {
        Self::new()
    }
}
