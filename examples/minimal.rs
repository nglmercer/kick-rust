//! Minimal Kick.com WebSocket example
//!
//! This is the simplest possible way to connect to Kick.com chat

use kick_rust::KickClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = KickClient::new();
    // client.on_raw_message(|data| {
    //     println!("{:#?}",data);
    // }).await;
    client.on_chat_message(|data| {
        println!("{:#?}: {}",data.sender.username,data.content);
    }).await;

    client.on_ready(|()| {
        println!("Connected");
    }).await;
    client.connect("spreen").await?;
    // Keep the program running
    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    }
}
