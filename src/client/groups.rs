use serde::{Deserialize, Serialize};

use super::Client;
use crate::binary::Node;
use crate::crypto;
use crate::error::{Error, Result};
use crate::jid::Jid;
use crate::proto::GroupInviteMessage;
use crate::store::KeyKind;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GroupParticipant {
    pub jid: Jid,
    pub phone_number: Option<Jid>,
    pub lid: Option<Jid>,
    pub is_admin: bool,
    pub is_super_admin: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GroupInfo {
    pub jid: Jid,
    pub subject: String,
    pub subject_owner: Option<Jid>,
    pub subject_time: i64,
    pub owner: Option<Jid>,
    pub creation: i64,
    pub description: Option<String>,
    pub description_id: Option<String>,
    pub announce: bool,
    pub locked: bool,
    pub ephemeral: u32,
    pub addressing_mode: String,
    #[serde(default)]
    pub join_approval: bool,
    #[serde(default)]
    pub members_can_add: bool,
    #[serde(default)]
    pub linked_parent: Option<Jid>,
    #[serde(default)]
    pub is_community: bool,
    #[serde(default)]
    pub is_community_announce: bool,
    pub participants: Vec<GroupParticipant>,
}

#[derive(Clone, Debug)]
pub struct JoinRequest {
    pub jid: Jid,
    pub method: Option<String>,
    pub requested_at: i64,
}

impl GroupInfo {
    pub fn is_lid_addressed(&self) -> bool {
        self.addressing_mode == "lid"
    }

    pub fn admins(&self) -> impl Iterator<Item = &GroupParticipant> {
        self.participants.iter().filter(|p| p.is_admin)
    }

    pub fn participant(&self, jid: &Jid) -> Option<&GroupParticipant> {
        self.participants.iter().find(|p| {
            p.jid.same_user(jid)
                || p.phone_number.as_ref().is_some_and(|n| n.same_user(jid))
                || p.lid.as_ref().is_some_and(|l| l.same_user(jid))
        })
    }

    pub fn is_admin(&self, jid: &Jid) -> bool {
        self.participant(jid).is_some_and(|p| p.is_admin)
    }

    pub(crate) fn parse(node: &Node) -> Result<Self> {
        let id = node.attr_string("id").ok_or_else(|| Error::binary("group without id"))?;
        let jid = if id.contains('@') { id.parse()? } else { Jid::group(id) };
        let description = node.child("description");
        Ok(GroupInfo {
            jid,
            subject: node.attr_string("subject").unwrap_or_default(),
            subject_owner: node.attr_jid("s_o"),
            subject_time: node.attr_i64("s_t").unwrap_or_default(),
            owner: node.attr_jid("creator"),
            creation: node.attr_i64("creation").unwrap_or_default(),
            description: description.and_then(|d| d.child("body")).and_then(Node::content_string),
            description_id: description.and_then(|d| d.attr_string("id")),
            announce: node.child("announcement").is_some(),
            locked: node.child("locked").is_some(),
            ephemeral: node.child("ephemeral").and_then(|e| e.attr_u64("expiration")).unwrap_or(0) as u32,
            addressing_mode: node.attr_string("addressing_mode").unwrap_or_else(|| "pn".into()),
            join_approval: node.child("membership_approval_mode").is_some(),
            members_can_add: node.child("member_add_mode").and_then(Node::content_string).as_deref()
                == Some("all_member_add"),
            linked_parent: node.child("linked_parent").and_then(|p| p.attr_jid("jid")),
            is_community: node.child("parent").is_some(),
            is_community_announce: node.child("default_sub_group").is_some(),
            participants: node
                .children_by("participant")
                .filter_map(|p| {
                    let kind = p.attr_str("type");
                    Some(GroupParticipant {
                        jid: p.attr_jid("jid")?,
                        phone_number: p.attr_jid("phone_number"),
                        lid: p.attr_jid("lid"),
                        is_admin: matches!(kind, Some("admin" | "superadmin")),
                        is_super_admin: kind == Some("superadmin"),
                    })
                })
                .collect(),
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParticipantAction {
    Add,
    Remove,
    Promote,
    Demote,
}

impl ParticipantAction {
    fn tag(self) -> &'static str {
        match self {
            ParticipantAction::Add => "add",
            ParticipantAction::Remove => "remove",
            ParticipantAction::Promote => "promote",
            ParticipantAction::Demote => "demote",
        }
    }
}

#[derive(Clone, Debug)]
pub struct ParticipantResult {
    pub jid: Jid,
    pub status: u16,
}

impl Client {
    fn group_iq(&self, to: Jid, kind: &'static str, body: Node) -> Node {
        Node::new("iq").attr("to", to).attr("type", kind).attr("xmlns", "w:g2").child_node(body)
    }

    pub async fn group_metadata(&self, group: &Jid) -> Result<GroupInfo> {
        let request = self.group_iq(group.clone(), "get", Node::new("query").attr("request", "interactive"));
        let response = self.query(request).await?;
        let mut info = GroupInfo::parse(response.child("group").ok_or_else(|| Error::binary("missing group node"))?)?;
        info.jid = group.clone();
        self.remember_group(&info).await;
        Ok(info)
    }

    pub async fn group_info(&self, group: &Jid) -> Result<GroupInfo> {
        if let Some(info) = self.inner.groups.get(group) {
            return Ok(info.clone());
        }
        let stored = self.inner.store.get(KeyKind::GroupMetadata, &group.to_string()).await?;
        if let Some(info) = stored.and_then(|raw| serde_json::from_slice::<GroupInfo>(&raw).ok()) {
            self.inner.groups.insert(group.clone(), info.clone());
            return Ok(info);
        }
        self.group_metadata(group).await
    }

    async fn remember_group(&self, info: &GroupInfo) {
        self.inner.groups.insert(info.jid.clone(), info.clone());
        match serde_json::to_vec(info) {
            Ok(raw) => {
                if let Err(err) = self.inner.store.set(KeyKind::GroupMetadata, &info.jid.to_string(), raw).await {
                    tracing::debug!(%err, "failed to persist group metadata");
                }
            }
            Err(err) => tracing::debug!(%err, "failed to serialize group metadata"),
        }
    }

    pub(crate) async fn forget_group(&self, group: &Jid) {
        self.inner.groups.remove(group);
        let _ = self.inner.store.remove(KeyKind::GroupMetadata, &group.to_string()).await;
    }

    pub(crate) fn refresh_group_later(&self, group: Jid) {
        let client = self.clone();
        tokio::spawn(async move {
            if let Err(err) = client.group_metadata(&group).await {
                tracing::debug!(%group, %err, "background group refresh failed");
            }
        });
    }

    pub async fn joined_groups(&self) -> Result<Vec<GroupInfo>> {
        let body = Node::new("participating").children([Node::new("participants"), Node::new("description")]);
        let response = self.query(self.group_iq(Jid::group_server(), "get", body)).await?;
        let groups: Vec<GroupInfo> = response
            .child("groups")
            .map(|g| g.children_by("group").filter_map(|n| GroupInfo::parse(n).ok()).collect())
            .unwrap_or_default();
        for group in &groups {
            self.remember_group(group).await;
        }
        Ok(groups)
    }

    pub async fn create_group(&self, subject: &str, participants: &[Jid]) -> Result<GroupInfo> {
        let body = Node::new("create")
            .attr("subject", subject.to_owned())
            .attr("key", hex::encode_upper(crypto::random_array::<8>()))
            .children(participants.iter().map(|p| Node::new("participant").attr("jid", p.to_non_device())));
        let response = self.query(self.group_iq(Jid::group_server(), "set", body)).await?;
        let info = GroupInfo::parse(response.child("group").ok_or_else(|| Error::binary("missing group node"))?)?;
        self.remember_group(&info).await;
        Ok(info)
    }

    pub async fn update_participants(
        &self,
        group: &Jid,
        participants: &[Jid],
        action: ParticipantAction,
    ) -> Result<Vec<ParticipantResult>> {
        let body = Node::new(action.tag())
            .children(participants.iter().map(|p| Node::new("participant").attr("jid", p.to_non_device())));
        let response = self.query(self.group_iq(group.clone(), "set", body)).await?;
        self.forget_group(group).await;
        if action == ParticipantAction::Remove {
            self.rotate_sender_key(group).await?;
        }
        Ok(response
            .child(action.tag())
            .map(|n| {
                n.children_by("participant")
                    .filter_map(|p| {
                        Some(ParticipantResult {
                            jid: p.attr_jid("jid")?,
                            status: p.attr_u64("error").unwrap_or(200) as u16,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default())
    }

    pub async fn set_group_subject(&self, group: &Jid, subject: &str) -> Result<()> {
        self.query(self.group_iq(group.clone(), "set", Node::new("subject").text(subject.to_owned()))).await?;
        self.forget_group(group).await;
        Ok(())
    }

    pub async fn set_group_description(&self, group: &Jid, description: Option<&str>) -> Result<()> {
        let previous = self.group_metadata(group).await?.description_id;
        let mut body = Node::new("description").attr_opt("prev", previous);
        match description {
            Some(text) => {
                body.set_attr("id", hex::encode_upper(crypto::random_array::<8>()));
                body.push_child(Node::new("body").text(text.to_owned()));
            }
            None => body.set_attr("delete", "true"),
        }
        self.query(self.group_iq(group.clone(), "set", body)).await?;
        self.forget_group(group).await;
        Ok(())
    }

    pub async fn set_group_announce(&self, group: &Jid, admins_only: bool) -> Result<()> {
        let tag = if admins_only { "announcement" } else { "not_announcement" };
        self.query(self.group_iq(group.clone(), "set", Node::new(tag))).await?;
        self.forget_group(group).await;
        Ok(())
    }

    pub async fn set_group_locked(&self, group: &Jid, locked: bool) -> Result<()> {
        let tag = if locked { "locked" } else { "unlocked" };
        self.query(self.group_iq(group.clone(), "set", Node::new(tag))).await?;
        self.forget_group(group).await;
        Ok(())
    }

    pub async fn set_group_ephemeral(&self, group: &Jid, seconds: u32) -> Result<()> {
        let body =
            if seconds == 0 { Node::new("not_ephemeral") } else { Node::new("ephemeral").attr("expiration", seconds) };
        self.query(self.group_iq(group.clone(), "set", body)).await?;
        self.forget_group(group).await;
        Ok(())
    }

    pub async fn leave_group(&self, group: &Jid) -> Result<()> {
        let body = Node::new("leave").child_node(Node::new("group").attr("id", group.clone()));
        self.query(self.group_iq(Jid::group_server(), "set", body)).await?;
        self.forget_group(group).await;
        Ok(())
    }

    pub async fn group_invite_code(&self, group: &Jid) -> Result<String> {
        let response = self.query(self.group_iq(group.clone(), "get", Node::new("invite"))).await?;
        response.child("invite").and_then(|i| i.attr_string("code")).ok_or_else(|| Error::binary("missing invite code"))
    }

    pub async fn revoke_group_invite(&self, group: &Jid) -> Result<String> {
        let response = self.query(self.group_iq(group.clone(), "set", Node::new("invite"))).await?;
        response.child("invite").and_then(|i| i.attr_string("code")).ok_or_else(|| Error::binary("missing invite code"))
    }

    pub async fn join_group(&self, invite: &str) -> Result<Jid> {
        let code = invite.rsplit('/').next().unwrap_or(invite).to_owned();
        let response =
            self.query(self.group_iq(Jid::group_server(), "set", Node::new("invite").attr("code", code))).await?;
        response.child("group").and_then(|g| g.attr_jid("jid")).ok_or_else(|| Error::binary("missing joined group"))
    }

    pub async fn group_invite_info(&self, invite: &str) -> Result<GroupInfo> {
        let code = invite.rsplit('/').next().unwrap_or(invite).to_owned();
        let response =
            self.query(self.group_iq(Jid::group_server(), "get", Node::new("invite").attr("code", code))).await?;
        GroupInfo::parse(response.child("group").ok_or_else(|| Error::binary("missing group node"))?)
    }

    pub async fn accept_group_invite(&self, from: &Jid, invite: &GroupInviteMessage) -> Result<Jid> {
        let group: Jid = invite.group_jid.as_deref().ok_or_else(|| Error::other("invite without group"))?.parse()?;
        let accept = Node::new("accept")
            .attr("code", invite.invite_code.clone().unwrap_or_default())
            .attr("expiration", invite.invite_expiration.unwrap_or_default())
            .attr("admin", from.to_non_device());
        let response = self.query(self.group_iq(group.clone(), "set", accept)).await?;
        Ok(response.attr_jid("from").unwrap_or(group))
    }

    pub async fn group_join_requests(&self, group: &Jid) -> Result<Vec<JoinRequest>> {
        let response =
            self.query(self.group_iq(group.clone(), "get", Node::new("membership_approval_requests"))).await?;
        Ok(response
            .child("membership_approval_requests")
            .map(|list| {
                list.children_by("membership_approval_request")
                    .filter_map(|request| {
                        Some(JoinRequest {
                            jid: request.attr_jid("jid")?,
                            method: request.attr_string("request_method"),
                            requested_at: request.attr_i64("request_time").unwrap_or_default(),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default())
    }

    pub async fn respond_join_requests(
        &self,
        group: &Jid,
        participants: &[Jid],
        approve: bool,
    ) -> Result<Vec<ParticipantResult>> {
        let action = if approve { "approve" } else { "reject" };
        let body = Node::new("membership_requests_action").child_node(
            Node::new(action)
                .children(participants.iter().map(|p| Node::new("participant").attr("jid", p.to_non_device()))),
        );
        let response = self.query(self.group_iq(group.clone(), "set", body)).await?;
        self.forget_group(group).await;
        Ok(response
            .path(&["membership_requests_action", action])
            .map(|n| {
                n.children_by("participant")
                    .filter_map(|p| {
                        Some(ParticipantResult {
                            jid: p.attr_jid("jid")?,
                            status: p.attr_u64("error").unwrap_or(200) as u16,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default())
    }

    pub async fn set_group_join_approval(&self, group: &Jid, enabled: bool) -> Result<()> {
        let body = Node::new("membership_approval_mode")
            .child_node(Node::new("group_join").attr("state", if enabled { "on" } else { "off" }));
        self.query(self.group_iq(group.clone(), "set", body)).await?;
        self.forget_group(group).await;
        Ok(())
    }

    pub async fn set_group_member_add_mode(&self, group: &Jid, all_members: bool) -> Result<()> {
        let mode = if all_members { "all_member_add" } else { "admin_add" };
        self.query(self.group_iq(group.clone(), "set", Node::new("member_add_mode").text(mode))).await?;
        self.forget_group(group).await;
        Ok(())
    }

    pub async fn set_group_picture(&self, group: &Jid, image: Vec<u8>) -> Result<()> {
        self.set_profile_picture(Some(group), image).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn group_info_survives_persistence() {
        let node = Node::new("group")
            .attr("id", "120363025246125486")
            .attr("subject", "Soblend")
            .attr("addressing_mode", "lid")
            .children([
                Node::new("participant").attr("jid", Jid::lid("111")).attr("type", "superadmin"),
                Node::new("participant").attr("jid", Jid::lid("222")),
            ]);
        let info = GroupInfo::parse(&node).unwrap();
        let restored: GroupInfo = serde_json::from_slice(&serde_json::to_vec(&info).unwrap()).unwrap();
        assert_eq!(restored.jid, Jid::group("120363025246125486"));
        assert!(restored.is_lid_addressed());
        assert!(restored.is_admin(&Jid::lid("111")));
        assert_eq!(restored.participants.len(), 2);
    }
}
