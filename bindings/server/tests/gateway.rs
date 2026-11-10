use std::net::SocketAddr;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use soblendwa_server::{serve, Hub, ServerConfig, Storage};
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use tokio_tungstenite::tungstenite::Message;

const TOKEN: &str = "secreto-de-prueba";

async fn start() -> (SocketAddr, oneshot::Sender<()>) {
    let hub = Hub::new(ServerConfig {
        token: Some(TOKEN.into()),
        storage: Storage::Memory,
        chat_store: true,
        ..Default::default()
    });
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (stop, stopped) = oneshot::channel::<()>();
    tokio::spawn(serve(listener, hub, async {
        let _ = stopped.await;
    }));
    (address, stop)
}

async fn http(address: SocketAddr, method: &str, path: &str, token: Option<&str>, body: Option<Value>) -> (u16, Value) {
    let client = reqwest::Client::new();
    let url = format!("http://{address}{path}");
    let mut request = if method == "GET" { client.get(url) } else { client.post(url) };
    if let Some(token) = token {
        request = request.header("authorization", format!("Bearer {token}"));
    }
    if let Some(body) = body {
        request = request.header("content-type", "application/json").body(body.to_string());
    }
    let response = request.send().await.unwrap();
    let status = response.status().as_u16();
    (status, serde_json::from_str(&response.text().await.unwrap()).unwrap_or(Value::Null))
}

#[tokio::test]
async fn http_api() {
    let (address, _stop) = start().await;
    let (status, info) = http(address, "GET", "/", None, None).await;
    assert_eq!(status, 200);
    assert!(info["methods"].as_array().unwrap().iter().any(|m| m == "sendText"));

    let (status, _) = http(address, "POST", "/api/bot1/status", None, None).await;
    assert_eq!(status, 401);
    let (status, _) = http(address, "POST", "/api/bot1/status", Some("otro"), None).await;
    assert_eq!(status, 401);
    let (status, reply) = http(address, "POST", "/api/bot1/status", Some(TOKEN), None).await;
    assert_eq!(status, 200);
    assert_eq!(reply["result"]["me"], Value::Null);

    let body = json!({ "chat": "5215511111111", "text": "hola" });
    let (status, reply) = http(address, "POST", "/api/bot1/sendText", Some(TOKEN), Some(body)).await;
    assert_eq!(status, 422);
    assert!(reply["error"]["message"].as_str().unwrap().contains("not logged in"));

    let (status, reply) = http(address, "POST", "/api/bot1/nope", Some(TOKEN), None).await;
    assert_eq!((status, reply["error"]["code"].as_str().unwrap()), (422, "INVALID_ARGUMENT"));
    assert!(reply["error"]["message"].as_str().unwrap().ends_with("unknown method \"nope\""));
    let (status, _) = http(address, "POST", "/api/mal%20nombre/status", Some(TOKEN), None).await;
    assert_eq!(status, 400);

    let (status, sessions) = http(address, "GET", "/sessions", Some(TOKEN), None).await;
    assert_eq!(status, 200);
    assert_eq!(sessions[0]["session"], "bot1");

    let client = reqwest::Client::new();
    let refused = client.get(format!("http://{address}/metrics")).send().await.unwrap();
    assert_eq!(refused.status().as_u16(), 401);
    let metrics = client.get(format!("http://{address}/metrics?token={TOKEN}")).send().await.unwrap();
    assert_eq!(metrics.status().as_u16(), 200);
    assert!(metrics.headers()["content-type"].to_str().unwrap().starts_with("text/plain"));
    let text = metrics.text().await.unwrap();
    assert!(text.contains("soblendwa_messages_received_total{session=\"bot1\"} 0"), "{text}");
    assert!(text.contains("soblendwa_connected{session=\"bot1\"}"), "{text}");
}

async fn next_json<S>(stream: &mut S) -> Value
where
    S: futures_util::Stream<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin,
{
    loop {
        let message = tokio::time::timeout(Duration::from_secs(30), stream.next()).await.unwrap().unwrap().unwrap();
        if let Message::Text(text) = message {
            return serde_json::from_str(&text).unwrap();
        }
    }
}

#[tokio::test]
async fn websocket_calls() {
    let (address, _stop) = start().await;
    let refused = tokio_tungstenite::connect_async(format!("ws://{address}/ws/bot2")).await;
    assert!(refused.is_err());

    let (socket, _) = tokio_tungstenite::connect_async(format!("ws://{address}/ws/bot2?token={TOKEN}")).await.unwrap();
    let (mut sink, mut stream) = socket.split();
    let hello = next_json(&mut stream).await;
    assert_eq!((hello["type"].as_str(), hello["session"].as_str()), (Some("hello"), Some("bot2")));

    for (id, method, params) in [
        (1, "status", json!({})),
        (2, "sendText", json!({ "chat": "5215511111111", "text": "hola" })),
        (3, "chats", json!({})),
        (4, "nope", json!({})),
    ] {
        let request = json!({ "id": id, "method": method, "params": params });
        sink.send(Message::Text(request.to_string())).await.unwrap();
    }
    let mut answers = std::collections::HashMap::new();
    while answers.len() < 4 {
        let frame = next_json(&mut stream).await;
        if let Some(id) = frame["id"].as_i64() {
            answers.insert(id, frame);
        }
    }
    assert_eq!(answers[&1]["result"]["connected"], false);
    assert!(answers[&2]["error"]["message"].as_str().unwrap().contains("not logged in"));
    assert_eq!(answers[&3]["result"], json!([]));
    assert!(answers[&4]["error"]["message"].as_str().unwrap().contains("unknown method"));
}

#[tokio::test]
#[ignore = "connects to the real WhatsApp servers"]
async fn live_qr() {
    let (address, _stop) = start().await;
    let (socket, _) = tokio_tungstenite::connect_async(format!("ws://{address}/ws/live?token={TOKEN}")).await.unwrap();
    let (_sink, mut stream) = socket.split();
    loop {
        let frame = next_json(&mut stream).await;
        if frame["event"]["type"] == "qr" {
            assert!(frame["event"]["code"].as_str().unwrap().starts_with("https://wa.me/settings/linked_devices#"));
            assert_eq!(frame["session"], "live");
            break;
        }
    }
}
