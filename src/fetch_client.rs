//! HTTP client module for fetching Kick API data
//!
//! This module provides a clean, modular interface for fetching data from the Kick API.
//! It focuses on using the official API endpoint: https://kick.com/api/v2/channels/{channel_name}
//! with multiple fallback strategies for reliability.

// Re-export from the new fetch module
pub use crate::fetch::{
    KickApiClient,
    types::{ClientConfig, ChannelInfo, FetchResult, StrategyConfig, FetchError},
};

// Re-export for backward compatibility
pub use KickApiClient as KickApiClientInterface;

/// Legacy compatibility layer
pub mod legacy {
    use crate::fetch::{KickApiClient, ClientConfig, ChannelInfo};
    use crate::fetch::types::FetchError;
    use crate::types::KickError;

    /// Convert legacy KickError to new FetchError
    impl From<FetchError> for KickError {
        fn from(error: FetchError) -> Self {
            match error {
                FetchError::Network(msg) => KickError::NetworkError(msg),
                FetchError::ChannelNotFound(name) => KickError::ChannelNotFound(name),
                FetchError::Json(msg) => KickError::NetworkError(format!("JSON error: {}", msg)),
                FetchError::Http(msg) => KickError::NetworkError(format!("HTTP error: {}", msg)),
                FetchError::RateLimit => KickError::NetworkError("Rate limit exceeded".to_string()),
                FetchError::InvalidResponse => KickError::InvalidMessage("Invalid response format".to_string()),
                FetchError::Timeout => KickError::NetworkError("Request timeout".to_string()),
                FetchError::AuthenticationRequired => KickError::Connection("Authentication required".to_string()),
            }
        }
    }

    /// Legacy client wrapper for backward compatibility
    pub struct LegacyKickApiClient {
        pub inner: KickApiClient,
    }

    impl LegacyKickApiClient {
        pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
            Ok(Self {
                inner: KickApiClient::new()?,
            })
        }

        pub fn with_config(config: ClientConfig) -> Result<Self, Box<dyn std::error::Error>> {
            Ok(Self {
                inner: KickApiClient::with_config(config)?,
            })
        }

        /// Legacy method - redirects to new implementation
        pub async fn get_channel_id_from_name(&self, channel_name: &str) -> Result<u64, KickError> {
            self.inner.get_channel_id(channel_name).await.map_err(Into::into)
        }

        /// Test if channel exists
        pub async fn test_channel_exists(&self, channel_name: &str) -> Result<bool, KickError> {
            self.inner.channel_exists(channel_name).await.map_err(Into::into)
        }

        /// Get full channel information
        pub async fn get_channel_info(&self, channel_name: &str) -> Result<ChannelInfo, KickError> {
            self.inner.get_channel(channel_name).await.map_err(Into::into)
        }
    }

    impl Default for LegacyKickApiClient {
        fn default() -> Self {
            Self::new().expect("Failed to create default LegacyKickApiClient")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fetch::types::*;
    use crate::types::KickError;

    #[tokio::test]
    async fn test_modular_client() {
        let client = KickApiClient::new();
        assert!(client.is_ok());

        if let Ok(client) = client {
            // Test connection
            let _result = client.test_connection().await;
            // This might fail in test environment, but should not panic

            // Test metrics (the test_connection call increments this)
            let metrics = client.get_metrics().await;
            assert_eq!(metrics.total_requests, 1);

            // Test strategy names
            let strategies = client.strategy_names();
            assert!(!strategies.is_empty());
        }
    }

    #[tokio::test]
    async fn test_legacy_compatibility() {
        let client = legacy::LegacyKickApiClient::new();
        assert!(client.is_ok());

        if let Ok(client) = client {
            // Test that legacy methods exist
            let strategies = client.inner.strategy_names();
            assert!(!strategies.is_empty());
        }
    }

    #[test]
    fn test_error_conversion() {
        let fetch_error = FetchError::ChannelNotFound("test".to_string());
        let kick_error: KickError = fetch_error.into();
        assert!(matches!(kick_error, KickError::ChannelNotFound(_)));
    }
}
