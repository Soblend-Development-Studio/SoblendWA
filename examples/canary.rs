use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::json;
use soblendwa::events::Event;
use soblendwa::{Client, EventStream, FileStore, Jid, Store};

const STEP: Duration = Duration::from_secs(45);
const PIXEL: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52, 0x00, 0x00, 0x00,
    0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f, 0x15, 0xc4, 0x89, 0x00, 0x00, 0x00, 0x0d, 0x49,
    0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0xf8, 0xcf, 0xc0, 0xf0, 0x1f, 0x00, 0x05, 0x00, 0x01, 0xff, 0x89, 0x99, 0x3d,
    0x1d, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
];

struct Probe {
    steps: Vec<serde_json::Value>,
    started: Instant,
}

impl Probe {
    fn pass(&mut self, name: &str, took: Duration) {
        println!("  ok   {name} ({} ms)", took.as_millis());
        self.steps.push(json!({ "step": name, "ok": true, "ms": took.as_millis() as u64 }));
    }
}

async fn wait_for<T>(
    events: &mut EventStream,
    what: &str,
    mut pick: impl FnMut(&Event) -> Option<T>,
) -> Result<T, String> {
    let waiting = async {
        while let Some(event) = events.recv().await {
            if let Event::AccountBlocked { code, reason, .. } = &event {
                return Err(format!("whatsapp blocked the account ({code}): {reason}"));
            }
            if let Event::LoggedOut { reason, .. } = &event {
                return Err(format!("the session was logged out: {reason}"));
            }
            if let Some(found) = pick(&event) {
                return Ok(found);
            }
        }
        Err("the event stream closed".into())
    };
    tokio::time::timeout(STEP, waiting).await.map_err(|_| format!("timed out waiting for {what}"))?
}

async fn open(dir: &str) -> Result<(Client, EventStream), String> {
    let store: Arc<dyn Store> = FileStore::open(dir).await.map_err(|e| e.to_string())?;
    let creds = store.load_creds().await.map_err(|e| e.to_string())?;
    if !creds.is_some_and(|c| c.is_logged_in()) {
        return Err(format!("{dir} is not a linked session; link it once with the bot example"));
    }
    let (client, mut events) =
        Client::builder().store(store).online_on_connect(false).build().await.map_err(|e| e.to_string())?;
    client.connect().await.map_err(|e| e.to_string())?;
    wait_for(&mut events, "the connection", |e| matches!(e, Event::Connected).then_some(())).await?;
    Ok((client, events))
}

async fn run(probe: &mut Probe, sender_dir: &str, receiver_dir: Option<&str>) -> Result<(), String> {
    let clock = Instant::now();
    let (sender, mut sender_events) = open(sender_dir).await?;
    probe.pass("connect sender", clock.elapsed());
    let mut receiver = None;
    if let Some(dir) = receiver_dir {
        let clock = Instant::now();
        receiver = Some(open(dir).await?);
        probe.pass("connect receiver", clock.elapsed());
    }
    let target: Jid = match &receiver {
        Some((client, _)) => client.me().ok_or("the receiver has no account")?.to_non_device(),
        None => sender.me().ok_or("the sender has no account")?.to_non_device(),
    };

    let clock = Instant::now();
    let latency = sender.ping().await.map_err(|e| format!("ping: {e}"))?;
    probe.pass(&format!("ping {} ms", latency.as_millis()), clock.elapsed());

    let marker = format!(
        "canary {}",
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
    );
    let clock = Instant::now();
    let id = sender.send_text(&target, &marker).await.map_err(|e| format!("send text: {e}"))?;
    let error = wait_for(&mut sender_events, "the text ack", |e| match e {
        Event::ServerAck(ack) if ack.id == id => Some(ack.error),
        _ => None,
    })
    .await?;
    if let Some(code) = error {
        return Err(format!("whatsapp rejected the text with error {code}"));
    }
    probe.pass("text accepted by whatsapp", clock.elapsed());

    if let Some((_, events)) = receiver.as_mut() {
        let clock = Instant::now();
        wait_for(events, "the text on the receiver", |e| match e {
            Event::Message(m) if m.text() == Some(marker.as_str()) => Some(()),
            Event::UndecryptableMessage { info, error } if info.id == id => {
                println!("  !!   undecryptable on the receiver: {error}");
                None
            }
            _ => None,
        })
        .await?;
        probe.pass("text decrypted by the receiver", clock.elapsed());
    }

    let clock = Instant::now();
    let image = sender.send_image(&target, PIXEL, Some(&marker)).await.map_err(|e| format!("send image: {e}"))?;
    wait_for(&mut sender_events, "the image ack", |e| match e {
        Event::ServerAck(ack) if ack.id == image => Some(()),
        _ => None,
    })
    .await?;
    probe.pass("image uploaded and accepted", clock.elapsed());

    if let Some((client, events)) = receiver.as_mut() {
        let clock = Instant::now();
        let message = wait_for(events, "the image on the receiver", |e| match e {
            Event::Message(m) if m.info.id == image => Some(m.message.clone()),
            _ => None,
        })
        .await?;
        let bytes = client.download_message(&message).await.map_err(|e| format!("download image: {e}"))?;
        if bytes != PIXEL {
            return Err("the downloaded image does not match what was sent".into());
        }
        probe.pass("image downloaded and verified", clock.elapsed());
    }

    for client in [Some(&sender), receiver.as_ref().map(|(c, _)| c)].into_iter().flatten() {
        let stats = client.stats();
        if stats.undecryptable > 0 || stats.panics > 0 {
            return Err(format!("{} undecryptable messages and {} panics", stats.undecryptable, stats.panics));
        }
    }
    sender.disconnect().await;
    if let Some((client, _)) = receiver {
        client.disconnect().await;
    }
    Ok(())
}

async fn alert(webhook: &str, text: &str) {
    let body = json!({ "text": text, "content": text });
    match reqwest::Client::new().post(webhook).json(&body).timeout(Duration::from_secs(15)).send().await {
        Ok(response) if response.status().is_success() => println!("alert sent"),
        Ok(response) => eprintln!("the alert webhook answered {}", response.status()),
        Err(err) => eprintln!("could not reach the alert webhook: {err}"),
    }
}

#[tokio::main]
async fn main() {
    println!("{}", soblendwa::credits());
    let mut args = std::env::args().skip(1);
    let Some(sender) = args.next() else {
        eprintln!("usage: cargo run --release --example canary -- <test session folder> [second test session folder]");
        eprintln!("set SOBLENDWA_ALERT_WEBHOOK to a Discord, Slack or any JSON webhook to be alerted on failure");
        std::process::exit(2);
    };
    let receiver = args.next();
    let webhook = std::env::var("SOBLENDWA_ALERT_WEBHOOK").ok();
    let mut probe = Probe { steps: Vec::new(), started: Instant::now() };

    println!("SoblendWA canary · {}", if receiver.is_some() { "two accounts" } else { "one account, message to self" });
    let outcome = run(&mut probe, &sender, receiver.as_deref()).await;
    let report = json!({
        "ok": outcome.is_ok(),
        "error": outcome.as_ref().err(),
        "steps": probe.steps,
        "totalMs": probe.started.elapsed().as_millis() as u64,
        "version": env!("CARGO_PKG_VERSION"),
    });
    println!("{report}");
    if let Err(err) = outcome {
        eprintln!("canary FAILED: {err}");
        if let Some(webhook) = webhook {
            alert(&webhook, &format!("SoblendWA canary failed: {err}. Something may have changed on WhatsApp.")).await;
        }
        std::process::exit(1);
    }
    println!("canary passed");
}
