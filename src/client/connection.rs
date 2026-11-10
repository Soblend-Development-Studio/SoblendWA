use std::sync::atomic::Ordering;
use std::sync::{Arc, LazyLock};
use std::time::{Duration, Instant};

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use futures_util::stream::{SplitSink, SplitStream};
use futures_util::{SinkExt, StreamExt};
use prost::Message as _;
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::header::{HeaderValue, ORIGIN};
use tokio_tungstenite::tungstenite::Message as WsMessage;
use tokio_tungstenite::{Connector, MaybeTlsStream, WebSocketStream};

use super::{Client, Link, Outcome};
use crate::binary::{self, Node};
use crate::crypto::{self, KeyPair};
use crate::error::{Error, Result};
use crate::events::Event;
use crate::jid::Jid;
use crate::noise::{self, FrameDecoder, Handshake, NoiseCipher};
use crate::proto::{
    AppVersion, ClientFinish, ClientHello, ClientPayload, DevicePairingRegistrationData, DeviceProps, HandshakeMessage,
    HistorySyncConfig, UserAgent, WebInfo,
};
use crate::store::Creds;

type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

const PING_TIMEOUT: Duration = Duration::from_secs(8);

static TLS: LazyLock<Arc<rustls::ClientConfig>> = LazyLock::new(|| {
    let mut roots = rustls::RootCertStore::empty();
    roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let config = rustls::ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
        .with_safe_default_protocol_versions()
        .expect("ring supports the default tls versions")
        .with_root_certificates(roots)
        .with_no_client_auth();
    Arc::new(config)
});

fn reconnect_delay(failures: u32) -> Option<Duration> {
    if failures == 0 {
        return None;
    }
    let base = Duration::from_millis(1000 * 2u64.pow((failures - 1).min(6))).min(Duration::from_secs(60));
    let spread = (base / 2).max(Duration::from_millis(750));
    let jitter = spread.mul_f64(f64::from(crypto::random_u32()) / f64::from(u32::MAX));
    Some(if failures == 1 { jitter } else { base + jitter })
}

struct Opened {
    sink: SplitSink<Socket, WsMessage>,
    stream: SplitStream<Socket>,
    writer: NoiseCipher,
    reader: NoiseCipher,
    decoder: FrameDecoder,
}

impl Client {
    pub(super) async fn supervise(self) {
        let mut stop = self.inner.stop.subscribe();
        let mut failures = 0u32;
        loop {
            if *stop.borrow_and_update() {
                break;
            }
            let outcome = self.run_once().await;
            let was_online = self.is_connected();
            self.teardown();
            tracing::info!(?outcome, "connection closed");

            let reconnect = match &outcome {
                Outcome::Stopped => false,
                Outcome::LoggedOut { reason, cause, code } => {
                    if let Err(err) = self.reset_session().await {
                        tracing::error!(%err, "failed to wipe the session after logout");
                    }
                    tracing::warn!(%reason, cause = cause.as_str(), ?code, "the session was logged out");
                    self.emit(Event::LoggedOut { reason: reason.clone(), cause: *cause, code: *code });
                    false
                }
                Outcome::Replaced => false,
                Outcome::Blocked { code, reason, expires } => {
                    tracing::error!(code, %reason, ?expires, "whatsapp blocked the account; not reconnecting");
                    self.emit(Event::AccountBlocked { code: *code, reason: reason.clone(), expires: *expires });
                    false
                }
                Outcome::Restart => true,
                Outcome::Outdated => {
                    self.forget_version().await;
                    true
                }
                Outcome::QrTimeout | Outcome::Lost(_) => self.inner.config.auto_reconnect,
            };
            if !matches!(outcome, Outcome::LoggedOut { .. }) {
                self.emit(Event::Disconnected { reason: outcome.reason(), reconnecting: reconnect });
            }
            if !reconnect {
                break;
            }

            failures = match outcome {
                Outcome::Restart | Outcome::QrTimeout => 0,
                _ if was_online => 1,
                _ => failures + 1,
            };
            if was_online {
                super::guard::Counters::bump(&self.inner.counters.reconnects);
            }
            if let Some(delay) = reconnect_delay(failures) {
                tracing::info!(?delay, failures, "reconnecting after a pause");
                tokio::select! {
                    _ = tokio::time::sleep(delay) => {}
                    _ = stop.changed() => {}
                }
            }
        }
        self.inner.running.store(false, Ordering::Release);
    }

