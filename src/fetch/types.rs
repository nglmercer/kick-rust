//! Types specific to the fetch module

use serde::{Deserialize, Serialize};

/// Result type for fetch operations
pub type FetchResult<T> = Result<T, FetchError>;

/// Error types for fetch operations
#[derive(Debug, Clone, thiserror::Error)]
pub enum FetchError {
    #[error("Network error: {0}")]
    Network(String),

    #[error("HTTP request failed: {0}")]
    Http(String),

    #[error("JSON parsing error: {0}")]
    Json(String),

    #[error("Channel not found: {0}")]
    ChannelNotFound(String),

    #[error("Rate limit exceeded")]
    RateLimit,

    #[error("Invalid response format")]
    InvalidResponse,

    #[error("Timeout error")]
    Timeout,

    #[error("Authentication required")]
    AuthenticationRequired,
}

/// Configuration for the HTTP client
#[derive(Debug, Clone)]
pub struct ClientConfig {
    /// User agent string for requests
    pub user_agent: Option<String>,
    /// Request timeout in seconds
    pub timeout_seconds: u64,
    /// Maximum number of retries for failed requests
    pub max_retries: u32,
    /// Delay between retries in milliseconds
    pub retry_delay_ms: u64,
    /// Enable request/response logging
    pub enable_logging: bool,
    /// Custom headers to include in all requests
    pub custom_headers: Vec<(String, String)>,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            user_agent: Some(crate::fetch::useragent::get_latest_chrome_user_agent().to_string()),
            timeout_seconds: 30,
            max_retries: 3,
            retry_delay_ms: 1000,
            enable_logging: false,
            custom_headers: Vec::new(),
        }
    }
}

/// Channel information from the Kick API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelInfo {
    /// Channel ID
    pub id: u64,
    /// Channel slug/name
    pub slug: String,
    /// Channel title
    pub title: Option<String>,
    /// Number of followers
    pub followers_count: Option<u64>,
    /// Number of subscribers
    pub subscribers_count: Option<u64>,
    /// Is channel currently live
    pub is_live: bool,
    /// Viewer count if live
    pub viewers_count: Option<u64>,
    /// Stream category
    pub category: Option<String>,
    /// Stream tags
    pub tags: Option<Vec<String>>,
    /// Channel language
    pub language: Option<String>,
    /// User information
    pub user: Option<UserInfo>,
    /// Chatroom information
    pub chatroom: Option<ChatroomInfo>,
}

/// User information associated with a channel
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfo {
    /// User ID
    pub id: u64,
    /// Username
    pub username: String,
    /// Display name
    pub display_name: Option<String>,
    /// Avatar URL
    pub avatar_url: Option<String>,
    /// Bio/description
    pub bio: Option<String>,
    /// Account creation date
    pub created_at: Option<String>,
}

/// Chatroom information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatroomInfo {
    /// Chatroom ID
    pub id: u64,
    /// Channel ID
    pub channel_id: u64,
    /// Chatroom name
    pub name: String,
    /// Chatroom type
    pub chatroom_type: Option<String>,
    /// Slow mode delay in seconds
    pub slow_mode: Option<u32>,
}

/// Strategy configuration for different fetch approaches
#[derive(Debug, Clone)]
pub struct StrategyConfig {
    /// Random delay range in milliseconds (for rate limiting)
    pub random_delay_range: (u64, u64),
}

impl Default for StrategyConfig {
    fn default() -> Self {
        Self {
            random_delay_range: (100, 500),
        }
    }
}

/// Request context for tracking fetch operations
#[derive(Debug, Clone)]
pub struct RequestContext {
    /// Channel name being requested
    pub channel_name: String,
    /// Strategy being used
    pub strategy: String,
    /// Attempt number
    pub attempt: u32,
    /// Request timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// User agent used
    pub user_agent: String,
}

impl RequestContext {
    pub fn new(channel_name: &str, strategy: &str) -> Self {
        Self {
            channel_name: channel_name.to_string(),
            strategy: strategy.to_string(),
            attempt: 1,
            timestamp: chrono::Utc::now(),
            user_agent: "unknown".to_string(),
        }
    }
}

/// Metrics for fetch operations
#[derive(Debug, Clone, Default)]
pub struct FetchMetrics {
    /// Total requests made
    pub total_requests: u64,
    /// Successful requests
    pub successful_requests: u64,
    /// Failed requests
    pub failed_requests: u64,
    /// Average response time in milliseconds
    pub avg_response_time_ms: f64,
    /// Cache hits
    pub cache_hits: u64,
    /// Rate limit hits
    pub rate_limit_hits: u64,
}

impl FetchMetrics {
    pub fn success_rate(&self) -> f64 {
        if self.total_requests == 0 {
            0.0
        } else {
            self.successful_requests as f64 / self.total_requests as f64 * 100.0
        }
    }

    pub fn record_success(&mut self, response_time_ms: u64) {
        self.total_requests += 1;
        self.successful_requests += 1;
        self.update_avg_response_time(response_time_ms);
    }

    pub fn record_failure(&mut self) {
        self.total_requests += 1;
        self.failed_requests += 1;
    }

    fn update_avg_response_time(&mut self, response_time_ms: u64) {
        if self.total_requests == 1 {
            self.avg_response_time_ms = response_time_ms as f64;
        } else {
            self.avg_response_time_ms = (self.avg_response_time_ms * (self.total_requests - 1) as f64 + response_time_ms as f64) / self.total_requests as f64;
        }
    }
}
