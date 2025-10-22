//! Main HTTP client for Kick API operations
//!
//! This module provides a clean, modern HTTP client that focuses on the official
//! Kick API endpoint: https://kick.com/api/v2/channels/{channel_name}
//! with multiple fallback strategies for reliability.

use crate::fetch::{
    strategies::StrategyManager,
    types::{ChannelInfo, ClientConfig, FetchResult, FetchError, RequestContext, FetchMetrics, StrategyConfig},
};
use reqwest::{Client, redirect::Policy};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{info, warn, debug, error};
use std::collections::HashMap;

/// Main HTTP client for Kick API operations
pub struct KickApiClient {
    /// HTTP client for making requests
    client: Client,
    /// Strategy manager for different fetching approaches
    strategy_manager: Arc<StrategyManager>,
    /// Client configuration
    config: ClientConfig,
    /// Strategy configuration
    strategy_config: StrategyConfig,
    /// Request metrics
    metrics: Arc<RwLock<FetchMetrics>>,
    /// Request cache (optional, for rate limiting)
    cache: Arc<RwLock<HashMap<String, CachedChannel>>>,
}

/// Cached channel information
#[derive(Debug, Clone)]
struct CachedChannel {
    channel: ChannelInfo,
    timestamp: Instant,
    ttl: Duration,
}

impl CachedChannel {
    fn new(channel: ChannelInfo, ttl: Duration) -> Self {
        Self {
            channel,
            timestamp: Instant::now(),
            ttl,
        }
    }

    fn is_expired(&self) -> bool {
        self.timestamp.elapsed() > self.ttl
    }
}

impl KickApiClient {
    /// Create a new Kick API client with default configuration
    pub fn new() -> FetchResult<Self> {
        let config = ClientConfig::default();
        Self::with_config(config)
    }