    fn teardown(&self) {
        self.inner.connected.store(false, Ordering::Release);
        self.inner.active.store(false, Ordering::Release);
        *self.inner.link.lock() = None;
        self.inner.pending.lock().clear();
        self.inner.in_flight.clear();
    }

    async fn run_once(&self) -> Outcome {
        let opened = match self.open().await {
            Ok(opened) => opened,
            Err(err) => return Outcome::Lost(err.to_string()),
        };
        let Opened { mut sink, mut stream, mut writer, mut reader, mut decoder } = opened;

        let generation = self.inner.generation.fetch_add(1, Ordering::AcqRel) + 1;
        let (outbound, mut outbound_rx) = mpsc::unbounded_channel::<Vec<u8>>();
        let (close, mut close_rx) = mpsc::unbounded_channel::<Outcome>();
        *self.inner.link.lock() = Some(Link { outbound, close: close.clone() });

        let writer_close = close.clone();
        let writer_task = tokio::spawn(async move {
            while let Some(first) = outbound_rx.recv().await {
                let mut batch = vec![first];
                while let Ok(next) = outbound_rx.try_recv() {
                    batch.push(next);
                }
                for plaintext in batch {
                    let sealed = match writer.encrypt(&plaintext).and_then(|c| noise::frame(None, &c)) {
                        Ok(sealed) => sealed,
                        Err(err) => {
                            let _ = writer_close.send(Outcome::Lost(err.to_string()));
                            return;
                        }
                    };
                    if let Err(err) = sink.feed(WsMessage::Binary(sealed)).await {
                        let _ = writer_close.send(Outcome::Lost(err.to_string()));
                        return;
                    }
                }
                if let Err(err) = sink.flush().await {
                    let _ = writer_close.send(Outcome::Lost(err.to_string()));
                    return;
                }
            }
            let _ = sink.close().await;
        });

        let mut stop = self.inner.stop.subscribe();
        let interval = self.inner.config.keepalive_interval;
        let mut keepalive = tokio::time::interval_at(tokio::time::Instant::now() + interval, interval);
        let mut last_seen = Instant::now();

        let outcome = loop {
            tokio::select! {
                biased;
                outcome = close_rx.recv() => break outcome.unwrap_or_else(|| Outcome::Lost("link dropped".into())),
                _ = stop.changed() => {
                    if *stop.borrow_and_update() {
                        break Outcome::Stopped;
                    }
                }
                incoming = stream.next() => match incoming {
                    Some(Ok(WsMessage::Binary(data))) => {
                        last_seen = Instant::now();
                        decoder.push(&data);
                        if let Err(err) = self.drain_frames(&mut decoder, &mut reader) {
                            break Outcome::Lost(err.to_string());
                        }
                    }
                    Some(Ok(WsMessage::Close(_))) | None => break Outcome::Lost("socket closed by the server".into()),
                    Some(Ok(_)) => {}
                    Some(Err(err)) => break Outcome::Lost(err.to_string()),
                },
                _ = keepalive.tick() => {
                    if last_seen.elapsed() > interval * 2 + PING_TIMEOUT {
                        break Outcome::Lost("keepalive timed out".into());
                    }
                    self.spawn_ping(generation);
                }
            }
        };

        writer_task.abort();
        outcome
    }

    fn drain_frames(&self, decoder: &mut FrameDecoder, reader: &mut NoiseCipher) -> Result<()> {
        while let Some(frame) = decoder.next_frame() {
            let plaintext = reader.decrypt(&frame)?;
            match binary::decode_frame(&plaintext) {
                Ok(node) => {
                    tracing::trace!(%node, "received");
                    self.dispatch(node);
                }
                Err(err) => tracing::warn!(%err, "dropping undecodable frame"),
            }
        }
        Ok(())
    }

    pub async fn ping(&self) -> Result<Duration> {
        let started = Instant::now();
        let ping = Node::new("iq")
            .attr("to", Jid::server())
            .attr("type", "get")
            .attr("xmlns", "w:p")
            .child_node(Node::new("ping"));
        self.query(ping).await?;
        let elapsed = started.elapsed();
        self.record_latency(elapsed);
        Ok(elapsed)
    }

    pub fn latency(&self) -> Option<Duration> {
        match self.inner.latency_micros.load(Ordering::Relaxed) {
            0 => None,
            micros => Some(Duration::from_micros(micros)),
        }
    }

    fn record_latency(&self, sample: Duration) {
        let sample = sample.as_micros().clamp(1, u128::from(u64::MAX)) as u64;
        let previous = self.inner.latency_micros.load(Ordering::Relaxed);
        let smoothed = if previous == 0 { sample } else { (previous * 3 + sample) / 4 };
        self.inner.latency_micros.store(smoothed, Ordering::Relaxed);
    }

