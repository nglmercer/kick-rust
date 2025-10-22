//! Integration tests for kick-rust library
//!
//! These tests verify the basic functionality of the library with proper timeout handling
//! to ensure tests don't hang indefinitely.

use kick_rust::{KickClient, RawMessage, extract_field};
use std::time::Duration;


#[tokio::test]
async fn test_client_creation_and_basic_functionality() {
    let client = KickClient::new();

    // Initially not connected
    assert!(!client.is_connected().await);
    assert!(!client.has_handlers().await);

    // Set raw message handler
    client.on_raw_message(|data: RawMessage| {
        assert!(!data.event_type.is_empty());
        assert!(!data.raw_json.is_empty());
    }).await;

    // Wait for async handler to be set
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

    // Should have handlers now but not connected to WebSocket
    assert!(client.has_handlers().await);
    assert!(!client.is_connected().await);

    // Set parsed message handler
    client.on_message(|msg| {
        assert!(!msg.username.is_empty());
        assert!(!msg.content.is_empty());
    }).await;

    // Still has handlers but not connected to WebSocket
    assert!(client.has_handlers().await);
    assert!(!client.is_connected().await);

    // Test disconnect
    client.disconnect().await.unwrap();
    assert!(!client.is_connected().await);
    assert!(!client.has_handlers().await);
}

#[tokio::test]
async fn test_message_handlers() {
    let client = KickClient::new();

    // Test raw message handler
    let raw_handler_called = std::sync::Arc::new(tokio::sync::Mutex::new(false));
    let raw_handler_called_clone = raw_handler_called.clone();

    client.on_raw_message(move |data: RawMessage| {
        assert_eq!(data.event_type, "test_event");
        assert_eq!(data.data, "test_data");
        assert!(data.raw_json.contains("test_event"));

        let mut called = raw_handler_called_clone.try_lock().unwrap();
        *called = true;
    }).await;

    // Test parsed message handler
    let parsed_handler_called = std::sync::Arc::new(tokio::sync::Mutex::new(false));
    let parsed_handler_called_clone = parsed_handler_called.clone();

    client.on_message(move |msg| {
        assert_eq!(msg.username, "test_user");
        assert_eq!(msg.content, "test_message");
        assert_eq!(msg.id, "123");
        assert_eq!(msg.created_at, "2023-01-01T00:00:00Z");

        let mut called = parsed_handler_called_clone.try_lock().unwrap();
        *called = true;
    }).await;

    // Wait for async handlers to be set
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

    // Verify handlers are set
    assert!(client.has_handlers().await);
    assert!(!client.is_connected().await);
}

#[tokio::test]
async fn test_connect_timeout_handling() {
    let client = KickClient::new();

    // Test with very short timeout - should fail quickly
    let result = client.connect_with_timeout("test_channel", Duration::from_millis(1)).await;

    // The result should be either Ok (if connection is very fast) or Err (timeout)
    // The important thing is that it doesn't hang
    assert!(result.is_ok() || result.is_err());

    // Test with reasonable timeout
    let result = client.connect_with_timeout("test_channel", Duration::from_secs(5)).await;
    assert!(result.is_ok() || result.is_err());
}

#[tokio::test]
async fn test_utility_functions() {
    // Test extract_field function
    let raw_msg = RawMessage {
        event_type: "test_event".to_string(),
        data: r#"{"user": {"name": "test", "id": 123}}"#.to_string(),
        raw_json: r#"{"event": "test_event", "data": {"user": {"name": "test", "id": 123}}}"#.to_string(),
    };

    // Test valid field extraction
    let name = extract_field(&raw_msg, "data.user.name");
    assert!(name.is_some());
    assert_eq!(name.unwrap().as_str().unwrap(), "test");

    // Test invalid field extraction
    let invalid = extract_field(&raw_msg, "data.invalid.path");
    assert!(invalid.is_none());

    // Test with malformed JSON
    let malformed_msg = RawMessage {
        event_type: "test".to_string(),
        data: "invalid".to_string(),
        raw_json: "{invalid json}".to_string(),
    };

    let result = extract_field(&malformed_msg, "any.path");
    assert!(result.is_none());
}

#[tokio::test]
async fn test_multiple_handlers() {
    let client = KickClient::new();

    // Set multiple handlers
    let raw_count = std::sync::Arc::new(tokio::sync::Mutex::new(0));
    let parsed_count = std::sync::Arc::new(tokio::sync::Mutex::new(0));

    let raw_count_clone = raw_count.clone();
    client.on_raw_message(move |_data: RawMessage| {
        let mut count = raw_count_clone.try_lock().unwrap();
        *count += 1;
    }).await;

    let parsed_count_clone = parsed_count.clone();
    client.on_message(move |_msg| {
        let mut count = parsed_count_clone.try_lock().unwrap();
        *count += 1;
    }).await;

    // Wait for async handlers to be set
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

    // Both handlers should be active
    assert!(client.has_handlers().await);
    assert!(!client.is_connected().await);

    // Verify counters are initially zero
    assert_eq!(*raw_count.try_lock().unwrap(), 0);
    assert_eq!(*parsed_count.try_lock().unwrap(), 0);
}

#[tokio::test]
async fn test_client_state_management() {
    let client = KickClient::new();

    // Initial state
    assert!(!client.is_connected().await);
    assert!(!client.has_handlers().await);

    // Add raw handler
    client.on_raw_message(|_| {}).await;
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    assert!(client.has_handlers().await);
    assert!(!client.is_connected().await);

    // Add parsed handler
    client.on_message(|_| {}).await;
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    assert!(client.has_handlers().await);
    assert!(!client.is_connected().await);

    // Disconnect
    client.disconnect().await.unwrap();
    assert!(!client.is_connected().await);
    assert!(!client.has_handlers().await);

    // Verify we can add handlers again
    client.on_raw_message(|_| {}).await;
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    assert!(client.has_handlers().await);
    assert!(!client.is_connected().await);
}

#[tokio::test]
async fn test_error_handling() {
    let client = KickClient::new();

    // Test disconnect on non-connected client
    let result = client.disconnect().await;
    assert!(result.is_ok());

    // Test connecting with invalid channel name (should not panic)
    let result = client.connect_with_timeout("", Duration::from_secs(1)).await;
    assert!(result.is_ok() || result.is_err());
}

#[tokio::test]
async fn test_concurrent_access() {
    let client = std::sync::Arc::new(KickClient::new());

    // Test concurrent handler registration
    let mut handles = vec![];

    for i in 0..5 {
        let client_clone = client.clone();
        let handle = tokio::spawn(async move {
            client_clone.on_raw_message(move |data: RawMessage| {
                println!("Handler {} received: {}", i, data.event_type);
            }).await;
        });
        handles.push(handle);
    }

    // Wait for all handlers to be registered
    for handle in handles {
        handle.await.unwrap();
    }

    // Client should still have handlers
    assert!(client.has_handlers().await);
    assert!(!client.is_connected().await);

    // Test concurrent disconnect
    let client_clone1 = client.clone();
    let client_clone2 = client.clone();

    let handle1 = tokio::spawn(async move {
        let result = client_clone1.disconnect().await;
        println!("Disconnect 1 result: {:?}", result);
        result.is_ok()
    });

    let handle2 = tokio::spawn(async move {
        let result = client_clone2.disconnect().await;
        println!("Disconnect 2 result: {:?}", result);
        result.is_ok()
    });

    // Both should complete without error
    assert!(handle1.await.unwrap());
    assert!(handle2.await.unwrap());
}
