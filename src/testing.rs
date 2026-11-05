use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use futures_util::{SinkExt, Stream, StreamExt};
use parking_lot::Mutex;
use prost::Message as _;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::{self, Message as WsMessage};

use crate::binary::{self, Content, Node};
use crate::client::{Client, ClientBuilder, EventStream, DEFAULT_VERSION};
use crate::crypto::{self, KeyPair};
use crate::error::{Error, Result};
use crate::events::Event;
use crate::jid::{Jid, GROUP_SERVER, USER_SERVER};
use crate::message::now_secs;
use crate::noise::{self, FrameDecoder, NoiseCipher, ServerHandshake, WA_HEADER};
use crate::proto::{AdvSignedDeviceIdentity, ClientPayload, HandshakeMessage};
use crate::store::{Creds, MemoryStore, Store};

const BOT_DEVICE: u16 = 1;
const CONNECT_WAIT: Duration = Duration::from_secs(15);

enum Control {
    Node(Node),
    Close,
}

#[derive(Clone, Default)]
struct Keys {
    registration: Vec<u8>,
    identity: Vec<u8>,
    signed: Option<Node>,
    one_time: Vec<Node>,
}

struct Group {
    subject: String,
    creator: Jid,
    created: i64,
    members: Vec<Jid>,
}

#[derive(Default)]
struct World {
    keys: HashMap<Jid, Keys>,
    devices: HashMap<Jid, Vec<u16>>,
    online: HashMap<Jid, mpsc::UnboundedSender<Control>>,
    mailbox: HashMap<Jid, Vec<Node>>,
    groups: HashMap<Jid, Group>,
    blocked: HashMap<String, u16>,
    refused: HashSet<String>,
    corrupt: HashMap<String, u32>,
    held_acks: HashSet<String>,
    media: HashMap<String, Vec<u8>>,
    uploads: u64,
    routed: u64,
}

struct Shared {
    server_static: KeyPair,
    certificate: Vec<u8>,
    world: Mutex<World>,
    counter: AtomicU64,
}

pub struct FakeWhatsApp {
    addr: SocketAddr,
    media_addr: SocketAddr,
    root: KeyPair,
    shared: Arc<Shared>,
    accept: tokio::task::JoinHandle<()>,
    media_accept: tokio::task::JoinHandle<()>,
}

impl std::fmt::Debug for FakeWhatsApp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FakeWhatsApp").field("addr", &self.addr).finish_non_exhaustive()
    }
}

impl Drop for FakeWhatsApp {
    fn drop(&mut self) {
        self.accept.abort();
        self.media_accept.abort();
    }
}

fn seal(writer: &mut NoiseCipher, node: &Node) -> Result<WsMessage> {
    Ok(WsMessage::Binary(noise::frame(None, &writer.encrypt(&binary::encode(node))?)?))
}

async fn next_frame<S>(stream: &mut S, decoder: &mut FrameDecoder, intro: &mut bool) -> Result<Option<Vec<u8>>>
where
    S: Stream<Item = std::result::Result<WsMessage, tungstenite::Error>> + Unpin,
{
    loop {
        if let Some(frame) = decoder.next_frame() {
            return Ok(Some(frame));
        }
        match stream.next().await {
            Some(Ok(WsMessage::Binary(data))) => {
                if std::mem::take(intro) {
                    let body = data
                        .strip_prefix(WA_HEADER.as_slice())
                        .ok_or_else(|| Error::Handshake("missing intro".into()))?;
                    decoder.push(body);
                } else {
                    decoder.push(&data);
                }
            }
            Some(Ok(WsMessage::Close(_))) | None => return Ok(None),
            Some(Ok(_)) => {}
            Some(Err(err)) => return Err(err.into()),
        }
    }
}

fn corrupt_encrypted(node: &mut Node) {
    if let Content::Nodes(children) = &mut node.content {
        for enc in children.iter_mut().filter(|child| child.tag == "enc") {
            let Some(mut body) = enc.content_bytes().map(<[u8]>::to_vec) else { continue };
            if let Some(last) = body.last_mut() {
                *last ^= 0xff;
            }
            *enc = enc.clone().bytes(body);
        }
    }
}

async fn read_http_body(reader: &mut BufReader<TcpStream>, length: Option<usize>, chunked: bool) -> Result<Vec<u8>> {
    let mut body = Vec::new();
    if chunked {
        loop {
            let mut size_line = String::new();
            reader.read_line(&mut size_line).await?;
            let size = usize::from_str_radix(size_line.trim(), 16).map_err(|err| Error::other(err.to_string()))?;
            let mut chunk = vec![0; size + 2];
            reader.read_exact(&mut chunk).await?;
            chunk.truncate(size);
            body.extend_from_slice(&chunk);
            if size == 0 {
                return Ok(body);
            }
        }
    }
    body.resize(length.unwrap_or(0), 0);
    reader.read_exact(&mut body).await?;
    Ok(body)
}

fn error_iq(id: &str, code: u16, text: &str) -> Node {
    Node::new("iq")
        .attr("id", id.to_owned())
        .attr("type", "error")
        .attr("from", Jid::server())
        .child_node(Node::new("error").attr("code", code.to_string()).attr("text", text.to_owned()))
}

impl Shared {
    async fn serve_media(self: Arc<Self>, tcp: TcpStream) -> Result<()> {
        let mut reader = BufReader::new(tcp);
        loop {
            let mut request = String::new();
            if reader.read_line(&mut request).await? == 0 {
                return Ok(());
            }
            let mut parts = request.split_whitespace();
            let (method, target) =
                (parts.next().unwrap_or_default().to_owned(), parts.next().unwrap_or("/").to_owned());
            let path = target.split('?').next().unwrap_or_default().to_owned();
            let (mut length, mut chunked) = (None, false);
            loop {
                let mut header = String::new();
                reader.read_line(&mut header).await?;
                let line = header.trim();
                if line.is_empty() {
                    break;
                }
                let lower = line.to_ascii_lowercase();
                if let Some(value) = lower.strip_prefix("content-length:") {
                    length = value.trim().parse().ok();
                }
                chunked |= lower.starts_with("transfer-encoding:") && lower.contains("chunked");
            }
            let body = read_http_body(&mut reader, length, chunked).await?;
            let (status, content_type, answer) = self.media_answer(&method, &path, body);
            let mut head = format!(
                "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\n\r\n",
                answer.len()
            )
            .into_bytes();
            if method != "HEAD" {
                head.extend_from_slice(&answer);
            }
            reader.get_mut().write_all(&head).await?;
        }
    }

    fn media_answer(&self, method: &str, path: &str, body: Vec<u8>) -> (&'static str, &'static str, Vec<u8>) {
        let mut world = self.world.lock();
        if method == "POST" {
            let serial = self.counter.fetch_add(1, Ordering::Relaxed);
            let direct_path = format!("/fake-media/{serial}{}", path.rsplit_once('/').map_or("", |(kind, _)| kind));
            world.media.insert(direct_path.clone(), body);
            world.uploads += 1;
            let answer =
                serde_json::json!({ "url": format!("http://fake.media{direct_path}"), "direct_path": direct_path });
            return ("200 OK", "application/json", answer.to_string().into_bytes());
        }
        match world.media.get(path) {
            Some(stored) => ("200 OK", "application/octet-stream", stored.clone()),
            None => ("404 Not Found", "text/plain", b"expired".to_vec()),
        }
    }

