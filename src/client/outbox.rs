use std::time::Instant;

use prost::Message as _;

use super::guard::{Counters, SendTimings};
use super::{Client, SendOptions, SentMessage};
use crate::jid::Jid;
use crate::message::now_secs;
use crate::proto::Message;
use crate::store::KeyKind;

const KEEP_FOR_SECS: i64 = 24 * 60 * 60;
const MAX_ATTEMPTS: u32 = 5;
const SECONDS_PER_DAY: i64 = 24 * 60 * 60;

#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct OutboxEntry {
    #[prost(string, tag = "1")]
    pub chat: String,
    #[prost(bytes = "vec", tag = "2")]
    pub message: Vec<u8>,
    #[prost(int64, tag = "3")]
    pub created_at: i64,
    #[prost(uint32, tag = "4")]
    pub attempts: u32,
    #[prost(string, optional, tag = "5")]
    pub edit: Option<String>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct RecentSend {
    #[prost(string, tag = "1")]
    pub chat: String,
    #[prost(bytes = "vec", tag = "2")]
    pub message: Vec<u8>,
    #[prost(string, optional, tag = "3")]
    pub sender_address: Option<String>,
}

fn day_of(seconds: i64) -> i64 {
    seconds / SECONDS_PER_DAY
}

fn edit_kind(raw: Option<&str>) -> Option<&'static str> {
    match raw? {
        "1" => Some("1"),
        "2" => Some("2"),
        "7" => Some("7"),
        "8" => Some("8"),
        _ => None,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingSend {
    pub id: String,
    pub chat: Jid,
    pub queued_at: i64,
    pub attempts: u32,
}

impl Client {
    pub(crate) fn outbox_enabled(&self, chat: &Jid) -> bool {
        self.inner.config.outbox && !chat.is_status_broadcast()
    }

    pub(crate) async fn park(&self, id: &str, chat: &Jid, message: &Message, edit: Option<&'static str>) {
        let entry = OutboxEntry {
            chat: chat.to_string(),
            message: message.encode_to_vec(),
            created_at: now_secs(),
            attempts: 0,
            edit: edit.map(str::to_owned),
        };
        if let Err(err) = self.inner.store.set(KeyKind::Outbox, id, entry.encode_to_vec()).await {
            tracing::warn!(%err, id, "could not keep the message in the outbox");
        }
    }

    pub(crate) async fn unpark(&self, id: &str) {
        if self.inner.config.outbox {
            let _ = self.inner.store.remove(KeyKind::Outbox, id).await;
        }
    }

    pub async fn pending_sends(&self) -> crate::Result<Vec<PendingSend>> {
        let mut pending = Vec::new();
        for id in self.inner.store.keys(KeyKind::Outbox).await? {
            let Some(raw) = self.inner.store.get(KeyKind::Outbox, &id).await? else { continue };
            let Ok(entry) = OutboxEntry::decode(raw.as_slice()) else { continue };
            let Ok(chat) = entry.chat.parse() else { continue };
            pending.push(PendingSend { id, chat, queued_at: entry.created_at, attempts: entry.attempts });
        }
        pending.sort_by_key(|p| p.queued_at);
        Ok(pending)
    }

    pub(crate) async fn keep_for_retries(&self, id: &str, chat: &Jid, message: &Message, sender_address: Option<&str>) {
        let entry = RecentSend {
            chat: chat.to_string(),
            message: message.encode_to_vec(),
            sender_address: sender_address.map(str::to_owned),
        };
        let key = format!("{}:{id}", day_of(now_secs()));
        if let Err(err) = self.inner.store.set(KeyKind::RecentSend, &key, entry.encode_to_vec()).await {
            tracing::debug!(%err, id, "could not keep the message for retries");
        }
    }

    pub(crate) async fn stored_send(&self, id: &str) -> Option<SentMessage> {
        let today = day_of(now_secs());
        for day in [today, today - 1] {
            let key = format!("{day}:{id}");
            let Ok(Some(raw)) = self.inner.store.get(KeyKind::RecentSend, &key).await else { continue };
            let entry = RecentSend::decode(raw.as_slice()).ok()?;
            return Some(SentMessage {
                chat: entry.chat.parse().ok()?,
                message: Message::decode(entry.message.as_slice()).ok()?,
                sender_address: entry.sender_address,
                sent_at: Instant::now(),
                timings: SendTimings::default(),
            });
        }
        None
    }

    async fn prune_recent_sends(&self) {
        let Ok(keys) = self.inner.store.keys(KeyKind::RecentSend).await else { return };
        let oldest = day_of(now_secs()) - 1;
        for key in keys {
            let expired = key.split_once(':').and_then(|(day, _)| day.parse::<i64>().ok()).is_none_or(|d| d < oldest);
            if expired {
                let _ = self.inner.store.remove(KeyKind::RecentSend, &key).await;
            }
        }
    }

    pub(crate) async fn flush_outbox(&self, generation: u64) {
        if !self.inner.config.outbox {
            return;
        }
        self.prune_recent_sends().await;
        let ids = match self.inner.store.keys(KeyKind::Outbox).await {
            Ok(ids) => ids,
            Err(err) => {
                tracing::debug!(%err, "the store cannot list the outbox; pending sends are kept for later");
                return;
            }
        };
        let mut entries = Vec::new();
        for id in ids.into_iter().filter(|id| !self.inner.in_flight.contains(id)) {
            let Ok(Some(raw)) = self.inner.store.get(KeyKind::Outbox, &id).await else { continue };
            match OutboxEntry::decode(raw.as_slice()) {
                Ok(entry) => entries.push((id, entry)),
                Err(_) => {
                    let _ = self.inner.store.remove(KeyKind::Outbox, &id).await;
                }
            }
        }
        entries.sort_by_key(|(_, entry)| entry.created_at);
        tracing::debug!(pending = entries.len(), "flushing the outbox");
        for (id, mut entry) in entries {
            if self.current_generation() != generation {
                tracing::debug!("the connection changed; its own flush takes over");
                return;
            }
            if self.inner.in_flight.contains(&id) {
                continue;
            }
            let expired = now_secs() - entry.created_at > KEEP_FOR_SECS || entry.attempts >= MAX_ATTEMPTS;
            let parsed = entry.chat.parse::<Jid>().ok().zip(Message::decode(entry.message.as_slice()).ok());
            let (false, Some((chat, message))) = (expired, parsed) else {
                tracing::warn!(id, attempts = entry.attempts, "dropping a pending send that could not be delivered");
                let _ = self.inner.store.remove(KeyKind::Outbox, &id).await;
                continue;
            };
            entry.attempts += 1;
            let _ = self.inner.store.set(KeyKind::Outbox, &id, entry.encode_to_vec()).await;
            let options =
                SendOptions { id: Some(id.clone()), edit: edit_kind(entry.edit.as_deref()), ..Default::default() };
            match self.resend(&chat, message, options).await {
                Ok(_) => Counters::bump(&self.inner.counters.outbox_resent),
                Err(err) => tracing::warn!(%err, id, "a pending send failed again; it stays in the outbox"),
            }
        }
    }
}