    fn spawn_ping(&self, generation: u64) {
        let client = self.clone();
        tokio::spawn(async move {
            match tokio::time::timeout(PING_TIMEOUT, client.ping()).await {
                Ok(Ok(_)) => {}
                Ok(Err(err)) => tracing::debug!(%err, "keepalive ping failed"),
                Err(_) if client.current_generation() == generation => {
                    tracing::warn!("whatsapp stopped answering pings, reconnecting");
                    client.end_link(Outcome::Lost("ping timed out".into()));
                }
                Err(_) => {}
            }
        });
    }

    async fn open(&self) -> Result<Opened> {
        let creds = self.inner.creds.read().clone();
        let version = self.wa_version().await;
        let timeout = self.inner.config.connect_timeout;
        tracing::debug!(?version, registered = creds.account.is_some(), "connecting to whatsapp web");

        let mut url = self.inner.config.ws_url.clone();
        if let Some(routing) = &creds.routing_info {
            url.push_str("?ED=");
            url.push_str(&URL_SAFE_NO_PAD.encode(routing));
        }
        let mut request = url.into_client_request()?;
        request.headers_mut().insert(ORIGIN, HeaderValue::from_static("https://web.whatsapp.com"));

        let connecting = async {
            let uri = request.uri();
            let host = uri.host().ok_or_else(|| Error::Other("websocket url has no host".into()))?.to_owned();
            let port = uri.port_u16().unwrap_or(if uri.scheme_str() == Some("ws") { 80 } else { 443 });
            let tcp = match &self.inner.config.proxy {
                Some(proxy) => proxy.connect(&host, port).await?,
                None => TcpStream::connect((host.as_str(), port)).await?,
            };
            tcp.set_nodelay(true)?;
            let connector = Connector::Rustls(TLS.clone());
            let (socket, _) =
                tokio_tungstenite::client_async_tls_with_config(request, tcp, None, Some(connector)).await?;
            Ok::<_, Error>(socket)
        };
        let socket = tokio::time::timeout(timeout, connecting).await.map_err(|_| Error::Timeout)??;
        let (mut sink, mut stream) = socket.split();

        let ephemeral = KeyPair::generate();
        let mut handshake = Handshake::new(&ephemeral.public);
        let hello = HandshakeMessage {
            client_hello: Some(ClientHello { ephemeral: Some(ephemeral.public.to_vec()), ..Default::default() }),
            ..Default::default()
        };
        let intro = noise::intro_header(creds.routing_info.as_deref());
        let hello_sent = Instant::now();
        sink.send(WsMessage::Binary(noise::frame(Some(&intro), &hello.encode_to_vec())?)).await?;

        let mut decoder = FrameDecoder::default();
        let reply = tokio::time::timeout(timeout, read_frame(&mut stream, &mut decoder))
            .await
            .map_err(|_| Error::Timeout)??;
        self.record_latency(hello_sent.elapsed());
        let server_hello = HandshakeMessage::decode(reply.as_slice())?
            .server_hello
            .ok_or_else(|| Error::Handshake("server did not answer with a hello".into()))?;
        let encrypted_static = handshake.process_server_hello(
            &server_hello,
            &ephemeral,
            &creds.noise_key,
            self.inner.config.trust_root.as_ref().unwrap_or(&noise::WA_ROOT_KEY),
        )?;

        let payload = client_payload(&creds, &self.inner.config, version);
        let encrypted_payload = handshake.encrypt(&payload.encode_to_vec())?;
        let finish = HandshakeMessage {
            client_finish: Some(ClientFinish { static_key: Some(encrypted_static), payload: Some(encrypted_payload) }),
            ..Default::default()
        };
        sink.send(WsMessage::Binary(noise::frame(None, &finish.encode_to_vec())?)).await?;

        let (writer, reader) = handshake.finish();
        Ok(Opened { sink, stream, writer, reader, decoder })
    }
}

async fn read_frame(stream: &mut SplitStream<Socket>, decoder: &mut FrameDecoder) -> Result<Vec<u8>> {
    loop {
        if let Some(frame) = decoder.next_frame() {
            return Ok(frame);
        }
        match stream.next().await {
            Some(Ok(WsMessage::Binary(data))) => decoder.push(&data),
            Some(Ok(WsMessage::Close(_))) | None => {
                return Err(Error::Handshake("socket closed during the handshake".into()))
            }
            Some(Ok(_)) => {}
            Some(Err(err)) => return Err(err.into()),
        }
    }
}

