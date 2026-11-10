// Hand-written serializers for the events that cross into Node and Python the most. Going through
// serde_json::Value cost ~18 µs per message, mostly allocations; writing straight to text is ~3 µs.
// The output has to stay identical to Event::to_json, matches_value checks that.
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use prost::Message as _;
use serde::ser::{SerializeMap, SerializeSeq, Serializer};
use serde::Serialize;

use crate::events::{ChatState, ChatStateKind, Event, MessageEvent, MessageInfo, Presence, Receipt, ServerAck};
use crate::jid::Jid;
use crate::json::{link_preview_json, media_json, quoted_json, reaction_json, receipt_name, reply_json, timings_json};

struct JidText<'a>(&'a Jid);

impl Serialize for JidText<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self.0)
    }
}

struct MaybeJid<'a>(&'a Option<Jid>);

impl Serialize for MaybeJid<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self.0 {
            Some(jid) => serializer.collect_str(jid),
            None => serializer.serialize_none(),
        }
    }
}

struct JidList<'a>(&'a [Jid]);

impl Serialize for JidList<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut seq = serializer.serialize_seq(Some(self.0.len()))?;
        for jid in self.0 {
            seq.serialize_element(&JidText(jid))?;
        }
        seq.end()
    }
}

struct InfoText<'a>(&'a MessageInfo);

impl Serialize for InfoText<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let info = self.0;
        let mut map = serializer.serialize_map(Some(15))?;
        map.serialize_entry("category", &info.category)?;
        map.serialize_entry("chat", &JidText(&info.chat))?;
        map.serialize_entry("edit", &info.edit)?;
        map.serialize_entry("elapsedMicros", &(info.elapsed().as_micros() as u64))?;
        map.serialize_entry("fromMe", &info.from_me)?;
        map.serialize_entry("id", &info.id)?;
        map.serialize_entry("isGroup", &info.is_group)?;
        map.serialize_entry("offline", &info.offline)?;
        map.serialize_entry("pushName", &info.push_name)?;
        map.serialize_entry("sender", &JidText(&info.sender))?;
        map.serialize_entry("senderAlt", &MaybeJid(&info.sender_alt))?;
        map.serialize_entry("senderLid", &MaybeJid(&info.sender_lid))?;
        map.serialize_entry("senderPhone", &MaybeJid(&info.sender_phone))?;
        map.serialize_entry("serverId", &info.server_id)?;
        map.serialize_entry("timestamp", &info.timestamp)?;
        map.end()
    }
}

struct MessageText<'a>(&'a MessageEvent);

impl Serialize for MessageText<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let event = self.0;
        let message = &event.message;
        let age = crate::message::now_secs().saturating_sub(event.info.timestamp).max(0);
        let quoted_id = message.context_info().and_then(|c| c.stanza_id.as_deref());
        let mut map = serializer.serialize_map(Some(15))?;
        map.serialize_entry("ageSecs", &age)?;
        map.serialize_entry("info", &InfoText(&event.info))?;
        map.serialize_entry("kind", message.stanza_type())?;
        map.serialize_entry("linkPreview", &link_preview_json(message))?;
        map.serialize_entry("media", &media_json(message))?;
        map.serialize_entry("mediaType", &message.media_type())?;
        map.serialize_entry("mentions", &JidList(&event.mentions()))?;
        map.serialize_entry("proto", &STANDARD.encode(event.raw.encode_to_vec()))?;
        map.serialize_entry("quoted", &quoted_json(message))?;
        map.serialize_entry("quotedId", &quoted_id)?;
        map.serialize_entry("reaction", &reaction_json(message))?;
        map.serialize_entry("reply", &reply_json(message))?;
        map.serialize_entry("text", &event.text())?;
        map.serialize_entry("type", "message")?;
        map.serialize_entry("viewOnce", &message.is_view_once())?;
        map.end()
    }
}

struct ReceiptText<'a>(&'a Receipt);

impl Serialize for ReceiptText<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let receipt = self.0;
        let mut map = serializer.serialize_map(Some(7))?;
        map.serialize_entry("chat", &JidText(&receipt.chat))?;
        map.serialize_entry("fromMe", &receipt.from_me)?;
        map.serialize_entry("ids", &receipt.ids)?;
        map.serialize_entry("receipt", receipt_name(&receipt.kind))?;
        map.serialize_entry("sender", &JidText(&receipt.sender))?;
        map.serialize_entry("timestamp", &receipt.timestamp)?;
        map.serialize_entry("type", "receipt")?;
        map.end()
    }
}

struct AckText<'a>(&'a ServerAck);

impl Serialize for AckText<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let ack = self.0;
        let mut map = serializer.serialize_map(Some(6))?;
        map.serialize_entry("chat", &JidText(&ack.chat))?;
        map.serialize_entry("error", &ack.error)?;
        map.serialize_entry("id", &ack.id)?;
        map.serialize_entry("latencyMicros", &ack.latency.map(|d| d.as_micros() as u64))?;
        map.serialize_entry("timings", &ack.timings.map(|t| timings_json(&t)))?;
        map.serialize_entry("type", "serverAck")?;
        map.end()
    }
}

