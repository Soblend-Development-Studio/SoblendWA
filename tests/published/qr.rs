// src/main.rs of a new crate that depends on soblendwa from crates.io: connects to WhatsApp until the first QR and
// stops. Nothing gets linked.

use std::sync::Arc;
use std::time::{Duration, Instant};

use soblendwa::{Client, Event, MemoryStore};

#[tokio::main]
async fn main() -> soblendwa::Result<()> {
    let started = Instant::now();
    let (client, mut events) = Client::builder().store(Arc::new(MemoryStore::new())).build().await?;
    client.connect().await?;
    let first_qr = async {
        while let Some(event) = events.recv().await {
            if matches!(event, Event::Qr(_)) {
                return true;
            }
        }
        false
    };
    let got = tokio::time::timeout(Duration::from_secs(60), first_qr).await.unwrap_or(false);
    client.disconnect().await;
    if !got {
        eprintln!("no QR within 60 s");
        std::process::exit(1);
    }
    println!("soblendwa {} on {}-{}: QR after {:?}", soblendwa::VERSION, std::env::consts::OS, std::env::consts::ARCH, started.elapsed());
    Ok(())
}
