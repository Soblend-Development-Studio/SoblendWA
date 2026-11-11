use std::future::Future;
use std::pin::Pin;

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use serde::de::DeserializeOwned;
use serde_json::{json, Value};
use soblendwa::{Error, Result};

use crate::{InteractiveSpec, MediaInput, SectionSpec, Session};

pub const METHODS: &[&str] = &[
    "status",
    "connect",
    "disconnect",
    "logout",
    "ping",
    "stats",
    "pendingSends",
    "elapsed",
    "requestPairCode",
    "sendText",
    "reply",
    "react",
    "read",
    "markPlayed",
    "sendMedia",
    "sendFile",
    "download",
    "downloadToFile",
    "sendPoll",
    "votePoll",
    "edit",
    "revoke",
    "typing",
    "setOnline",
    "sendInteractive",
    "sendButtons",
    "sendList",
    "sendContacts",
    "sendMentionAll",
    "sendGroupInvite",
    "setMemberLabel",
    "sendProto",
    "groupInfo",
    "joinedGroups",
    "onWhatsApp",
    "profilePicture",
    "block",
    "unblock",
    "modifyChat",
    "labelChat",
    "labelMessage",
    "editLabel",
    "deleteLabel",
    "saveQuickReply",
    "chats",
    "contacts",
    "messages",
    "search",
    "botList",
    "reachoutTimelock",
    "newChatMessageCap",
    "disappearingModes",
    "restart",
    "simulateOutage",
    "damageSessions",
    "groupMetadata",
    "groupCreate",
    "groupParticipants",
    "groupSubject",
    "groupDescription",
    "groupSetting",
    "groupPicture",
    "groupLeave",
    "groupInviteCode",
    "groupRevokeInvite",
    "groupJoin",
    "groupInviteInfo",
    "groupRequests",
    "groupRespondRequests",
    "setProfilePicture",
    "removeProfilePicture",
    "setAbout",
    "fetchAbout",
    "setPushName",
    "businessProfile",
    "blocklist",
    "privacySettings",
    "setPrivacy",
    "lidForPhone",
    "phoneForLid",
];

struct Params<'a>(&'a Value);

impl Params<'_> {
    fn missing(key: &str) -> Error {
        Error::InvalidArgument(format!("missing parameter \"{key}\""))
    }

    fn text(&self, key: &str) -> Result<String> {
        self.optional_text(key).ok_or_else(|| Self::missing(key))
    }

    fn optional_text(&self, key: &str) -> Option<String> {
        match &self.0[key] {
            Value::String(text) => Some(text.clone()),
            Value::Number(number) => Some(number.to_string()),
            _ => None,
        }
    }

    fn flag(&self, key: &str, default: bool) -> bool {
        self.0[key].as_bool().unwrap_or(default)
    }

    fn number(&self, key: &str) -> Option<u64> {
        self.0[key].as_u64().or_else(|| self.0[key].as_str()?.parse().ok())
    }

    fn texts(&self, key: &str) -> Result<Vec<String>> {
        let list = self.0[key].as_array().ok_or_else(|| Self::missing(key))?;
        Ok(list.iter().filter_map(|v| v.as_str().map(str::to_owned).or_else(|| Some(v.to_string()))).collect())
    }

    fn bytes(&self, key: &str) -> Result<Vec<u8>> {
        let raw = self.text(key)?;
        STANDARD.decode(raw.trim()).map_err(|_| Error::InvalidArgument(format!("parameter \"{key}\" must be base64")))
    }

    fn list(&self, key: &str) -> Result<Vec<String>> {
        if self.0[key].is_null() {
            return Ok(Vec::new());
        }
        self.texts(key)
    }

    fn parse<T: DeserializeOwned>(&self) -> Result<T> {
        serde_json::from_value(self.0.clone()).map_err(Error::from)
    }

    fn field<T: DeserializeOwned>(&self, key: &str) -> Result<T> {
        serde_json::from_value(self.0[key].clone())
            .map_err(|err| Error::InvalidArgument(format!("parameter \"{key}\": {err}")))
    }
}