struct PresenceText<'a>(&'a Presence);

impl Serialize for PresenceText<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let presence = self.0;
        let mut map = serializer.serialize_map(Some(4))?;
        map.serialize_entry("available", &presence.available)?;
        map.serialize_entry("from", &JidText(&presence.from))?;
        map.serialize_entry("lastSeen", &presence.last_seen)?;
        map.serialize_entry("type", "presence")?;
        map.end()
    }
}

struct ChatStateText<'a>(&'a ChatState);

impl Serialize for ChatStateText<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let state = self.0;
        let name = match state.state {
            ChatStateKind::Composing => "composing",
            ChatStateKind::Recording => "recording",
            ChatStateKind::Paused => "paused",
        };
        let mut map = serializer.serialize_map(Some(4))?;
        map.serialize_entry("chat", &JidText(&state.chat))?;
        map.serialize_entry("sender", &JidText(&state.sender))?;
        map.serialize_entry("state", name)?;
        map.serialize_entry("type", "chatState")?;
        map.end()
    }
}

impl Event {
    pub fn to_json_string(&self) -> String {
        let written = match self {
            Event::Message(message) => serde_json::to_string(&MessageText(message)),
            Event::Receipt(receipt) => serde_json::to_string(&ReceiptText(receipt)),
            Event::ServerAck(ack) => serde_json::to_string(&AckText(ack)),
            Event::Presence(presence) => serde_json::to_string(&PresenceText(presence)),
            Event::ChatState(state) => serde_json::to_string(&ChatStateText(state)),
            _ => return self.to_json().to_string(),
        };
        written.unwrap_or_else(|_| self.to_json().to_string())
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use serde_json::Value;

    use super::*;
    use crate::client::SendTimings;
    use crate::events::ReceiptType;
    use crate::proto::{ContextInfo, ExtendedTextMessage, Message};

    fn steady(mut value: Value) -> Value {
        if let Some(info) = value.get_mut("info") {
            info["elapsedMicros"] = Value::from(0);
        }
        value
    }

    fn same(event: Event) {
        let text: Value = serde_json::from_str(&event.to_json_string()).unwrap();
        assert_eq!(steady(text), steady(event.to_json()), "{}", event.kind());
    }

    #[test]
    fn matches_value() {
        let info = MessageInfo {
            id: "3EB0ABC".into(),
            chat: Jid::group("120363000000000001"),
            sender: Jid::phone("5215512345678").with_device(3),
            sender_alt: Some(Jid::lid("174342050709710")),
            sender_phone: Some(Jid::phone("5215512345678")),
            sender_lid: Some(Jid::lid("174342050709710")),
            from_me: false,
            is_group: true,
            push_name: Some("Ana \"la\" de\nventas".into()),
            timestamp: 1_700_000_000,
            category: None,
            offline: true,
            edit: None,
            server_id: Some("77".into()),
            received_at: Instant::now(),
        };
        let quoted = ExtendedTextMessage {
            text: Some("mira @5215522222222 https://www.rust-lang.org".into()),
            matched_text: Some("https://www.rust-lang.org".into()),
            title: Some("Rust".into()),
            context_info: Some(ContextInfo {
                stanza_id: Some("3EB0QUOTED".into()),
                participant: Some("5215522222222@s.whatsapp.net".into()),
                mentioned_jid: vec!["5215522222222@s.whatsapp.net".into()],
                quoted_message: Some(Box::new(Message::text("antes"))),
                ..Default::default()
            }),
            ..Default::default()
        };
        let messages =
            [Message::text("!ping"), Message { extended_text_message: Some(Box::new(quoted)), ..Default::default() }];
        for message in messages {
            same(Event::Message(Box::new(MessageEvent { info: info.clone(), raw: message.clone(), message })));
        }
        for kind in [ReceiptType::Read, ReceiptType::Other("raro".into())] {
            same(Event::Receipt(Receipt {
                chat: Jid::phone("5215522222222"),
                sender: Jid::phone("5215522222222").with_device(2),
                ids: vec!["A".into(), "B".into()],
                kind,
                timestamp: 1_700_000_001,
                from_me: true,
            }));
        }
        let timings = SendTimings { encrypt: Duration::from_micros(40), ..Default::default() };
        for (latency, timings) in [(None, None), (Some(Duration::from_millis(31)), Some(timings))] {
            same(Event::ServerAck(ServerAck {
                id: "3EB0SENT".into(),
                chat: Jid::group("120363000000000001"),
                error: Some(479),
                latency,
                timings,
            }));
        }
        same(Event::Presence(Presence { from: Jid::phone("5215522222222"), available: false, last_seen: Some(9) }));
        same(Event::ChatState(ChatState {
            chat: Jid::phone("5215522222222"),
            sender: Jid::phone("5215522222222"),
            state: ChatStateKind::Recording,
        }));
        same(Event::Qr("2@abc".into()));
    }
}
