use std::collections::HashMap;

use super::Client;
use crate::binary::Node;
use crate::error::{Error, Result};
use crate::jid::Jid;
use crate::proto::{FutureProofMessage, Message, ProtocolMessage};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrivacySetting {
    LastSeen,
    Online,
    ProfilePicture,
    About,
    ReadReceipts,
    GroupsAdd,
    Calls,
    Messages,
}

impl PrivacySetting {
    pub fn as_str(self) -> &'static str {
        match self {
            PrivacySetting::LastSeen => "last",
            PrivacySetting::Online => "online",
            PrivacySetting::ProfilePicture => "profile",
            PrivacySetting::About => "status",
            PrivacySetting::ReadReceipts => "readreceipts",
            PrivacySetting::GroupsAdd => "groupadd",
            PrivacySetting::Calls => "calladd",
            PrivacySetting::Messages => "messages",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrivacyValue {
    Everyone,
    Contacts,
    ContactsExcept,
    Nobody,
    MatchLastSeen,
    Known,
}

impl PrivacyValue {
    pub fn as_str(self) -> &'static str {
        match self {
            PrivacyValue::Everyone => "all",
            PrivacyValue::Contacts => "contacts",
            PrivacyValue::ContactsExcept => "contact_blacklist",
            PrivacyValue::Nobody => "none",
            PrivacyValue::MatchLastSeen => "match_last_seen",
            PrivacyValue::Known => "known",
        }
    }
}

#[derive(Clone, Debug)]
pub struct About {
    pub jid: Jid,
    pub text: Option<String>,
    pub set_at: i64,
}

#[derive(Clone, Debug, Default)]
pub struct BusinessProfile {
    pub jid: Option<Jid>,
    pub description: String,
    pub address: Option<String>,
    pub email: Option<String>,
    pub website: Option<String>,
    pub category: Option<String>,
}

impl Client {
    pub(super) fn server_iq(&self, xmlns: &'static str, kind: &'static str) -> Node {
        Node::new("iq").attr("to", Jid::server()).attr("type", kind).attr("xmlns", xmlns)
    }

    fn profile_target(&self, target: Option<&Jid>) -> Option<Jid> {
        target.map(Jid::to_non_device).filter(|jid| !self.is_me(jid))
    }

    pub async fn profile_picture_url(&self, jid: &Jid, full_size: bool) -> Result<Option<String>> {
        let mut picture =
            Node::new("picture").attr("type", if full_size { "image" } else { "preview" }).attr("query", "url");
        if let Some(token) = self.profile_tc_token_node(jid).await {
            picture.push_child(token);
        }
        let request =
            self.server_iq("w:profile:picture", "get").attr("target", jid.to_non_device()).child_node(picture);
        match self.query(request).await {
            Ok(response) => Ok(response.child("picture").and_then(|p| p.attr_string("url"))),
            Err(Error::Server { code: 401 | 404, .. }) => Ok(None),
            Err(err) => Err(err),
        }
    }

    pub async fn set_profile_picture(&self, target: Option<&Jid>, image: Vec<u8>) -> Result<()> {
        let jpeg = tokio::task::spawn_blocking(move || super::enrich::profile_picture(&image))
            .await
            .map_err(|err| Error::Media(err.to_string()))??;
        let request = self
            .server_iq("w:profile:picture", "set")
            .attr_opt("target", self.profile_target(target))
            .child_node(Node::new("picture").attr("type", "image").bytes(jpeg));
        self.query(request).await.map(drop)
    }

    pub async fn remove_profile_picture(&self, target: Option<&Jid>) -> Result<()> {
        let request = self.server_iq("w:profile:picture", "set").attr_opt("target", self.profile_target(target));
        self.query(request).await.map(drop)
    }

    pub async fn set_about(&self, text: &str) -> Result<()> {
        let request = self.server_iq("status", "set").child_node(Node::new("status").bytes(text.as_bytes().to_vec()));
        self.query(request).await.map(drop)
    }

    pub async fn fetch_about(&self, jids: &[Jid]) -> Result<Vec<About>> {
        let users = jids.iter().map(|jid| Node::new("user").attr("jid", jid.to_non_device())).collect();
        let response = self.query(self.usync("interactive", vec![Node::new("status")], users)).await?;
        Ok(response
            .path(&["usync", "list"])
            .map(Node::nodes)
            .unwrap_or_default()
            .iter()
            .filter_map(|user| {
                let jid = user.attr_jid("jid")?;
                let status = user.child("status")?;
                let text = status.content_string().filter(|t| !t.is_empty());
                Some(About { jid, text, set_at: status.attr_i64("t").unwrap_or_default() })
            })
            .collect())
    }

