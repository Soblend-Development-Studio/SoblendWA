use serde_json::{json, Value};
use soblendwa::{Error, Jid, ParticipantAction, PrivacySetting, PrivacyValue, Result};

use crate::{jid, Session};

fn jids(values: &[String]) -> Result<Vec<Jid>> {
    values.iter().map(|value| jid(value)).collect()
}

fn value_of<T: serde::Serialize>(value: T) -> Result<Value> {
    serde_json::to_value(value).map_err(Error::from)
}

fn invalid(message: String) -> Error {
    Error::InvalidArgument(message)
}

pub fn participant_action(name: &str) -> Result<ParticipantAction> {
    Ok(match name {
        "add" => ParticipantAction::Add,
        "remove" | "kick" => ParticipantAction::Remove,
        "promote" => ParticipantAction::Promote,
        "demote" => ParticipantAction::Demote,
        other => {
            return Err(invalid(format!("unknown participant action \"{other}\", use add, remove, promote or demote")))
        }
    })
}

fn privacy_setting(name: &str) -> Result<PrivacySetting> {
    Ok(match name {
        "lastSeen" | "last" => PrivacySetting::LastSeen,
        "online" => PrivacySetting::Online,
        "profilePicture" | "profile" => PrivacySetting::ProfilePicture,
        "about" | "status" => PrivacySetting::About,
        "readReceipts" | "readreceipts" => PrivacySetting::ReadReceipts,
        "groupsAdd" | "groupadd" => PrivacySetting::GroupsAdd,
        "calls" | "calladd" => PrivacySetting::Calls,
        "messages" => PrivacySetting::Messages,
        other => return Err(invalid(format!("unknown privacy setting \"{other}\""))),
    })
}

fn privacy_value(name: &str) -> Result<PrivacyValue> {
    Ok(match name {
        "everyone" | "all" => PrivacyValue::Everyone,
        "contacts" => PrivacyValue::Contacts,
        "contactsExcept" | "contact_blacklist" => PrivacyValue::ContactsExcept,
        "nobody" | "none" => PrivacyValue::Nobody,
        "matchLastSeen" | "match_last_seen" => PrivacyValue::MatchLastSeen,
        "known" => PrivacyValue::Known,
        other => return Err(invalid(format!("unknown privacy value \"{other}\""))),
    })
}

fn flag(value: &Value, setting: &str) -> Result<bool> {
    value.as_bool().ok_or_else(|| invalid(format!("the group setting \"{setting}\" needs true or false")))
}

impl Session {
    pub async fn group_metadata(&self, chat: &str) -> Result<Value> {
        value_of(self.client().group_metadata(&jid(chat)?).await?)
    }

    pub async fn group_create(&self, subject: &str, participants: &[String]) -> Result<Value> {
        value_of(self.client().create_group(subject, &jids(participants)?).await?)
    }

    pub async fn group_participants(&self, chat: &str, participants: &[String], action: &str) -> Result<Value> {
        let results =
            self.client().update_participants(&jid(chat)?, &jids(participants)?, participant_action(action)?).await?;
        Ok(Value::Array(
            results
                .iter()
                .map(|r| json!({ "jid": r.jid.to_string(), "status": r.status, "ok": r.status == 200 }))
                .collect(),
        ))
    }

    pub async fn group_subject(&self, chat: &str, subject: &str) -> Result<()> {
        self.client().set_group_subject(&jid(chat)?, subject).await
    }

    pub async fn group_description(&self, chat: &str, description: Option<&str>) -> Result<()> {
        self.client().set_group_description(&jid(chat)?, description).await
    }

    pub async fn group_setting(&self, chat: &str, setting: &str, value: &Value) -> Result<()> {
        let group = jid(chat)?;
        let client = self.client();
        match setting {
            "announce" | "announcement" => client.set_group_announce(&group, flag(value, setting)?).await,
            "locked" | "restrict" => client.set_group_locked(&group, flag(value, setting)?).await,
            "joinApproval" => client.set_group_join_approval(&group, flag(value, setting)?).await,
            "memberAdd" => {
                let all = match value {
                    Value::Bool(all) => *all,
                    Value::String(mode) if mode == "all" => true,
                    Value::String(mode) if mode == "admins" => false,
                    _ => return Err(invalid("\"memberAdd\" takes \"all\", \"admins\" or a boolean".into())),
                };
                client.set_group_member_add_mode(&group, all).await
            }
            "ephemeral" | "disappearing" => {
                let seconds = match value {
                    Value::Bool(false) | Value::Null => 0,
                    other => other.as_u64().ok_or_else(|| invalid("\"ephemeral\" takes seconds".into()))? as u32,
                };
                client.set_group_ephemeral(&group, seconds).await
            }
            other => Err(invalid(format!(
                "unknown group setting \"{other}\", use announce, locked, joinApproval, memberAdd or ephemeral"
            ))),
        }
    }

