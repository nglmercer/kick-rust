//! User-Agent management for robust web scraping
//!
//! This module provides realistic, up-to-date User-Agent strings that rotate
//! to avoid detection and ensure long-term compatibility with web services.

use std::sync::atomic::{AtomicUsize, Ordering};
use once_cell::sync::Lazy;

/// Current User-Agent rotation index
static UA_INDEX: AtomicUsize = AtomicUsize::new(0);

/// Collection of realistic, up-to-date User-Agent strings
static USER_AGENTS: Lazy<Vec<&'static str>> = Lazy::new(|| {
    vec![
        // Chrome on Windows (most common)
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36",
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36",
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/129.0.0.0 Safari/537.36",

        // Chrome on macOS
        "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36",
        "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36",

        // Firefox on Windows
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:132.0) Gecko/20100101 Firefox/132.0",
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:131.0) Gecko/20100101 Firefox/131.0",

        // Firefox on macOS
        "Mozilla/5.0 (Macintosh; Intel Mac OS X 10.15; rv:132.0) Gecko/20100101 Firefox/132.0",
        "Mozilla/5.0 (Macintosh; Intel Mac OS X 10.15; rv:131.0) Gecko/20100101 Firefox/131.0",

        // Edge on Windows
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36 Edg/131.0.0.0",
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36 Edg/130.0.0.0",

        // Safari on macOS
        "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.1 Safari/605.1.15",
        "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Safari/605.1.15",

        // Chrome on Linux
        "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36",
        "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36",

        // Firefox on Linux
        "Mozilla/5.0 (X11; Linux x86_64; rv:132.0) Gecko/20100101 Firefox/132.0",
        "Mozilla/5.0 (X11; Linux x86_64; rv:131.0) Gecko/20100101 Firefox/131.0",
    ]
});

/// Get a rotating User-Agent string
///
/// This function rotates through different User-Agent strings to avoid
/// detection and ensure realistic browser fingerprinting.
pub fn get_rotating_user_agent() -> &'static str {
    let index = UA_INDEX.fetch_add(1, Ordering::Relaxed) % USER_AGENTS.len();
    USER_AGENTS[index]
}

/// Get a random User-Agent string
pub fn get_random_user_agent() -> &'static str {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let index = rng.gen_range(0..USER_AGENTS.len());
    USER_AGENTS[index]
}

/// Get the latest Chrome User-Agent (most compatible)
pub fn get_latest_chrome_user_agent() -> &'static str {
    USER_AGENTS[0]
}

/// Get a specific User-Agent by index
pub fn get_user_agent(index: usize) -> Option<&'static str> {
    USER_AGENTS.get(index).copied()
}

/// Get the total number of available User-Agents
pub fn get_user_agent_count() -> usize {
    USER_AGENTS.len()
}

/// Check if a User-Agent string is up-to-date
pub fn is_user_agent_current(user_agent: &str) -> bool {
    // Check if the user agent contains recent browser versions
    user_agent.contains("Chrome/13") || // Chrome 130+
    user_agent.contains("Firefox/13") || // Firefox 130+
    user_agent.contains("Safari/605") || // Safari 18+
    user_agent.contains("Edg/13") // Edge 130+
}

/// Generate a realistic browser fingerprint
pub fn generate_browser_fingerprint() -> BrowserFingerprint {
    let user_agent = get_rotating_user_agent();

    BrowserFingerprint {
        user_agent: user_agent.to_string(),
        sec_ch_ua: extract_sec_ch_ua(user_agent),
        sec_ch_ua_mobile: "false".to_string(),
        sec_ch_ua_platform: extract_platform(user_agent),
        accept_language: "en-US,en;q=0.9".to_string(),
        accept_encoding: "gzip, deflate, br".to_string(),
    }
}

/// Browser fingerprint for realistic request headers
#[derive(Debug, Clone)]
pub struct BrowserFingerprint {
    pub user_agent: String,
    pub sec_ch_ua: String,
    pub sec_ch_ua_mobile: String,
    pub sec_ch_ua_platform: String,
    pub accept_language: String,
    pub accept_encoding: String,
}

