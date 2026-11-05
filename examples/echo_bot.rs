use std::sync::Arc;

use soblendwa::events::Event;
use soblendwa::{Client, MemoryStore, Message, DEFAULT_VERSION};
use tracing_subscriber::EnvFilter;

fn decode_root(hex_text: &str) -> Option<[u8; 32]> {
    hex::decode(hex_text).ok()?.try_into().ok()
}

#[tokio::main]
async fn main() -> soblendwa::Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("off")))
        .init();
    let mut args = std::env::args().skip(1);
    let (Some(url), Some(root)) = (args.next(), args.next().as_deref().and_then(decode_root)) else {
        eprintln!("usage: echo_bot <websocket url> <noise root public key in hex>");
        eprintln!("the session lives in memory: this bot is only for fake servers");
        std::process::exit(2);
    };
    let (client, mut events) = Client::builder()
        .store(Arc::new(MemoryStore::new()))
        .ws_url(url)
        .trust_root(root)
        .version(DEFAULT_VERSION)
        .send_limit(None)
        .build()
        .await?;
    client.connect().await?;
    while let Some(event) = events.recv().await {
        match event {
            Event::Qr(code) => println!("QR {code}"),
            Event::Connected => println!("READY"),
            Event::Message(message) if !message.info.from_me => {
                let Some(text) = message.text().map(str::to_owned) else { continue };
                let client = client.clone();
                tokio::spawn(async move {
                    let reply = Message::text(format!("echo:{text}"));
                    if let Err(err) = client.send_message(message.chat(), reply).await {
                        println!("ERROR {err}");
                    }
                });
            }
            Event::UndecryptableMessage { error, .. } => println!("ERROR undecryptable: {error}"),
            Event::Disconnected { reason, reconnecting } => println!("DISCONNECTED {reason:?} {reconnecting}"),
            _ => {}
        }
    }
    Ok(())
}
