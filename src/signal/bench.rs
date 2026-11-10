use std::sync::Arc;
use std::time::Instant;

use super::{EncType, PreKeyBundle, SignalRepository};
use crate::binary::{self, Node};
use crate::crypto::KeyPair;
use crate::jid::Jid;
use crate::store::{Creds, MemoryStore};

fn peer() -> (SignalRepository, Creds) {
    let creds = Creds::generate();
    (SignalRepository::new(Arc::new(MemoryStore::new()), &creds), creds)
}

#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn throughput() {
    let (alice, _) = peer();
    let (bob, bob_creds) = peer();
    let one_time = KeyPair::generate();
    bob.store_pre_keys(&[(1, one_time.clone())]).await.unwrap();
    let bundle = PreKeyBundle {
        registration_id: bob_creds.registration_id,
        identity_key: bob_creds.identity_key.public,
        signed_pre_key_id: 1,
        signed_pre_key: bob_creds.signed_pre_key.key_pair.public,
        signed_pre_key_signature: bob_creds.signed_pre_key.signature,
        pre_key: Some((1, one_time.public)),
    };
    alice.inject_bundle("bob.0", &bundle).await.unwrap();
    let (_, first) = alice.encrypt("bob.0", b"hello").await.unwrap();
    bob.decrypt("alice.0", EncType::PreKey, &first).await.unwrap();
    let (_, ack) = bob.encrypt("alice.0", b"ack").await.unwrap();
    alice.decrypt("bob.0", EncType::Message, &ack).await.unwrap();

    let stored = alice.store.get(crate::store::KeyKind::Session, "bob.0").await.unwrap().unwrap();
    let started = Instant::now();
    for _ in 0..20_000 {
        let record = super::session::SessionRecord::from_bytes(&stored).unwrap();
        std::hint::black_box(record.to_bytes());
    }
    println!("BENCH session protobuf round trip: {:?} ({} bytes)", started.elapsed() / 20_000, stored.len());

    let payload = vec![7u8; 120];
    let rounds = 20_000;
    let started = Instant::now();
    for _ in 0..rounds {
        let (kind, ct) = alice.encrypt("bob.0", &payload).await.unwrap();
        bob.decrypt("alice.0", kind, &ct).await.unwrap();
    }
    let elapsed = started.elapsed();
    println!("BENCH signal 1:1 encrypt+decrypt: {:.0} msg/s", rounds as f64 / elapsed.as_secs_f64());

    let started = Instant::now();
    for i in 0..rounds {
        let (ct, dist) = alice.group_encrypt("g@g.us", "alice.0", &payload).await.unwrap();
        if i == 0 {
            bob.process_distribution("g@g.us", "alice.0", &dist).await.unwrap();
        }
        bob.group_decrypt("g@g.us", "alice.0", &ct).await.unwrap();
    }
    let elapsed = started.elapsed();
    println!("BENCH signal group encrypt+decrypt: {:.0} msg/s", rounds as f64 / elapsed.as_secs_f64());

    let pair = KeyPair::generate();
    let other = KeyPair::generate();
    let started = Instant::now();
    for _ in 0..2000 {
        std::hint::black_box(pair.shared_secret(&other.public));
    }
    let dh = started.elapsed() / 2000;
    let signature = pair.sign(b"bench");
    let started = Instant::now();
    for _ in 0..2000 {
        std::hint::black_box(crate::crypto::verify(&pair.public, b"bench", &signature));
    }
    let verify = started.elapsed() / 2000;
    let started = Instant::now();
    for _ in 0..2000 {
        std::hint::black_box(KeyPair::generate());
    }
    let keygen = started.elapsed() / 2000;
    println!("BENCH x25519 {dh:?} · xeddsa verify {verify:?} · keygen {keygen:?}");

    let bundles: Vec<(String, PreKeyBundle)> = (0..1000u32)
        .map(|i| {
            let one_time = KeyPair::generate();
            let bundle = PreKeyBundle { pre_key: Some((i, one_time.public)), ..bundle.clone() };
            (format!("member{i}.0"), bundle)
        })
        .collect();
    let (sequential, _) = peer();
    let started = Instant::now();
    for (address, bundle) in &bundles {
        sequential.inject_bundles(vec![(address.clone(), bundle.clone())]).await;
    }
    let one_by_one = started.elapsed();
    let (parallel, _) = peer();
    let started = Instant::now();
    let results = parallel.inject_bundles(bundles).await;
    let batched = started.elapsed();
    assert!(results.iter().all(|(_, r)| r.is_ok()));
    println!("BENCH 1000 group sessions: one by one {one_by_one:?} · parallel {batched:?}");

    let stanza = Node::new("message")
        .attr("id", "3EB0A1B2C3D4E5F6A7B8C9")
        .attr("to", Jid::group("120363025246125486"))
        .attr("type", "text")
        .children((0..20).map(|i| {
            Node::new("to")
                .attr("jid", Jid::phone("5215512345678").with_device(i))
                .child_node(Node::new("enc").attr("v", "2").attr("type", "msg").bytes(vec![1u8; 140]))
        }));
    let rounds = 200_000;
    let started = Instant::now();
    let mut total = 0usize;
    for _ in 0..rounds {
        let encoded = binary::encode(&stanza);
        total += binary::decode_frame(&encoded).unwrap().nodes().len();
    }
    let elapsed = started.elapsed();
    assert_eq!(total, rounds * 20);
    println!(
        "BENCH binary encode+decode (21 nodes, {} bytes): {:.0} stanzas/s",
        binary::encode(&stanza).len(),
        rounds as f64 / elapsed.as_secs_f64()
    );
}