    async fn serve(self: Arc<Self>, tcp: TcpStream) -> Result<()> {
        let mut ws = tokio_tungstenite::accept_async(tcp).await?;
        let mut decoder = FrameDecoder::default();
        let mut intro = true;
        let hello = next_frame(&mut ws, &mut decoder, &mut intro).await?.ok_or(Error::NotConnected)?;
        let client_ephemeral: [u8; 32] = HandshakeMessage::decode(hello.as_slice())?
            .client_hello
            .and_then(|h| h.ephemeral)
            .and_then(|e| e.try_into().ok())
            .ok_or_else(|| Error::Handshake("client hello without an ephemeral key".into()))?;
        let (handshake, server_hello) =
            ServerHandshake::respond(&client_ephemeral, &self.server_static, &self.certificate)?;
        let reply = HandshakeMessage { server_hello: Some(server_hello), ..Default::default() };
        ws.send(WsMessage::Binary(noise::frame(None, &reply.encode_to_vec())?)).await?;
        let finish = next_frame(&mut ws, &mut decoder, &mut intro).await?.ok_or(Error::NotConnected)?;
        let finish = HandshakeMessage::decode(finish.as_slice())?
            .client_finish
            .ok_or_else(|| Error::Handshake("client did not finish the handshake".into()))?;
        let (_, payload, mut writer, mut reader) = handshake.complete(&finish)?;
        let payload = ClientPayload::decode(payload.as_slice())?;
        let user = payload
            .username
            .ok_or_else(|| Error::Handshake("the fake server only accepts linked sessions".into()))?
            .to_string();
        let device = Jid { user: user.clone(), device: payload.device.unwrap_or(0) as u16, server: USER_SERVER.into() };

        let (mut sink, mut stream) = ws.split();
        let (blocked, refused) = {
            let world = self.world.lock();
            (world.blocked.get(&user).copied(), world.refused.contains(&user))
        };
        if refused {
            let _ = sink.close().await;
            return Ok(());
        }
        if let Some(code) = blocked {
            sink.send(seal(&mut writer, &Node::new("failure").attr("reason", code.to_string()))?).await?;
            let _ = sink.close().await;
            return Ok(());
        }

        let (link, mut outbound) = mpsc::unbounded_channel::<Control>();
        let backlog = {
            let mut world = self.world.lock();
            if let Some(previous) = world.online.insert(device.clone(), link.clone()) {
                let _ = previous.send(Control::Close);
            }
            let devices = world.devices.entry(device.to_non_device()).or_default();
            if !devices.contains(&device.device) {
                devices.push(device.device);
            }
            world.mailbox.remove(&device).unwrap_or_default()
        };
        let writer_task = tokio::spawn(async move {
            while let Some(control) = outbound.recv().await {
                let sealed = match control {
                    Control::Node(node) => seal(&mut writer, &node),
                    Control::Close => break,
                };
                let Ok(message) = sealed else { break };
                if sink.send(message).await.is_err() {
                    break;
                }
            }
            let _ = sink.close().await;
        });
        let _ = link.send(Control::Node(Node::new("success").attr("t", now_secs())));
        for mut node in backlog {
            node.set_attr("offline", "1");
            let _ = link.send(Control::Node(node));
        }

        let mut no_intro = false;
        while let Ok(Some(frame)) = next_frame(&mut stream, &mut decoder, &mut no_intro).await {
            let Ok(plaintext) = reader.decrypt(&frame) else { break };
            match binary::decode_frame(&plaintext) {
                Ok(node) => self.handle(&device, &link, node),
                Err(err) => tracing::debug!(%err, "fake whatsapp dropped an undecodable frame"),
            }
        }
        {
            let mut world = self.world.lock();
            if world.online.get(&device).is_some_and(|current| current.same_channel(&link)) {
                world.online.remove(&device);
            }
        }
        writer_task.abort();
        Ok(())
    }

    fn handle(&self, from: &Jid, link: &mpsc::UnboundedSender<Control>, node: Node) {
        match node.tag.as_ref() {
            "iq" => {
                let _ = link.send(Control::Node(self.answer_iq(from, &node)));
            }
            "message" => self.route_message(from, link, &node),
            "receipt" => self.route_receipt(from, &node),
            _ => {}
        }
    }

    fn answer_iq(&self, from: &Jid, iq: &Node) -> Node {
        let id = iq.attr_string("id").unwrap_or_default();
        let result = |children: Vec<Node>| {
            Node::new("iq").attr("id", id.clone()).attr("type", "result").attr("from", Jid::server()).children(children)
        };
        let kind = iq.attr_str("type").unwrap_or_default();
        match iq.attr_str("xmlns").unwrap_or_default() {
            "passive" | "w:p" | "presence" => result(Vec::new()),
            "encrypt" => match self.encrypt_iq(from, iq, kind) {
                Some(children) => result(children),
                None => error_iq(&id, 400, "bad-request"),
            },
            "usync" => result(vec![self.usync(iq)]),
            "w:m" => result(vec![Node::new("media_conn")
                .attr("auth", "fake-media-auth")
                .attr("ttl", "3600")
                .child_node(Node::new("host").attr("hostname", "fake.media"))]),
            "w:g2" => match self.groups_iq(from, iq, kind) {
                Some(children) => result(children),
                None => error_iq(&id, 404, "item-not-found"),
            },
            _ => error_iq(&id, 501, "feature-not-implemented"),
        }
    }

    fn encrypt_iq(&self, from: &Jid, iq: &Node, kind: &str) -> Option<Vec<Node>> {
        let mut world = self.world.lock();
        if kind == "get" && iq.child("count").is_some() {
            let count = world.keys.get(from).map_or(0, |k| k.one_time.len());
            return Some(vec![Node::new("count").attr("value", count.to_string())]);
        }
        if kind == "set" {
            let keys = world.keys.entry(from.clone()).or_default();
            if let Some(rotate) = iq.child("rotate") {
                keys.signed = rotate.child("skey").cloned();
                return Some(Vec::new());
            }
            if let Some(registration) = iq.child_bytes("registration") {
                keys.registration = registration.to_vec();
            }
            if let Some(identity) = iq.child_bytes("identity") {
                keys.identity = identity.to_vec();
            }
            if let Some(list) = iq.child("list") {
                keys.one_time.extend(list.children_by("key").cloned());
            }
            if let Some(signed) = iq.child("skey") {
                keys.signed = Some(signed.clone());
            }
            return Some(Vec::new());
        }
        let wanted = iq.child("key")?;
        let users = wanted
            .children_by("user")
            .filter_map(|user| user.attr_jid("jid"))
            .map(|jid| match world.keys.get_mut(&jid) {
                Some(keys) if keys.signed.is_some() => {
                    let one_time = (!keys.one_time.is_empty()).then(|| keys.one_time.remove(0));
                    Node::new("user").attr("jid", &jid).children(
                        [
                            Some(Node::new("registration").bytes(keys.registration.clone())),
                            Some(Node::new("type").bytes(vec![crypto::KEY_TYPE])),
                            Some(Node::new("identity").bytes(keys.identity.clone())),
                            keys.signed.clone(),
                            one_time,
                        ]
                        .into_iter()
                        .flatten(),
                    )
                }
                _ => Node::new("user")
                    .attr("jid", &jid)
                    .child_node(Node::new("error").attr("code", "404").attr("text", "item-not-found")),
            })
            .collect::<Vec<_>>();
        Some(vec![Node::new("list").children(users)])
    }

