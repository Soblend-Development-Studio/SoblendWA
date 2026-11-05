use std::sync::atomic::Ordering;

use super::Client;
use crate::binary::Node;
use crate::error::Result;
use crate::jid::Jid;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChatPresence {
    Composing,
    Recording,
    Paused,
}

impl Client {
    pub async fn set_presence(&self, available: bool) -> Result<()> {
        let name = self
            .inner
            .config
            .push_name
            .clone()
            .or_else(|| self.inner.creds.read().push_name.clone())
            .map(|n| n.replace('@', ""));
        let Some(name) = name.filter(|n| !n.is_empty()) else {
            tracing::debug!("skipping presence update because no push name is configured");
            return Ok(());
        };
        let presence =
            Node::new("presence").attr("type", if available { "available" } else { "unavailable" }).attr("name", name);
        self.send_node(&presence)?;
        self.inner.active.store(available, Ordering::Release);
        Ok(())
    }

    pub async fn subscribe_presence(&self, jid: &Jid) -> Result<()> {
        self.send_node(&Node::new("presence").attr("type", "subscribe").attr("to", jid.to_non_device()))
    }

    pub async fn send_chat_presence(&self, chat: &Jid, state: ChatPresence) -> Result<()> {
        let me = if chat.is_lid() { self.lid().or_else(|| self.me()) } else { self.me() };
        let child = match state {
            ChatPresence::Composing => Node::new("composing"),
            ChatPresence::Recording => Node::new("composing").attr("media", "audio"),
            ChatPresence::Paused => Node::new("paused"),
        };
        let node = Node::new("chatstate").attr_opt("from", me).attr("to", chat.to_non_device()).child_node(child);
        self.send_node(&node)
    }

    pub async fn create_call_link(&self, video: bool, start_time_secs: Option<i64>) -> Result<String> {
        let mut link = Node::new("link_create").attr("media", if video { "video" } else { "audio" });
        if let Some(start) = start_time_secs {
            link.push_child(Node::new("event").attr("start_time", start));
        }
        let request =
            Node::new("call").attr("id", self.generate_tag()).attr("to", Jid::new("", "call")).child_node(link);
        let response = self.query(request).await?;
        let token = response
            .child("link_create")
            .and_then(|l| l.attr_string("token"))
            .ok_or_else(|| crate::error::Error::other("the server did not return a call link"))?;
        Ok(format!("https://call.whatsapp.com/{}/{token}", if video { "video" } else { "voice" }))
    }

    pub async fn reject_call(&self, call_id: &str, caller: &Jid) -> Result<()> {
        let me = self.require_me()?;
        let node = Node::new("call").attr("from", me).attr("to", caller).child_node(
            Node::new("reject").attr("call-id", call_id.to_owned()).attr("call-creator", caller).attr("count", "0"),
        );
        self.send_node(&node)
    }
}