fn done<T: serde::Serialize>(value: T) -> Result<Value> {
    serde_json::to_value(value).map_err(Error::from)
}

#[derive(serde::Deserialize)]
struct Pair {
    id: Option<String>,
    name: Option<String>,
    text: Option<String>,
    phone: Option<serde_json::Value>,
}

type Pending<T> = Pin<Box<dyn Future<Output = Result<T>> + Send>>;

impl Session {
    // call() for someone about to wait for their turn in a chat: media starts uploading right now instead of
    // waiting in line too.
    pub fn begin(&self, method: &str, params: Value) -> Pending<Value> {
        let p = Params(&params);
        let started = || -> Result<Option<Pending<String>>> {
            Ok(match method {
                "sendMedia" => {
                    let input: MediaInput = p.parse()?;
                    Some(Box::pin(self.send_media(&p.text("chat")?, &p.text("kind")?, p.bytes("data")?, input)))
                }
                "sendFile" => {
                    let input: MediaInput = p.parse()?;
                    Some(Box::pin(self.send_media_file(&p.text("chat")?, &p.text("kind")?, &p.text("path")?, input)))
                }
                _ => None,
            })
        };
        match started() {
            Ok(Some(sending)) => Box::pin(async move { done(sending.await?) }),
            Err(err) => Box::pin(std::future::ready(Err(err))),
            Ok(None) => {
                let (session, method) = (self.clone(), method.to_owned());
                Box::pin(async move { session.call(&method, &params).await })
            }
        }
    }

