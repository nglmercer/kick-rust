//! Message parser for processing Kick.com WebSocket events

use crate::types::*;
use regex::Regex;
use std::collections::HashMap;

/// Parser for processing WebSocket messages from Kick.com
pub struct MessageParser;

impl MessageParser {
    /// Helper function to extract data from either a JSON object or a JSON string
    fn extract_data<T: serde::de::DeserializeOwned>(data: serde_json::Value) -> Result<T> {
        match data {
            serde_json::Value::String(s) => {
                // Data is a string containing JSON - parse it
                serde_json::from_str(&s).map_err(|e| KickError::InvalidMessage(format!("Failed to parse JSON string: {}", e)))
            }
            serde_json::Value::Object(_) => {
                // Data is already a JSON object - parse it directly
                serde_json::from_value(data).map_err(|e| KickError::InvalidMessage(format!("Failed to parse JSON object: {}", e)))
            }
            _ => {
                Err(KickError::InvalidMessage("Data is neither a string nor an object".to_string()))
            }
        }
    }

    /// Parse a raw WebSocket message and return the processed event
    pub fn parse_message(raw_message: &str) -> Result<Option<ParsedMessage>> {
        // Validate that the message is not empty
        if raw_message.trim().is_empty() {
            return Err(KickError::InvalidMessage("Empty message".to_string()));
        }

        let message: WebSocketMessage = serde_json::from_str(raw_message)?;

        // Validate that the event exists
        if message.event.is_empty() {
            return Err(KickError::InvalidMessage("Empty event".to_string()));
        }

        // Ignore Pusher system events - return None instead of error
        if message.event.starts_with("pusher:") || message.event.starts_with("pusher_internal:") {
            return Ok(None);
        }

        // Map events to standard types and parse data
        match message.event.as_str() {
            "App\\Events\\ChatMessageEvent" => {
                let data: RawChatMessageData = Self::extract_data(message.data)?;
                Ok(Some(ParsedMessage {
                    r#type: KickEventType::ChatMessage,
                    data: KickEventData::ChatMessage(Self::parse_chat_message(data)),
                }))
            }
            "App\\Events\\MessageDeletedEvent" => {
                let data: RawMessageDeletedData = Self::extract_data(message.data)?;
                Ok(Some(ParsedMessage {
                    r#type: KickEventType::MessageDeleted,
                    data: KickEventData::MessageDeleted(Self::parse_message_deleted(data)),
                }))
            }
            "App\\Events\\UserBannedEvent" => {
                let data: RawUserBannedData = Self::extract_data(message.data)?;
                Ok(Some(ParsedMessage {
                    r#type: KickEventType::UserBanned,
                    data: KickEventData::UserBanned(Self::parse_user_banned(data)),
                }))
            }
            "App\\Events\\UserUnbannedEvent" => {
                let data: RawUserUnbannedData = Self::extract_data(message.data)?;
                Ok(Some(ParsedMessage {
                    r#type: KickEventType::UserUnbanned,
                    data: KickEventData::UserUnbanned(Self::parse_user_unbanned(data)),
                }))
            }
            "App\\Events\\SubscriptionEvent" => {
                let data: RawSubscriptionData = Self::extract_data(message.data)?;
                Ok(Some(ParsedMessage {
                    r#type: KickEventType::Subscription,
                    data: KickEventData::Subscription(Self::parse_subscription(data)),
                }))
            }
            "App\\Events\\GiftedSubscriptionsEvent" => {
                let data: RawGiftedSubscriptionsData = Self::extract_data(message.data)?;
                Ok(Some(ParsedMessage {
                    r#type: KickEventType::GiftedSubscriptions,
                    data: KickEventData::GiftedSubscriptions(Self::parse_gifted_subscriptions(data)),
                }))
            }
            "App\\Events\\PinnedMessageCreatedEvent" => {
                let data: RawPinnedMessageCreatedData = Self::extract_data(message.data)?;
                Ok(Some(ParsedMessage {
                    r#type: KickEventType::PinnedMessageCreated,
                    data: KickEventData::PinnedMessageCreated(Self::parse_pinned_message_created(data)),
                }))
            }
            "App\\Events\\StreamHostEvent" => {
                let data: RawStreamHostData = Self::extract_data(message.data)?;
                Ok(Some(ParsedMessage {
                    r#type: KickEventType::StreamHost,
                    data: KickEventData::StreamHost(Self::parse_stream_host(data)),
                }))
            }
            "App\\Events\\PollUpdateEvent" => {
                let data: RawPollUpdateData = Self::extract_data(message.data)?;
                Ok(Some(ParsedMessage {
                    r#type: KickEventType::PollUpdate,
                    data: KickEventData::PollUpdate(Self::parse_poll_update(data)),
                }))
            }
            "App\\Events\\PollDeleteEvent" => {
                let data: RawPollDeleteData = Self::extract_data(message.data)?;
                Ok(Some(ParsedMessage {
                    r#type: KickEventType::PollDelete,
                    data: KickEventData::PollDelete(Self::parse_poll_delete(data)),
                }))
            }
            _ => Err(KickError::UnknownEventType(message.event)),
        }
    }

