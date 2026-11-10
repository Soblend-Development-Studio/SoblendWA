use std::time::{SystemTime, UNIX_EPOCH};

use crate::crypto;
use crate::events::MessageEvent;
use crate::jid::Jid;
use crate::proto::{
    ButtonsResponseMessage, ContactMessage, ContactsArrayMessage, ContextInfo, EventMessage, ExtendedTextMessage,
    GroupInviteMessage, LimitSharing, ListResponseMessage, LocationMessage, MemberLabel, Message, MessageContextInfo,
    MessageKey, PinInChatMessage, PollCreationMessage, PollOption, ProductMessage, ProductSnapshot, ProtocolMessage,
    ReactionMessage, SingleSelectReply, TemplateButtonReplyMessage,
};

pub(crate) fn now_millis() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or_default()
}

pub(crate) fn now_secs() -> i64 {
    now_millis() / 1000
}

pub fn generate_message_id(me: Option<&Jid>) -> String {
    let mut seed = Vec::with_capacity(8 + 32 + 16);
    seed.extend_from_slice(&(now_secs() as u64).to_be_bytes());
    if let Some(me) = me {
        seed.extend_from_slice(me.user.as_bytes());
        seed.extend_from_slice(b"@c.us");
    }
    seed.extend_from_slice(&crypto::random_array::<16>());
    let hash = crypto::sha256(&seed);
    format!("3EB0{}", &hex::encode_upper(hash)[..18])
}

pub(crate) fn pad(mut bytes: Vec<u8>) -> Vec<u8> {
    let mut size = crypto::random_array::<1>()[0] & 0x0f;
    if size == 0 {
        size = 0x0f;
    }
    bytes.extend(std::iter::repeat_n(size, size as usize));
    bytes
}

fn secret_context() -> Box<MessageContextInfo> {
    Box::new(MessageContextInfo { message_secret: Some(crypto::random_vec(32)), ..Default::default() })
}

pub(crate) fn unpad(bytes: &[u8]) -> Option<&[u8]> {
    let size = *bytes.last()? as usize;
    (size <= bytes.len()).then(|| &bytes[..bytes.len() - size])
}

impl Message {
    pub fn text(text: impl Into<String>) -> Self {
        Message { conversation: Some(text.into()), ..Default::default() }
    }

    pub fn reaction(key: MessageKey, emoji: impl Into<String>) -> Self {
        Message {
            reaction_message: Some(Box::new(ReactionMessage {
                key: Some(key),
                text: Some(emoji.into()),
                grouping_key: None,
                sender_timestamp_ms: Some(now_millis()),
            })),
            ..Default::default()
        }
    }

    pub fn location(latitude: f64, longitude: f64) -> Self {
        Message {
            location_message: Some(Box::new(LocationMessage {
                degrees_latitude: Some(latitude),
                degrees_longitude: Some(longitude),
                ..Default::default()
            })),
            ..Default::default()
        }
    }

    pub fn contact(display_name: impl Into<String>, vcard: impl Into<String>) -> Self {
        Message {
            contact_message: Some(Box::new(ContactMessage {
                display_name: Some(display_name.into()),
                vcard: Some(vcard.into()),
                context_info: None,
            })),
            ..Default::default()
        }
    }

