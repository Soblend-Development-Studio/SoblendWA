use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use prost::Message as _;
use serde_json::{json, Value};

use super::Client;
use crate::binary::Node;
use crate::error::{Error, Result};
use crate::jid::Jid;
use crate::proto::Message;

mod query_id {
    pub const CREATE: &str = "8823471724422422";
    pub const UPDATE: &str = "24250201037901610";
    pub const METADATA: &str = "6563316087068696";
    pub const SUBSCRIBERS: &str = "9783111038412085";
    pub const FOLLOW: &str = "24404358912487870";
    pub const UNFOLLOW: &str = "9767147403369991";
    pub const MUTE: &str = "29766401636284406";
    pub const UNMUTE: &str = "9864994326891137";
    pub const ADMIN_COUNT: &str = "7130823597031706";
    pub const CHANGE_OWNER: &str = "7341777602580933";
    pub const DEMOTE: &str = "6551828931592903";
    pub const DELETE: &str = "30062808666639665";
}

#[derive(Clone, Debug, Default)]
pub struct NewsletterInfo {
    pub jid: Jid,
    pub name: Option<String>,
    pub description: Option<String>,
    pub invite: Option<String>,
    pub subscribers: Option<u64>,
    pub verified: bool,
    pub picture_path: Option<String>,
    pub muted: bool,
    pub role: Option<String>,
    pub created: Option<i64>,
    pub raw: Value,
}

impl NewsletterInfo {
    fn parse(value: Value) -> Option<Self> {
        let root = if value.get("id").is_some() { &value } else { value.get("result")? };
        let thread = &root["thread_metadata"];
        let viewer = &root["viewer_metadata"];
        let text = |v: &Value| v.get("text").and_then(Value::as_str).map(str::to_owned);
        let number = |v: &Value| v.as_str().and_then(|s| s.parse().ok()).or_else(|| v.as_u64());
        Some(NewsletterInfo {
            jid: root["id"].as_str()?.parse().ok()?,
            name: text(&thread["name"]),
            description: text(&thread["description"]),
            invite: thread["invite"].as_str().map(str::to_owned),
            subscribers: number(&thread["subscribers_count"]),
            verified: thread["verification"].as_str() == Some("VERIFIED"),
            picture_path: thread["picture"]["direct_path"].as_str().map(str::to_owned),
            muted: viewer["mute"].as_str() == Some("ON"),
            role: viewer["role"].as_str().map(str::to_owned),
            created: number(&thread["creation_time"]).map(|n| n as i64),
            raw: value.clone(),
        })
    }
}

#[derive(Clone, Debug)]
pub struct NewsletterMessage {
    pub server_id: String,
    pub id: Option<String>,
    pub timestamp: i64,
    pub views: Option<u64>,
    pub message: Option<Message>,
    pub reactions: Vec<(String, u64)>,
}

impl Client {
    pub(super) async fn mex(&self, query_id: &'static str, variables: Value, path: &str) -> Result<Value> {
        let body = serde_json::to_vec(&json!({ "variables": variables }))?;
        let request = Node::new("iq")
            .attr("to", Jid::server())
            .attr("type", "get")
            .attr("xmlns", "w:mex")
            .child_node(Node::new("query").attr("query_id", query_id).bytes(body));
        let response = self.query(request).await?;
        let raw = response
            .child("result")
            .and_then(Node::content_bytes)
            .ok_or_else(|| Error::other("graphql answer without a result"))?;
        let data: Value = serde_json::from_slice(raw)?;
        if let Some(errors) = data["errors"].as_array().filter(|e| !e.is_empty()) {
            let text = errors.iter().filter_map(|e| e["message"].as_str()).collect::<Vec<_>>().join(", ");
            let code = errors[0]["extensions"]["error_code"].as_u64().unwrap_or(400) as u16;
            return Err(Error::Server { code, text });
        }
        let payload = if path.is_empty() { &data["data"] } else { &data["data"][path] };
        if payload.is_null() {
            return Err(Error::other(format!("graphql answer for {path} is empty")));
        }
        Ok(payload.clone())
    }