    pub async fn group_picture(&self, chat: &str, image: Vec<u8>) -> Result<()> {
        self.client().set_group_picture(&jid(chat)?, image).await
    }

    pub async fn group_leave(&self, chat: &str) -> Result<()> {
        self.client().leave_group(&jid(chat)?).await
    }

    pub async fn group_invite_code(&self, chat: &str) -> Result<String> {
        self.client().group_invite_code(&jid(chat)?).await
    }

    pub async fn group_revoke_invite(&self, chat: &str) -> Result<String> {
        self.client().revoke_group_invite(&jid(chat)?).await
    }

    pub async fn group_join(&self, invite: &str) -> Result<String> {
        Ok(self.client().join_group(invite).await?.to_string())
    }

    pub async fn group_invite_info(&self, invite: &str) -> Result<Value> {
        value_of(self.client().group_invite_info(invite).await?)
    }

    pub async fn group_requests(&self, chat: &str) -> Result<Value> {
        let requests = self.client().group_join_requests(&jid(chat)?).await?;
        Ok(Value::Array(
            requests
                .iter()
                .map(|r| json!({ "jid": r.jid.to_string(), "method": r.method, "requestedAt": r.requested_at }))
                .collect(),
        ))
    }

    pub async fn group_respond_requests(&self, chat: &str, participants: &[String], approve: bool) -> Result<Value> {
        let results = self.client().respond_join_requests(&jid(chat)?, &jids(participants)?, approve).await?;
        Ok(Value::Array(
            results
                .iter()
                .map(|r| json!({ "jid": r.jid.to_string(), "status": r.status, "ok": r.status == 200 }))
                .collect(),
        ))
    }

    pub async fn set_profile_picture(&self, image: Vec<u8>, target: Option<&str>) -> Result<()> {
        let target = target.map(jid).transpose()?;
        self.client().set_profile_picture(target.as_ref(), image).await
    }

    pub async fn remove_profile_picture(&self, target: Option<&str>) -> Result<()> {
        let target = target.map(jid).transpose()?;
        self.client().remove_profile_picture(target.as_ref()).await
    }

    pub async fn set_about(&self, text: &str) -> Result<()> {
        self.client().set_about(text).await
    }

    pub async fn fetch_about(&self, who: &[String]) -> Result<Value> {
        let found = self.client().fetch_about(&jids(who)?).await?;
        Ok(Value::Array(
            found.iter().map(|a| json!({ "jid": a.jid.to_string(), "text": a.text, "setAt": a.set_at })).collect(),
        ))
    }

    pub async fn set_push_name(&self, name: &str) -> Result<()> {
        self.client().set_push_name(name).await
    }

    pub async fn business_profile(&self, who: &str) -> Result<Value> {
        Ok(match self.client().business_profile(&jid(who)?).await? {
            Some(profile) => json!({
                "jid": profile.jid.map(|j| j.to_string()),
                "description": profile.description,
                "address": profile.address,
                "email": profile.email,
                "website": profile.website,
                "category": profile.category,
            }),
            None => Value::Null,
        })
    }

    pub async fn blocklist(&self) -> Result<Value> {
        let blocked = self.client().blocklist().await?;
        Ok(Value::Array(blocked.iter().map(|j| Value::String(j.to_string())).collect()))
    }

    pub async fn privacy_settings(&self) -> Result<Value> {
        value_of(self.client().privacy_settings().await?)
    }

    pub async fn set_privacy(&self, setting: &str, value: &str) -> Result<()> {
        self.client().set_privacy(privacy_setting(setting)?, privacy_value(value)?).await
    }

    pub async fn lid_for_phone(&self, phone: &str) -> Result<Option<String>> {
        Ok(self.client().lid_for_phone(&jid(phone)?).await?.map(|j| j.to_string()))
    }

    pub async fn phone_for_lid(&self, lid: &str) -> Result<Option<String>> {
        let lid = if lid.contains('@') { lid.parse()? } else { Jid::lid(lid) };
        Ok(self.client().phone_for_lid(&lid).await?.map(|j| j.to_string()))
    }

    pub async fn restart(&self) -> Result<()> {
        self.client().disconnect().await;
        self.client().connect().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn baileys_names() {
        assert_eq!(participant_action("kick").unwrap(), ParticipantAction::Remove);
        assert_eq!(privacy_setting("groupadd").unwrap(), PrivacySetting::GroupsAdd);
        assert_eq!(privacy_value("contact_blacklist").unwrap(), PrivacyValue::ContactsExcept);
        assert_eq!(participant_action("ban").unwrap_err().code(), "INVALID_ARGUMENT");
    }
}
