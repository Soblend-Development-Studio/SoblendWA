use std::path::PathBuf;

use clap::Parser;
use qrcode::render::unicode::Dense1x2;
use qrcode::QrCode;
use serde_json::Value;
use soblendwa_server::{fake_phone, serve, Hub, ServerConfig, Storage};
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;

#[global_allocator]
static ALLOCATOR: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[derive(Parser, Debug)]
#[command(
    name = "soblendwa-server",
    version,
    about = "WhatsApp for any language, over WebSocket and HTTP. By Soblend Development Studio."
)]
struct Args {
    #[arg(long, default_value = "127.0.0.1", env = "SOBLENDWA_HOST")]
    host: String,
    #[arg(long, default_value_t = 8787, env = "SOBLENDWA_PORT")]
    port: u16,
    #[arg(long, env = "SOBLENDWA_TOKEN")]
    token: Option<String>,
    #[arg(long, default_value = "sessions", env = "SOBLENDWA_DIR")]
    dir: PathBuf,
    #[arg(long, env = "SOBLENDWA_SQLITE", conflicts_with = "memory")]
    sqlite: Option<PathBuf>,
    #[arg(long, env = "SOBLENDWA_MONGO", conflicts_with_all = ["sqlite", "memory"])]
    mongo: Option<String>,
    #[arg(long, env = "SOBLENDWA_MONGO_DB", default_value = "soblendwa")]
    mongo_db: String,
    #[arg(long)]
    no_link_previews: bool,
    #[arg(long)]
    memory: bool,
    #[arg(long, env = "SOBLENDWA_WEBHOOK")]
    webhook: Option<String>,
    #[arg(long, env = "SOBLENDWA_PROXY")]
    proxy: Option<String>,
    #[arg(long, env = "SOBLENDWA_PUSH_NAME")]
    push_name: Option<String>,
    #[arg(long)]
    no_chat_store: bool,
    #[arg(long)]
    fake: bool,
    #[arg(long = "session", default_value = "default")]
    sessions: Vec<String>,
    #[arg(long, env = "SOBLENDWA_OPTIONS")]
    options: Option<String>,
}

fn client_options(raw: Option<&str>) -> Result<soblendwa_bridge::Options, String> {
    let Some(raw) = raw else { return Ok(soblendwa_bridge::Options::default()) };
    let text = match raw.strip_prefix('@') {
        Some(path) => std::fs::read_to_string(path).map_err(|err| format!("--options {raw}: {err}"))?,
        None => raw.to_owned(),
    };
    let value: Value = serde_json::from_str(&text).map_err(|err| format!("--options is not valid JSON: {err}"))?;
    soblendwa_bridge::options_from_value(value).map_err(|err| format!("--options: {err}"))
}

fn rand_jitter() -> u32 {
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.subsec_nanos());
    nanos ^ std::process::id().rotate_left(16)
}

fn show_qr(session: &str, code: &str) {
    match QrCode::new(code.as_bytes()) {
        Ok(qr) => println!(
            "\n[{session}] Scan with WhatsApp > Linked devices:\n{}",
            qr.render::<Dense1x2>().quiet_zone(true).build()
        ),
        Err(_) => println!("[{session}] QR: {code}"),
    }
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("soblendwa=info,soblendwa_server=info")),
        )
        .init();
    let args = Args::parse();
    println!("{}", soblendwa_bridge::credits());

    let storage = match (&args.sqlite, args.memory) {
        _ if args.mongo.is_some() => {
            Storage::Mongo { url: args.mongo.clone().unwrap_or_default(), database: args.mongo_db.clone() }
        }
        (Some(path), _) => Storage::Sqlite(path.clone()),
        (None, true) => Storage::Memory,
        (None, false) => Storage::Directory(args.dir.clone()),
    };
    if args.token.is_none() && args.host != "127.0.0.1" && args.host != "localhost" {
        tracing::warn!("listening on {} without --token: anyone on the network can use your WhatsApp", args.host);
    }
    let fake = if args.fake {
        let fake = std::sync::Arc::new(soblendwa::testing::FakeWhatsApp::start().await.map_err(std::io::Error::other)?);
        println!("FAKE WHATSAPP on {}: nothing leaves this machine, sessions live in memory", fake.url());
        for name in &args.sessions {
            println!("  [{name}] fake account +{}", fake_phone(name));
        }
        Some(fake)
    } else {
        None
    };
    let client = client_options(args.options.as_deref()).unwrap_or_else(|err| {
        eprintln!("{err}");
        std::process::exit(2)
    });
    let hub = Hub::new(ServerConfig {
        client,
        token: args.token.clone(),
        storage,
        webhook: args.webhook.clone(),
        proxy: args.proxy.clone(),
        push_name: args.push_name.clone(),
        chat_store: !args.no_chat_store,
        fake: fake.clone(),
        link_previews: !args.no_link_previews,
    });

    for (index, name) in args.sessions.iter().enumerate() {
        if index > 0 {
            let spread = 400 + u64::from(rand_jitter()) % 800;
            tokio::time::sleep(std::time::Duration::from_millis(spread)).await;
        }
        let live = match hub.open(name).await {
            Ok(live) => live,
            Err(err) => {
                eprintln!("could not open session {name}: {err}");
                continue;
            }
        };
        let mut events = live.subscribe();
        let name = name.clone();
        tokio::spawn(async move {
            while let Ok(frame) = events.recv().await {
                let Ok(frame) = serde_json::from_str::<Value>(&frame) else { continue };
                let event = &frame["event"];
                match event["type"].as_str() {
                    Some("qr") => show_qr(&name, event["code"].as_str().unwrap_or_default()),
                    Some("connected") => println!("[{name}] online"),
                    Some("accountBlocked") => println!(
                        "[{name}] blocked by whatsapp ({}): {}. Not reconnecting.",
                        event["code"],
                        event["reason"].as_str().unwrap_or_default()
                    ),
                    Some("loggedOut") => {
                        println!("[{name}] logged out: {}", event["reason"].as_str().unwrap_or_default())
                    }
                    _ => {}
                }
            }
        });
    }

    let listener = TcpListener::bind((args.host.as_str(), args.port)).await?;
    println!(
        "Listening on http://{}  ·  WebSocket ws://{}/ws/<session>",
        listener.local_addr()?,
        listener.local_addr()?
    );
    serve(listener, hub, async {
        let _ = tokio::signal::ctrl_c().await;
        println!("Shutting down, saving sessions…");
    })
    .await
}
