use std::time::Duration;

use serde_json::{json, Value};

use super::business::Product;
use super::{Client, MediaKind, MediaOptions};
use crate::binary::Node;
use crate::error::{Error, Result};
use crate::jid::Jid;
use crate::message::now_secs;
use crate::proto::{Message, ProductSnapshot};

const REACHOUT_TIMELOCK: &str = "23983697327930364";
const MESSAGE_CAPPING_INFO: &str = "24503548349331633";
const INVITE_LIFETIME: i64 = 3 * 24 * 60 * 60;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BotInfo {
    pub jid: Jid,
    pub persona_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DisappearingMode {
    pub jid: Jid,
    pub duration: Duration,
    pub set_at: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReachoutTimelock {
    pub active: bool,
    pub ends_at: Option<i64>,
    pub enforcement: Option<String>,
}

pub(crate) fn parse_bot_list(response: &Node) -> Vec<BotInfo> {
    let Some(bots) = response.child("bot") else { return Vec::new() };
    bots.children_by("section")
        .filter(|section| section.attr_is("type", "all"))
        .flat_map(|section| section.children_by("bot"))
        .filter_map(|bot| Some(BotInfo { jid: bot.attr_jid("jid")?, persona_id: bot.attr_string("persona_id") }))
        .collect()
}

pub(crate) fn parse_disappearing(response: &Node) -> Vec<DisappearingMode> {
    response
        .path(&["usync", "list"])
        .map(Node::nodes)
        .unwrap_or_default()
        .iter()
        .filter_map(|user| {
            let mode = user.child("disappearing_mode")?;
            Some(DisappearingMode {
                jid: user.attr_jid("jid")?,
                duration: Duration::from_secs(mode.attr_u64("duration").unwrap_or(0)),
                set_at: mode.attr_i64("t").unwrap_or(0),
            })
        })
        .collect()
}

pub(crate) fn parse_timelock(value: &Value) -> ReachoutTimelock {
    let ends = value["time_enforcement_ends"]
        .as_str()
        .and_then(|t| t.parse::<i64>().ok())
        .or_else(|| value["time_enforcement_ends"].as_i64())
        .filter(|t| *t > 0);
    ReachoutTimelock {
        active: value["is_active"].as_bool().unwrap_or(false),
        ends_at: ends,
        enforcement: value["enforcement_type"].as_str().map(str::to_owned),
    }
}

impl Client {
    pub async fn bot_list(&self) -> Result<Vec<BotInfo>> {
        let request = Node::new("iq")
            .attr("to", Jid::server())
            .attr("type", "get")
            .attr("xmlns", "bot")
            .child_node(Node::new("bot").attr("v", "2"));
        Ok(parse_bot_list(&self.query(request).await?))
    }

    pub async fn disappearing_modes(&self, jids: &[Jid]) -> Result<Vec<DisappearingMode>> {
        let users = jids.iter().map(|jid| Node::new("user").attr("jid", jid.to_non_device())).collect();
        let request = self.usync("interactive", vec![Node::new("disappearing_mode")], users);
        Ok(parse_disappearing(&self.query(request).await?))
    }

    pub async fn reachout_timelock(&self) -> Result<ReachoutTimelock> {
        let raw = self.mex(REACHOUT_TIMELOCK, json!({}), "xwa2_fetch_account_reachout_timelock").await?;
        Ok(parse_timelock(&raw))
    }

    pub async fn new_chat_message_cap(&self) -> Result<Value> {
        let variables = json!({ "input": { "type": "INDIVIDUAL_NEW_CHAT_MSG" } });
        self.mex(MESSAGE_CAPPING_INFO, variables, "xwa2_message_capping_info").await
    }

    pub async fn clean_dirty_bits(&self, kind: &str, since: Option<i64>) -> Result<()> {
        let clean = Node::new("clean").attr("type", kind.to_owned()).attr_opt("timestamp", since);
        let request = Node::new("iq")
            .attr("to", Jid::server())
            .attr("type", "set")
            .attr("xmlns", "urn:xmpp:whatsapp:dirty")
            .attr("id", self.generate_tag())
            .child_node(clean);
        self.send_node(&request)
    }

    pub async fn set_member_label(&self, group: &Jid, label: &str) -> Result<String> {
        if !group.is_group() {
            return Err(Error::InvalidJid(format!("{group} is not a group")));
        }
        self.send_message(group, Message::member_label(label)).await
    }

    pub async fn send_group_invite(&self, to: &Jid, group: &Jid, caption: Option<&str>) -> Result<String> {
        let info = self.group_info(group).await?;
        let code = self.group_invite_code(group).await?;
        let mut invite = Message::group_invite(group, info.subject, code, now_secs() + INVITE_LIFETIME, caption);
        if let Ok(Some(url)) = self.profile_picture_url(group, false).await {
            let thumbnail = self.inner.http.get(&url).timeout(Duration::from_secs(5)).send().await;
            if let Ok(response) = thumbnail.and_then(|r| r.error_for_status()) {
                if let (Some(slot), Ok(bytes)) = (invite.group_invite_message.as_mut(), response.bytes().await) {
                    slot.jpeg_thumbnail = Some(bytes.to_vec());
                }
            }
        }
        self.send_message(to, invite).await
    }

    pub async fn send_product(
        &self,
        chat: &Jid,
        owner: &Jid,
        product: &Product,
        image: &[u8],
        body: Option<&str>,
    ) -> Result<String> {
        let prepared = self.prepare_media(image, MediaKind::Image, MediaOptions::default()).await?;
        let snapshot = ProductSnapshot {
            product_image: prepared.image_message.map(|image| *image),
            product_id: Some(product.id.clone()),
            title: Some(product.name.clone()),
            description: product.description.clone(),
            currency_code: product.currency.clone(),
            price_amount_1000: product.price.map(|p| p as i64),
            retailer_id: product.retailer_id.clone(),
            url: product.url.clone(),
            product_image_count: Some(1),
            sale_price_amount_1000: None,
        };
        self.send_message(chat, Message::product(snapshot, owner, body, None)).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bots_and_timelocks() {
        let bots = Node::new("iq").child_node(
            Node::new("bot").children([
                Node::new("section").attr("type", "all").children([Node::new("bot")
                    .attr("jid", Jid::new("867051314767696", "bot"))
                    .attr("persona_id", "meta_ai")]),
                Node::new("section")
                    .attr("type", "featured")
                    .child_node(Node::new("bot").attr("jid", Jid::new("1", "bot"))),
            ]),
        );
        let list = parse_bot_list(&bots);
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].persona_id.as_deref(), Some("meta_ai"));

        let usync = Node::new("iq").child_node(
            Node::new("usync").child_node(
                Node::new("list").children([Node::new("user")
                    .attr("jid", Jid::phone("5215511111111"))
                    .child_node(Node::new("disappearing_mode").attr("duration", "604800").attr("t", "1700000000"))]),
            ),
        );
        let modes = parse_disappearing(&usync);
        assert_eq!(modes[0].duration, Duration::from_secs(604_800));

        let lock = parse_timelock(&json!({ "is_active": true, "time_enforcement_ends": "1800000000" }));
        assert!(lock.active && lock.ends_at == Some(1_800_000_000));
        assert_eq!(parse_timelock(&json!({ "time_enforcement_ends": "0" })).ends_at, None);
    }
}