    pub async fn call(&self, method: &str, params: &Value) -> Result<Value> {
        let p = Params(params);
        match method {
            "status" => done(json!({ "me": self.me(), "connected": self.is_connected() })),
            "connect" => done(self.connect().await?),
            "disconnect" => {
                self.disconnect().await;
                Ok(Value::Null)
            }
            "logout" => done(self.logout().await?),
            "ping" => done(self.ping().await?),
            "stats" => Ok(self.client().stats().to_json()),
            "pendingSends" => {
                let pending = self.client().pending_sends().await?;
                done(
                    pending
                        .iter()
                        .map(|p| {
                            json!({
                                "id": p.id, "chat": p.chat.to_string(),
                                "queuedAt": p.queued_at, "attempts": p.attempts,
                            })
                        })
                        .collect::<Vec<_>>(),
                )
            }
            "elapsed" => done(self.elapsed_ms(&p.text("messageId")?)),
            "requestPairCode" => done(self.pair_code(&p.text("phone")?).await?),
            "sendText" => {
                let (chat, text) = (p.text("chat")?, p.text("text")?);
                let quoted = p.optional_text("quotedId");
                let mentions = if params["mentions"].is_null() { Vec::new() } else { p.texts("mentions")? };
                done(self.send_text_full(&chat, &text, quoted.as_deref(), &params["preview"], &mentions).await?)
            }
            "reply" => done(self.reply(&p.text("messageId")?, &p.text("text")?).await?),
            "react" => done(self.react(&p.text("messageId")?, &p.text("emoji")?).await?),
            "read" => {
                let original = self.recall(&p.text("messageId")?)?;
                done(self.client().read_message(&original).await?)
            }
            "markPlayed" => done(self.mark_played(&p.text("messageId")?).await?),
            "sendMedia" => {
                let input: MediaInput = p.parse()?;
                done(self.send_media(&p.text("chat")?, &p.text("kind")?, p.bytes("data")?, input).await?)
            }
            "sendFile" => {
                let input: MediaInput = p.parse()?;
                done(self.send_media_file(&p.text("chat")?, &p.text("kind")?, &p.text("path")?, input).await?)
            }
            "download" => {
                let data = self.download(&p.text("messageId")?).await?;
                done(json!({ "base64": STANDARD.encode(&data), "size": data.len() }))
            }
            "downloadToFile" => done(self.download_to_file(&p.text("messageId")?, &p.text("path")?).await?),
            "sendPoll" => {
                let selectable = p.number("selectable").unwrap_or(1) as u32;
                done(self.send_poll(&p.text("chat")?, &p.text("name")?, p.texts("options")?, selectable).await?)
            }
            "votePoll" => done(self.vote_poll(&p.text("chat")?, &p.text("pollId")?, p.texts("options")?).await?),
            "edit" => done(self.edit(&p.text("chat")?, &p.text("messageId")?, &p.text("text")?).await?),
            "revoke" => done(self.revoke(&p.text("chat")?, &p.text("messageId")?).await?),
            "typing" => {
                let state = p.optional_text("state").unwrap_or_else(|| "composing".into());
                done(self.chat_presence(&p.text("chat")?, &state).await?)
            }
            "setOnline" => done(self.set_presence(p.flag("available", true)).await?),
            "sendInteractive" => {
                let spec: InteractiveSpec = p.parse()?;
                done(self.send_interactive(&p.text("chat")?, spec).await?)
            }
            "sendButtons" => {
                let buttons: Vec<Pair> = p.field("buttons")?;
                let pairs =
                    buttons.into_iter().map(|b| (b.id.unwrap_or_default(), b.text.unwrap_or_default())).collect();
                let footer = p.optional_text("footer");
                done(self.send_buttons(&p.text("chat")?, &p.text("text")?, footer.as_deref(), pairs).await?)
            }
            "sendList" => {
                let sections: Vec<SectionSpec> = p.field("sections")?;
                let footer = p.optional_text("footer");
                let (chat, title, text, button) =
                    (p.text("chat")?, p.text("title")?, p.text("text")?, p.text("buttonText")?);
                done(self.send_list(&chat, &title, &text, &button, footer.as_deref(), sections).await?)
            }
            "sendContacts" => {
                let cards: Vec<Pair> = p.field("cards")?;
                let pairs = cards
                    .into_iter()
                    .map(|c| {
                        let phone = match c.phone {
                            Some(Value::String(text)) => text,
                            Some(other) => other.to_string(),
                            None => String::new(),
                        };
                        (c.name.unwrap_or_default(), phone)
                    })
                    .collect();
                done(self.send_contacts(&p.text("chat")?, pairs).await?)
            }
            "sendMentionAll" => done(self.send_mention_all(&p.text("chat")?, &p.text("text")?).await?),
            "sendGroupInvite" => {
                let caption = p.optional_text("caption");
                done(self.send_group_invite(&p.text("to")?, &p.text("group")?, caption.as_deref()).await?)
            }
            "setMemberLabel" => done(self.set_member_label(&p.text("group")?, &p.text("label")?).await?),
            "sendProto" => done(self.send_proto(&p.text("chat")?, &p.bytes("proto")?).await?),
            "groupInfo" => self.group_info(&p.text("chat")?).await,
            "joinedGroups" => self.joined_groups().await,
            "onWhatsApp" => self.on_whatsapp(p.texts("phones")?).await,
            "profilePicture" => done(self.profile_picture(&p.text("jid")?, p.flag("fullSize", false)).await?),
            "block" => done(self.block(&p.text("jid")?, true).await?),
            "unblock" => done(self.block(&p.text("jid")?, false).await?),
            "modifyChat" => done(self.modify_chat(&p.text("chat")?, &p.text("action")?, p.number("seconds")).await?),
            "labelChat" => {
                done(self.label(&p.text("chat")?, &p.text("labelId")?, None, p.flag("labeled", true)).await?)
            }
            "labelMessage" => {
                let message = p.text("messageId")?;
                done(self.label(&p.text("chat")?, &p.text("labelId")?, Some(&message), p.flag("labeled", true)).await?)
            }
            "editLabel" => {
                let color = p.number("color").unwrap_or(0) as i32;
                done(self.edit_label(&p.text("id")?, Some(&p.text("name")?), color).await?)
            }
            "deleteLabel" => done(self.edit_label(&p.text("id")?, None, 0).await?),
            "saveQuickReply" => done(self.save_quick_reply(&p.text("shortcut")?, &p.text("text")?).await?),
            "chats" => self.chats(),
            "contacts" => self.contacts(),
            "messages" => {
                let limit = p.number("limit").unwrap_or(50) as usize;
                self.messages(&p.text("chat")?, limit, p.number("before").map(|b| b as i64))
            }
            "search" => self.search(&p.text("text")?, p.number("limit").unwrap_or(50) as usize),
            "botList" => self.account("bots", Vec::new()).await,
            "reachoutTimelock" => self.account("timelock", Vec::new()).await,
            "newChatMessageCap" => self.account("messageCap", Vec::new()).await,
            "disappearingModes" => self.account("disappearing", p.texts("jids")?).await,
            "restart" => done(self.restart().await?),
            "simulateOutage" => {
                self.client().simulate_outage();
                Ok(Value::Null)
            }
            "damageSessions" => done(self.client().damage_sessions(&crate::jid(&p.text("jid")?)?).await?),
            "groupMetadata" => self.group_metadata(&p.text("chat")?).await,
            "groupCreate" => self.group_create(&p.text("subject")?, &p.list("participants")?).await,
            "groupParticipants" => {
                let action = p.text("action")?;
                self.group_participants(&p.text("chat")?, &p.texts("participants")?, &action).await
            }
            "groupSubject" => done(self.group_subject(&p.text("chat")?, &p.text("subject")?).await?),
            "groupDescription" => {
                let description = p.optional_text("description");
                done(self.group_description(&p.text("chat")?, description.as_deref()).await?)
            }
            "groupSetting" => {
                let setting = p.text("setting")?;
                done(self.group_setting(&p.text("chat")?, &setting, &params["value"]).await?)
            }
            "groupPicture" => done(self.group_picture(&p.text("chat")?, p.bytes("data")?).await?),
            "groupLeave" => done(self.group_leave(&p.text("chat")?).await?),
            "groupInviteCode" => done(self.group_invite_code(&p.text("chat")?).await?),
            "groupRevokeInvite" => done(self.group_revoke_invite(&p.text("chat")?).await?),
            "groupJoin" => done(self.group_join(&p.text("invite")?).await?),
            "groupInviteInfo" => self.group_invite_info(&p.text("invite")?).await,
            "groupRequests" => self.group_requests(&p.text("chat")?).await,
            "groupRespondRequests" => {
                let approve = p.flag("approve", true);
                self.group_respond_requests(&p.text("chat")?, &p.texts("participants")?, approve).await
            }
            "setProfilePicture" => {
                let target = p.optional_text("target");
                done(self.set_profile_picture(p.bytes("data")?, target.as_deref()).await?)
            }
            "removeProfilePicture" => {
                let target = p.optional_text("target");
                done(self.remove_profile_picture(target.as_deref()).await?)
            }
            "setAbout" => done(self.set_about(&p.text("text")?).await?),
            "fetchAbout" => self.fetch_about(&p.texts("jids")?).await,
            "setPushName" => done(self.set_push_name(&p.text("name")?).await?),
            "businessProfile" => self.business_profile(&p.text("jid")?).await,
            "blocklist" => self.blocklist().await,
            "privacySettings" => self.privacy_settings().await,
            "setPrivacy" => done(self.set_privacy(&p.text("setting")?, &p.text("value")?).await?),
            "lidForPhone" => done(self.lid_for_phone(&p.text("phone")?).await?),
            "phoneForLid" => done(self.phone_for_lid(&p.text("lid")?).await?),
            other => Err(Error::InvalidArgument(format!("unknown method \"{other}\""))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn params() {
        let value = json!({ "chat": 5215511111111u64, "data": "aG9sYQ==", "options": ["a", 2], "flag": true });
        let p = Params(&value);
        assert_eq!(p.text("chat").unwrap(), "5215511111111");
        assert_eq!(p.bytes("data").unwrap(), b"hola");
        assert_eq!(p.texts("options").unwrap(), ["a", "2"]);
        assert!(p.flag("flag", false) && p.flag("other", true));
        assert!(p.text("missing").unwrap_err().to_string().contains("missing parameter"));
    }
}