    /// Create a new Kick API client with custom configuration
    pub fn with_config(config: ClientConfig) -> FetchResult<Self> {
        let mut builder = Client::builder()
            .timeout(Duration::from_secs(config.timeout_seconds))
            .cookie_store(true)
            .redirect(Policy::limited(5));

        // Set user agent
        let user_agent = config.user_agent.as_deref()
            .unwrap_or("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36");
        builder = builder.user_agent(user_agent);

        // Build the HTTP client
        let client = builder.build()
            .map_err(|e| FetchError::Network(format!("Failed to build HTTP client: {}", e)))?;

        // Create strategy manager
        let strategy_config = StrategyConfig::default();
        let strategy_manager = Arc::new(StrategyManager::new()?);

        Ok(Self {
            client,
            strategy_manager,
            config,
            strategy_config,
            metrics: Arc::new(RwLock::new(FetchMetrics::default())),
            cache: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Create a client with both client and strategy configurations
    pub fn with_full_config(
        client_config: ClientConfig,
        strategy_config: StrategyConfig,
    ) -> FetchResult<Self> {
        let mut builder = Client::builder()
            .timeout(Duration::from_secs(client_config.timeout_seconds))
            .cookie_store(true)
            .redirect(Policy::limited(5));

        // Set user agent
        let user_agent = client_config.user_agent.as_deref()
            .unwrap_or("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36");
        builder = builder.user_agent(user_agent);

        // Add custom headers using default_headers
        let mut headers = reqwest::header::HeaderMap::new();
        for (key, value) in &client_config.custom_headers {
            if let (Ok(name), Ok(val)) = (
                reqwest::header::HeaderName::from_bytes(key.as_bytes()),
                reqwest::header::HeaderValue::from_str(value),
            ) {
                headers.insert(name, val);
            }
        }
        builder = builder.default_headers(headers);

        // Build the HTTP client
        let client = builder.build()
            .map_err(|e| FetchError::Network(format!("Failed to build HTTP client: {}", e)))?;

        // Create strategy manager with custom config
        let strategy_manager = Arc::new(StrategyManager::with_config(&strategy_config)?);

        Ok(Self {
            client,
            strategy_manager,
            config: client_config,
            strategy_config,
            metrics: Arc::new(RwLock::new(FetchMetrics::default())),
            cache: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Fetch channel information by channel name
    /// This is the main method that uses the official API endpoint
    pub async fn get_channel(&self, channel_name: &str) -> FetchResult<ChannelInfo> {
        let start_time = Instant::now();
        let context = RequestContext::new(channel_name, "auto");

        info!("Fetching channel: {}", channel_name);

        // Check cache first
        if let Some(cached) = self.check_cache(channel_name).await {
            debug!("Channel {} found in cache", channel_name);
            self.record_metrics(start_time, true).await;
            return Ok(cached);
        }

        // Try to fetch with retries
        let result = self.fetch_with_retries(channel_name, &context).await;

        // Record metrics
        let success = result.is_ok();
        self.record_metrics(start_time, success).await;

        // Cache successful results
        if let Ok(ref channel) = result {
            self.cache_channel(channel_name, channel).await;
        }

        result
    }

    /// Fetch channel information with automatic retries
    async fn fetch_with_retries(&self, channel_name: &str, _context: &RequestContext) -> FetchResult<ChannelInfo> {
        let mut last_error = None;

        for attempt in 1..=self.config.max_retries {
            debug!("Attempt {} for channel {}", attempt, channel_name);

            match self.strategy_manager.fetch_channel(&self.client, channel_name, &self.strategy_config).await {
                Ok(channel) => {
                    info!("Successfully fetched channel {} on attempt {}", channel_name, attempt);
                    return Ok(channel);
                }
                Err(e) => {
                    warn!("Attempt {} failed for channel {}: {}", attempt, channel_name, e);
                    last_error = Some(e.clone());

                    // Don't retry on certain errors
                    match &e {
                        FetchError::ChannelNotFound(_) => break,
                        FetchError::AuthenticationRequired => break,
                        _ => {
                            // Add delay before retry
                            if attempt < self.config.max_retries {
                                let delay = self.config.retry_delay_ms * attempt as u64;
                                debug!("Waiting {}ms before retry", delay);
                                tokio::time::sleep(Duration::from_millis(delay)).await;
                            }
                        }
                    }
                }
            }
        }

        Err(last_error.unwrap_or_else(|| FetchError::ChannelNotFound(format!("All retries failed for channel: {}", channel_name))))
    }

    /// Check if channel exists (faster version that only gets basic info)
    pub async fn channel_exists(&self, channel_name: &str) -> FetchResult<bool> {
        match self.get_channel(channel_name).await {
            Ok(_) => Ok(true),
            Err(FetchError::ChannelNotFound(_)) => Ok(false),
            Err(e) => Err(e),
        }
    }

    /// Get channel ID only (lightweight request)
    pub async fn get_channel_id(&self, channel_name: &str) -> FetchResult<u64> {
        let channel = self.get_channel(channel_name).await?;
        Ok(channel.id)
    }

    /// Get chatroom ID only (for WebSocket subscription)
    pub async fn get_chatroom_id(&self, channel_name: &str) -> FetchResult<u64> {
        let channel = self.get_channel(channel_name).await?;
        channel.chatroom
            .map(|chatroom| chatroom.id)
            .ok_or_else(|| FetchError::InvalidResponse)
    }

    /// Get multiple channels in parallel
    pub async fn get_channels(&self, channel_names: &[&str]) -> Vec<FetchResult<ChannelInfo>> {
        let futures: Vec<_> = channel_names.iter()
            .map(|&name| self.get_channel(name))
            .collect();

        futures_util::future::join_all(futures).await
    }

    /// Get current metrics
    pub async fn get_metrics(&self) -> FetchMetrics {
        self.metrics.read().await.clone()
    }

    /// Reset metrics
    pub async fn reset_metrics(&self) {
        *self.metrics.write().await = FetchMetrics::default();
    }

    /// Clear cache
    pub async fn clear_cache(&self) {
        self.cache.write().await.clear();
    }

    /// Get available strategy names
    pub fn strategy_names(&self) -> Vec<&'static str> {
        self.strategy_manager.strategy_names()
    }

    /// Test connection to Kick API
    pub async fn test_connection(&self) -> FetchResult<()> {
        let test_channel = "kick"; // Official Kick channel
        match self.get_channel(test_channel).await {
            Ok(_) => {
                info!("Connection test successful");
                Ok(())
            }
            Err(e) => {
                error!("Connection test failed: {}", e);
                Err(e)
            }
        }
    }

    /// Check cache for channel information
    async fn check_cache(&self, channel_name: &str) -> Option<ChannelInfo> {
        let cache = self.cache.read().await;
        cache.get(channel_name)
            .filter(|cached| !cached.is_expired())
            .map(|cached| cached.channel.clone())
    }

    /// Cache channel information
    async fn cache_channel(&self, channel_name: &str, channel: &ChannelInfo) {
        let ttl = Duration::from_secs(300); // 5 minutes cache
        let cached = CachedChannel::new(channel.clone(), ttl);

        let mut cache = self.cache.write().await;
        cache.insert(channel_name.to_string(), cached);

        // Clean up expired entries
        cache.retain(|_, cached| !cached.is_expired());
    }

    /// Record metrics for a request
    async fn record_metrics(&self, start_time: Instant, success: bool) {
        let response_time = start_time.elapsed().as_millis() as u64;
        let mut metrics = self.metrics.write().await;

        if success {
            metrics.record_success(response_time);
        } else {
            metrics.record_failure();
        }
    }
}

impl Default for KickApiClient {
    fn default() -> Self {
        Self::new().expect("Failed to create default KickApiClient")
    }
}

/// Builder for creating KickApiClient with custom configuration
pub struct KickApiClientBuilder {
    client_config: ClientConfig,
    strategy_config: StrategyConfig,
}

impl KickApiClientBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self {
            client_config: ClientConfig::default(),
            strategy_config: StrategyConfig::default(),
        }
    }

    /// Set user agent
    pub fn user_agent<S: Into<String>>(mut self, user_agent: S) -> Self {
        self.client_config.user_agent = Some(user_agent.into());
        self
    }

    /// Set timeout in seconds
    pub fn timeout(mut self, seconds: u64) -> Self {
        self.client_config.timeout_seconds = seconds;
        self
    }

    /// Set maximum retries
    pub fn max_retries(mut self, retries: u32) -> Self {
        self.client_config.max_retries = retries;
        self
    }

    /// Set retry delay in milliseconds
    pub fn retry_delay(mut self, delay_ms: u64) -> Self {
        self.client_config.retry_delay_ms = delay_ms;
        self
    }

    /// Enable/disable logging
    pub fn enable_logging(mut self, enable: bool) -> Self {
        self.client_config.enable_logging = enable;
        self
    }

    /// Add custom header
    pub fn header<K: Into<String>, V: Into<String>>(mut self, key: K, value: V) -> Self {
        self.client_config.custom_headers.push((key.into(), value.into()));
        self
    }

    /// Set random delay range for rate limiting
    pub fn random_delay(mut self, min_ms: u64, max_ms: u64) -> Self {
        self.strategy_config.random_delay_range = (min_ms, max_ms);
        self
    }



    /// Build the client
    pub fn build(self) -> FetchResult<KickApiClient> {
        KickApiClient::with_full_config(self.client_config, self.strategy_config)
    }
}

impl Default for KickApiClientBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_client_creation() {
        let client = KickApiClient::new();
        assert!(client.is_ok());
    }

    #[tokio::test]
    async fn test_client_builder() {
        let client = KickApiClientBuilder::new()
            .timeout(60)
            .max_retries(5)
            .build();

        assert!(client.is_ok());
    }

    #[tokio::test]
    async fn test_metrics() {
        let client = KickApiClient::new().unwrap();

        // Initial metrics should be empty
        let metrics = client.get_metrics().await;
        assert_eq!(metrics.total_requests, 0);
        assert_eq!(metrics.success_rate(), 0.0);

        // Reset metrics
        client.reset_metrics().await;
        let metrics = client.get_metrics().await;
        assert_eq!(metrics.total_requests, 0);
    }

    #[tokio::test]
    async fn test_cache_operations() {
        let client = KickApiClient::new().unwrap();

        // Clear cache
        client.clear_cache().await;

        // Check strategy names
        let strategies = client.strategy_names();
        assert!(!strategies.is_empty());
    }

    #[tokio::test]
    async fn test_cached_channel() {
        let channel = ChannelInfo {
            id: 12345,
            slug: "test".to_string(),
            title: Some("Test".to_string()),
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

        let cached = CachedChannel::new(channel.clone(), Duration::from_secs(60));
        assert!(!cached.is_expired());

        let cached_expired = CachedChannel::new(channel, Duration::from_millis(1));
        tokio::time::sleep(Duration::from_millis(10)).await;
        assert!(cached_expired.is_expired());
    }
}
