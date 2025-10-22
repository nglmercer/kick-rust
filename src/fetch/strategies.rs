//! Different fetching strategies for Kick API data
//!
//! This module provides various strategies for fetching channel information,
//! focusing on the official API endpoint with fallback mechanisms.

use async_trait::async_trait;
use crate::fetch::{
    types::{ChannelInfo, FetchResult, FetchError, StrategyConfig},
    parsers::JsonParser,
    useragent::generate_browser_fingerprint,
};
use reqwest::Client;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{info, warn, debug};


/// Base strategy trait for all fetching approaches
#[async_trait]
pub trait FetchStrategy {
    /// Get the name of this strategy
    fn name(&self) -> &'static str;

    /// Fetch channel information using this strategy
    async fn fetch_channel(&self, client: &Client, channel_name: &str, config: &StrategyConfig) -> FetchResult<ChannelInfo>;

    /// Check if this strategy is available/enabled
    fn is_enabled(&self, _config: &StrategyConfig) -> bool {
        true
    }
}









/// Curl-based strategy for better Cloudflare compatibility
pub struct CurlStrategy {
    _parser: JsonParser, // Keep for potential future use
}

impl CurlStrategy {
    pub fn new() -> Self {
        Self {
            _parser: JsonParser, // Keep for potential future use
        }
    }

    async fn make_curl_request(&self, url: &str) -> Result<String, FetchError> {
        // Generate realistic browser fingerprint
        let fingerprint = generate_browser_fingerprint();

        let mut headers = curl::easy::List::new();
        for header in fingerprint.get_curl_headers() {
            headers.append(&header).unwrap();
        }

        let mut easy = curl::easy::Easy::new();
        easy.url(url).unwrap();
        easy.http_headers(headers).unwrap();
        easy.follow_location(true).unwrap();
        easy.timeout(Duration::from_secs(30)).unwrap();

        let mut data = Vec::new();
        {
            let mut transfer = easy.transfer();

            transfer.write_function(|new_data| {
                data.extend_from_slice(new_data);
                Ok(new_data.len())
            }).unwrap();

            match transfer.perform() {
                Ok(_) => {
                    // Transfer is dropped here, releasing the borrow on easy
                }
                Err(e) => {
                    warn!("Curl request failed: {}", e);
                    return Err(FetchError::Network(format!("Curl request failed: {}", e)));
                }
            }
        }

        // Now we can safely use easy again
        let response_code = easy.response_code().unwrap_or(0);
        debug!("Curl response code: {}", response_code);

        if response_code == 200 {
            // Try to decompress the response if needed
            let decompressed_data = self.try_decompress(&data).await?;
            let response_text = String::from_utf8(decompressed_data)
                .map_err(|e| FetchError::Network(format!("Invalid UTF-8 response: {}", e)))?;
            Ok(response_text)
        } else if response_code == 404 {
            Err(FetchError::ChannelNotFound("Channel not found".to_string()))
        } else if response_code == 429 {
            Err(FetchError::RateLimit)
        } else {
            let error_text = String::from_utf8(data).unwrap_or_default();
            Err(FetchError::Http(format!("HTTP {}: {}", response_code, error_text)))
        }
    }
}

#[async_trait]
impl FetchStrategy for CurlStrategy {
    fn name(&self) -> &'static str {
        "curl"
    }

    fn is_enabled(&self, _config: &StrategyConfig) -> bool {
        true
    }

    async fn fetch_channel(&self, _client: &Client, channel_name: &str, _config: &StrategyConfig) -> FetchResult<ChannelInfo> {
        let api_url = format!("https://kick.com/api/v2/channels/{}", channel_name);
        debug!("Fetching channel {} using curl: {}", channel_name, api_url);

        let response_text = self.make_curl_request(&api_url).await?;
        JsonParser::parse_channel_response(&response_text)
    }

}

impl CurlStrategy {
    async fn try_decompress(&self, data: &[u8]) -> FetchResult<Vec<u8>> {
        // Try to detect and decompress gzip/deflate/brotli compressed data
        if data.len() >= 2 {
            // Check for gzip magic number
            if data[0] == 0x1f && data[1] == 0x8b {
                debug!("Detected gzip compression, decompressing...");
                return self.decompress_gzip(data).await;
            }
            // Check for brotli magic number
            if data.len() >= 6 && &data[0..6] == b"\x8b\x02\x80\x00\x00\x00" {
                debug!("Detected brotli compression, decompressing...");
                return self.decompress_brotli(data).await;
            }
        }

        // Try to decompress as gzip anyway (some responses don't have proper magic numbers)
        if let Ok(decompressed) = self.decompress_gzip(data).await {
            debug!("Successfully decompressed as gzip");
            return Ok(decompressed);
        }

        // Try brotli
        if let Ok(decompressed) = self.decompress_brotli(data).await {
            debug!("Successfully decompressed as brotli");
            return Ok(decompressed);
        }

        // If all decompression attempts fail, assume it's not compressed
        debug!("No compression detected or decompression failed, using raw data");
        Ok(data.to_vec())
    }