    /// Parse a chat message event
    pub fn parse_chat_message(data: RawChatMessageData) -> ChatMessageEvent {
        ChatMessageEvent {
            id: data.id,
            content: Self::clean_emotes(&data.content),
            message_type: data.message_type,
            created_at: data.created_at,
            sender: data.sender,
            chatroom: data.chatroom,
        }
    }

    /// Parse a message deleted event
    pub fn parse_message_deleted(data: RawMessageDeletedData) -> MessageDeletedEvent {
        MessageDeletedEvent {
            message_id: data.message_id,
            chatroom_id: data.chatroom_id,
            event_type: "message_deleted".to_string(),
        }
    }

    /// Parse a user banned event
    pub fn parse_user_banned(data: RawUserBannedData) -> UserBannedEvent {
        let username = data.username
            .or(data.banned_username)
            .unwrap_or_else(|| "unknown".to_string());

        UserBannedEvent {
            username,
            event_type: "user_banned".to_string(),
        }
    }

    /// Parse a user unbanned event
    pub fn parse_user_unbanned(data: RawUserUnbannedData) -> UserUnbannedEvent {
        let username = data.username.unwrap_or_else(|| "unknown".to_string());

        UserUnbannedEvent {
            username,
            event_type: "user_unbanned".to_string(),
        }
    }

    /// Parse a subscription event
    pub fn parse_subscription(data: RawSubscriptionData) -> SubscriptionEvent {
        SubscriptionEvent {
            username: data.username,
            months: data.months,
            event_type: "subscription".to_string(),
        }
    }

