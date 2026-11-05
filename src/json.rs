use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use prost::Message as _;
use serde_json::{json, Map, Value};

use crate::binary::{Content, Node};
use crate::events::{
    CallKind, ChatStateKind, ChatUpdate, DisconnectReason, Event, EventResponseKind, GroupAction, MessageEvent,
    MessageInfo, ReceiptType,
};
use crate::interactive::ReplySource;
use crate::jid::Jid;
use crate::proto;

fn jid(value: &Jid) -> Value {
    Value::String(value.to_string())
}

fn jids(values: &[Jid]) -> Value {
    Value::Array(values.iter().map(jid).collect())
}

fn opt_jid(value: &Option<Jid>) -> Value {
    value.as_ref().map_or(Value::Null, jid)
}

pub fn node_to_json(node: &Node) -> Value {
    let attrs: Map<String, Value> =
        node.attrs.iter().map(|(k, v)| (k.to_string(), Value::String(v.to_string()))).collect();
    let content = match &node.content {
        Content::None => Value::Null,
        Content::Text(text) => Value::String(text.to_string()),
        Content::Bytes(bytes) => match std::str::from_utf8(bytes) {
            Ok(text) if !text.chars().any(char::is_control) => Value::String(text.to_owned()),
            _ => json!({ "base64": STANDARD.encode(bytes) }),
        },
        Content::Nodes(children) => Value::Array(children.iter().map(node_to_json).collect()),
    };
    json!({ "tag": node.tag.to_string(), "attrs": attrs, "content": content })
}

impl MessageInfo {
    pub fn to_json(&self) -> Value {
        json!({
            "id": self.id,
            "chat": jid(&self.chat),
            "sender": jid(&self.sender),
            "senderAlt": opt_jid(&self.sender_alt),
            "senderPhone": opt_jid(&self.sender_phone),
            "senderLid": opt_jid(&self.sender_lid),
            "fromMe": self.from_me,
            "isGroup": self.is_group,
            "pushName": self.push_name,
            "timestamp": self.timestamp,
            "category": self.category,
            "offline": self.offline,
            "edit": self.edit,
            "serverId": self.server_id,
            "elapsedMicros": self.elapsed().as_micros() as u64,
        })
    }
}

pub fn media_json(message: &proto::Message) -> Value {
    let m = message.unwrapped();
    let view_once = message.is_view_once();
    if let Some(x) = &m.image_message {
        json!({ "type": "image", "mimetype": x.mimetype, "size": x.file_length, "caption": x.caption,
            "width": x.width, "height": x.height, "viewOnce": view_once || x.view_once == Some(true) })
    } else if let Some(x) = &m.video_message {
        let kind = if x.gif_playback == Some(true) { "gif" } else { "video" };
        json!({ "type": kind, "mimetype": x.mimetype, "size": x.file_length, "caption": x.caption,
            "seconds": x.seconds, "width": x.width, "height": x.height, "viewOnce": view_once || x.view_once == Some(true) })
    } else if let Some(x) = &m.ptv_message {
        json!({ "type": "ptv", "mimetype": x.mimetype, "size": x.file_length, "seconds": x.seconds,
            "width": x.width, "height": x.height, "viewOnce": view_once })
    } else if let Some(x) = &m.audio_message {
        let kind = if x.ptt == Some(true) { "ptt" } else { "audio" };
        json!({ "type": kind, "mimetype": x.mimetype, "size": x.file_length, "seconds": x.seconds,
            "viewOnce": view_once || x.view_once == Some(true) })
    } else if let Some(x) = &m.document_message {
        json!({ "type": "document", "mimetype": x.mimetype, "size": x.file_length, "caption": x.caption,
            "fileName": x.file_name, "title": x.title, "pages": x.page_count, "viewOnce": view_once })
    } else if let Some(x) = &m.sticker_message {
        json!({ "type": "sticker", "mimetype": x.mimetype, "size": x.file_length, "width": x.width,
            "height": x.height, "animated": x.is_animated == Some(true), "viewOnce": view_once })
    } else {
        Value::Null
    }
}