fn client_payload(creds: &Creds, config: &super::Config, version: [u32; 3]) -> ClientPayload {
    let user_agent = UserAgent {
        platform: Some(UserAgent::PLATFORM_WEB),
        app_version: Some(AppVersion {
            primary: Some(version[0]),
            secondary: Some(version[1]),
            tertiary: Some(version[2]),
            ..Default::default()
        }),
        mcc: Some("000".into()),
        mnc: Some("000".into()),
        os_version: Some("0.1".into()),
        device: Some("Desktop".into()),
        os_build_number: Some("0.1".into()),
        release_channel: Some(UserAgent::RELEASE_CHANNEL_RELEASE),
        locale_language_iso_639_1: Some("en".into()),
        locale_country_iso_3166_1_alpha_2: Some(config.country_code.clone()),
        ..Default::default()
    };
    let mut payload = ClientPayload {
        connect_type: Some(ClientPayload::CONNECT_TYPE_WIFI_UNKNOWN),
        connect_reason: Some(ClientPayload::CONNECT_REASON_USER_ACTIVATED),
        user_agent: Some(user_agent),
        web_info: Some(WebInfo { web_sub_platform: Some(0), ..Default::default() }),
        push_name: config.push_name.clone(),
        ..Default::default()
    };

    match (&creds.me, &creds.account) {
        (Some(me), Some(_)) => {
            payload.username = me.user.parse().ok();
            payload.device = Some(u32::from(me.device));
            payload.passive = Some(true);
            payload.pull = Some(true);
            payload.lid_db_migrated = Some(false);
        }
        _ => {
            let version_text = format!("{}.{}.{}", version[0], version[1], version[2]);
            let props = DeviceProps {
                os: Some(config.device_name.clone()),
                version: Some(AppVersion {
                    primary: Some(10),
                    secondary: Some(15),
                    tertiary: Some(7),
                    ..Default::default()
                }),
                platform_type: Some(config.platform.device_props_type()),
                require_full_sync: Some(false),
                history_sync_config: Some(HistorySyncConfig {
                    storage_quota_mb: Some(10240),
                    inline_initial_payload_in_e2ee_msg: Some(true),
                    support_call_log_history: Some(false),
                    support_bot_user_agent_chat_history: Some(true),
                    support_cag_reactions_and_polls: Some(true),
                    support_biz_hosted_msg: Some(true),
                    support_recent_sync_chunk_message_count_tuning: Some(true),
                    support_hosted_group_msg: Some(true),
                    support_fbid_bot_chat_history: Some(true),
                    support_message_association: Some(true),
                    support_group_history: Some(false),
                    ..Default::default()
                }),
            };
            payload.passive = Some(false);
            payload.pull = Some(false);
            payload.device_pairing_data = Some(DevicePairingRegistrationData {
                e_regid: Some(creds.registration_id.to_be_bytes().to_vec()),
                e_keytype: Some(vec![crypto::KEY_TYPE]),
                e_ident: Some(creds.identity_key.public.to_vec()),
                e_skey_id: Some(creds.signed_pre_key.id.to_be_bytes()[1..].to_vec()),
                e_skey_val: Some(creds.signed_pre_key.key_pair.public.to_vec()),
                e_skey_sig: Some(creds.signed_pre_key.signature.to_vec()),
                build_hash: Some(crypto::md5(version_text.as_bytes()).to_vec()),
                device_props: Some(props.encode_to_vec()),
            });
        }
    }
    payload
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff() {
        assert_eq!(reconnect_delay(0), None);
        let first: Vec<Duration> = (0..200).map(|_| reconnect_delay(1).unwrap()).collect();
        assert!(first.iter().all(|d| *d <= Duration::from_millis(750)));
        let distinct: std::collections::HashSet<_> = first.iter().map(|d| d.as_millis() / 50).collect();
        assert!(distinct.len() > 5, "sessions must not reconnect in lockstep");
        for failures in 2..12 {
            let delay = reconnect_delay(failures).unwrap();
            let base = Duration::from_millis(1000 * 2u64.pow((failures - 1).min(6))).min(Duration::from_secs(60));
            assert!(
                delay >= base && delay <= base + (base / 2).max(Duration::from_millis(750)),
                "{failures}: {delay:?}"
            );
        }
        assert!(reconnect_delay(30).unwrap() <= Duration::from_secs(90));
    }
}
