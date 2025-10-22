//! Parsing utilities for HTML and JSON responses

use crate::fetch::types::{ChannelInfo, UserInfo, ChatroomInfo, FetchError, FetchResult};
use regex::Regex;
// use scraper::Html; // No longer needed since we only use curl strategy
use serde_json::Value;

use tracing::{debug, warn};

/// HTML parser for extracting channel data from web pages
pub struct HtmlParser {
    channel_regex: Regex,
    user_regex: Regex,
    chatroom_regex: Regex,
    json_regex: Regex,
}

impl HtmlParser {
    /// Create a new HTML parser with pre-compiled regex patterns
    pub fn new() -> Result<Self, FetchError> {
        Ok(Self {
            channel_regex: Regex::new(r#""id":(\d+),"slug":"([^"]+)""#)
                .map_err(|e| FetchError::Json(format!("Failed to compile channel regex: {}", e)))?,
            user_regex: Regex::new(r#""id":(\d+),"username":"([^"]+)","display_name":"([^"]*)""#)
                .map_err(|e| FetchError::Json(format!("Failed to compile user regex: {}", e)))?,
            chatroom_regex: Regex::new(r#""chatroom":\{"id":(\d+),"channel_id":(\d+),"name":"([^"]+)""#)
                .map_err(|e| FetchError::Json(format!("Failed to compile chatroom regex: {}", e)))?,
            json_regex: Regex::new(r#"<script[^>]*>window\.__NUXT__\s*=\s*({.*?});?</script>"#)
                .map_err(|e| FetchError::Json(format!("Failed to compile JSON regex: {}", e)))?,
        })
    }

    /// Parse channel information from HTML content
    pub fn parse_channel_from_html(&self, html: &str) -> FetchResult<ChannelInfo> {
        debug!("Parsing channel from HTML content (length: {})", html.len());

        // Try to extract Nuxt state first
        if let Ok(channel) = self.extract_from_nuxt_state(html) {
            return Ok(channel);
        }

        // Fallback to regex-based extraction
        self.extract_with_regex(html)
    }

    /// Extract channel data from Nuxt state
    fn extract_from_nuxt_state(&self, html: &str) -> FetchResult<ChannelInfo> {
        let captures = self.json_regex.captures(html)
            .ok_or_else(|| FetchError::InvalidResponse)?;

        let json_str = captures.get(1)
            .ok_or_else(|| FetchError::InvalidResponse)?
            .as_str();

        let nuxt_data: Value = serde_json::from_str(json_str)
            .map_err(|e| FetchError::Json(format!("Failed to parse Nuxt state: {}", e)))?;

        self.extract_channel_from_nuxt_data(&nuxt_data)
    }

    /// Extract channel information from Nuxt data structure
    fn extract_channel_from_nuxt_data(&self, nuxt_data: &Value) -> FetchResult<ChannelInfo> {
        // Navigate through the Nuxt structure to find channel data
        if let Some(data) = nuxt_data.get("data") {
            if let Some(channel_data) = data.get(0) {
                if let Some(channel) = channel_data.get("channel") {
                    return self.parse_channel_json(channel);
                }
            }
        }

        // Try alternative paths
        if let Some(pinia) = nuxt_data.get("pinia") {
            for (_, value) in pinia.as_object().unwrap_or(&serde_json::Map::new()) {
                if let Some(channel) = value.get("channel") {
                    return self.parse_channel_json(channel);
                }
            }
        }

        Err(FetchError::InvalidResponse)
    }

    /// Parse channel information from JSON value
    fn parse_channel_json(&self, channel_json: &Value) -> FetchResult<ChannelInfo> {
        let id = channel_json.get("id")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| FetchError::InvalidResponse)?;

        let slug = channel_json.get("slug")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let user = if let Some(user_json) = channel_json.get("user") {
            Some(self.parse_user_json(user_json)?)
        } else {
            None
        };

        let chatroom = if let Some(chatroom_json) = channel_json.get("chatroom") {
            Some(self.parse_chatroom_json(chatroom_json)?)
        } else {
            None
        };

        Ok(ChannelInfo {
            id,
            slug,
            title: channel_json.get("title").and_then(|v| v.as_str()).map(String::from),
            followers_count: channel_json.get("followers_count").and_then(|v| v.as_u64()),
            subscribers_count: channel_json.get("subscribers_count").and_then(|v| v.as_u64()),
            is_live: channel_json.get("is_live").and_then(|v| v.as_bool()).unwrap_or(false),
            viewers_count: channel_json.get("viewers_count").and_then(|v| v.as_u64()),
            category: channel_json.get("category").and_then(|v| v.as_str()).map(String::from),
            tags: channel_json.get("tags").and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect()),
            language: channel_json.get("language").and_then(|v| v.as_str()).map(String::from),
            user,
            chatroom,
        })
    }

    /// Parse user information from JSON
    fn parse_user_json(&self, user_json: &Value) -> FetchResult<UserInfo> {
        Ok(UserInfo {
            id: user_json.get("id").and_then(|v| v.as_u64()).ok_or_else(|| FetchError::InvalidResponse)?,
            username: user_json.get("username").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            display_name: user_json.get("display_name").and_then(|v| v.as_str()).map(String::from),
            avatar_url: user_json.get("avatar_url").and_then(|v| v.as_str()).map(String::from),
            bio: user_json.get("bio").and_then(|v| v.as_str()).map(String::from),
            created_at: user_json.get("created_at").and_then(|v| v.as_str()).map(String::from),
        })
    }

    /// Parse chatroom information from JSON
    fn parse_chatroom_json(&self, chatroom_json: &Value) -> FetchResult<ChatroomInfo> {
        Ok(ChatroomInfo {
            id: chatroom_json.get("id").and_then(|v| v.as_u64()).ok_or_else(|| FetchError::InvalidResponse)?,
            channel_id: chatroom_json.get("channel_id").and_then(|v| v.as_u64()).unwrap_or(0),
            name: chatroom_json.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            chatroom_type: chatroom_json.get("type").and_then(|v| v.as_str()).map(String::from),
            slow_mode: chatroom_json.get("slow_mode").and_then(|v| v.as_u64()).map(|v| v as u32),
        })
    }

    /// Extract channel information using regex patterns
    fn extract_with_regex(&self, html: &str) -> FetchResult<ChannelInfo> {
        let mut channel_info = ChannelInfo {
            id: 0,
            slug: String::new(),
            title: None,
            followers_count: None,
            subscribers_count: None,
            is_live: false,
            viewers_count: None,
            category: None,
            tags: None,
            language: None,
            user: None,
            chatroom: None,
        };

        // Extract channel info
        if let Some(captures) = self.channel_regex.captures(html) {
            if let (Some(id), Some(slug)) = (captures.get(1), captures.get(2)) {
                channel_info.id = id.as_str().parse().unwrap_or(0);
                channel_info.slug = slug.as_str().to_string();
            }
        }

        // Extract user info
        if let Some(captures) = self.user_regex.captures(html) {
            if let (Some(id), Some(username)) = (captures.get(1), captures.get(2)) {
                channel_info.user = Some(UserInfo {
                    id: id.as_str().parse().unwrap_or(0),
                    username: username.as_str().to_string(),
                    display_name: captures.get(3).map(|m| m.as_str().to_string()),
                    avatar_url: None,
                    bio: None,
                    created_at: None,
                });
            }
        }

        // Extract chatroom info
        if let Some(captures) = self.chatroom_regex.captures(html) {
            if let (Some(id), Some(channel_id), Some(name)) = (captures.get(1), captures.get(2), captures.get(3)) {
                channel_info.chatroom = Some(ChatroomInfo {
                    id: id.as_str().parse().unwrap_or(0),
                    channel_id: channel_id.as_str().parse().unwrap_or(0),
                    name: name.as_str().to_string(),
                    chatroom_type: None,
                    slow_mode: None,
                });
            }
        }

        if channel_info.id == 0 {
            return Err(FetchError::ChannelNotFound("Could not extract channel ID".to_string()));
        }

        Ok(channel_info)
    }
}

impl Default for HtmlParser {
    fn default() -> Self {
        Self::new().unwrap_or_else(|_| {
            warn!("Failed to create HtmlParser with regex, using fallback");
            Self {
                channel_regex: Regex::new(r#""id":(\d+)""#).unwrap(),
                user_regex: Regex::new(r#""username":"([^"]+)""#).unwrap(),
                chatroom_regex: Regex::new(r#""chatroom":\{"id":(\d+)""#).unwrap(),
                json_regex: Regex::new(r#"window\.__NUXT__"#).unwrap(),
            }
        })
    }
}

/// JSON parser for API responses
pub struct JsonParser;

impl JsonParser {
    /// Parse channel information from Kick API JSON response
    pub fn parse_channel_response(json_str: &str) -> FetchResult<ChannelInfo> {
        debug!("Parsing channel from JSON response (length: {})", json_str.len());

        let json: Value = serde_json::from_str(json_str)
            .map_err(|e| FetchError::Json(format!("Failed to parse JSON: {}", e)))?;

        // Handle different response formats
        if let Some(data) = json.get("data") {
            Self::parse_channel_json(data)
        } else if json.get("id").is_some() {
            Self::parse_channel_json(&json)
        } else {
            Err(FetchError::InvalidResponse)
        }
    }

    /// Parse channel from JSON value
    fn parse_channel_json(json: &Value) -> FetchResult<ChannelInfo> {
        let id = json.get("id")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| FetchError::InvalidResponse)?;

        let slug = json.get("slug")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let user = if let Some(user_json) = json.get("user") {
            Some(Self::parse_user_json(user_json)?)
        } else {
            None
        };

        let chatroom = if let Some(chatroom_json) = json.get("chatroom") {
            Some(Self::parse_chatroom_json(chatroom_json)?)
        } else {
            None
        };

        Ok(ChannelInfo {
            id,
            slug,
            title: json.get("title").and_then(|v| v.as_str()).map(String::from),
            followers_count: json.get("followers_count").and_then(|v| v.as_u64()),
            subscribers_count: json.get("subscribers_count").and_then(|v| v.as_u64()),
            is_live: json.get("is_live").and_then(|v| v.as_bool()).unwrap_or(false),
            viewers_count: json.get("viewers_count").and_then(|v| v.as_u64()),
            category: json.get("category").and_then(|v| v.as_str()).map(String::from),
            tags: json.get("tags").and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect()),
            language: json.get("language").and_then(|v| v.as_str()).map(String::from),
            user,
            chatroom,
        })
    }

    /// Parse user information from JSON
    fn parse_user_json(user_json: &Value) -> FetchResult<UserInfo> {
        Ok(UserInfo {
            id: user_json.get("id").and_then(|v| v.as_u64()).ok_or_else(|| FetchError::InvalidResponse)?,
            username: user_json.get("username").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            display_name: user_json.get("display_name").and_then(|v| v.as_str()).map(String::from),
            avatar_url: user_json.get("avatar_url").and_then(|v| v.as_str()).map(String::from),
            bio: user_json.get("bio").and_then(|v| v.as_str()).map(String::from),
            created_at: user_json.get("created_at").and_then(|v| v.as_str()).map(String::from),
        })
    }

    /// Parse chatroom information from JSON
    fn parse_chatroom_json(chatroom_json: &Value) -> FetchResult<ChatroomInfo> {
        Ok(ChatroomInfo {
            id: chatroom_json.get("id").and_then(|v| v.as_u64()).ok_or_else(|| FetchError::InvalidResponse)?,
            channel_id: chatroom_json.get("channel_id").and_then(|v| v.as_u64()).unwrap_or(0),
            name: chatroom_json.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            chatroom_type: chatroom_json.get("type").and_then(|v| v.as_str()).map(String::from),
            slow_mode: chatroom_json.get("slow_mode").and_then(|v| v.as_u64()).map(|v| v as u32),
        })
    }

    /// Extract channel ID from various JSON formats
    pub fn extract_channel_id(json_str: &str) -> FetchResult<u64> {
        let json: Value = serde_json::from_str(json_str)
            .map_err(|e| FetchError::Json(format!("Failed to parse JSON: {}", e)))?;

        // Try different paths for channel ID
        if let Some(id) = json.get("id").and_then(|v| v.as_u64()) {
            return Ok(id);
        }

        if let Some(data) = json.get("data") {
            if let Some(id) = data.get("id").and_then(|v| v.as_u64()) {
                return Ok(id);
            }
        }

        if let Some(channel) = json.get("channel") {
            if let Some(id) = channel.get("id").and_then(|v| v.as_u64()) {
                return Ok(id);
            }
        }

        Err(FetchError::InvalidResponse)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_json_parser_valid_response() {
        let json = r#"
        {
            "id": 12345,
            "slug": "testchannel",
            "title": "Test Channel",
            "is_live": true,
            "viewers_count": 100,
            "user": {
                "id": 12345,
                "username": "testuser",
                "display_name": "Test User"
            },
            "chatroom": {
                "id": 54321,
                "channel_id": 12345,
                "name": "testchannel"
            }
        }
        "#;

        let result = JsonParser::parse_channel_response(json);
        assert!(result.is_ok());

        let channel = result.unwrap();
        assert_eq!(channel.id, 12345);
        assert_eq!(channel.slug, "testchannel");
        assert_eq!(channel.title, Some("Test Channel".to_string()));
        assert!(channel.is_live);
        assert_eq!(channel.viewers_count, Some(100));
    }

    #[test]
    fn test_extract_channel_id() {
        let json = r#"{"id": 12345, "slug": "test"}"#;
        assert_eq!(JsonParser::extract_channel_id(json).unwrap(), 12345);

        let json = r#"{"data": {"id": 67890, "slug": "test"}}"#;
        assert_eq!(JsonParser::extract_channel_id(json).unwrap(), 67890);
    }
}