    pub async fn newsletter_create(&self, name: &str, description: Option<&str>) -> Result<NewsletterInfo> {
        let variables = json!({ "input": { "name": name, "description": description } });
        let raw = self.mex(query_id::CREATE, variables, "xwa2_newsletter_create").await?;
        NewsletterInfo::parse(raw).ok_or_else(|| Error::other("unexpected newsletter create answer"))
    }

    pub async fn newsletter_info(&self, jid: &Jid) -> Result<NewsletterInfo> {
        self.newsletter_lookup("JID", &jid.to_string()).await
    }

    pub async fn newsletter_info_by_invite(&self, invite: &str) -> Result<NewsletterInfo> {
        let code = invite.rsplit('/').next().unwrap_or(invite);
        self.newsletter_lookup("INVITE", code).await
    }

    async fn newsletter_lookup(&self, kind: &str, key: &str) -> Result<NewsletterInfo> {
        let variables = json!({
            "fetch_creation_time": true,
            "fetch_full_image": true,
            "fetch_viewer_metadata": true,
            "input": { "key": key, "type": kind }
        });
        let raw = self.mex(query_id::METADATA, variables, "xwa2_newsletter").await?;
        NewsletterInfo::parse(raw).ok_or_else(|| Error::other("unexpected newsletter metadata answer"))
    }

    async fn newsletter_action(&self, jid: &Jid, id: &'static str, path: &str) -> Result<()> {
        self.mex(id, json!({ "newsletter_id": jid.to_string() }), path).await.map(drop)
    }

    pub async fn newsletter_follow(&self, jid: &Jid) -> Result<()> {
        self.newsletter_action(jid, query_id::FOLLOW, "xwa2_newsletter_join_v2").await
    }

    pub async fn newsletter_unfollow(&self, jid: &Jid) -> Result<()> {
        self.newsletter_action(jid, query_id::UNFOLLOW, "xwa2_newsletter_leave_v2").await
    }

    pub async fn newsletter_mute(&self, jid: &Jid, muted: bool) -> Result<()> {
        match muted {
            true => self.newsletter_action(jid, query_id::MUTE, "xwa2_newsletter_mute_v2").await,
            false => self.newsletter_action(jid, query_id::UNMUTE, "xwa2_newsletter_unmute_v2").await,
        }
    }

    pub async fn newsletter_delete(&self, jid: &Jid) -> Result<()> {
        self.newsletter_action(jid, query_id::DELETE, "xwa2_newsletter_delete_v2").await
    }

    pub async fn newsletter_subscribers(&self, jid: &Jid) -> Result<u64> {
        let raw = self
            .mex(query_id::SUBSCRIBERS, json!({ "newsletter_id": jid.to_string() }), "xwa2_newsletter_subscribers")
            .await?;
        Ok(raw["subscribers"].as_u64().or_else(|| raw["subscribers"].as_str()?.parse().ok()).unwrap_or_default())
    }

    pub async fn newsletter_admin_count(&self, jid: &Jid) -> Result<u64> {
        let raw = self
            .mex(query_id::ADMIN_COUNT, json!({ "newsletter_id": jid.to_string() }), "xwa2_newsletter_admin")
            .await?;
        Ok(raw["admin_count"].as_u64().unwrap_or_default())
    }

    pub async fn newsletter_change_owner(&self, jid: &Jid, new_owner: &Jid) -> Result<()> {
        let variables = json!({ "newsletter_id": jid.to_string(), "user_id": new_owner.to_non_device().to_string() });
        self.mex(query_id::CHANGE_OWNER, variables, "xwa2_newsletter_change_owner").await.map(drop)
    }

    pub async fn newsletter_demote(&self, jid: &Jid, admin: &Jid) -> Result<()> {
        let variables = json!({ "newsletter_id": jid.to_string(), "user_id": admin.to_non_device().to_string() });
        self.mex(query_id::DEMOTE, variables, "xwa2_newsletter_demote").await.map(drop)
    }

