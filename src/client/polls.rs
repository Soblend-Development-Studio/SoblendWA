use prost::Message as _;
use serde::{Deserialize, Serialize};

use super::Client;
use crate::crypto;
use crate::error::{Error, Result};
use crate::events::{EventResponse, EventResponseKind, MessageInfo, PollVote};
use crate::jid::Jid;
use crate::message::now_millis;
use crate::proto::{
    EncEventResponseMessage, EventMessage, EventResponseMessage, Message, MessageKey, PollCreationMessage,
    PollEncValue, PollUpdateMessage, PollVoteMessage,
};
use crate::serde_b64;
use crate::store::KeyKind;

const POLL_VOTE: &[u8] = b"Poll Vote";
const EVENT_RESPONSE: &[u8] = b"Event Response";

#[derive(Serialize, Deserialize)]
struct AddonRecord {
    #[serde(with = "serde_b64::array")]
    secret: [u8; 32],
    options: Vec<String>,
    creators: Vec<Jid>,
    chat: Option<Jid>,
}

pub(crate) fn poll_of(message: &Message) -> Option<&PollCreationMessage> {
    let m = message.unwrapped();
    m.poll_creation_message
        .as_deref()
        .or(m.poll_creation_message_v2.as_deref())
        .or(m.poll_creation_message_v3.as_deref())
}

pub(crate) fn event_of(message: &Message) -> Option<&EventMessage> {
    message.unwrapped().event_message.as_deref()
}

pub(crate) fn has_addon_secret(message: &Message) -> bool {
    poll_of(message).is_some() || event_of(message).is_some()
}

fn secret_of(message: &Message) -> Option<[u8; 32]> {
    [message, message.unwrapped()]
        .into_iter()
        .find_map(|m| m.message_context_info.as_ref()?.message_secret.as_deref()?.try_into().ok())
}

fn addon_key(secret: &[u8; 32], id: &str, creator: &Jid, actor: &Jid, use_case: &[u8]) -> [u8; 32] {
    let base = crypto::hmac_sha256(&[0u8; 32], &[secret]);
    let creator = creator.to_string();
    let actor = actor.to_string();
    crypto::hmac_sha256(&base, &[id.as_bytes(), creator.as_bytes(), actor.as_bytes(), use_case, &[1]])
}

fn addon_aad(id: &str, actor: &Jid) -> Vec<u8> {
    format!("{id}\0{actor}").into_bytes()
}

fn unique(jids: impl IntoIterator<Item = Jid>) -> Vec<Jid> {
    let mut out: Vec<Jid> = Vec::new();
    for jid in jids.into_iter().map(|j| j.to_non_device()) {
        if !out.contains(&jid) {
            out.push(jid);
        }
    }
    out
}

impl Client {
    pub(crate) fn own_identities(&self) -> Vec<Jid> {
        unique([self.me(), self.lid()].into_iter().flatten())
    }

    pub(crate) async fn remember_poll(&self, id: &str, chat: &Jid, message: &Message, creators: Vec<Jid>) {
        let Some(secret) = secret_of(message).filter(|_| has_addon_secret(message)) else { return };
        let options = poll_of(message)
            .map(|poll| poll.options.iter().filter_map(|o| o.option_name.clone()).collect())
            .unwrap_or_default();
        let record = AddonRecord { secret, options, creators: unique(creators), chat: Some(chat.clone()) };
        match serde_json::to_vec(&record) {
            Ok(raw) => {
                if let Err(err) = self.inner.store.set(KeyKind::PollSecret, id, raw).await {
                    tracing::debug!(%err, "failed to persist an addon secret");
                }
            }
            Err(err) => tracing::debug!(%err, "failed to serialize an addon secret"),
        }
    }

    pub(crate) async fn remember_incoming_poll(&self, info: &MessageInfo, message: &Message) {
        if !has_addon_secret(message) {
            return;
        }
        let creators = if info.from_me {
            self.own_identities()
        } else {
            [Some(info.sender.clone()), info.sender_alt.clone()].into_iter().flatten().collect()
        };
        self.remember_poll(&info.id, &info.chat, message, creators).await;
    }

