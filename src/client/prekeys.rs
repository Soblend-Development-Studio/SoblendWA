use super::Client;
use crate::binary::Node;
use crate::crypto::{KeyPair, KEY_TYPE};
use crate::error::Result;
use crate::jid::Jid;
use crate::message::now_secs;
use crate::store::{Creds, SignedPreKey};

pub(crate) const MIN_PRE_KEYS: u32 = 5;
pub(crate) const UPLOAD_BATCH: u32 = 200;
const INITIAL_UPLOAD: u32 = 812;
const MAX_PRE_KEY_ID: u32 = 0x00ff_ffff;
const SIGNED_PRE_KEY_LIFETIME: i64 = 30 * 86_400;

pub(crate) fn id_bytes(id: u32) -> Vec<u8> {
    id.to_be_bytes()[1..].to_vec()
}

pub(crate) fn pre_key_node(id: u32, pair: &KeyPair) -> Node {
    Node::new("key").children([Node::new("id").bytes(id_bytes(id)), Node::new("value").bytes(pair.public.to_vec())])
}

pub(crate) fn signed_pre_key_node(creds: &Creds) -> Node {
    let signed = &creds.signed_pre_key;
    Node::new("skey").children([
        Node::new("id").bytes(id_bytes(signed.id)),
        Node::new("value").bytes(signed.key_pair.public.to_vec()),
        Node::new("signature").bytes(signed.signature.to_vec()),
    ])
}

impl Client {
    pub(crate) async fn ensure_pre_keys(&self) -> Result<()> {
        let request = Node::new("iq")
            .attr("to", Jid::server())
            .attr("type", "get")
            .attr("xmlns", "encrypt")
            .child_node(Node::new("count"));
        let response = self.query(request).await?;
        let available = response.child("count").and_then(|c| c.attr_u64("value")).unwrap_or(0) as u32;
        if available >= MIN_PRE_KEYS {
            return Ok(());
        }
        self.upload_pre_keys(if available == 0 { INITIAL_UPLOAD } else { UPLOAD_BATCH }).await
    }

    pub(crate) async fn generate_pre_keys(&self, count: u32) -> Result<Vec<(u32, KeyPair)>> {
        let mut start = 0;
        self.update_creds(|creds| {
            start = creds.next_pre_key_id.max(1);
            let mut next = start + count;
            if next > MAX_PRE_KEY_ID {
                start = 1;
                next = 1 + count;
            }
            creds.next_pre_key_id = next;
            creds.first_unuploaded_pre_key_id = next;
        })
        .await?;
        let keys: Vec<(u32, KeyPair)> = (start..start + count).map(|id| (id, KeyPair::generate())).collect();
        self.signal().store_pre_keys(&keys).await?;
        Ok(keys)
    }

    pub async fn rotate_signed_pre_key(&self) -> Result<()> {
        let _guard = self.inner.pre_key_lock.lock().await;
        let creds = self.creds();
        let next_id = if creds.signed_pre_key.id >= MAX_PRE_KEY_ID { 1 } else { creds.signed_pre_key.id + 1 };
        let next = SignedPreKey::generate(&creds.identity_key, next_id);
        let mut rotated = creds.clone();
        rotated.signed_pre_key = next.clone();
        let request = Node::new("iq")
            .attr("to", Jid::server())
            .attr("type", "set")
            .attr("xmlns", "encrypt")
            .child_node(Node::new("rotate").child_node(signed_pre_key_node(&rotated)));
        self.query(request).await?;
        self.signal().rotate_signed_pre_key(next.id, next.key_pair.clone()).await?;
        self.update_creds(|c| {
            c.signed_pre_key = next;
            c.signed_pre_key_rotated_at = now_secs();
        })
        .await?;
        tracing::info!(id = next_id, "rotated the signed pre-key");
        Ok(())
    }

    pub(crate) async fn maintain_signed_pre_key(&self) -> Result<()> {
        let rotated_at = self.creds().signed_pre_key_rotated_at;
        if rotated_at == 0 {
            return self.update_creds(|c| c.signed_pre_key_rotated_at = now_secs()).await;
        }
        if now_secs() - rotated_at > SIGNED_PRE_KEY_LIFETIME {
            self.rotate_signed_pre_key().await?;
        }
        Ok(())
    }

    pub(crate) async fn upload_pre_keys(&self, count: u32) -> Result<()> {
        let _guard = self.inner.pre_key_lock.lock().await;
        let keys = self.generate_pre_keys(count).await?;
        let creds = self.creds();
        let request =
            Node::new("iq").attr("to", Jid::server()).attr("type", "set").attr("xmlns", "encrypt").children([
                Node::new("registration").bytes(creds.registration_id.to_be_bytes().to_vec()),
                Node::new("type").bytes(vec![KEY_TYPE]),
                Node::new("identity").bytes(creds.identity_key.public.to_vec()),
                Node::new("list").children(keys.iter().map(|(id, pair)| pre_key_node(*id, pair))),
                signed_pre_key_node(&creds),
            ]);
        self.query(request).await?;
        tracing::info!(count, "uploaded pre-keys");
        Ok(())
    }
}