    /// Parse a gifted subscriptions event
    pub fn parse_gifted_subscriptions(data: RawGiftedSubscriptionsData) -> GiftedSubscriptionsEvent {
        let gifter = data.gifted_by
            .or_else(|| {
                if let Some(gifter_value) = &data.gifter {
                    if let Some(gifter_str) = gifter_value.as_str() {
                        Some(gifter_str.to_string())
                    } else if let Some(gifter_obj) = gifter_value.as_object() {
                        gifter_obj.get("username")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .unwrap_or_else(|| "unknown".to_string());

        let recipients = data.recipients
            .into_iter()
            .map(|r| {
                if let Some(r_str) = r.as_str() {
                    r_str.to_string()
                } else if let Some(r_obj) = r.as_object() {
                    r_obj.get("username")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown")
                        .to_string()
                } else {
                    "unknown".to_string()
                }
            })
            .collect();

        GiftedSubscriptionsEvent {
            gifted_by: gifter,
            recipients,
            event_type: "gifted_subscriptions".to_string(),
        }
    }

    /// Parse a pinned message created event
    pub fn parse_pinned_message_created(data: RawPinnedMessageCreatedData) -> PinnedMessageCreatedEvent {
        PinnedMessageCreatedEvent {
            message: Self::parse_chat_message(data.message),
            event_type: "pinned_message_created".to_string(),
        }
    }

    /// Parse a stream host event
    pub fn parse_stream_host(data: RawStreamHostData) -> StreamHostEvent {
        let hoster = if let Some(hoster_str) = data.hoster.as_str() {
            hoster_str.to_string()
        } else if let Some(hoster_obj) = data.hoster.as_object() {
            hoster_obj.get("username")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string()
        } else {
            "unknown".to_string()
        };

        let hosted_channel = if let Some(hosted_str) = data.hosted_channel.as_str() {
            hosted_str.to_string()
        } else if let Some(hosted_obj) = data.hosted_channel.as_object() {
            hosted_obj.get("username")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string()
        } else {
            "unknown".to_string()
        };

        StreamHostEvent {
            hoster,
            hosted_channel,
            event_type: "stream_host".to_string(),
        }
    }

    /// Parse a poll update event
    pub fn parse_poll_update(data: RawPollUpdateData) -> PollUpdateEvent {
        let options = data.options
            .into_iter()
            .map(|opt| PollOption {
                id: opt.id,
                text: opt.text,
                votes: opt.votes.unwrap_or(0),
            })
            .collect();

        PollUpdateEvent {
            poll_id: data.id,
            question: data.question,
            options,
            event_type: "poll_update".to_string(),
        }
    }

    /// Parse a poll delete event
    pub fn parse_poll_delete(data: RawPollDeleteData) -> PollDeleteEvent {
        PollDeleteEvent {
            poll_id: data.id,
            event_type: "poll_delete".to_string(),
        }
    }

    /// Clean emote codes from message content
    pub fn clean_emotes(content: &str) -> String {
        if content.is_empty() {
            return String::new();
        }

        // Replace emote codes like [emote:123:emoteName] with the emote name
        let emote_regex = Regex::new(r"\[emote:(\d+):(\w+)\]").unwrap();
        emote_regex.replace_all(content, "$2").to_string()
    }

    /// Check if a message is valid
    pub fn is_valid_message(message: &str) -> bool {
        if message.trim().is_empty() {
            return false;
        }

        match serde_json::from_str::<WebSocketMessage>(message) {
            Ok(parsed) => {
                !parsed.event.is_empty() && parsed.data != serde_json::Value::Null
            }
            Err(_) => false,
        }
    }

    /// Check if a message is a Kick event (not Pusher system event)
    pub fn is_kick_event(message: &str) -> bool {
        if let Ok(parsed) = serde_json::from_str::<WebSocketMessage>(message) {
            !parsed.event.starts_with("pusher:") &&
            !parsed.event.starts_with("pusher_internal:") &&
            !parsed.event.is_empty()
        } else {
            false
        }
    }

    /// Extract the event type from a raw message
    pub fn extract_event_type(raw_message: &str) -> Result<String> {
        if raw_message.trim().is_empty() {
            return Err(KickError::InvalidMessage("Empty message".to_string()));
        }

        let message: WebSocketMessage = serde_json::from_str(raw_message)?;

        if message.event.is_empty() {
            return Err(KickError::InvalidMessage("Empty event".to_string()));
        }

        // Ignore Pusher system events
        if message.event.starts_with("pusher:") || message.event.starts_with("pusher_internal:") {
            return Err(KickError::InvalidMessage(format!("Pusher system event: {}", message.event)));
        }

        Ok(message.event)
    }

    /// Map Kick event names to standard event types
    pub fn map_kick_event_to_standard(event_name: &str) -> Option<KickEventType> {
        let event_map: HashMap<&str, KickEventType> = HashMap::from([
            ("App\\Events\\ChatMessageEvent", KickEventType::ChatMessage),
            ("App\\Events\\MessageDeletedEvent", KickEventType::MessageDeleted),
            ("App\\Events\\UserBannedEvent", KickEventType::UserBanned),
            ("App\\Events\\UserUnbannedEvent", KickEventType::UserUnbanned),
            ("App\\Events\\SubscriptionEvent", KickEventType::Subscription),
            ("App\\Events\\GiftedSubscriptionsEvent", KickEventType::GiftedSubscriptions),
            ("App\\Events\\PinnedMessageCreatedEvent", KickEventType::PinnedMessageCreated),
            ("App\\Events\\StreamHostEvent", KickEventType::StreamHost),
            ("App\\Events\\PollUpdateEvent", KickEventType::PollUpdate),
            ("App\\Events\\PollDeleteEvent", KickEventType::PollDelete),
        ]);

        event_map.get(event_name).cloned()
    }
}

/// Parse a custom message structure from raw JSON data
pub fn parse_custom_message<T: serde::de::DeserializeOwned>(raw_message: &str) -> crate::types::Result<T> {
    serde_json::from_str(raw_message)
        .map_err(|e| KickError::InvalidMessage(format!("Failed to parse custom message: {}", e)))
}

/// Extract a field from a RawMessage using dot notation
pub fn extract_field(raw_message: &RawMessage, field_path: &str) -> Option<serde_json::Value> {
    // Parse the raw JSON
    let parsed: serde_json::Value = serde_json::from_str(&raw_message.raw_json).ok()?;

    // Split the field path by dots and navigate the JSON
    let mut current = &parsed;
    for part in field_path.split('.') {
        match current {
            serde_json::Value::Object(map) => {
                current = map.get(part)?;
            }
            serde_json::Value::Array(arr) => {
                if let Ok(index) = part.parse::<usize>() {
                    current = arr.get(index)?;
                } else {
                    return None;
                }
            }
            _ => return None,
        }
    }

    Some(current.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clean_emotes() {
        let content = "Hello [emote:123:wave] world [emote:456:heart]!";
        let cleaned = MessageParser::clean_emotes(content);
        assert_eq!(cleaned, "Hello wave world heart!");
    }

    #[test]
    fn test_empty_content() {
        let cleaned = MessageParser::clean_emotes("");
        assert_eq!(cleaned, "");
    }

    #[test]
    fn test_is_valid_message() {
        let valid_json = r#"{"event":"test","data":{"key":"value"}}"#;
        let invalid_json = r#"{"event":"test"}"#;
        let empty = "";

        assert!(MessageParser::is_valid_message(valid_json));
        assert!(!MessageParser::is_valid_message(invalid_json));
        assert!(!MessageParser::is_valid_message(empty));
    }

    #[test]
    fn test_extract_event_type() {
        let message = r#"{"event":"App\\Events\\ChatMessageEvent","data":{}}"#;
        let event_type = MessageParser::extract_event_type(message).unwrap();
        assert_eq!(event_type, "App\\Events\\ChatMessageEvent");

        // Test that Pusher events return None
        let pusher_message = r#"{"event":"pusher:connection_established","data":"{}"}"#;
        let result = MessageParser::parse_message(pusher_message);
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn test_map_kick_event_to_standard() {
        assert_eq!(
            MessageParser::map_kick_event_to_standard("App\\Events\\ChatMessageEvent"),
            Some(KickEventType::ChatMessage)
        );
        assert_eq!(
            MessageParser::map_kick_event_to_standard("UnknownEvent"),
            None
        );
    }
}