    pub async fn newsletter_update(
        &self,
        jid: &Jid,
        name: Option<&str>,
        description: Option<&str>,
        picture: Option<Vec<u8>>,
    ) -> Result<()> {
        let mut updates = serde_json::Map::new();
        if let Some(name) = name {
            updates.insert("name".into(), json!(name));
        }
        if let Some(description) = description {
            updates.insert("description".into(), json!(description));
        }
        if let Some(image) = picture {
            let encoded = if image.is_empty() {
                String::new()
            } else {
                let jpeg = tokio::task::spawn_blocking(move || super::enrich::profile_picture(&image))
                    .await
                    .map_err(|err| Error::Media(err.to_string()))??;
                STANDARD.encode(jpeg)
            };
            updates.insert("picture".into(), json!(encoded));
        }
        updates.insert("settings".into(), Value::Null);
        let variables = json!({ "newsletter_id": jid.to_string(), "updates": updates });
        self.mex(query_id::UPDATE, variables, "xwa2_newsletter_update").await.map(drop)
    }

    pub async fn newsletter_react(&self, jid: &Jid, server_id: &str, reaction: Option<&str>) -> Result<()> {
        let mut stanza = Node::new("message")
            .attr("to", jid.clone())
            .attr("type", "reaction")
            .attr("server_id", server_id.to_owned())
            .attr("id", self.generate_tag());
        if reaction.is_none() {
            stanza.set_attr("edit", "7");
        }
        stanza.push_child(Node::new("reaction").attr_opt("code", reaction.map(str::to_owned)));
        self.send_node(&stanza)
    }

    pub async fn newsletter_messages(
        &self,
        jid: &Jid,
        count: u32,
        before: Option<u64>,
    ) -> Result<Vec<NewsletterMessage>> {
        let request =
            Node::new("iq").attr("to", Jid::server()).attr("type", "get").attr("xmlns", "newsletter").child_node(
                Node::new("messages")
                    .attr("type", "jid")
                    .attr("jid", jid.clone())
                    .attr("count", count)
                    .attr_opt("before", before),
            );
        let response = self.query(request).await?;
        let list = response.child("messages").map(Node::nodes).unwrap_or_default();
        Ok(list
            .iter()
            .filter(|n| n.tag == "message")
            .filter_map(|n| {
                Some(NewsletterMessage {
                    server_id: n.attr_string("server_id")?,
                    id: n.attr_string("id"),
                    timestamp: n.attr_i64("t").unwrap_or_default(),
                    views: n.child("views_count").and_then(|v| v.attr_u64("count")),
                    message: n.child_bytes("plaintext").and_then(|raw| Message::decode(raw).ok()),
                    reactions: n
                        .child("reactions")
                        .map(|r| {
                            r.children_by("reaction")
                                .filter_map(|x| Some((x.attr_string("code")?, x.attr_u64("count").unwrap_or(0))))
                                .collect()
                        })
                        .unwrap_or_default(),
                })
            })
            .collect())
    }

    pub async fn newsletter_live_updates(&self, jid: &Jid) -> Result<Option<u64>> {
        let request = Node::new("iq")
            .attr("to", jid.clone())
            .attr("type", "set")
            .attr("xmlns", "newsletter")
            .child_node(Node::new("live_updates"));
        let response = self.query(request).await?;
        Ok(response.child("live_updates").and_then(|n| n.attr_u64("duration")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_newsletter_metadata() {
        let raw = json!({
            "id": "120363000000000001@newsletter",
            "thread_metadata": {
                "name": { "text": "Soblend" },
                "description": { "text": "Canal oficial" },
                "invite": "AbCdEf",
                "subscribers_count": "1500",
                "verification": "VERIFIED",
                "creation_time": "1700000000",
                "picture": { "direct_path": "/v/t61/abc" }
            },
            "viewer_metadata": { "mute": "OFF", "role": "OWNER" }
        });
        let info = NewsletterInfo::parse(raw).unwrap();
        assert_eq!(info.jid.to_string(), "120363000000000001@newsletter");
        assert_eq!(info.name.as_deref(), Some("Soblend"));
        assert_eq!(info.subscribers, Some(1500));
        assert!(info.verified && !info.muted);
        assert_eq!(info.role.as_deref(), Some("OWNER"));
        assert_eq!(info.created, Some(1_700_000_000));
    }
}