impl BrowserFingerprint {
    /// Get headers for curl requests
    pub fn get_curl_headers(&self) -> Vec<String> {
        vec![
            format!("Accept: application/json, text/plain, */*"),
            format!("Accept-Language: {}", self.accept_language),
            // format!("Accept-Encoding: {}", self.accept_encoding), // Skip to avoid compression issues
            format!("Cache-Control: no-cache"),
            format!("Pragma: no-cache"),
            format!("Sec-Ch-Ua: {}", self.sec_ch_ua),
            format!("Sec-Ch-Ua-Mobile: {}", self.sec_ch_ua_mobile),
            format!("Sec-Ch-Ua-Platform: {}", self.sec_ch_ua_platform),
            format!("Sec-Fetch-Dest: empty"),
            format!("Sec-Fetch-Mode: cors"),
            format!("Sec-Fetch-Site: same-origin"),
            format!("Referer: https://kick.com/"),
            format!("Origin: https://kick.com"),
            format!("User-Agent: {}", self.user_agent),
        ]
    }
}

/// Extract Sec-CH-UA from User-Agent
fn extract_sec_ch_ua(user_agent: &str) -> String {
    if user_agent.contains("Chrome") {
        if user_agent.contains("Edg") {
            // Edge
            let version = extract_version(user_agent, "Edg/");
            format!("\"Microsoft Edge\";v=\"{}\", \"Chromium\";v=\"{}\", \"Not_A Brand\";v=\"99\"",
                    version.unwrap_or("131"),
                    extract_version(user_agent, "Chrome/").unwrap_or("131"))
        } else {
            // Chrome
            let version = extract_version(user_agent, "Chrome/").unwrap_or("131");
            format!("\"Google Chrome\";v=\"{}\", \"Chromium\";v=\"{}\", \"Not_A Brand\";v=\"99\"",
                    version, version)
        }
    } else if user_agent.contains("Firefox") {
        "\"Firefox\";v=\"132\"".to_string()
    } else if user_agent.contains("Safari") {
        "\"Safari\";v=\"18\"".to_string()
    } else {
        "\"Not_A Brand\";v=\"99\"".to_string()
    }
}

/// Extract platform from User-Agent
fn extract_platform(user_agent: &str) -> String {
    if user_agent.contains("Windows") {
        "\"Windows\"".to_string()
    } else if user_agent.contains("Macintosh") {
        "\"macOS\"".to_string()
    } else if user_agent.contains("Linux") {
        "\"Linux\"".to_string()
    } else {
        "\"Unknown\"".to_string()
    }
}

/// Extract version number from User-Agent string
fn extract_version(user_agent: &str, prefix: &str) -> Option<&'static str> {
    // This is simplified - in production you'd want more robust parsing
    if let Some(start) = user_agent.find(prefix) {
        let start = start + prefix.len();
        if let Some(end) = user_agent[start..].find(|c| c == ' ' || c == ';') {
            let version = &user_agent[start..start + end];
            // Convert to static string (simplified for this example)
            match version {
                "131.0.0.0" => Some("131"),
                "130.0.0.0" => Some("130"),
                "129.0.0.0" => Some("129"),
                "132.0" => Some("132"),
                "131.0" => Some("131"),
                "130.0" => Some("130"),
                _ => Some("131"), // Default to latest
            }
        } else {
            Some("131") // Default to latest
        }
    } else {
        Some("131") // Default to latest
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rotating_user_agent() {
        // Reset the counter to ensure consistent test behavior
        UA_INDEX.store(0, Ordering::Relaxed);

        let ua1 = get_rotating_user_agent();
        let ua2 = get_rotating_user_agent();

        assert!(!ua1.is_empty());
        assert!(!ua2.is_empty());
        // They might be the same if we have many agents, but they should be valid
        assert!(is_user_agent_current(ua1));
        assert!(is_user_agent_current(ua2));
    }

    #[test]
    fn test_random_user_agent() {
        let ua = get_random_user_agent();
        assert!(!ua.is_empty());
        assert!(is_user_agent_current(&ua));
    }

    #[test]
    fn test_browser_fingerprint() {
        let fingerprint = generate_browser_fingerprint();
        assert!(!fingerprint.user_agent.is_empty());
        assert!(!fingerprint.sec_ch_ua.is_empty());
        assert!(!fingerprint.sec_ch_ua_platform.is_empty());

        let headers = fingerprint.get_curl_headers();
        assert!(!headers.is_empty());

        // Check that User-Agent is included
        assert!(headers.iter().any(|h| h.starts_with("User-Agent:")));
    }

    #[test]
    fn test_user_agent_count() {
        let count = get_user_agent_count();
        assert!(count > 10);
    }
}