    async fn resolve_lid(&self, phone: &Jid) -> Result<Option<Jid>> {
        if let Some(lid) = self.lid_for_phone(phone).await? {
            return Ok(Some(lid));
        }
        let users = vec![Node::new("user").attr("jid", phone.to_non_device())];
        let response = self.query(self.usync("message", vec![Node::new("lid")], users)).await?;
        for user in response.path(&["usync", "list"]).map(Node::nodes).unwrap_or_default() {
            if let (Some(pn), Some(lid)) = (user.attr_jid("jid"), user.child("lid").and_then(|l| l.attr_jid("val"))) {
                self.remember_lid(&pn, &lid).await;
            }
        }
        self.lid_for_phone(phone).await
    }

    async fn update_block(&self, jid: &Jid, block: bool) -> Result<()> {
        let user = jid.to_non_device();
        let (lid, phone) = if user.is_lid() {
            (Some(user.clone()), self.phone_for_lid(&user).await?)
        } else {
            (self.resolve_lid(&user).await?, Some(user.clone()))
        };
        let action = if block { "block" } else { "unblock" };
        let item = match (lid, phone) {
            (Some(lid), phone) => {
                Node::new("item").attr("action", action).attr("jid", lid).attr_opt("pn_jid", phone.filter(|_| block))
            }
            (None, _) => Node::new("item").attr("action", action).attr("jid", user),
        };
        self.query(self.server_iq("blocklist", "set").child_node(item)).await.map(drop)
    }

    pub async fn block(&self, jid: &Jid) -> Result<()> {
        self.update_block(jid, true).await
    }

    pub async fn unblock(&self, jid: &Jid) -> Result<()> {
        self.update_block(jid, false).await
    }

    pub async fn blocklist(&self) -> Result<Vec<Jid>> {
        let response = self.query(self.server_iq("blocklist", "get")).await?;
        Ok(response
            .child("list")
            .map(|l| l.children_by("item").filter_map(|i| i.attr_jid("jid")).collect())
            .unwrap_or_default())
    }

    pub async fn privacy_settings(&self) -> Result<HashMap<String, String>> {
        let response = self.query(self.server_iq("privacy", "get").child_node(Node::new("privacy"))).await?;
        Ok(response
            .child("privacy")
            .map(|p| {
                p.children_by("category")
                    .filter_map(|c| Some((c.attr_string("name")?, c.attr_string("value")?)))
                    .collect()
            })
            .unwrap_or_default())
    }

    pub async fn set_privacy(&self, setting: PrivacySetting, value: PrivacyValue) -> Result<()> {
        let category = Node::new("category").attr("name", setting.as_str()).attr("value", value.as_str());
        let request = self.server_iq("privacy", "set").child_node(Node::new("privacy").child_node(category));
        self.query(request).await.map(drop)
    }

    pub async fn set_default_disappearing(&self, seconds: u32) -> Result<()> {
        let request = self
            .server_iq("disappearing_mode", "set")
            .child_node(Node::new("disappearing_mode").attr("duration", seconds));
        self.query(request).await.map(drop)
    }

    pub async fn set_disappearing(&self, chat: &Jid, seconds: u32) -> Result<()> {
        if chat.is_group() {
            return self.set_group_ephemeral(chat, seconds).await;
        }
        let setting = Message {
            protocol_message: Some(Box::new(ProtocolMessage {
                r#type: Some(ProtocolMessage::EPHEMERAL_SETTING),
                ephemeral_expiration: Some(seconds),
                ..Default::default()
            })),
            ..Default::default()
        };
        let wrapped = Message {
            ephemeral_message: Some(Box::new(FutureProofMessage { message: Some(Box::new(setting)) })),
            ..Default::default()
        };
        self.send_message(chat, wrapped).await.map(drop)
    }

    pub async fn business_profile(&self, jid: &Jid) -> Result<Option<BusinessProfile>> {
        let request = self.server_iq("w:biz", "get").child_node(
            Node::new("business_profile")
                .attr("v", "244")
                .child_node(Node::new("profile").attr("jid", jid.to_non_device())),
        );
        let response = self.query(request).await?;
        let Some(profile) = response.path(&["business_profile", "profile"]) else { return Ok(None) };
        let text = |tag: &str| profile.child(tag).and_then(Node::content_string);
        Ok(Some(BusinessProfile {
            jid: profile.attr_jid("jid"),
            description: text("description").unwrap_or_default(),
            address: text("address"),
            email: text("email"),
            website: text("website"),
            category: profile.path(&["categories", "category"]).and_then(Node::content_string),
        }))
    }
}