    pub fn poll<I, S>(name: impl Into<String>, options: I, selectable: u32) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let poll = Box::new(PollCreationMessage {
            enc_key: None,
            name: Some(name.into()),
            options: options.into_iter().map(|o| PollOption { option_name: Some(o.into()) }).collect(),
            selectable_options_count: Some(selectable),
            context_info: None,
        });
        let mut message = Message { message_context_info: Some(secret_context()), ..Default::default() };
        if selectable == 1 {
            message.poll_creation_message_v3 = Some(poll);
        } else {
            message.poll_creation_message = Some(poll);
        }
        message
    }

    pub fn announcement_poll<I, S>(name: impl Into<String>, options: I, selectable: u32) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let mut message = Message::poll(name, options, selectable);
        message.poll_creation_message_v2 =
            message.poll_creation_message_v3.take().or_else(|| message.poll_creation_message.take());
        message
    }

    pub fn contacts<I, A, B>(display_name: impl Into<String>, cards: I) -> Self
    where
        I: IntoIterator<Item = (A, B)>,
        A: Into<String>,
        B: Into<String>,
    {
        let mut cards: Vec<ContactMessage> = cards
            .into_iter()
            .map(|(name, vcard)| ContactMessage {
                display_name: Some(name.into()),
                vcard: Some(vcard.into()),
                context_info: None,
            })
            .collect();
        if cards.len() == 1 {
            return Message { contact_message: cards.pop().map(Box::new), ..Default::default() };
        }
        Message {
            contacts_array_message: Some(Box::new(ContactsArrayMessage {
                display_name: Some(display_name.into()),
                contacts: cards,
                context_info: None,
            })),
            ..Default::default()
        }
    }

    pub fn vcard(name: &str, phone: &str) -> String {
        let digits: String = phone.chars().filter(char::is_ascii_digit).collect();
        format!("BEGIN:VCARD\nVERSION:3.0\nFN:{name}\nTEL;type=CELL;type=VOICE;waid={digits}:+{digits}\nEND:VCARD")
    }

    pub fn group_invite(
        group: &Jid,
        subject: impl Into<String>,
        invite_code: impl Into<String>,
        expiration: i64,
        caption: Option<&str>,
    ) -> Self {
        Message {
            group_invite_message: Some(Box::new(GroupInviteMessage {
                group_jid: Some(group.to_string()),
                invite_code: Some(invite_code.into()),
                invite_expiration: Some(expiration),
                group_name: Some(subject.into()),
                jpeg_thumbnail: None,
                caption: caption.map(str::to_owned),
                context_info: None,
            })),
            ..Default::default()
        }
    }

    pub fn product(product: ProductSnapshot, business_owner: &Jid, body: Option<&str>, footer: Option<&str>) -> Self {
        Message {
            product_message: Some(Box::new(ProductMessage {
                product: Some(product),
                business_owner_jid: Some(business_owner.to_non_device().to_string()),
                body: body.map(str::to_owned),
                footer: footer.map(str::to_owned),
                context_info: None,
            })),
            ..Default::default()
        }
    }

    pub fn share_phone_number() -> Self {
        Message::protocol(ProtocolMessage::SHARE_PHONE_NUMBER, |_| {})
    }

    pub fn request_phone_number() -> Self {
        Message { request_phone_number_message: Some(Box::default()), ..Default::default() }
    }

    pub fn limit_sharing(limited: bool) -> Self {
        Message::protocol(ProtocolMessage::LIMIT_SHARING, |p| {
            p.limit_sharing = Some(LimitSharing {
                sharing_limited: Some(limited),
                trigger: Some(1),
                limit_sharing_setting_timestamp: Some(now_millis()),
                initiated_by_me: Some(true),
            })
        })
    }

    pub fn member_label(label: &str) -> Self {
        let label: String = label.chars().take(30).collect();
        Message::protocol(ProtocolMessage::GROUP_MEMBER_LABEL_CHANGE, |p| {
            p.member_label = Some(MemberLabel { label: Some(label.clone()), label_timestamp: Some(now_secs()) })
        })
    }

    pub fn button_reply(id: impl Into<String>, text: impl Into<String>) -> Self {
        Message {
            buttons_response_message: Some(Box::new(ButtonsResponseMessage {
                selected_button_id: Some(id.into()),
                selected_display_text: Some(text.into()),
                context_info: None,
                r#type: Some(ButtonsResponseMessage::DISPLAY_TEXT),
            })),
            ..Default::default()
        }
    }

    pub fn template_reply(id: impl Into<String>, text: impl Into<String>, index: u32) -> Self {
        Message {
            template_button_reply_message: Some(Box::new(TemplateButtonReplyMessage {
                selected_id: Some(id.into()),
                selected_display_text: Some(text.into()),
                context_info: None,
                selected_index: Some(index),
            })),
            ..Default::default()
        }
    }

    pub fn list_reply(title: impl Into<String>, row_id: impl Into<String>) -> Self {
        Message {
            list_response_message: Some(Box::new(ListResponseMessage {
                title: Some(title.into()),
                list_type: Some(1),
                single_select_reply: Some(SingleSelectReply { selected_row_id: Some(row_id.into()) }),
                context_info: None,
                description: None,
            })),
            ..Default::default()
        }
    }

    pub fn video_note(mut video: Message) -> Self {
        if let Some(note) = video.video_message.take() {
            video.ptv_message = Some(note);
        }
        video
    }

    pub fn mention_all(mut self) -> Self {
        self.context_info_mut().non_jid_mentions = Some(1);
        self
    }

    fn protocol(kind: i32, fill: impl FnOnce(&mut ProtocolMessage)) -> Self {
        let mut protocol = ProtocolMessage { r#type: Some(kind), ..Default::default() };
        fill(&mut protocol);
        Message { protocol_message: Some(Box::new(protocol)), ..Default::default() }
    }
    pub fn event(name: impl Into<String>, start_time_secs: i64) -> Self {
        Message {
            event_message: Some(Box::new(EventMessage {
                name: Some(name.into()),
                start_time: Some(start_time_secs),
                is_canceled: Some(false),
                extra_guests_allowed: Some(false),
                ..Default::default()
            })),
            message_context_info: Some(secret_context()),
            ..Default::default()
        }
    }

    pub fn event_details(
        mut self,
        description: Option<&str>,
        end_time_secs: Option<i64>,
        location: Option<(f64, f64, &str)>,
        join_link: Option<&str>,
        extra_guests_allowed: bool,
    ) -> Self {
        if let Some(event) = self.event_message.as_mut() {
            event.description = description.map(str::to_owned);
            event.end_time = end_time_secs;
            event.location = location.map(|(latitude, longitude, name)| LocationMessage {
                degrees_latitude: Some(latitude),
                degrees_longitude: Some(longitude),
                name: Some(name.to_owned()),
                ..Default::default()
            });
            event.join_link = join_link.map(str::to_owned);
            event.extra_guests_allowed = Some(extra_guests_allowed);
        }
        self
    }

    pub fn cancel_event(mut self) -> Self {
        if let Some(event) = self.event_message.as_mut() {
            event.is_canceled = Some(true);
        }
        self
    }

    pub fn pin(key: MessageKey, pinned: bool, duration_secs: u32) -> Self {
        let kind = if pinned { PinInChatMessage::PIN_FOR_ALL } else { PinInChatMessage::UNPIN_FOR_ALL };
        Message {
            pin_in_chat_message: Some(Box::new(PinInChatMessage {
                key: Some(key),
                r#type: Some(kind),
                sender_timestamp_ms: Some(now_millis()),
            })),
            message_context_info: Some(Box::new(MessageContextInfo {
                message_add_on_duration_in_secs: Some(if pinned { duration_secs } else { 0 }),
                ..Default::default()
            })),
            ..Default::default()
        }
    }

    pub fn revoke(key: MessageKey) -> Self {
        Message {
            protocol_message: Some(Box::new(ProtocolMessage {
                key: Some(key),
                r#type: Some(ProtocolMessage::REVOKE),
                ..Default::default()
            })),
            ..Default::default()
        }
    }

    pub fn edit(key: MessageKey, replacement: Message) -> Self {
        Message {
            protocol_message: Some(Box::new(ProtocolMessage {
                key: Some(key),
                r#type: Some(ProtocolMessage::MESSAGE_EDIT),
                edited_message: Some(Box::new(replacement)),
                timestamp_ms: Some(now_millis()),
                ..Default::default()
            })),
            ..Default::default()
        }
    }

    pub fn reply_to(mut self, quoted: &MessageEvent) -> Self {
        let info = &quoted.info;
        let context = self.context_info_mut();
        context.stanza_id = Some(info.id.clone());
        context.participant = Some(info.sender.to_non_device().to_string());
        context.quoted_message = Some(Box::new(quoted.message.clone()));
        if info.is_group {
            context.remote_jid = Some(info.chat.to_string());
        }
        self
    }

    pub fn mention<'a>(mut self, jids: impl IntoIterator<Item = &'a Jid>) -> Self {
        let context = self.context_info_mut();
        context.mentioned_jid.extend(jids.into_iter().map(|j| j.to_non_device().to_string()));
        self
    }

    pub fn forwarded(mut self) -> Self {
        let context = self.context_info_mut();
        context.is_forwarded = Some(true);
        context.forwarding_score = Some(context.forwarding_score.unwrap_or(0) + 1);
        self
    }

    pub fn unwrapped(&self) -> &Message {
        let inner = [
            &self.ephemeral_message,
            &self.view_once_message,
            &self.view_once_message_v2,
            &self.view_once_message_v2_extension,
            &self.document_with_caption_message,
            &self.edited_message,
            &self.group_mentioned_message,
            &self.bot_invoke_message,
            &self.lottie_sticker_message,
            &self.status_mention_message,
            &self.associated_child_message,
            &self.group_status_mention_message,
            &self.poll_creation_message_v4,
            &self.group_status_message,
            &self.bot_task_message,
            &self.question_message,
            &self.group_status_message_v2,
            &self.bot_forwarded_message,
            &self.question_reply_message,
        ]
        .into_iter()
        .find_map(|wrapper| wrapper.as_ref().and_then(|w| w.message.as_deref()));
        match inner {
            Some(message) => message.unwrapped(),
            None => self,
        }
    }

    pub fn is_view_once(&self) -> bool {
        if self.view_once_message.is_some()
            || self.view_once_message_v2.is_some()
            || self.view_once_message_v2_extension.is_some()
        {
            return true;
        }
        [
            &self.ephemeral_message,
            &self.document_with_caption_message,
            &self.edited_message,
            &self.associated_child_message,
            &self.bot_invoke_message,
            &self.group_mentioned_message,
        ]
        .into_iter()
        .filter_map(|wrapper| wrapper.as_ref().and_then(|w| w.message.as_deref()))
        .any(Message::is_view_once)
    }

    pub fn get_text(&self) -> Option<&str> {
        let message = self.unwrapped();
        message
            .conversation
            .as_deref()
            .or_else(|| message.extended_text_message.as_ref().and_then(|m| m.text.as_deref()))
            .or_else(|| message.image_message.as_ref().and_then(|m| m.caption.as_deref()))
            .or_else(|| message.video_message.as_ref().and_then(|m| m.caption.as_deref()))
            .or_else(|| message.document_message.as_ref().and_then(|m| m.caption.as_deref()))
            .or_else(|| message.buttons_response_message.as_ref().and_then(|m| m.selected_display_text.as_deref()))
            .or_else(|| message.list_response_message.as_ref().and_then(|m| m.title.as_deref()))
            .or_else(|| message.template_button_reply_message.as_ref().and_then(|m| m.selected_display_text.as_deref()))
            .or_else(|| {
                let reply = message.interactive_response_message.as_ref()?;
                reply.body.as_ref()?.text.as_deref()
            })
            .or_else(|| message.buttons_message.as_ref().and_then(|m| m.content_text.as_deref()))
            .or_else(|| message.list_message.as_ref().and_then(|m| m.description.as_deref()))
            .or_else(|| {
                let interactive = message.interactive_message.as_ref()?;
                interactive.body.as_ref()?.text.as_deref()
            })
            .or_else(|| {
                let template = message.template_message.as_ref()?;
                template.hydrated_template.as_ref()?.hydrated_content_text.as_deref()
            })
    }

    pub fn context_info(&self) -> Option<&ContextInfo> {
        let m = self.unwrapped();
        m.extended_text_message
            .as_ref()
            .and_then(|x| x.context_info.as_ref())
            .or_else(|| m.image_message.as_ref().and_then(|x| x.context_info.as_ref()))
            .or_else(|| m.video_message.as_ref().and_then(|x| x.context_info.as_ref()))
            .or_else(|| m.audio_message.as_ref().and_then(|x| x.context_info.as_ref()))
            .or_else(|| m.document_message.as_ref().and_then(|x| x.context_info.as_ref()))
            .or_else(|| m.sticker_message.as_ref().and_then(|x| x.context_info.as_ref()))
            .or_else(|| m.location_message.as_ref().and_then(|x| x.context_info.as_ref()))
            .or_else(|| m.contact_message.as_ref().and_then(|x| x.context_info.as_ref()))
            .or_else(|| m.buttons_response_message.as_ref().and_then(|x| x.context_info.as_ref()))
            .or_else(|| m.list_response_message.as_ref().and_then(|x| x.context_info.as_ref()))
            .or_else(|| m.template_button_reply_message.as_ref().and_then(|x| x.context_info.as_ref()))
            .or_else(|| m.interactive_response_message.as_ref().and_then(|x| x.context_info.as_ref()))
            .or_else(|| m.interactive_message.as_ref().and_then(|x| x.context_info.as_ref()))
            .or_else(|| m.buttons_message.as_ref().and_then(|x| x.context_info.as_ref()))
            .or_else(|| m.list_message.as_ref().and_then(|x| x.context_info.as_ref()))
    }

    pub fn context_info_mut(&mut self) -> &mut ContextInfo {
        if let Some(text) = self.conversation.take() {
            self.extended_text_message = Some(Box::new(ExtendedTextMessage { text: Some(text), ..Default::default() }));
        }
        let slot = match self {
            Message { view_once_message: Some(wrapper), .. } if wrapper.message.is_some() => {
                return wrapper.message.get_or_insert_with(Default::default).context_info_mut();
            }
            Message { extended_text_message: Some(m), .. } => &mut m.context_info,
            Message { image_message: Some(m), .. } => &mut m.context_info,
            Message { video_message: Some(m), .. } => &mut m.context_info,
            Message { audio_message: Some(m), .. } => &mut m.context_info,
            Message { document_message: Some(m), .. } => &mut m.context_info,
            Message { sticker_message: Some(m), .. } => &mut m.context_info,
            Message { location_message: Some(m), .. } => &mut m.context_info,
            Message { contact_message: Some(m), .. } => &mut m.context_info,
            Message { interactive_message: Some(m), .. } => &mut m.context_info,
            Message { buttons_message: Some(m), .. } => &mut m.context_info,
            Message { list_message: Some(m), .. } => &mut m.context_info,
            Message { template_message: Some(m), .. } => &mut m.context_info,
            Message { extended_text_message: empty, .. } => {
                let text = ExtendedTextMessage { text: Some(String::new()), ..Default::default() };
                &mut empty.get_or_insert_with(|| Box::new(text)).context_info
            }
        };
        slot.get_or_insert_with(Default::default)
    }

    pub fn has_content(&self) -> bool {
        use prost::encoding::message::encoded_len;
        use prost::Message as _;
        let bookkeeping = self.sender_key_distribution_message.as_deref().map_or(0, |m| encoded_len(2, m))
            + self.message_context_info.as_deref().map_or(0, |m| encoded_len(35, m));
        self.encoded_len() > bookkeeping
    }

    pub fn stanza_type(&self) -> &'static str {
        let m = self.unwrapped();
        if m.poll_creation_message.is_some()
            || m.poll_creation_message_v2.is_some()
            || m.poll_creation_message_v3.is_some()
            || m.poll_update_message.is_some()
        {
            "poll"
        } else if m.reaction_message.is_some() {
            "reaction"
        } else if m.event_message.is_some() {
            "event"
        } else if self.media_type().is_some() {
            "media"
        } else {
            "text"
        }
    }

    pub fn media_type(&self) -> Option<&'static str> {
        let m = self.unwrapped();
        if m.image_message.is_some() {
            Some("image")
        } else if let Some(video) = &m.video_message {
            Some(if video.gif_playback == Some(true) { "gif" } else { "video" })
        } else if m.ptv_message.is_some() {
            Some("ptv")
        } else if let Some(audio) = &m.audio_message {
            Some(if audio.ptt == Some(true) { "ptt" } else { "audio" })
        } else if m.contact_message.is_some() {
            Some("vcard")
        } else if m.document_message.is_some() {
            Some("document")
        } else if m.contacts_array_message.is_some() {
            Some("contact_array")
        } else if m.sticker_message.is_some() {
            Some("sticker")
        } else if m.product_message.is_some() {
            Some("product")
        } else if m.group_invite_message.is_some() {
            Some("url")
        } else if m.list_message.is_some() {
            Some("list")
        } else if m.list_response_message.is_some() {
            Some("list_response")
        } else if m.buttons_response_message.is_some() {
            Some("buttons_response")
        } else if m.interactive_response_message.is_some() {
            Some("native_flow_response")
        } else {
            None
        }
    }
}