fn link_preview_json(message: &proto::Message) -> Value {
    let Some(text) = &message.unwrapped().extended_text_message else { return Value::Null };
    if text.matched_text.is_none() && text.title.is_none() {
        return Value::Null;
    }
    json!({
        "url": text.matched_text,
        "title": text.title,
        "description": text.description,
        "video": text.preview_type == Some(1),
        "hasThumbnail": text.jpeg_thumbnail.is_some(),
        "hasHighQualityThumbnail": text.thumbnail_direct_path.is_some(),
        "thumbnailWidth": text.thumbnail_width,
        "thumbnailHeight": text.thumbnail_height,
    })
}

fn quoted_json(message: &proto::Message) -> Value {
    let Some(context) = message.context_info() else { return Value::Null };
    let Some(id) = &context.stanza_id else { return Value::Null };
    let quoted = context.quoted_message.as_deref();
    json!({
        "id": id,
        "sender": context.participant,
        "chat": context.remote_jid,
        "text": quoted.and_then(|q| q.get_text()),
        "mediaType": quoted.and_then(|q| q.media_type()),
        "media": quoted.map_or(Value::Null, media_json),
    })
}

impl MessageEvent {
    pub fn to_json(&self) -> Value {
        let context = self.message.context_info();
        let age = crate::message::now_secs().saturating_sub(self.info.timestamp).max(0);
        json!({
            "info": self.info.to_json(),
            "kind": self.message.stanza_type(),
            "mediaType": self.message.media_type(),
            "text": self.text(),
            "media": media_json(&self.message),
            "quoted": quoted_json(&self.message),
            "viewOnce": self.message.is_view_once(),
            "linkPreview": link_preview_json(&self.message),
            "ageSecs": age,
            "quotedId": context.and_then(|c| c.stanza_id.clone()),
            "mentions": jids(&self.mentions()),
            "reaction": self.message.reaction_message.as_ref().map(|r| json!({
                "targetId": r.key.as_ref().and_then(|k| k.id.clone()),
                "emoji": r.text,
            })),
            "reply": self.message.interactive_reply().map(|reply| json!({
                "id": reply.id,
                "text": reply.text,
                "source": match reply.source {
                    ReplySource::Buttons => "buttons",
                    ReplySource::List => "list",
                    ReplySource::Template => "template",
                    ReplySource::NativeFlow => "nativeFlow",
                },
                "flow": reply.flow,
                "params": reply.params,
            })),
            "proto": STANDARD.encode(self.raw.encode_to_vec()),
        })
    }
}

fn micros(duration: std::time::Duration) -> u64 {
    duration.as_micros() as u64
}

pub(crate) fn timings_json(timings: &crate::client::SendTimings) -> Value {
    json!({
        "queuedMicros": micros(timings.queued),
        "uploadMicros": timings.upload.map(micros),
        "prepareMicros": micros(timings.prepare),
        "encryptMicros": micros(timings.encrypt),
        "serverMicros": timings.server.map(micros),
        "totalMicros": micros(timings.total()),
    })
}

fn disconnect_reason(reason: &DisconnectReason) -> Value {
    match reason {
        DisconnectReason::ConnectionLost(why) => json!({ "type": "connectionLost", "detail": why }),
        DisconnectReason::RestartRequired => json!({ "type": "restartRequired" }),
        DisconnectReason::Replaced => json!({ "type": "replaced" }),
        DisconnectReason::QrTimeout => json!({ "type": "qrTimeout" }),
        DisconnectReason::ClientOutdated => json!({ "type": "clientOutdated" }),
        DisconnectReason::AccountBlocked(code) => json!({ "type": "accountBlocked", "code": code }),
        DisconnectReason::Manual => json!({ "type": "manual" }),
    }
}