    async fn load_addon(&self, id: &str) -> Option<AddonRecord> {
        let raw = self.inner.store.get(KeyKind::PollSecret, id).await.ok()??;
        serde_json::from_slice(&raw).ok()
    }

    fn actors(&self, info: &MessageInfo) -> Vec<Jid> {
        let mut actors = vec![info.sender.clone()];
        actors.extend(info.sender_alt.clone());
        if info.from_me {
            actors.extend(self.own_identities());
        }
        unique(actors)
    }

    fn open_addon(
        record: &AddonRecord,
        id: &str,
        actors: &[Jid],
        value: (&[u8], &[u8]),
        use_case: &[u8],
    ) -> Option<(Jid, Vec<u8>)> {
        let (payload, iv) = value;
        actors.iter().find_map(|actor| {
            record.creators.iter().find_map(|creator| {
                let key = addon_key(&record.secret, id, creator, actor, use_case);
                crypto::aes_gcm_decrypt(&key, iv, &addon_aad(id, actor), payload)
                    .ok()
                    .map(|plain| (actor.clone(), plain))
            })
        })
    }

    pub(crate) async fn decode_poll_vote(&self, info: &MessageInfo, update: &PollUpdateMessage) -> Option<PollVote> {
        let poll_id = update.poll_creation_message_key.as_ref()?.id.clone()?;
        let record = self.load_addon(&poll_id).await?;
        let vote = update.vote.as_ref()?;
        let value = (vote.enc_payload.as_deref()?, vote.enc_iv.as_deref()?);
        let Some((voter, plain)) = Self::open_addon(&record, &poll_id, &self.actors(info), value, POLL_VOTE) else {
            tracing::debug!(%poll_id, "could not decrypt a poll vote with any known identity");
            return None;
        };
        let decoded = PollVoteMessage::decode(plain.as_slice()).ok()?;
        let selected = decoded
            .selected_options
            .iter()
            .map(|hash| {
                record
                    .options
                    .iter()
                    .find(|option| crypto::sha256(option.as_bytes()).as_slice() == hash.as_slice())
                    .cloned()
                    .unwrap_or_else(|| hex::encode(hash))
            })
            .collect();
        Some(PollVote { chat: info.chat.clone(), poll_id, voter, selected, timestamp: info.timestamp })
    }

    pub(crate) async fn decode_event_response(
        &self,
        info: &MessageInfo,
        response: &EncEventResponseMessage,
    ) -> Option<EventResponse> {
        let event_id = response.event_creation_message_key.as_ref()?.id.clone()?;
        let record = self.load_addon(&event_id).await?;
        let value = (response.enc_payload.as_deref()?, response.enc_iv.as_deref()?);
        let (responder, plain) = Self::open_addon(&record, &event_id, &self.actors(info), value, EVENT_RESPONSE)?;
        let decoded = EventResponseMessage::decode(plain.as_slice()).ok()?;
        let kind = match decoded.response.unwrap_or_default() {
            EventResponseMessage::GOING => EventResponseKind::Going,
            EventResponseMessage::NOT_GOING => EventResponseKind::NotGoing,
            EventResponseMessage::MAYBE => EventResponseKind::Maybe,
            _ => EventResponseKind::Unknown,
        };
        Some(EventResponse {
            chat: info.chat.clone(),
            event_id,
            responder,
            kind,
            extra_guests: decoded.extra_guest_count.unwrap_or_default().max(0) as u32,
            timestamp: info.timestamp,
        })
    }