    fn usync(&self, iq: &Node) -> Node {
        let world = self.world.lock();
        let users = iq
            .path(&["usync", "list"])
            .map(Node::nodes)
            .unwrap_or_default()
            .iter()
            .filter_map(|user| {
                if let Some(contact) = user.child("contact").and_then(Node::content_string) {
                    let jid = Jid::phone(&contact);
                    let known = world.devices.contains_key(&jid);
                    return Some(
                        Node::new("user")
                            .attr("jid", &jid)
                            .child_node(Node::new("contact").attr("type", if known { "in" } else { "out" })),
                    );
                }
                let jid = user.attr_jid("jid")?.to_non_device();
                let devices = world.devices.get(&jid).cloned().unwrap_or_default();
                let list = devices.iter().map(|id| {
                    let device = Node::new("device").attr("id", id.to_string());
                    if *id == 0 {
                        device
                    } else {
                        device.attr("key-index", id.to_string())
                    }
                });
                Some(
                    Node::new("user")
                        .attr("jid", &jid)
                        .child_node(Node::new("devices").child_node(Node::new("device-list").children(list))),
                )
            })
            .collect::<Vec<_>>();
        Node::new("usync").child_node(Node::new("list").children(users))
    }

    fn group_node(jid: &Jid, group: &Group) -> Node {
        Node::new("group")
            .attr("id", jid.user.clone())
            .attr("subject", group.subject.clone())
            .attr("creation", group.created)
            .attr("creator", &group.creator)
            .attr("addressing_mode", "pn")
            .children(group.members.iter().map(|member| {
                let participant = Node::new("participant").attr("jid", member);
                if *member == group.creator {
                    participant.attr("type", "superadmin")
                } else {
                    participant
                }
            }))
    }

    fn groups_iq(&self, from: &Jid, iq: &Node, kind: &str) -> Option<Vec<Node>> {
        let me = from.to_non_device();
        if kind == "set" {
            let create = iq.child("create")?;
            let members = create.children_by("participant").filter_map(|p| p.attr_jid("jid")).collect::<Vec<_>>();
            let jid = self.new_group(create.attr_string("subject").unwrap_or_default(), me, members);
            let world = self.world.lock();
            return Some(vec![Self::group_node(&jid, world.groups.get(&jid)?)]);
        }
        let world = self.world.lock();
        if iq.child("participating").is_some() {
            let groups = world
                .groups
                .iter()
                .filter(|(_, group)| group.members.contains(&me))
                .map(|(jid, group)| Self::group_node(jid, group));
            return Some(vec![Node::new("groups").children(groups)]);
        }
        let jid = iq.attr_jid("to")?;
        world.groups.get(&jid).map(|group| vec![Self::group_node(&jid, group)])
    }

    fn new_group(&self, subject: String, creator: Jid, members: Vec<Jid>) -> Jid {
        let serial = self.counter.fetch_add(1, Ordering::Relaxed);
        let jid = Jid::new(format!("120363{serial:012}"), GROUP_SERVER);
        let mut all = vec![creator.clone()];
        all.extend(members.into_iter().map(|m| m.to_non_device()).filter(|m| *m != creator));
        let group = Group { subject, creator, created: now_secs(), members: all };
        self.world.lock().groups.insert(jid.clone(), group);
        jid
    }

    fn deliver(world: &mut World, device: &Jid, mut node: Node) {
        world.routed += 1;
        if let Some(left) = world.corrupt.get_mut(&device.user).filter(|left| **left > 0) {
            *left -= 1;
            corrupt_encrypted(&mut node);
        }
        match world.online.get(device) {
            Some(link) => {
                let _ = link.send(Control::Node(node));
            }
            None => world.mailbox.entry(device.clone()).or_default().push(node),
        }
    }

    fn route_message(&self, from: &Jid, link: &mpsc::UnboundedSender<Control>, node: &Node) {
        let (Some(id), Some(to)) = (node.attr_string("id"), node.attr_jid("to")) else { return };
        let now = now_secs();
        if !self.world.lock().held_acks.contains(&from.user) {
            let ack = Node::new("ack").attr("class", "message").attr("id", id.clone()).attr("from", &to).attr("t", now);
            let _ = link.send(Control::Node(ack));
        }

        let mut per_device: HashMap<Jid, Node> = HashMap::new();
        if let Some(participants) = node.child("participants") {
            for target in participants.children_by("to") {
                if let (Some(jid), Some(enc)) = (target.attr_jid("jid"), target.child("enc")) {
                    per_device.insert(jid, enc.clone());
                }
            }
        }
        let shared: Vec<Node> = node.children_by("enc").cloned().collect();
        let identity = node.child("device-identity").cloned();
        let kind = node.attr_string("type").unwrap_or_else(|| "text".into());
        let envelope = |device: &Jid, sender_attrs: Node| -> Option<Node> {
            let own = per_device.get(device).cloned();
            if own.is_none() && shared.is_empty() {
                return None;
            }
            let needs_identity = own.iter().chain(shared.iter()).any(|enc| enc.attr_is("type", "pkmsg"));
            let children =
                own.into_iter().chain(shared.iter().cloned()).chain(identity.clone().filter(|_| needs_identity));
            Some(sender_attrs.attr("id", id.clone()).attr("type", kind.clone()).attr("t", now).children(children))
        };

        let mut world = self.world.lock();
        let resend_target =
            if to.is_group() { node.attr_jid("participant") } else { node.has("device_fanout").then(|| to.clone()) };
        if let Some(device) = resend_target {
            let attrs = if to.is_group() {
                Node::new("message").attr("from", &to).attr("participant", from)
            } else {
                Node::new("message").attr("from", from)
            };
            if let Some(message) = envelope(&device, attrs) {
                Self::deliver(&mut world, &device, message);
            }
            return;
        }
        if to.is_group() {
            let Some(members) = world.groups.get(&to).map(|g| g.members.clone()) else { return };
            for member in members {
                let devices = world.devices.get(&member).cloned().unwrap_or_default();
                for device in devices.into_iter().map(|d| member.with_device(d)).filter(|d| d != from) {
                    let attrs = Node::new("message").attr("from", &to).attr("participant", from);
                    if let Some(message) = envelope(&device, attrs) {
                        Self::deliver(&mut world, &device, message);
                    }
                }
            }
            return;
        }
        for device in per_device.keys().cloned().collect::<Vec<_>>() {
            let attrs = if device.user == from.user {
                Node::new("message").attr("from", from).attr("recipient", &to)
            } else {
                Node::new("message").attr("from", from)
            };
            if let Some(message) = envelope(&device, attrs) {
                Self::deliver(&mut world, &device, message);
            }
        }
    }