fn receipt_kind(kind: &ReceiptType) -> String {
    match kind {
        ReceiptType::Delivered => "delivered".into(),
        ReceiptType::Sender => "sender".into(),
        ReceiptType::Read => "read".into(),
        ReceiptType::ReadSelf => "read-self".into(),
        ReceiptType::Played => "played".into(),
        ReceiptType::PlayedSelf => "played-self".into(),
        ReceiptType::Retry => "retry".into(),
        ReceiptType::Inactive => "inactive".into(),
        ReceiptType::PeerMessage => "peer_msg".into(),
        ReceiptType::HistorySync => "hist_sync".into(),
        ReceiptType::ServerError => "server-error".into(),
        ReceiptType::Other(other) => other.clone(),
    }
}

fn group_action(action: &GroupAction) -> Value {
    match action {
        GroupAction::Created => json!({ "type": "created" }),
        GroupAction::Add => json!({ "type": "add" }),
        GroupAction::Remove => json!({ "type": "remove" }),
        GroupAction::Promote => json!({ "type": "promote" }),
        GroupAction::Demote => json!({ "type": "demote" }),
        GroupAction::Leave => json!({ "type": "leave" }),
        GroupAction::Subject(subject) => json!({ "type": "subject", "subject": subject }),
        GroupAction::Description(text) => json!({ "type": "description", "description": text }),
        GroupAction::Announce(on) => json!({ "type": "announce", "enabled": on }),
        GroupAction::Locked(on) => json!({ "type": "locked", "enabled": on }),
        GroupAction::Ephemeral(seconds) => json!({ "type": "ephemeral", "seconds": seconds }),
        GroupAction::Other(name) => json!({ "type": "other", "name": name }),
    }
}

fn chat_update(update: &ChatUpdate) -> Value {
    match update {
        ChatUpdate::Muted { chat, until } => json!({ "type": "muted", "chat": jid(chat), "until": until }),
        ChatUpdate::Archived { chat, archived } => {
            json!({ "type": "archived", "chat": jid(chat), "archived": archived })
        }
        ChatUpdate::Pinned { chat, pinned } => json!({ "type": "pinned", "chat": jid(chat), "pinned": pinned }),
        ChatUpdate::MarkedRead { chat, read } => json!({ "type": "markedRead", "chat": jid(chat), "read": read }),
        ChatUpdate::Locked { chat, locked } => json!({ "type": "locked", "chat": jid(chat), "locked": locked }),
        ChatUpdate::Deleted { chat } => json!({ "type": "deleted", "chat": jid(chat) }),
        ChatUpdate::Cleared { chat } => json!({ "type": "cleared", "chat": jid(chat) }),
        ChatUpdate::Starred { chat, message_id, from_me, starred } => json!({
            "type": "starred", "chat": jid(chat), "messageId": message_id, "fromMe": from_me, "starred": starred,
        }),
        ChatUpdate::DeletedForMe { chat, message_id, from_me } => json!({
            "type": "deletedForMe", "chat": jid(chat), "messageId": message_id, "fromMe": from_me,
        }),
        ChatUpdate::Contact { jid: who, full_name, first_name, lid, phone } => json!({
            "type": "contact", "jid": jid(who), "fullName": full_name, "firstName": first_name,
            "lid": opt_jid(lid), "phone": opt_jid(phone),
        }),
        ChatUpdate::ContactRemoved { jid: who } => json!({ "type": "contactRemoved", "jid": jid(who) }),
        ChatUpdate::PushName { name } => json!({ "type": "pushName", "name": name }),
        ChatUpdate::Label { id, name, color, deleted } => json!({
            "type": "label", "id": id, "name": name, "color": color, "deleted": deleted,
        }),
        ChatUpdate::ChatLabel { label, chat, labeled } => json!({
            "type": "chatLabel", "label": label, "chat": jid(chat), "labeled": labeled,
        }),
        ChatUpdate::MessageLabel { label, chat, message_id, labeled } => json!({
            "type": "messageLabel", "label": label, "chat": jid(chat), "messageId": message_id, "labeled": labeled,
        }),
        ChatUpdate::Other { index } => json!({ "type": "other", "index": index }),
    }
}

