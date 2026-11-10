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