impl From<&str> for Message {
    fn from(value: &str) -> Self {
        Message::text(value)
    }
}

impl From<String> for Message {
    fn from(value: String) -> Self {
        Message::text(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn padding_roundtrip() {
        for _ in 0..50 {
            let padded = pad(b"soblend".to_vec());
            assert!(padded.len() > 7 && padded.len() <= 7 + 15);
            assert_eq!(unpad(&padded).unwrap(), b"soblend");
        }
    }

    #[test]
    fn builders() {
        let single = Message::poll("¿Pizza o tacos?", ["Pizza", "Tacos"], 1);
        assert!(single.poll_creation_message_v3.is_some() && single.poll_creation_message.is_none());
        let multiple = Message::poll("¿Qué te gusta?", ["Rust", "Go", "Zig"], 0);
        assert!(multiple.poll_creation_message.is_some() && multiple.poll_creation_message_v3.is_none());
        assert!(Message::announcement_poll("Aviso", ["Sí", "No"], 1).poll_creation_message_v2.is_some());

        let one = Message::contacts("Ana", [("Ana", Message::vcard("Ana", "+52 1 55 1111 1111"))]);
        assert!(one.contact_message.unwrap().vcard.unwrap().contains("waid=5215511111111"));
        let many = Message::contacts("2 contactos", [("Ana", "v1"), ("Luis", "v2")]);
        assert_eq!(many.contacts_array_message.unwrap().contacts.len(), 2);

        let invite = Message::group_invite(&Jid::group("1203630001"), "Equipo", "AbCdEf", 10, Some("únete"));
        assert_eq!((invite.media_type(), invite.stanza_type()), (Some("url"), "media"));

        let label = Message::member_label("una etiqueta demasiado larga para whatsapp");
        let protocol = label.protocol_message.unwrap();
        assert_eq!(protocol.r#type, Some(ProtocolMessage::GROUP_MEMBER_LABEL_CHANGE));
        assert_eq!(protocol.member_label.unwrap().label.unwrap().chars().count(), 30);

        assert_eq!(Message::text("hola @todos").mention_all().context_info().unwrap().non_jid_mentions, Some(1));
        assert_eq!(Message::list_reply("Al pastor", "pastor").interactive_reply().unwrap().id, "pastor");
        assert_eq!(Message::share_phone_number().protocol_message.unwrap().r#type, Some(11));
        let event = Message::event("Lanzamiento", 100).event_details(Some("En vivo"), Some(200), None, None, true);
        assert_eq!(event.event_message.as_ref().and_then(|e| e.end_time), Some(200));
    }

    #[test]
    fn message_ids() {
        let id = generate_message_id(Some(&Jid::phone("5215512345678")));
        assert_eq!(id.len(), 22);
        assert!(id.starts_with("3EB0"));
    }

    #[test]
    fn protocol_only() {
        let skdm = Message {
            sender_key_distribution_message: Some(Box::new(crate::proto::SenderKeyDistributionMessage {
                group_id: Some("1@g.us".into()),
                axolotl_sender_key_distribution_message: Some(vec![1; 40]),
            })),
            message_context_info: Some(Box::new(MessageContextInfo {
                device_list_metadata_version: Some(2),
                message_secret: None,
                message_add_on_duration_in_secs: None,
                message_association: None,
            })),
            ..Default::default()
        };
        assert!(!skdm.has_content());
        let mut text = skdm.clone();
        text.conversation = Some("hola".into());
        assert!(text.has_content());
        assert!(!Message::default().has_content());
    }

    #[test]
    fn mention_upgrade() {
        let jid = Jid::phone("5215512345678");
        let message = Message::text("hola @5215512345678").mention([&jid]);
        assert!(message.conversation.is_none());
        assert_eq!(message.get_text(), Some("hola @5215512345678"));
        assert_eq!(message.context_info().unwrap().mentioned_jid, vec!["5215512345678@s.whatsapp.net"]);
    }
}