impl Event {
    pub fn kind(&self) -> &'static str {
        match self {
            Event::Qr(_) => "qr",
            Event::PairSuccess { .. } => "pairSuccess",
            Event::Connected => "connected",
            Event::Disconnected { .. } => "disconnected",
            Event::LoggedOut { .. } => "loggedOut",
            Event::AccountBlocked { .. } => "accountBlocked",
            Event::OfflineSyncComplete { .. } => "offlineSyncComplete",
            Event::Message(_) => "message",
            Event::UndecryptableMessage { .. } => "undecryptableMessage",
            Event::MessageRecovered { .. } => "messageRecovered",
            Event::PictureChanged { .. } => "pictureChanged",
            Event::AboutChanged { .. } => "aboutChanged",
            Event::BlocklistChanged { .. } => "blocklistChanged",
            Event::PrivacyChanged { .. } => "privacyChanged",
            Event::Receipt(_) => "receipt",
            Event::Presence(_) => "presence",
            Event::ChatState(_) => "chatState",
            Event::GroupUpdate(_) => "groupUpdate",
            Event::Call(_) => "call",
            Event::PollVote(_) => "pollVote",
            Event::EventResponse(_) => "eventResponse",
            Event::HistorySync(_) => "historySync",
            Event::ServerAck(_) => "serverAck",
            Event::ChatUpdate { .. } => "chatUpdate",
            Event::Notification(_) => "notification",
            Event::Unhandled(_) => "unhandled",
        }
    }

    pub fn to_json(&self) -> Value {
        let body = match self {
            Event::Qr(code) => json!({ "code": code }),
            Event::PairSuccess { id, lid, platform, business_name } => json!({
                "id": jid(id), "lid": opt_jid(lid), "platform": platform, "businessName": business_name,
            }),
            Event::Connected => json!({}),
            Event::Disconnected { reason, reconnecting } => {
                json!({ "reason": disconnect_reason(reason), "reconnecting": reconnecting })
            }
            Event::LoggedOut { reason } => json!({ "reason": reason }),
            Event::AccountBlocked { code, reason, expires } => {
                json!({ "code": code, "reason": reason, "expires": expires })
            }
            Event::OfflineSyncComplete { count } => json!({ "count": count }),
            Event::Message(message) => message.to_json(),
            Event::UndecryptableMessage { info, error } => json!({ "info": info.to_json(), "error": error }),
            Event::MessageRecovered { id, chat, sender } => {
                json!({ "id": id, "chat": jid(chat), "sender": jid(sender) })
            }
            Event::PictureChanged { jid: who, author, picture_id, removed, timestamp } => json!({
                "jid": jid(who), "author": opt_jid(author), "pictureId": picture_id,
                "removed": removed, "timestamp": timestamp,
            }),
            Event::AboutChanged { jid: who, about, timestamp } => {
                json!({ "jid": jid(who), "about": about, "timestamp": timestamp })
            }
            Event::BlocklistChanged { items } => json!({
                "items": items.iter().map(|item| json!({
                    "jid": jid(&item.jid), "phone": opt_jid(&item.phone), "blocked": item.blocked,
                })).collect::<Vec<_>>(),
            }),
            Event::PrivacyChanged { settings } => json!({
                "settings": settings.iter().map(|(name, value)| json!({ "name": name, "value": value })).collect::<Vec<_>>(),
            }),
            Event::Receipt(receipt) => json!({
                "chat": jid(&receipt.chat), "sender": jid(&receipt.sender), "ids": receipt.ids,
                "receipt": receipt_kind(&receipt.kind), "timestamp": receipt.timestamp, "fromMe": receipt.from_me,
            }),
            Event::Presence(presence) => json!({
                "from": jid(&presence.from), "available": presence.available, "lastSeen": presence.last_seen,
            }),
            Event::ChatState(state) => json!({
                "chat": jid(&state.chat),
                "sender": jid(&state.sender),
                "state": match state.state {
                    ChatStateKind::Composing => "composing",
                    ChatStateKind::Recording => "recording",
                    ChatStateKind::Paused => "paused",
                },
            }),
            Event::GroupUpdate(update) => json!({
                "group": jid(&update.group), "author": opt_jid(&update.author), "action": group_action(&update.action),
                "participants": jids(&update.participants), "timestamp": update.timestamp,
            }),
            Event::Call(call) => json!({
                "id": call.id,
                "from": jid(&call.from),
                "call": match call.kind {
                    CallKind::Offer => "offer",
                    CallKind::Accept => "accept",
                    CallKind::Reject => "reject",
                    CallKind::Terminate => "terminate",
                },
                "video": call.video,
                "isGroup": call.is_group,
                "timestamp": call.timestamp,
            }),
            Event::PollVote(vote) => json!({
                "chat": jid(&vote.chat), "pollId": vote.poll_id, "voter": jid(&vote.voter),
                "selected": vote.selected, "timestamp": vote.timestamp,
            }),
            Event::EventResponse(response) => json!({
                "chat": jid(&response.chat),
                "eventId": response.event_id,
                "responder": jid(&response.responder),
                "response": match response.kind {
                    EventResponseKind::Going => "going",
                    EventResponseKind::NotGoing => "notGoing",
                    EventResponseKind::Maybe => "maybe",
                    EventResponseKind::Unknown => "unknown",
                },
                "extraGuests": response.extra_guests,
                "timestamp": response.timestamp,
            }),
            Event::HistorySync(history) => json!({
                "syncType": history.sync_type,
                "progress": history.progress,
                "chunkOrder": history.chunk_order,
                "conversations": history.conversations.iter().map(|c| json!({
                    "id": c.id, "name": c.name, "unreadCount": c.unread_count, "messages": c.messages.len(),
                    "archived": c.archived, "pinned": c.pinned, "timestamp": c.conversation_timestamp,
                })).collect::<Vec<_>>(),
                "pushNames": history.pushnames.iter().map(|p| json!({ "id": p.id, "name": p.pushname })).collect::<Vec<_>>(),
                "lidMappings": history.phone_number_to_lid_mappings.iter()
                    .map(|m| json!({ "pn": m.pn_jid, "lid": m.lid_jid })).collect::<Vec<_>>(),
            }),
            Event::ServerAck(ack) => json!({
                "id": ack.id, "chat": jid(&ack.chat), "error": ack.error,
                "latencyMicros": ack.latency.map(|d| d.as_micros() as u64),
                "timings": ack.timings.map(|t| timings_json(&t)),
            }),
            Event::ChatUpdate { update, timestamp, initial_sync } => json!({
                "update": chat_update(update), "timestamp": timestamp, "initialSync": initial_sync,
            }),
            Event::Notification(node) | Event::Unhandled(node) => json!({ "node": node_to_json(node) }),
        };
        let mut out = Map::new();
        out.insert("type".into(), Value::String(self.kind().into()));
        match body {
            Value::Object(fields) => out.extend(fields),
            other => drop(out.insert("data".into(), other)),
        }
        Value::Object(out)
    }
}