    async fn seal_addon(
        &self,
        chat: &Jid,
        id: &str,
        use_case: &[u8],
        plain: &[u8],
    ) -> Result<(MessageKey, Vec<u8>, [u8; 12])> {
        let record = self.load_addon(id).await.ok_or_else(|| Error::other(format!("unknown poll or event {id}")))?;
        let creator = record.creators.first().cloned().ok_or_else(|| Error::other("missing creator"))?;
        let me = self.require_me()?;
        let actor = match (chat.is_lid() || creator.is_lid(), self.lid()) {
            (true, Some(lid)) => lid.to_non_device(),
            _ => me.to_non_device(),
        };
        let iv = crypto::random_array::<12>();
        let key = addon_key(&record.secret, id, &creator, &actor, use_case);
        let payload = crypto::aes_gcm_encrypt(&key, &iv, &addon_aad(id, &actor), plain)?;
        let from_me = self.own_identities().contains(&creator);
        let parent = MessageKey {
            remote_jid: Some(chat.to_string()),
            from_me: Some(from_me),
            id: Some(id.to_owned()),
            participant: (chat.is_group() && !from_me).then(|| creator.to_string()),
        };
        Ok((parent, payload, iv))
    }

    pub async fn vote_poll(&self, chat: &Jid, poll_id: &str, options: &[&str]) -> Result<String> {
        let selected = PollVoteMessage {
            selected_options: options.iter().map(|o| crypto::sha256(o.as_bytes()).to_vec()).collect(),
        };
        let (key, payload, iv) = self.seal_addon(chat, poll_id, POLL_VOTE, &selected.encode_to_vec()).await?;
        let update = PollUpdateMessage {
            poll_creation_message_key: Some(key),
            vote: Some(PollEncValue { enc_payload: Some(payload), enc_iv: Some(iv.to_vec()) }),
            sender_timestamp_ms: Some(now_millis()),
        };
        self.send_message(chat, Message { poll_update_message: Some(Box::new(update)), ..Default::default() }).await
    }

    pub async fn respond_event(
        &self,
        chat: &Jid,
        event_id: &str,
        going: EventResponseKind,
        extra_guests: u32,
    ) -> Result<String> {
        let response = EventResponseMessage {
            response: Some(match going {
                EventResponseKind::Going => EventResponseMessage::GOING,
                EventResponseKind::NotGoing => EventResponseMessage::NOT_GOING,
                EventResponseKind::Maybe | EventResponseKind::Unknown => EventResponseMessage::MAYBE,
            }),
            timestamp_ms: Some(now_millis()),
            extra_guest_count: Some(extra_guests as i32),
        };
        let (key, payload, iv) = self.seal_addon(chat, event_id, EVENT_RESPONSE, &response.encode_to_vec()).await?;
        let sealed = EncEventResponseMessage {
            event_creation_message_key: Some(key),
            enc_payload: Some(payload),
            enc_iv: Some(iv.to_vec()),
        };
        let message = Message { enc_event_response_message: Some(Box::new(sealed)), ..Default::default() };
        self.send_message(chat, message).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn addon_encryption_roundtrip() {
        let secret = crypto::random_array::<32>();
        let creator = Jid::phone("5215511111111");
        let voter = Jid::lid("998877");
        let record = AddonRecord { secret, options: vec!["Rust".into()], creators: vec![creator.clone()], chat: None };
        for use_case in [POLL_VOTE, EVENT_RESPONSE] {
            let key = addon_key(&secret, "3EB0ID", &creator, &voter, use_case);
            let iv = crypto::random_array::<12>();
            let sealed = crypto::aes_gcm_encrypt(&key, &iv, &addon_aad("3EB0ID", &voter), b"payload").unwrap();
            let (actor, plain) =
                Client::open_addon(&record, "3EB0ID", &[Jid::phone("1"), voter.clone()], (&sealed, &iv), use_case)
                    .unwrap();
            assert_eq!(actor, voter);
            assert_eq!(plain, b"payload");
        }
        let key = addon_key(&secret, "3EB0ID", &creator, &voter, POLL_VOTE);
        let iv = [0u8; 12];
        let sealed = crypto::aes_gcm_encrypt(&key, &iv, &addon_aad("3EB0ID", &voter), b"x").unwrap();
        assert!(Client::open_addon(&record, "3EB0ID", &[voter], (&sealed, &iv), EVENT_RESPONSE).is_none());
    }
}