    async fn decompress_gzip(&self, data: &[u8]) -> FetchResult<Vec<u8>> {
        use flate2::read::GzDecoder;
        use std::io::Read;

        let mut decoder = GzDecoder::new(data);
        let mut decompressed = Vec::new();
        decoder.read_to_end(&mut decompressed)
            .map_err(|e| FetchError::Network(format!("Gzip decompression failed: {}", e)))?;
        Ok(decompressed)
    }

    async fn decompress_brotli(&self, data: &[u8]) -> FetchResult<Vec<u8>> {
        use brotli::Decompressor;
        use std::io::Read;

        let mut decompressed = Vec::new();
        let mut decoder = Decompressor::new(data, 4096);
        decoder.read_to_end(&mut decompressed)
            .map_err(|e| FetchError::Network(format!("Brotli decompression failed: {}", e)))?;
        Ok(decompressed)
    }
}

/// Strategy manager that handles multiple fetching strategies
pub struct StrategyManager {
    strategies: Vec<Box<dyn FetchStrategy + Send + Sync>>,
}

impl StrategyManager {
    /// Create a new strategy manager with default strategies
    pub fn new() -> Result<Self, FetchError> {
        let strategies: Vec<Box<dyn FetchStrategy + Send + Sync>> = vec![
            Box::new(CurlStrategy::new()), // Only use curl strategy as it works reliably
        ];

        Ok(Self { strategies })
    }

    /// Create a strategy manager with custom configuration
    pub fn with_config(_config: &StrategyConfig) -> Result<Self, FetchError> {
        let strategies: Vec<Box<dyn FetchStrategy + Send + Sync>> = vec![
            Box::new(CurlStrategy::new()), // Only use curl strategy as it works reliably
        ];

        Ok(Self { strategies })
    }

    /// Fetch channel information using available strategies
    pub async fn fetch_channel(
        &self,
        client: &Client,
        channel_name: &str,
        config: &StrategyConfig,
    ) -> FetchResult<ChannelInfo> {
        let mut last_error = None;

        for strategy in &self.strategies {
            if !strategy.is_enabled(config) {
                debug!("Skipping disabled strategy: {}", strategy.name());
                continue;
            }

            info!("Trying strategy: {}", strategy.name());

            match strategy.fetch_channel(client, channel_name, config).await {
                Ok(channel) => {
                    info!("Successfully fetched channel {} using strategy: {}", channel_name, strategy.name());
                    return Ok(channel);
                }
                Err(e) => {
                    warn!("Strategy {} failed for channel {}: {}", strategy.name(), channel_name, e);
                    last_error = Some(e);

                    // Add delay between strategies
                    sleep(Duration::from_millis(500)).await;
                }
            }
        }

        Err(last_error.unwrap_or_else(|| FetchError::ChannelNotFound(format!("No strategies succeeded for channel: {}", channel_name))))
    }

    /// Get the names of all available strategies
    pub fn strategy_names(&self) -> Vec<&'static str> {
        self.strategies.iter().map(|s| s.name()).collect()
    }
}

impl Default for StrategyManager {
    fn default() -> Self {
        Self::new().unwrap_or_else(|_| {
            warn!("Failed to create StrategyManager, using curl strategy only");
            Self {
                strategies: vec![
                    Box::new(CurlStrategy::new()),
                ],
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fetch::types::StrategyConfig;

    #[tokio::test]
    async fn test_strategy_manager_creation() {
        let manager = StrategyManager::new();
        assert!(manager.is_ok());

        if let Ok(manager) = manager {
            let names = manager.strategy_names();
            assert!(!names.is_empty());
            assert!(names.contains(&"curl"));
        }
    }

    #[tokio::test]
    async fn test_strategy_manager_with_config() {
        let config = StrategyConfig::default();

        let manager = StrategyManager::with_config(&config);
        assert!(manager.is_ok());

        if let Ok(manager) = manager {
            let names = manager.strategy_names();
            assert_eq!(names.len(), 1);
            assert_eq!(names[0], "curl");
        }
    }

    #[test]
    fn test_curl_strategy() {
        let strategy = CurlStrategy::new();
        assert_eq!(strategy.name(), "curl");

        let config = StrategyConfig::default();
        assert!(strategy.is_enabled(&config));
    }
}
