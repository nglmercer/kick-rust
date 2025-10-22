//! HTTP fetching module for Kick API
//!
//! This module provides a clean, modular interface for fetching data from the Kick API.
//! It focuses on using the official API endpoint: https://kick.com/api/v2/channels/{channel_name}

pub mod client;
pub mod strategies;
pub mod parsers;
pub mod types;
pub mod useragent;

pub use client::KickApiClient;
pub use types::{ClientConfig, FetchResult, ChannelInfo};