impl crate::client::Stats {
    pub fn to_json(&self) -> Value {
        json!({
            "connected": self.connected,
            "uptimeSecs": self.uptime.as_secs(),
            "received": self.received,
            "sent": self.sent,
            "undecryptable": self.undecryptable,
            "recovered": self.recovered,
            "duplicates": self.duplicates,
            "awaitingRecovery": self.awaiting_recovery,
            "reconnects": self.reconnects,
            "rateLimited": self.rate_limited,
            "mediaUploaded": self.media_uploaded,
            "mediaReused": self.media_reused,
            "outboxResent": self.outbox_resent,
            "panics": self.panics,
            "cachedDevices": self.cached_devices,
            "cachedGroups": self.cached_groups,
            "pendingQueries": self.pending_queries,
            "openLanes": self.open_lanes,
            "latencyMicros": self.latency.map(micros),
        })
    }
}
#[cfg(test)]
mod tests {
    use std::time::Instant;

    use super::*;
    use crate::proto::Message;

    #[test]
    fn messages_become_flat_json() {
        let info = MessageInfo {
            id: "3EB0ABC".into(),
            chat: Jid::group("120363000000000001"),
            sender: Jid::phone("5215512345678"),
            sender_alt: None,
            sender_phone: None,
            sender_lid: None,
            from_me: false,
            is_group: true,
            push_name: Some("Ana".into()),
            timestamp: 1_700_000_000,
            category: None,
            offline: false,
            edit: None,
            server_id: None,
            received_at: Instant::now(),
        };
        let message = Message::text("!ping");
        let event = Event::Message(Box::new(MessageEvent { info, raw: message.clone(), message }));
        let value = event.to_json();
        assert_eq!(value["type"], "message");
        assert_eq!(value["text"], "!ping");
        assert_eq!(value["info"]["chat"], "120363000000000001@g.us");
        let proto = STANDARD.decode(value["proto"].as_str().unwrap()).unwrap();
        assert_eq!(Message::decode(proto.as_slice()).unwrap().get_text(), Some("!ping"));
    }

