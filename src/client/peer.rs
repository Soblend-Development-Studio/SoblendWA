use std::time::Instant;

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use prost::Message as _;

use super::Client;
use crate::binary::Node;
use crate::error::Result;
use crate::events::{Event, MessageEvent, MessageInfo};
use crate::jid::Jid;
use crate::message::{generate_message_id, now_millis, pad};
use crate::proto::{
    AppStateSyncKeyId, AppStateSyncKeyRequest, HistorySyncOnDemandRequest, Message, MessageKey,
    PeerDataOperationRequestMessage, PeerDataOperationRequestResponseMessage, PlaceholderMessageResendRequest,
    ProtocolMessage, WebMessageInfo,
};
use crate::signal::EncType;

impl Client {
    pub(crate) async fn send_peer_message(&self, message: Message) -> Result<String> {
        let me = self.require_me()?;
        let phone = me.to_non_device();
        self.ensure_sessions(std::slice::from_ref(&phone)).await?;
        let id = generate_message_id(Some(&me));
        let (kind, ciphertext) = self.signal().encrypt(&phone.signal_address(), &pad(message.encode_to_vec())).await?;
        let mut content = vec![Node::new("enc").attr("v", "2").attr("type", kind.as_str()).bytes(ciphertext)];
        if kind == EncType::PreKey {
            if let Some(account) = self.creds().account {
                content.push(Node::new("device-identity").bytes(account));
            }
        }
        content.push(Node::new("meta").attr("appdata", "default"));
        let stanza = Node::new("message")
            .attr("id", id.clone())
            .attr("to", phone)
            .attr("type", "text")
            .attr("category", "peer")
            .attr("push_priority", "high_force")
            .children(content);
        self.send_node(&stanza)?;
        Ok(id)
    }

    pub(crate) async fn request_app_state_keys(&self, key_ids: Vec<String>) {
        let fresh: Vec<Vec<u8>> = key_ids
            .into_iter()
            .filter(|id| self.inner.requested_keys.insert(id.clone()))
            .filter_map(|id| STANDARD.decode(id).ok())
            .collect();
        if fresh.is_empty() {
            return;
        }
        let request = Message {
            protocol_message: Some(Box::new(ProtocolMessage {
                r#type: Some(ProtocolMessage::APP_STATE_SYNC_KEY_REQUEST),
                app_state_sync_key_request: Some(AppStateSyncKeyRequest {
                    key_ids: fresh.into_iter().map(|key_id| AppStateSyncKeyId { key_id: Some(key_id) }).collect(),
                }),
                ..Default::default()
            })),
            ..Default::default()
        };
        if let Err(err) = self.send_peer_message(request).await {
            tracing::warn!(%err, "could not ask the phone for app state keys");
        }
    }

    fn peer_data_request(request: PeerDataOperationRequestMessage) -> Message {
        Message {
            protocol_message: Some(Box::new(ProtocolMessage {
                r#type: Some(ProtocolMessage::PEER_DATA_OPERATION_REQUEST),
                peer_data_operation_request_message: Some(request),
                ..Default::default()
            })),
            ..Default::default()
        }
    }

    pub async fn request_placeholder_resend(&self, keys: Vec<MessageKey>) -> Result<String> {
        let request = PeerDataOperationRequestMessage {
            request_type: Some(PeerDataOperationRequestMessage::PLACEHOLDER_MESSAGE_RESEND),
            history_sync_on_demand_request: None,
            placeholder_message_resend_request: keys
                .into_iter()
                .map(|key| PlaceholderMessageResendRequest { message_key: Some(key) })
                .collect(),
        };
        self.send_peer_message(Self::peer_data_request(request)).await
    }

    pub async fn request_history(
        &self,
        chat: &Jid,
        oldest: &MessageKey,
        oldest_timestamp_ms: i64,
        count: i32,
    ) -> Result<String> {
        let request = PeerDataOperationRequestMessage {
            request_type: Some(PeerDataOperationRequestMessage::HISTORY_SYNC_ON_DEMAND),
            history_sync_on_demand_request: Some(HistorySyncOnDemandRequest {
                chat_jid: Some(chat.to_non_device().to_string()),
                oldest_msg_id: oldest.id.clone(),
                oldest_msg_from_me: oldest.from_me,
                on_demand_msg_count: Some(count.clamp(1, 50)),
                oldest_msg_timestamp_ms: Some(if oldest_timestamp_ms > 0 { oldest_timestamp_ms } else { now_millis() }),
                account_lid: self.lid().map(|lid| lid.to_non_device().to_string()),
            }),
            placeholder_message_resend_request: Vec::new(),
        };
        self.send_peer_message(Self::peer_data_request(request)).await
    }

    pub(crate) fn process_peer_response(&self, response: &PeerDataOperationRequestResponseMessage) {
        for result in &response.results {
            let Some(bytes) =
                result.placeholder_message_resend_response.as_ref().and_then(|r| r.web_message_info_bytes.as_deref())
            else {
                continue;
            };
            match WebMessageInfo::decode(bytes).ok().and_then(|info| self.message_event_from_web(info)) {
                Some(event) => self.emit(Event::Message(Box::new(event))),
                None => tracing::debug!("phone answered a resend request with an unreadable message"),
            }
        }
    }

    pub(crate) fn message_event_from_web(&self, info: WebMessageInfo) -> Option<MessageEvent> {
        let key = info.key?;
        let chat: Jid = key.remote_jid.as_deref()?.parse().ok()?;
        let from_me = key.from_me.unwrap_or(false);
        let sender = match (from_me, key.participant.as_deref().or(info.participant.as_deref())) {
            (true, _) => self.me()?,
            (false, Some(participant)) => participant.parse().ok()?,
            (false, None) => chat.clone(),
        };
        let raw = *info.message?;
        Some(MessageEvent {
            message: raw.unwrapped().clone(),
            info: MessageInfo {
                id: key.id?,
                is_group: chat.is_group(),
                chat,
                sender,
                sender_alt: None,
                sender_phone: None,
                sender_lid: None,
                from_me,
                push_name: info.push_name,
                timestamp: info.message_timestamp.unwrap_or_default() as i64,
                category: None,
                offline: true,
                edit: None,
                server_id: None,
                received_at: Instant::now(),
            },
            raw,
        })
    }
}
