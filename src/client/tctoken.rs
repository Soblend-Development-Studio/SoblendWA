use std::sync::atomic::{AtomicBool, Ordering};

use prost::Message as _;

use super::Client;
use crate::binary::Node;
use crate::error::Result;
use crate::jid::Jid;
use crate::message::now_secs;
use crate::proto::StoredTcToken;
use crate::store::KeyKind;

const BUCKET_SECONDS: i64 = 604_800;
const BUCKETS: i64 = 4;

pub(crate) struct ServerProps {
    pub privacy_token_on_direct: AtomicBool,
    pub privacy_token_on_profile_picture: AtomicBool,
    pub issue_tokens_to_lid: AtomicBool,
}

impl Default for ServerProps {
    fn default() -> Self {
        ServerProps {
            privacy_token_on_direct: AtomicBool::new(true),
            privacy_token_on_profile_picture: AtomicBool::new(true),
            issue_tokens_to_lid: AtomicBool::new(false),
        }
    }
}

fn expired(timestamp: i64) -> bool {
    let cutoff = (now_secs() / BUCKET_SECONDS - (BUCKETS - 1)) * BUCKET_SECONDS;
    timestamp < cutoff
}

fn needs_fresh_issue(issued_at: Option<i64>) -> bool {
    issued_at.is_none_or(|issued| now_secs() / BUCKET_SECONDS > issued / BUCKET_SECONDS)
}

fn is_regular_user(jid: &Jid) -> bool {
    let bot = (jid.user.len() == 11 && jid.user.starts_with("1313555"))
        || (jid.user.len() == 11 && jid.user.starts_with("131655500"));
    jid.is_user() && jid.user != "0" && !bot
}

fn flag(value: Option<&str>) -> Option<bool> {
    value.map(|v| v == "true" || v == "1")
}

impl Client {
    pub(crate) async fn fetch_server_props(&self) -> Result<()> {
        let request = Node::new("iq")
            .attr("to", Jid::server())
            .attr("type", "get")
            .attr("xmlns", "abt")
            .child_node(Node::new("props").attr("protocol", "1"));
        let response = self.query(request).await?;
        let Some(props) = response.child("props") else { return Ok(()) };
        let lookup = |code: &str, name: &str| {
            props.children_by("prop").find_map(|prop| {
                let key = prop.attr_str("name").or_else(|| prop.attr_str("config_code"))?;
                (key == code || key == name)
                    .then(|| prop.attr_str("value").or_else(|| prop.attr_str("config_value")))?
            })
        };
        let settings = &self.inner.server_props;
        if let Some(on) = flag(lookup("10518", "privacy_token_sending_on_all_1_on_1_messages")) {
            settings.privacy_token_on_direct.store(on, Ordering::Relaxed);
        }
        if let Some(on) = flag(lookup("9666", "profile_scraping_privacy_token_in_photo_iq")) {
            settings.privacy_token_on_profile_picture.store(on, Ordering::Relaxed);
        }
        if let Some(on) = flag(lookup("14303", "lid_trusted_token_issue_to_lid")) {
            settings.issue_tokens_to_lid.store(on, Ordering::Relaxed);
        }
        Ok(())
    }

    async fn tc_storage_jid(&self, jid: &Jid) -> Jid {
        let user = jid.to_non_device();
        if user.is_lid() {
            return user;
        }
        self.lid_for_phone(&user).await.ok().flatten().unwrap_or(user)
    }

    async fn load_tc_token(&self, storage: &Jid) -> Option<StoredTcToken> {
        let raw = self.inner.store.get(KeyKind::TcToken, &storage.to_string()).await.ok()??;
        StoredTcToken::decode(raw.as_slice()).ok()
    }

    async fn save_tc_token(&self, storage: &Jid, entry: &StoredTcToken) -> Result<()> {
        self.inner.store.set(KeyKind::TcToken, &storage.to_string(), entry.encode_to_vec()).await
    }

    async fn tc_token_for(&self, jid: &Jid) -> Option<Vec<u8>> {
        let entry = self.load_tc_token(&self.tc_storage_jid(jid).await).await?;
        (!entry.token.is_empty() && !expired(entry.timestamp)).then_some(entry.token)
    }