struct SlowReads {
    inner: Arc<dyn crate::store::Store>,
    delay: std::time::Duration,
}

#[async_trait::async_trait]
impl crate::store::Store for SlowReads {
    async fn load_creds(&self) -> crate::Result<Option<Creds>> {
        self.inner.load_creds().await
    }

    async fn save_creds(&self, creds: &Creds) -> crate::Result<()> {
        self.inner.save_creds(creds).await
    }

    async fn get(&self, kind: crate::store::KeyKind, key: &str) -> crate::Result<Option<Vec<u8>>> {
        tokio::time::sleep(self.delay).await;
        self.inner.get(kind, key).await
    }

    async fn set(&self, kind: crate::store::KeyKind, key: &str, value: Vec<u8>) -> crate::Result<()> {
        self.inner.set(kind, key, value).await
    }

    async fn remove(&self, kind: crate::store::KeyKind, key: &str) -> crate::Result<()> {
        self.inner.remove(kind, key).await
    }

    async fn clear(&self) -> crate::Result<()> {
        self.inner.clear().await
    }
}

async fn talking_to(bot: &SignalRepository, bot_creds: &Creds, peers: usize) -> Vec<SignalRepository> {
    let mut out = Vec::with_capacity(peers);
    for i in 0..peers {
        let (peer, _) = peer();
        let one_time = KeyPair::generate();
        let id = 1000 + i as u32;
        bot.store_pre_keys(&[(id, one_time.clone())]).await.unwrap();
        let bundle = PreKeyBundle {
            registration_id: bot_creds.registration_id,
            identity_key: bot_creds.identity_key.public,
            signed_pre_key_id: bot_creds.signed_pre_key.id,
            signed_pre_key: bot_creds.signed_pre_key.key_pair.public,
            signed_pre_key_signature: bot_creds.signed_pre_key.signature,
            pre_key: Some((id, one_time.public)),
        };
        peer.inject_bundle("bot.0", &bundle).await.unwrap();
        let (kind, first) = peer.encrypt("bot.0", b"hola").await.unwrap();
        bot.decrypt(&format!("peer{i}.0"), kind, &first).await.unwrap();
        let (kind, answer) = bot.encrypt(&format!("peer{i}.0"), b"hola").await.unwrap();
        peer.decrypt("bot.0", kind, &answer).await.unwrap();
        out.push(peer);
    }
    out
}

async fn decrypt_all(bot: &Arc<SignalRepository>, inbox: Vec<Vec<(EncType, Vec<u8>)>>) -> std::time::Duration {
    let started = Instant::now();
    let tasks: Vec<_> = inbox
        .into_iter()
        .enumerate()
        .map(|(i, messages)| {
            let bot = bot.clone();
            tokio::spawn(async move {
                for (kind, data) in messages {
                    bot.decrypt(&format!("peer{i}.0"), kind, &data).await.unwrap();
                }
            })
        })
        .collect();
    for task in tasks {
        task.await.unwrap();
    }
    started.elapsed()
}

async fn inbox(peers: &[SignalRepository], each: usize) -> Vec<Vec<(EncType, Vec<u8>)>> {
    let mut out = Vec::new();
    for peer in peers {
        let mut messages = Vec::with_capacity(each);
        for _ in 0..each {
            messages.push(peer.encrypt("bot.0", &[7u8; 120]).await.unwrap());
        }
        out.push(messages);
    }
    out
}

fn contention_round(threads: usize) {
    let runtime = tokio::runtime::Builder::new_multi_thread().worker_threads(threads).enable_all().build().unwrap();
    runtime.block_on(async {
        let creds = Creds::generate();
        let store: Arc<dyn crate::store::Store> = Arc::new(MemoryStore::new());
        let bot = SignalRepository::new(store.clone(), &creds);
        let peers = talking_to(&bot, &creds, 32).await;

        let warm = Arc::new(bot);
        let each = 300;
        let took = decrypt_all(&warm, inbox(&peers, each).await).await;
        println!(
            "BENCH {threads} thread(s), 32 chats in cache: {:.0} decrypts/s",
            (32 * each) as f64 / took.as_secs_f64()
        );

        let delay = std::time::Duration::from_millis(5);
        let slow: Arc<dyn crate::store::Store> = Arc::new(SlowReads { inner: store, delay });
        let restarted = Arc::new(SignalRepository::new(slow, &creds));
        let took = decrypt_all(&restarted, inbox(&peers, 1).await).await;
        println!("BENCH {threads} thread(s), 32 chats after a restart, {delay:?} per store read: {took:?}");
    });
}

#[test]
#[ignore]
fn contention() {
    for threads in [1, 4] {
        contention_round(threads);
    }
}
