use super::groups::GroupInfo;
use super::Client;
use crate::binary::Node;
use crate::crypto;
use crate::error::{Error, Result};
use crate::jid::Jid;

#[derive(Clone, Debug)]
pub struct LinkedGroup {
    pub jid: Jid,
    pub subject: String,
    pub owner: Option<Jid>,
    pub created: i64,
    pub size: Option<u64>,
}

impl Client {
    fn community_iq(&self, to: Jid, kind: &'static str, body: Node) -> Node {
        Node::new("iq").attr("to", to).attr("type", kind).attr("xmlns", "w:g2").child_node(body)
    }

    pub async fn create_community(&self, subject: &str, description: &str) -> Result<GroupInfo> {
        let body = Node::new("create").attr("subject", subject.to_owned()).children([
            Node::new("description")
                .attr("id", hex::encode_upper(crypto::random_array::<6>()))
                .child_node(Node::new("body").bytes(description.as_bytes().to_vec())),
            Node::new("parent").attr("default_membership_approval_mode", "request_required"),
            Node::new("allow_non_admin_sub_group_creation"),
            Node::new("create_general_chat"),
        ]);
        let response = self.query(self.community_iq(Jid::group_server(), "set", body)).await?;
        GroupInfo::parse(response.child("group").ok_or_else(|| Error::binary("missing community node"))?)
    }

    pub async fn create_community_group(
        &self,
        community: &Jid,
        subject: &str,
        participants: &[Jid],
    ) -> Result<GroupInfo> {
        let mut children: Vec<Node> =
            participants.iter().map(|p| Node::new("participant").attr("jid", p.to_non_device())).collect();
        children.push(Node::new("linked_parent").attr("jid", community.clone()));
        let body = Node::new("create")
            .attr("subject", subject.to_owned())
            .attr("key", hex::encode_upper(crypto::random_array::<8>()))
            .children(children);
        let response = self.query(self.community_iq(Jid::group_server(), "set", body)).await?;
        GroupInfo::parse(response.child("group").ok_or_else(|| Error::binary("missing group node"))?)
    }

    pub async fn link_group(&self, community: &Jid, group: &Jid) -> Result<()> {
        let body = Node::new("links").child_node(
            Node::new("link").attr("link_type", "sub_group").child_node(Node::new("group").attr("jid", group.clone())),
        );
        self.query(self.community_iq(community.clone(), "set", body)).await?;
        self.forget_group(group).await;
        Ok(())
    }

    pub async fn unlink_group(&self, community: &Jid, group: &Jid) -> Result<()> {
        let body = Node::new("unlink")
            .attr("unlink_type", "sub_group")
            .child_node(Node::new("group").attr("jid", group.clone()));
        self.query(self.community_iq(community.clone(), "set", body)).await?;
        self.forget_group(group).await;
        Ok(())
    }

    pub async fn community_groups(&self, community: &Jid) -> Result<Vec<LinkedGroup>> {
        let response = self.query(self.community_iq(community.clone(), "get", Node::new("sub_groups"))).await?;
        Ok(response
            .child("sub_groups")
            .map(|list| {
                list.children_by("group")
                    .filter_map(|g| {
                        let id = g.attr_string("id")?;
                        Some(LinkedGroup {
                            jid: if id.contains('@') { id.parse().ok()? } else { Jid::group(id) },
                            subject: g.attr_string("subject").unwrap_or_default(),
                            owner: g.attr_jid("creator"),
                            created: g.attr_i64("creation").unwrap_or_default(),
                            size: g.attr_u64("size"),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default())
    }

    pub async fn leave_community(&self, community: &Jid) -> Result<()> {
        let body = Node::new("leave").child_node(Node::new("community").attr("id", community.clone()));
        self.query(self.community_iq(Jid::group_server(), "set", body)).await?;
        self.forget_group(community).await;
        Ok(())
    }

    pub async fn set_community_join_approval(&self, community: &Jid, enabled: bool) -> Result<()> {
        let body = Node::new("membership_approval_mode")
            .child_node(Node::new("community_join").attr("state", if enabled { "on" } else { "off" }));
        self.query(self.community_iq(community.clone(), "set", body)).await?;
        self.forget_group(community).await;
        Ok(())
    }
}