    #[test]
    fn wrapped_media_and_quotes_come_out_flat() {
        use crate::proto::{ContextInfo, FutureProofMessage, ImageMessage};
        let quoted = Message {
            image_message: Some(Box::new(ImageMessage {
                mimetype: Some("image/jpeg".into()),
                caption: Some("la waifu de ayer".into()),
                ..Default::default()
            })),
            ..Default::default()
        };
        let image = ImageMessage {
            mimetype: Some("image/webp".into()),
            caption: Some("!s".into()),
            file_length: Some(2048),
            width: Some(512),
            height: Some(512),
            context_info: Some(ContextInfo {
                stanza_id: Some("3EB0QUOTED".into()),
                participant: Some("5215522222222@s.whatsapp.net".into()),
                quoted_message: Some(Box::new(quoted)),
                ..Default::default()
            }),
            ..Default::default()
        };
        let wrap = |message: Message| FutureProofMessage { message: Some(Box::new(message)) };
        let view_once = Message {
            view_once_message_v2: Some(Box::new(wrap(Message {
                image_message: Some(Box::new(image)),
                ..Default::default()
            }))),
            ..Default::default()
        };
        let message = Message { ephemeral_message: Some(Box::new(wrap(view_once))), ..Default::default() };

        let media = media_json(&message);
        assert_eq!((media["type"].as_str(), media["caption"].as_str()), (Some("image"), Some("!s")));
        assert_eq!((media["size"].as_u64(), media["viewOnce"].as_bool()), (Some(2048), Some(true)));
        let quoted = quoted_json(&message);
        assert_eq!(quoted["id"], "3EB0QUOTED");
        assert_eq!(quoted["sender"], "5215522222222@s.whatsapp.net");
        assert_eq!(
            (quoted["text"].as_str(), quoted["media"]["mimetype"].as_str()),
            (Some("la waifu de ayer"), Some("image/jpeg"))
        );
        assert_eq!(message.get_text(), Some("!s"));
    }

    #[test]
    fn nodes_keep_binary_payloads_readable() {
        let node = Node::new("iq").attr("id", "1").children([Node::new("raw").bytes(vec![0u8, 159, 146, 150])]);
        let value = node_to_json(&node);
        assert_eq!(value["attrs"]["id"], "1");
        assert_eq!(value["content"][0]["content"]["base64"], "AJ+Slg==");
    }
}