    pub(crate) async fn direct_tc_token_node(&self, chat: &Jid) -> Option<Node> {
        if !self.inner.server_props.privacy_token_on_direct.load(Ordering::Relaxed) {
            return None;
        }
        self.tc_token_for(chat).await.map(|token| Node::new("tctoken").bytes(token))
    }

    pub(crate) async fn profile_tc_token_node(&self, jid: &Jid) -> Option<Node> {
        if !self.inner.server_props.privacy_token_on_profile_picture.load(Ordering::Relaxed)
            || !jid.is_user()
            || self.is_me(jid)
        {
            return None;
        }
        self.tc_token_for(jid).await.map(|token| Node::new("tctoken").bytes(token))
    }

    pub(crate) async fn on_privacy_token_notification(&self, node: &Node) -> Result<()> {
        let (Some(tokens), Some(from)) = (node.child("tokens"), node.attr_jid("from")) else { return Ok(()) };
        let storage = match node.attr_jid("sender_lid").filter(Jid::is_lid) {
            Some(lid) => lid.to_non_device(),
            None => self.tc_storage_jid(&from).await,
        };
        self.store_tc_tokens(tokens, &storage).await
    }

    async fn store_tc_tokens(&self, tokens: &Node, storage: &Jid) -> Result<()> {
        if !is_regular_user(storage) {
            return Ok(());
        }
        for token in tokens.children_by("token").filter(|t| t.attr_is("type", "trusted_contact")) {
            let (Some(bytes), Some(incoming)) = (token.content_bytes(), token.attr_i64("t").filter(|t| *t > 0)) else {
                continue;
            };
            let mut entry = self.load_tc_token(storage).await.unwrap_or_default();
            if entry.timestamp > incoming {
                continue;
            }
            entry.token = bytes.to_vec();
            entry.timestamp = incoming;
            self.save_tc_token(storage, &entry).await?;
        }
        Ok(())
    }

    pub(crate) fn issue_tc_token_later(&self, chat: Jid) {
        let client = self.clone();
        tokio::spawn(async move {
            if let Err(err) = client.issue_tc_token(&chat).await {
                tracing::debug!(%chat, %err, "privacy token issuance failed");
            }
        });
    }

    async fn issue_tc_token(&self, chat: &Jid) -> Result<()> {
        let storage = self.tc_storage_jid(chat).await;
        if !is_regular_user(&storage) {
            return Ok(());
        }
        let current = self.load_tc_token(&storage).await.unwrap_or_default();
        if !needs_fresh_issue(current.sender_timestamp) || !self.inner.issuing_tokens.insert(storage.clone()) {
            return Ok(());
        }
        let target = if self.inner.server_props.issue_tokens_to_lid.load(Ordering::Relaxed) {
            storage.clone()
        } else if chat.is_lid() {
            self.phone_for_lid(chat).await.ok().flatten().unwrap_or_else(|| chat.to_non_device())
        } else {
            chat.to_non_device()
        };
        let issued_at = now_secs();
        let request =
            Node::new("iq").attr("to", Jid::server()).attr("type", "set").attr("xmlns", "privacy").child_node(
                Node::new("tokens").child_node(
                    Node::new("token").attr("jid", target).attr("t", issued_at).attr("type", "trusted_contact"),
                ),
            );
        let result = self.query(request).await;
        self.inner.issuing_tokens.remove(&storage);
        let response = result?;
        if let Some(tokens) = response.child("tokens") {
            self.store_tc_tokens(tokens, &storage).await?;
        }
        let mut entry = self.load_tc_token(&storage).await.unwrap_or_default();
        entry.sender_timestamp = Some(issued_at);
        self.save_tc_token(&storage, &entry).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_windows_follow_whatsapp_buckets() {
        assert!(!expired(now_secs()));
        assert!(!expired(now_secs() - 14 * 86_400));
        assert!(expired(now_secs() - 40 * 86_400));
        assert!(needs_fresh_issue(None));
        assert!(!needs_fresh_issue(Some(now_secs())));
        assert!(needs_fresh_issue(Some(now_secs() - 8 * 86_400)));
    }

    #[test]
    fn bots_and_psa_are_not_regular_users() {
        assert!(is_regular_user(&Jid::phone("5215512345678")));
        assert!(!is_regular_user(&Jid::phone("0")));
        assert!(!is_regular_user(&Jid::phone("13135550002")));
        assert!(!is_regular_user(&Jid::group("1203630")));
    }
}