    fn route_receipt(&self, from: &Jid, node: &Node) {
        let (Some(id), Some(to)) = (node.attr_string("id"), node.attr_jid("to")) else { return };
        let target = if to.is_group() { node.attr_jid("participant") } else { Some(to.clone()) };
        let Some(target) = target.map(|t| t.to_non_device()) else { return };
        let mut world = self.world.lock();
        let is_retry = node.attr_is("type", "retry");
        let requester = if is_retry { from.clone() } else { from.to_non_device() };
        let receipt = Node::new("receipt")
            .attr("id", id)
            .attr("from", if to.is_group() { to.clone() } else { requester.clone() })
            .attr_opt("participant", to.is_group().then(|| requester.clone()))
            .attr_opt("type", node.attr_string("type"))
            .attr("t", now_secs())
            .children(node.nodes().to_vec());
        for device in world.devices.get(&target).cloned().unwrap_or_default() {
            let device = target.with_device(device);
            if is_retry {
                Self::deliver(&mut world, &device, receipt.clone());
            } else if let Some(link) = world.online.get(&device) {
                let _ = link.send(Control::Node(receipt.clone()));
            }
        }
    }
}

impl FakeWhatsApp {
    pub async fn start() -> Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let addr = listener.local_addr()?;
        let root = KeyPair::generate();
        let server_static = KeyPair::generate();
        let certificate = noise::issue_certificate_chain(&root, &server_static.public);
        let shared =
            Arc::new(Shared { server_static, certificate, world: Mutex::default(), counter: AtomicU64::new(1) });
        let serving = shared.clone();
        let accept = tokio::spawn(async move {
            while let Ok((tcp, _)) = listener.accept().await {
                let shared = serving.clone();
                tokio::spawn(async move {
                    if let Err(err) = shared.serve(tcp).await {
                        tracing::debug!(%err, "fake whatsapp connection ended");
                    }
                });
            }
        });
        let media_listener = TcpListener::bind("127.0.0.1:0").await?;
        let media_addr = media_listener.local_addr()?;
        let serving = shared.clone();
        let media_accept = tokio::spawn(async move {
            while let Ok((tcp, _)) = media_listener.accept().await {
                let shared = serving.clone();
                tokio::spawn(async move {
                    if let Err(err) = shared.serve_media(tcp).await {
                        tracing::debug!(%err, "fake media connection ended");
                    }
                });
            }
        });
        Ok(FakeWhatsApp { addr, media_addr, root, shared, accept, media_accept })
    }

    pub fn url(&self) -> String {
        format!("ws://{}/ws/chat", self.addr)
    }

    pub fn trust_root(&self) -> [u8; 32] {
        self.root.public
    }

    pub fn media_endpoint(&self) -> String {
        format!("http://{}", self.media_addr)
    }

    pub fn media_uploads(&self) -> u64 {
        self.shared.world.lock().uploads
    }

    pub fn expire_media(&self) {
        self.shared.world.lock().media.clear();
    }

    pub fn jid(phone: &str) -> Jid {
        Jid::phone(phone)
    }

    pub fn account(&self, phone: &str) -> Creds {
        let mut creds = Creds::generate();
        let identity = AdvSignedDeviceIdentity {
            details: Some(crypto::random_vec(24)),
            account_signature_key: Some(crypto::random_vec(32)),
            account_signature: Some(crypto::random_vec(64)),
            device_signature: Some(crypto::random_vec(64)),
        };
        creds.me = Some(Jid::phone(phone).with_device(BOT_DEVICE));
        creds.push_name = Some(format!("Fake {phone}"));
        creds.account = Some(identity.encode_to_vec());
        creds.registered = true;
        creds.platform = Some("web".into());
        creds
    }

    pub fn builder(&self) -> ClientBuilder {
        let mut builder = Client::builder().version(DEFAULT_VERSION);
        builder = builder.ws_url(self.url()).trust_root(self.trust_root()).media_endpoint(self.media_endpoint());
        builder
    }

    pub async fn connect(&self, phone: &str) -> Result<(Client, EventStream)> {
        let store: Arc<dyn Store> = Arc::new(MemoryStore::new());
        store.save_creds(&self.account(phone)).await?;
        self.connect_with(self.builder().store(store)).await
    }

    pub async fn connect_with(&self, builder: ClientBuilder) -> Result<(Client, EventStream)> {
        let (client, mut events) = builder.build().await?;
        client.connect().await?;
        let waiting = async {
            while let Some(event) = events.recv().await {
                if matches!(event, Event::Connected) {
                    return Ok(());
                }
            }
            Err(Error::NotConnected)
        };
        tokio::time::timeout(CONNECT_WAIT, waiting).await.map_err(|_| Error::Timeout)??;
        Ok((client, events))
    }

    pub fn create_group(&self, subject: &str, creator: &str, members: &[&str]) -> Jid {
        let members = members.iter().map(|m| Jid::phone(m)).collect();
        self.shared.new_group(subject.to_owned(), Jid::phone(creator), members)
    }

    pub fn drop_connections(&self, phone: &str) {
        let user = Jid::phone(phone);
        let world = self.shared.world.lock();
        for (device, link) in &world.online {
            if device.to_non_device() == user {
                let _ = link.send(Control::Close);
            }
        }
    }

    pub fn refuse_connections(&self, phone: &str, refuse: bool) {
        let user = Jid::phone(phone).user;
        {
            let mut world = self.shared.world.lock();
            if refuse {
                world.refused.insert(user);
            } else {
                world.refused.remove(&user);
            }
        }
        if refuse {
            self.drop_connections(phone);
        }
    }

    pub fn block(&self, phone: &str, code: u16) {
        self.shared.world.lock().blocked.insert(Jid::phone(phone).user, code);
        self.drop_connections(phone);
    }

    pub fn unblock(&self, phone: &str) {
        self.shared.world.lock().blocked.remove(&Jid::phone(phone).user);
    }

    pub fn hold_acks(&self, phone: &str, hold: bool) {
        let user = Jid::phone(phone).user;
        let mut world = self.shared.world.lock();
        if hold {
            world.held_acks.insert(user);
        } else {
            world.held_acks.remove(&user);
        }
    }

    pub fn push_node(&self, phone: &str, node: Node) {
        let user = Jid::phone(phone);
        let world = self.shared.world.lock();
        for (device, link) in &world.online {
            if device.to_non_device() == user {
                let _ = link.send(Control::Node(node.clone()));
            }
        }
    }

    pub fn corrupt_next(&self, phone: &str, messages: u32) {
        self.shared.world.lock().corrupt.insert(Jid::phone(phone).user, messages);
    }

    pub fn routed(&self) -> u64 {
        self.shared.world.lock().routed
    }

    pub fn is_online(&self, phone: &str) -> bool {
        let user = Jid::phone(phone);
        self.shared.world.lock().online.keys().any(|device| device.to_non_device() == user)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::{DisconnectReason, MessageEvent};
    use crate::{SendLimit, SendTimings};

    const BOT: &str = "5215511111111";
    const ANA: &str = "5215522222222";
    const CARLOS: &str = "5215533333333";
    const WAIT: Duration = Duration::from_secs(10);

    async fn next_event<T>(events: &mut EventStream, mut pick: impl FnMut(Event) -> Option<T>) -> T {
        let found = async {
            while let Some(event) = events.recv().await {
                if let Some(found) = pick(event) {
                    return found;
                }
            }
            panic!("the event stream closed");
        };
        tokio::time::timeout(WAIT, found).await.expect("timed out waiting for an event")
    }

    async fn next_text(events: &mut EventStream) -> MessageEvent {
        next_event(events, |event| match event {
            Event::Message(message) if message.text().is_some() => Some(*message),
            _ => None,
        })
        .await
    }

    async fn ack_timings(events: &mut EventStream, id: &str) -> SendTimings {
        next_event(events, |event| match event {
            Event::ServerAck(ack) if ack.id == id => ack.timings,
            _ => None,
        })
        .await
    }

    #[tokio::test]
    async fn two_accounts_talk_end_to_end_through_the_fake_server() {
        let fake = FakeWhatsApp::start().await.unwrap();
        let (bot, mut bot_events) = fake.connect(BOT).await.unwrap();
        let (ana, mut ana_events) = fake.connect(ANA).await.unwrap();

        ana.send_text(&Jid::phone(BOT), "!ping").await.unwrap();
        let command = next_text(&mut bot_events).await;
        assert_eq!(command.text(), Some("!ping"));
        assert_eq!(command.info.sender_phone, Some(Jid::phone(ANA)));
        assert!(!command.info.offline);

        let id = bot.reply(&command, crate::proto::Message::text("Pong")).await.unwrap();
        let timings = ack_timings(&mut bot_events, &id).await;
        assert!(timings.encrypt > Duration::ZERO && timings.server.is_some(), "{timings:?}");
        let answer = next_text(&mut ana_events).await;
        assert_eq!((answer.text(), answer.info.id.as_str()), (Some("Pong"), id.as_str()));
        assert_eq!(answer.quoted().and_then(|q| q.get_text()), Some("!ping"));

        for round in 0..3 {
            ana.send_text(&Jid::phone(BOT), format!("mensaje {round}")).await.unwrap();
            assert_eq!(next_text(&mut bot_events).await.text(), Some(format!("mensaje {round}").as_str()));
        }
        let stats = bot.stats();
        assert_eq!((stats.received, stats.sent, stats.undecryptable, stats.panics), (4, 1, 0, 0));
        assert!(stats.connected);
    }

    #[tokio::test]
    async fn groups_fan_out_with_sender_keys() {
        let fake = FakeWhatsApp::start().await.unwrap();
        let (bot, mut bot_events) = fake.connect(BOT).await.unwrap();
        let (ana, mut ana_events) = fake.connect(ANA).await.unwrap();
        let (_carlos, mut carlos_events) = fake.connect(CARLOS).await.unwrap();
        let group = fake.create_group("pruebas", ANA, &[BOT, CARLOS]);

        ana.send_text(&group, "hola grupo").await.unwrap();
        for events in [&mut bot_events, &mut carlos_events] {
            let message = next_text(events).await;
            assert_eq!((message.text(), &message.info.chat), (Some("hola grupo"), &group));
            assert_eq!(message.info.sender_phone, Some(Jid::phone(ANA)));
        }
        bot.send_text(&group, "el bot responde en el grupo").await.unwrap();
        bot.send_text(&group, "y sigue con la misma sender key").await.unwrap();
        for events in [&mut ana_events, &mut carlos_events] {
            assert_eq!(next_text(events).await.text(), Some("el bot responde en el grupo"));
            assert_eq!(next_text(events).await.text(), Some("y sigue con la misma sender key"));
        }
    }

    #[tokio::test]
    async fn a_dropped_session_reconnects_and_flushes_its_outbox() {
        let fake = FakeWhatsApp::start().await.unwrap();
        let (bot, mut bot_events) = fake.connect(BOT).await.unwrap();
        let (ana, mut ana_events) = fake.connect(ANA).await.unwrap();
        ana.send_text(&Jid::phone(BOT), "hola").await.unwrap();
        next_text(&mut bot_events).await;

        fake.refuse_connections(BOT, true);
        next_event(&mut bot_events, |event| {
            matches!(event, Event::Disconnected { reconnecting: true, .. }).then_some(())
        })
        .await;
        let parked = bot.send_text(&Jid::phone(ANA), "escrito sin conexión").await.unwrap();
        let pending = bot.pending_sends().await.unwrap();
        assert_eq!(pending.iter().map(|p| p.id.as_str()).collect::<Vec<_>>(), vec![parked.as_str()]);

        fake.refuse_connections(BOT, false);
        next_event(&mut bot_events, |event| matches!(event, Event::Connected).then_some(())).await;
        let delivered = next_text(&mut ana_events).await;
        assert_eq!((delivered.text(), delivered.info.id.as_str()), (Some("escrito sin conexión"), parked.as_str()));
        tokio::time::timeout(WAIT, async {
            while !bot.pending_sends().await.unwrap().is_empty() {
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .expect("the outbox never emptied");
        let stats = bot.stats();
        assert_eq!((stats.outbox_resent, stats.reconnects), (1, 1));
    }

    async fn undecryptable_id(events: &mut EventStream) -> String {
        next_event(events, |event| match event {
            Event::UndecryptableMessage { info, .. } => Some(info.id),
            _ => None,
        })
        .await
    }

    async fn recovered_id(events: &mut EventStream) -> String {
        next_event(events, |event| match event {
            Event::MessageRecovered { id, .. } => Some(id),
            _ => None,
        })
        .await
    }

    #[tokio::test]
    async fn a_corrupted_message_is_recovered_through_a_retry() {
        let fake = FakeWhatsApp::start().await.unwrap();
        let (bot, mut bot_events) = fake.connect(BOT).await.unwrap();
        let (ana, _ana_events) = fake.connect(ANA).await.unwrap();
        ana.send_text(&Jid::phone(BOT), "hola").await.unwrap();
        assert_eq!(next_text(&mut bot_events).await.text(), Some("hola"));

        fake.corrupt_next(BOT, 1);
        let id = ana.send_text(&Jid::phone(BOT), "este llega roto").await.unwrap();
        assert_eq!(undecryptable_id(&mut bot_events).await, id);
        assert_eq!(bot.stats().awaiting_recovery, 1);
        let recovered = next_text(&mut bot_events).await;
        assert_eq!((recovered.text(), recovered.info.id.as_str()), (Some("este llega roto"), id.as_str()));
        assert_eq!(recovered_id(&mut bot_events).await, id);
        let stats = bot.stats();
        assert_eq!((stats.undecryptable, stats.recovered, stats.awaiting_recovery), (1, 1, 0));

        ana.send_text(&Jid::phone(BOT), "y la sesión sigue sana").await.unwrap();
        assert_eq!(next_text(&mut bot_events).await.text(), Some("y la sesión sigue sana"));
        assert_eq!(bot.stats().undecryptable, 1);
    }

    #[tokio::test]
    async fn a_group_message_without_its_sender_key_is_recovered() {
        let fake = FakeWhatsApp::start().await.unwrap();
        let (bot, mut bot_events) = fake.connect(BOT).await.unwrap();
        let (ana, _ana_events) = fake.connect(ANA).await.unwrap();
        let (_carlos, mut carlos_events) = fake.connect(CARLOS).await.unwrap();
        let group = fake.create_group("pruebas", ANA, &[BOT, CARLOS]);

        fake.corrupt_next(BOT, 1);
        let id = ana.send_text(&group, "llega sin sender key").await.unwrap();
        assert_eq!(undecryptable_id(&mut bot_events).await, id);
        let recovered = next_text(&mut bot_events).await;
        assert_eq!((recovered.text(), &recovered.info.chat), (Some("llega sin sender key"), &group));
        assert_eq!(recovered_id(&mut bot_events).await, id);
        assert_eq!(next_text(&mut carlos_events).await.text(), Some("llega sin sender key"));

        ana.send_text(&group, "ya tiene la sender key").await.unwrap();
        assert_eq!(next_text(&mut bot_events).await.text(), Some("ya tiene la sender key"));
        let stats = bot.stats();
        assert_eq!((stats.undecryptable, stats.recovered, stats.awaiting_recovery), (1, 1, 0));
    }

    #[tokio::test]
    async fn unconfirmed_sends_are_resent_only_after_a_real_drop_and_never_delivered_twice() {
        let fake = FakeWhatsApp::start().await.unwrap();
        let (bot, mut bot_events) = fake.connect(BOT).await.unwrap();
        let (ana, mut ana_events) = fake.connect(ANA).await.unwrap();
        fake.hold_acks(BOT, true);

        let mut ids = Vec::new();
        for round in 0..3 {
            ids.push(bot.send_text(&Jid::phone(ANA), format!("sin confirmar {round}")).await.unwrap());
        }
        for round in 0..3 {
            assert_eq!(next_text(&mut ana_events).await.text(), Some(format!("sin confirmar {round}").as_str()));
        }
        assert_eq!(bot.pending_sends().await.unwrap().len(), 3);

        bot.flush_outbox(bot.current_generation()).await;
        assert_eq!(bot.stats().outbox_resent, 0, "messages waiting for their ack must not be sent again");

        fake.hold_acks(BOT, false);
        fake.drop_connections(BOT);
        next_event(&mut bot_events, |event| matches!(event, Event::Connected).then_some(())).await;
        tokio::time::timeout(WAIT, async {
            while !bot.pending_sends().await.unwrap().is_empty() {
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .expect("the outbox never emptied");
        assert_eq!(bot.stats().outbox_resent, 3);

        ana.send_text(&Jid::phone(BOT), "control").await.unwrap();
        bot.send_text(&Jid::phone(ANA), "mensaje nuevo").await.unwrap();
        let next = next_text(&mut ana_events).await;
        assert_eq!(next.text(), Some("mensaje nuevo"), "the three resends must not reach the bot of ana again");
        assert_eq!(ana.stats().duplicates, 3);
        assert_eq!(ana.stats().received, 3 + 1);
    }

    #[tokio::test]
    async fn a_login_from_a_dead_connection_does_not_touch_the_new_one() {
        let fake = FakeWhatsApp::start().await.unwrap();
        let (bot, mut bot_events) = fake.connect(BOT).await.unwrap();
        let stale = bot.current_generation();
        let old_login = crate::binary::Node::new("success");
        fake.drop_connections(BOT);
        next_event(&mut bot_events, |event| matches!(event, Event::Disconnected { .. }).then_some(())).await;
        next_event(&mut bot_events, |event| matches!(event, Event::Connected).then_some(())).await;
        assert!(bot.current_generation() > stale);
        bot.on_success(&old_login, stale).await.unwrap();
        tokio::time::sleep(Duration::from_millis(100)).await;
        while let Ok(event) = bot_events.try_recv() {
            assert!(!matches!(event, Event::Connected), "a dead connection announced a login");
        }
        assert!(bot.is_connected());
    }

    fn cover_png() -> Vec<u8> {
        let image = image::RgbImage::from_pixel(800, 420, image::Rgb([40, 90, 220]));
        let mut out = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgb8(image).write_to(&mut out, image::ImageFormat::Png).unwrap();
        out.into_inner()
    }

    struct Site {
        origin: String,
        hits: Arc<AtomicU64>,
        task: tokio::task::JoinHandle<()>,
    }

    impl Drop for Site {
        fn drop(&mut self) {
            self.task.abort();
        }
    }

    async fn website() -> Site {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let hits = Arc::new(AtomicU64::new(0));
        let counter = hits.clone();
        let task = tokio::spawn(async move {
            while let Ok((mut tcp, _)) = listener.accept().await {
                let counter = counter.clone();
                tokio::spawn(async move {
                    let mut buffer = vec![0; 4096];
                    let read = tcp.read(&mut buffer).await.unwrap_or(0);
                    let request = String::from_utf8_lossy(&buffer[..read]).into_owned();
                    let path = request.split_whitespace().nth(1).unwrap_or("/").to_owned();
                    counter.fetch_add(1, Ordering::Relaxed);
                    let (kind, body) = if path.starts_with("/cover") {
                        ("image/png", cover_png())
                    } else {
                        let page = "<html><head><title>Respaldo</title>
                            <meta property='og:title' content='Título &amp; más'>
                            <meta property='og:description' content='Una descripción'>
                            <meta property='og:image' content='/cover.png'>
                            <meta property='og:video' content='https://x.io/v.mp4'>
                            </head><body>contenido</body></html>";
                        ("text/html; charset=utf-8", page.as_bytes().to_vec())
                    };
                    let head = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    );
                    let _ = tcp.write_all(head.as_bytes()).await;
                    let _ = tcp.write_all(&body).await;
                });
            }
        });
        Site { origin, hits, task }
    }

    async fn bot_with_previews(fake: &FakeWhatsApp, config: crate::LinkPreviewConfig) -> (Client, EventStream) {
        let store: Arc<dyn Store> = Arc::new(MemoryStore::new());
        store.save_creds(&fake.account(BOT)).await.unwrap();
        fake.connect_with(fake.builder().store(store).link_previews(config)).await.unwrap()
    }

    fn local_previews() -> crate::LinkPreviewConfig {
        crate::LinkPreviewConfig { allow_private_hosts: true, ..Default::default() }
    }

    #[tokio::test]
    async fn links_get_a_rich_preview_automatically_and_it_is_reused() {
        let fake = FakeWhatsApp::start().await.unwrap();
        let site = website().await;
        let (bot, _bot_events) = bot_with_previews(&fake, local_previews()).await;
        let (ana, mut ana_events) = fake.connect(ANA).await.unwrap();

        let link = format!("{}/articulo", site.origin);
        bot.send_text(&Jid::phone(ANA), format!("mira {link}.")).await.unwrap();
        let received = next_text(&mut ana_events).await;
        let text = received.message.extended_text_message.as_ref().expect("a preview travels in extended text");
        assert_eq!(text.text.as_deref(), Some(format!("mira {link}.").as_str()));
        assert_eq!(text.matched_text.as_deref(), Some(link.as_str()));
        assert_eq!(text.title.as_deref(), Some("Título & más"));
        assert_eq!(text.description.as_deref(), Some("Una descripción"));
        assert_eq!(text.preview_type, Some(1));
        assert_eq!((text.thumbnail_width, text.thumbnail_height), (Some(800), Some(420)));
        let inline = text.jpeg_thumbnail.as_deref().unwrap();
        assert_eq!(crate::imaging::image_dimensions(inline), Some((192, 100)));

        let thumbnail = received.message.link_thumbnail().expect("the sharp thumbnail was uploaded");
        let sharp = ana.download(&thumbnail).await.unwrap();
        assert_eq!(crate::imaging::image_dimensions(&sharp), Some((800, 420)));

        let (hits, uploads) = (site.hits.load(Ordering::Relaxed), fake.media_uploads());
        assert_eq!((hits, uploads), (2, 1));
        bot.send_text(&Jid::phone(ANA), format!("otra vez {link}")).await.unwrap();
        let again = next_text(&mut ana_events).await;
        assert_eq!(again.message.extended_text_message.as_ref().unwrap().title.as_deref(), Some("Título & más"));
        assert_eq!(
            (site.hits.load(Ordering::Relaxed), fake.media_uploads()),
            (2, 1),
            "preview and thumbnail are reused"
        );
    }

    #[tokio::test]
    async fn private_links_are_never_fetched_unless_allowed() {
        let fake = FakeWhatsApp::start().await.unwrap();
        let site = website().await;
        let (bot, _bot_events) = bot_with_previews(&fake, crate::LinkPreviewConfig::default()).await;
        let (_ana, mut ana_events) = fake.connect(ANA).await.unwrap();

        bot.send_text(&Jid::phone(ANA), format!("interno {}/articulo", site.origin)).await.unwrap();
        let received = next_text(&mut ana_events).await;
        assert!(received.message.extended_text_message.is_none(), "no preview for a private host");
        assert_eq!(site.hits.load(Ordering::Relaxed), 0, "the bot must not touch the private host");
    }

    #[tokio::test]
    async fn each_message_can_turn_the_preview_off_or_write_its_own() {
        let fake = FakeWhatsApp::start().await.unwrap();
        let site = website().await;
        let (bot, _bot_events) = bot_with_previews(&fake, local_previews()).await;
        let (_ana, mut ana_events) = fake.connect(ANA).await.unwrap();
        let link = format!("{}/articulo", site.origin);

        let off = crate::SendOptions { link_preview: crate::PreviewChoice::Off, ..Default::default() };
        bot.send_message_with(&Jid::phone(ANA), crate::proto::Message::text(link.clone()), off).await.unwrap();
        assert!(next_text(&mut ana_events).await.message.extended_text_message.is_none());
        assert_eq!(site.hits.load(Ordering::Relaxed), 0);

        let custom = crate::LinkPreview {
            title: Some("Mi título".into()),
            description: Some("Escrito a mano".into()),
            thumbnail: Some(vec![1, 2, 3]),
            ..Default::default()
        };
        let options =
            crate::SendOptions { link_preview: crate::PreviewChoice::Custom(Box::new(custom)), ..Default::default() };
        bot.send_message_with(&Jid::phone(ANA), crate::proto::Message::text(link.clone()), options).await.unwrap();
        let received = next_text(&mut ana_events).await;
        let text = received.message.extended_text_message.as_ref().unwrap();
        assert_eq!((text.title.as_deref(), text.matched_text.as_deref()), (Some("Mi título"), Some(link.as_str())));
        assert_eq!(text.jpeg_thumbnail.as_deref(), Some(&[1u8, 2, 3][..]));
        assert_eq!(site.hits.load(Ordering::Relaxed), 0, "a custom preview needs no fetch");

        let (off_bot, _events) = {
            let store: Arc<dyn Store> = Arc::new(MemoryStore::new());
            store.save_creds(&fake.account(CARLOS)).await.unwrap();
            let disabled = crate::LinkPreviewConfig { enabled: false, ..local_previews() };
            fake.connect_with(fake.builder().store(store).link_previews(disabled)).await.unwrap()
        };
        off_bot.send_text(&Jid::phone(ANA), link.clone()).await.unwrap();
        assert!(next_text(&mut ana_events).await.message.extended_text_message.is_none());
        off_bot.send_text_with_preview(&Jid::phone(ANA), link).await.unwrap();
        assert!(next_text(&mut ana_events).await.message.extended_text_message.is_some());
    }

    async fn next_media(events: &mut EventStream) -> MessageEvent {
        next_event(events, |event| match event {
            Event::Message(message) if message.message.media_type().is_some() => Some(*message),
            _ => None,
        })
        .await
    }

    #[tokio::test]
    async fn media_travels_end_to_end_and_repeats_are_not_uploaded_again() {
        let fake = FakeWhatsApp::start().await.unwrap();
        let (bot, _bot_events) = fake.connect(BOT).await.unwrap();
        let (ana, mut ana_events) = fake.connect(ANA).await.unwrap();
        let picture = cover_png();

        bot.send_image(&Jid::phone(ANA), &picture, Some("portada")).await.unwrap();
        let first = next_media(&mut ana_events).await;
        assert_eq!(first.text(), Some("portada"));
        assert_eq!(ana.download_message(&first.message).await.unwrap(), picture);
        assert_eq!(fake.media_uploads(), 1);

        bot.send_image(&Jid::phone(ANA), &picture, Some("la misma")).await.unwrap();
        let second = next_media(&mut ana_events).await;
        assert_eq!(ana.download_message(&second.message).await.unwrap(), picture);
        assert_eq!(fake.media_uploads(), 1, "the same file must not be uploaded twice");
        assert_eq!(bot.stats().media_reused, 1);

        let dir = std::env::temp_dir().join(format!("soblendwa-fake-media-{}", crate::crypto::random_u32()));
        std::fs::create_dir_all(&dir).unwrap();
        let (source, target) = (dir.join("grande.bin"), dir.join("copia.bin"));
        let big = crate::crypto::random_vec(3 * 1024 * 1024 + 17);
        std::fs::write(&source, &big).unwrap();
        bot.send_media_file(&Jid::phone(ANA), &source, crate::MediaKind::Document, crate::MediaOptions::default())
            .await
            .unwrap();
        let document = next_media(&mut ana_events).await;
        ana.download_to_file(document.message.media().unwrap(), &target).await.unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), big);
        let _ = std::fs::remove_dir_all(dir);

        fake.expire_media();
        assert!(ana.download_message(&first.message).await.is_err());
    }

    #[tokio::test]
    async fn a_retry_that_arrives_after_a_restart_is_still_answered() {
        let fake = FakeWhatsApp::start().await.unwrap();
        let store: Arc<dyn Store> = Arc::new(MemoryStore::new());
        store.save_creds(&fake.account(BOT)).await.unwrap();
        let (bot, mut bot_events) = fake.connect_with(fake.builder().store(store.clone())).await.unwrap();
        let (ana, mut ana_events) = fake.connect(ANA).await.unwrap();
        ana.send_text(&Jid::phone(BOT), "hola").await.unwrap();
        next_text(&mut bot_events).await;

        fake.refuse_connections(ANA, true);
        next_event(&mut ana_events, |event| matches!(event, Event::Disconnected { .. }).then_some(())).await;
        fake.corrupt_next(ANA, 1);
        let id = bot.send_text(&Jid::phone(ANA), "llega roto y tarde").await.unwrap();
        next_event(&mut bot_events, |event| match event {
            Event::ServerAck(ack) if ack.id == id => Some(()),
            _ => None,
        })
        .await;
        bot.disconnect().await;
        drop((bot, bot_events));

        fake.refuse_connections(ANA, false);
        assert_eq!(undecryptable_id(&mut ana_events).await, id);

        let (_restarted, _events) = fake.connect_with(fake.builder().store(store)).await.unwrap();
        let recovered = next_text(&mut ana_events).await;
        assert_eq!((recovered.text(), recovered.info.id.as_str()), (Some("llega roto y tarde"), id.as_str()));
        assert_eq!(recovered_id(&mut ana_events).await, id);
        assert_eq!(ana.stats().awaiting_recovery, 0);
    }

    #[tokio::test]
    async fn wrapped_messages_are_read_through_every_wrapper() {
        use crate::proto::{FutureProofMessage, Message};
        let wrap = |message: Message| Some(Box::new(FutureProofMessage { message: Some(Box::new(message)) }));
        let fake = FakeWhatsApp::start().await.unwrap();
        let (bot, _bot_events) = fake.connect(BOT).await.unwrap();
        let (_ana, mut ana_events) = fake.connect(ANA).await.unwrap();
        let wrappers = [
            Message { associated_child_message: wrap(Message::text("hijo de álbum")), ..Default::default() },
            Message { bot_invoke_message: wrap(Message::text("a Meta AI")), ..Default::default() },
            Message { group_mentioned_message: wrap(Message::text("mención de grupo")), ..Default::default() },
            Message { group_status_message_v2: wrap(Message::text("estado de grupo")), ..Default::default() },
        ];
        for message in wrappers {
            let expected = message.get_text().unwrap().to_owned();
            bot.send_message(&Jid::phone(ANA), message).await.unwrap();
            assert_eq!(next_text(&mut ana_events).await.text(), Some(expected.as_str()));
        }
    }

    #[tokio::test]
    async fn account_notifications_become_typed_events() {
        use crate::events::BlocklistChange;
        let fake = FakeWhatsApp::start().await.unwrap();
        let (_bot, mut events) = fake.connect(BOT).await.unwrap();
        let ana = Jid::phone(ANA);

        let notification = |kind: &str| {
            Node::new("notification")
                .attr("id", "1")
                .attr("type", kind.to_owned())
                .attr("from", &ana)
                .attr("t", 1_700_000_000i64)
        };
        fake.push_node(
            BOT,
            notification("picture")
                .child_node(Node::new("set").attr("jid", &ana).attr("id", "99").attr("author", &ana)),
        );
        let (jid, id, removed) = next_event(&mut events, |event| match event {
            Event::PictureChanged { jid, picture_id, removed, .. } => Some((jid, picture_id, removed)),
            _ => None,
        })
        .await;
        assert_eq!((jid, id.as_deref(), removed), (ana.clone(), Some("99"), false));

        fake.push_node(BOT, notification("status").child_node(Node::new("set").bytes(b"en el gym".to_vec())));
        let about = next_event(&mut events, |event| match event {
            Event::AboutChanged { about, .. } => Some(about),
            _ => None,
        })
        .await;
        assert_eq!(about, "en el gym");

        fake.push_node(
            BOT,
            notification("account_sync").children([
                Node::new("blocklist").child_node(Node::new("item").attr("jid", &ana).attr("action", "unblock")),
                Node::new("privacy").child_node(Node::new("category").attr("name", "last").attr("value", "none")),
            ]),
        );
        let items = next_event(&mut events, |event| match event {
            Event::BlocklistChanged { items } => Some(items),
            _ => None,
        })
        .await;
        assert_eq!(items, vec![BlocklistChange { jid: ana.clone(), phone: None, blocked: false }]);
        let settings = next_event(&mut events, |event| match event {
            Event::PrivacyChanged { settings } => Some(settings),
            _ => None,
        })
        .await;
        assert_eq!(settings, vec![("last".to_owned(), "none".to_owned())]);
    }

    #[tokio::test]
    async fn a_locked_account_stops_reconnecting_and_keeps_its_session() {
        let fake = FakeWhatsApp::start().await.unwrap();
        let (bot, mut bot_events) = fake.connect(BOT).await.unwrap();
        fake.block(BOT, 403);
        let (code, reason) = next_event(&mut bot_events, |event| match event {
            Event::AccountBlocked { code, reason, .. } => Some((code, reason)),
            _ => None,
        })
        .await;
        assert_eq!(code, 403);
        assert!(reason.contains("locked"), "{reason}");
        let disconnected = next_event(&mut bot_events, |event| match event {
            Event::Disconnected { reason, reconnecting } => Some((reason, reconnecting)),
            _ => None,
        })
        .await;
        assert_eq!(disconnected, (DisconnectReason::AccountBlocked(403), false));
        assert!(bot.store().load_creds().await.unwrap().is_some_and(|creds| creds.is_logged_in()));
        tokio::time::sleep(Duration::from_millis(1500)).await;
        assert!(!fake.is_online(BOT));
        assert!(bot.send_text(&Jid::phone(ANA), "no debería salir").await.is_err());
    }

    #[tokio::test]
    async fn a_runaway_loop_cannot_flood_a_chat() {
        let fake = FakeWhatsApp::start().await.unwrap();
        let store: Arc<dyn Store> = Arc::new(MemoryStore::new());
        store.save_creds(&fake.account(BOT)).await.unwrap();
        let limit = SendLimit { per_minute: 600, burst: 100, per_chat_per_minute: 6, per_chat_burst: 3 };
        let (bot, _bot_events) = fake.connect_with(fake.builder().store(store).send_limit(Some(limit))).await.unwrap();
        let (_ana, mut ana_events) = fake.connect(ANA).await.unwrap();

        let mut outcomes = Vec::new();
        for round in 0..10 {
            outcomes.push(bot.send_text(&Jid::phone(ANA), format!("spam {round}")).await);
        }
        assert_eq!(outcomes.iter().filter(|o| o.is_ok()).count(), 3);
        assert!(outcomes[3..].iter().all(|o| matches!(o, Err(Error::RateLimited { .. }))));
        assert_eq!(bot.stats().rate_limited, 7);
        for round in 0..3 {
            assert_eq!(next_text(&mut ana_events).await.text(), Some(format!("spam {round}").as_str()));
        }
    }
}
