use std::collections::HashSet;

use futures_util::future::join_all;

use super::Client;
use crate::binary::Node;
use crate::error::Result;
use crate::jid::{Jid, HOSTED_LID_SERVER, HOSTED_SERVER};
use crate::message::now_secs;
use crate::store::KeyKind;

const DEVICE_REFRESH_AFTER: i64 = 15 * 60;
const USYNC_BATCH: usize = 500;
const PREFETCH_LIMIT: usize = 3000;

fn encode_device_list(fetched_at: i64, devices: &[Jid]) -> Vec<u8> {
    let mut out = fetched_at.to_string();
    for device in devices {
        out.push('\n');
        out.push_str(&device.to_string());
    }
    out.into_bytes()
}

fn decode_device_list(raw: &[u8]) -> Option<(i64, Vec<Jid>)> {
    let text = std::str::from_utf8(raw).ok()?;
    let mut lines = text.lines();
    let fetched_at = lines.next()?.parse().ok()?;
    let devices = lines.map(str::parse).collect::<Result<_>>().ok()?;
    Some((fetched_at, devices))
}

#[derive(Clone, Debug)]
pub struct OnWhatsApp {
    pub query: String,
    pub jid: Jid,
    pub exists: bool,
    pub lid: Option<Jid>,
}

impl Client {
    pub(crate) fn usync(&self, context: &'static str, query: Vec<Node>, users: Vec<Node>) -> Node {
        Node::new("iq").attr("to", Jid::server()).attr("type", "get").attr("xmlns", "usync").child_node(
            Node::new("usync")
                .attr("context", context)
                .attr("mode", "query")
                .attr("sid", self.generate_tag())
                .attr("last", "true")
                .attr("index", "0")
                .children([Node::new("query").children(query), Node::new("list").children(users)]),
        )
    }

    pub async fn on_whatsapp(&self, phones: &[&str]) -> Result<Vec<OnWhatsApp>> {
        let users = phones.iter().map(|phone| {
            let digits: String = phone.chars().filter(char::is_ascii_digit).collect();
            Node::new("user").child_node(Node::new("contact").text(format!("+{digits}")))
        });
        let request = self.usync("interactive", vec![Node::new("contact"), Node::new("lid")], users.collect());
        let response = self.query(request).await?;
        let mut out = Vec::new();
        for user in response.path(&["usync", "list"]).map(Node::nodes).unwrap_or_default() {
            let Some(jid) = user.attr_jid("jid") else { continue };
            let contact = user.child("contact");
            let lid = user.child("lid").and_then(|l| l.attr_jid("val"));
            if let Some(lid) = &lid {
                self.remember_lid(&jid, lid).await;
            }
            out.push(OnWhatsApp {
                query: contact.and_then(Node::content_string).unwrap_or_default(),
                exists: contact.is_some_and(|c| c.attr_is("type", "in")),
                jid,
                lid,
            });
        }
        Ok(out)
    }

    pub(crate) async fn devices_for(&self, users: &[Jid]) -> Result<Vec<Jid>> {
        let mut found = Vec::new();
        let mut missing = Vec::new();
        let mut stale = Vec::new();
        let mut seen = HashSet::new();
        for user in users {
            if user.device != 0 {
                found.push(user.clone());
                continue;
            }
            let key = user.to_non_device();
            if !seen.insert(key.clone()) {
                continue;
            }
            match self.cached_devices(&key).await {
                Some((fetched_at, devices)) => {
                    if now_secs() - fetched_at > DEVICE_REFRESH_AFTER {
                        stale.push(key);
                    }
                    found.extend(devices);
                }
                None => missing.push(key),
            }
        }

        if !missing.is_empty() {
            found.extend(self.fetch_devices_once(&missing).await?);
        }
        if !stale.is_empty() {
            self.refresh_devices_later(stale);
        }

        let creds = self.creds();
        let own_device = creds.me.as_ref().map_or(0, |me| me.device);
        let mut unique = HashSet::new();
        found.retain(|device| {
            let is_own_user = [&creds.me, &creds.lid].into_iter().flatten().any(|own| own.same_user(device));
            let is_this_device = is_own_user && device.device == own_device;
            !is_this_device && unique.insert(device.clone())
        });
        Ok(found)
    }

    async fn cached_devices(&self, user: &Jid) -> Option<(i64, Vec<Jid>)> {
        if let Some(entry) = self.inner.devices.get(user) {
            return Some(entry.clone());
        }
        let raw = self.inner.store.get(KeyKind::DeviceList, &user.to_string()).await.ok()??;
        let entry = decode_device_list(&raw)?;
        self.inner.devices.insert(user.clone(), entry.clone());
        Some(entry)
    }

    async fn fetch_devices_once(&self, users: &[Jid]) -> Result<Vec<Jid>> {
        let mut sorted = users.to_vec();
        sorted.sort();
        sorted.dedup();
        let mut guards = Vec::with_capacity(sorted.len());
        for user in &sorted {
            let lock = self.inner.fetch_locks.entry(user.clone()).or_default().clone();
            guards.push(lock.lock_owned().await);
        }
        let mut found = Vec::new();
        let mut still_missing = Vec::new();
        for user in sorted {
            match self.inner.devices.get(&user) {
                Some(entry) => found.extend(entry.1.iter().cloned()),
                None => still_missing.push(user),
            }
        }
        if !still_missing.is_empty() {
            found.extend(self.fetch_devices(&still_missing).await?);
        }
        drop(guards);
        Ok(found)
    }

    pub(crate) async fn prefetch_devices(&self, users: impl IntoIterator<Item = Jid>) -> usize {
        let mut pending = Vec::new();
        for user in users {
            let user = user.to_non_device();
            if self.cached_devices(&user).await.is_none() {
                pending.push(user);
            }
        }
        pending.sort();
        pending.dedup();
        pending.truncate(PREFETCH_LIMIT);
        for chunk in pending.chunks(USYNC_BATCH) {
            if let Err(err) = self.fetch_devices_once(chunk).await {
                tracing::debug!(%err, "device prefetch stopped");
                break;
            }
        }
        pending.len()
    }

    async fn fetch_devices(&self, users: &[Jid]) -> Result<Vec<Jid>> {
        let requests = users.chunks(USYNC_BATCH).map(|chunk| {
            let nodes = chunk.iter().map(|jid| Node::new("user").attr("jid", jid)).collect();
            self.query(self.usync("message", vec![Node::new("devices").attr("version", "2"), Node::new("lid")], nodes))
        });
        let mut found = Vec::new();
        for response in join_all(requests).await {
            let response = response?;
            for user in response.path(&["usync", "list"]).map(Node::nodes).unwrap_or_default() {
                let Some(jid) = user.attr_jid("jid").map(|j| j.to_non_device()) else { continue };
                if let Some(lid) = user.child("lid").and_then(|l| l.attr_jid("val")) {
                    self.remember_lid(&jid, &lid).await;
                }
                let Some(list) = user.path(&["devices", "device-list"]) else { continue };
                let devices: Vec<Jid> = list
                    .children_by("device")
                    .filter_map(|device| {
                        let id = device.attr_u64("id")? as u16;
                        let hosted = device.attr_is("is_hosted", "true")
                            || matches!(jid.server.as_str(), HOSTED_SERVER | HOSTED_LID_SERVER);
                        (!hosted && (id == 0 || device.has("key-index"))).then(|| jid.with_device(id))
                    })
                    .collect();
                self.cache_devices(&jid, devices.clone()).await;
                found.extend(devices);
            }
        }
        Ok(found)
    }

    async fn cache_devices(&self, user: &Jid, devices: Vec<Jid>) {
        let fetched_at = now_secs();
        let encoded = encode_device_list(fetched_at, &devices);
        self.inner.devices.insert(user.clone(), (fetched_at, devices));
        if let Err(err) = self.inner.store.set(KeyKind::DeviceList, &user.to_string(), encoded).await {
            tracing::debug!(%err, "failed to persist device list");
        }
    }

    fn refresh_devices_later(&self, users: Vec<Jid>) {
        let users: Vec<Jid> = users.into_iter().filter(|u| self.inner.refreshing.insert(u.clone())).collect();
        if users.is_empty() {
            return;
        }
        let client = self.clone();
        tokio::spawn(async move {
            if let Err(err) = client.fetch_devices(&users).await {
                tracing::debug!(%err, "background device refresh failed");
            }
            for user in &users {
                client.inner.refreshing.remove(user);
            }
        });
    }

    pub(crate) async fn devices_changed(&self, user: &Jid) {
        let user = user.to_non_device();
        self.inner.devices.remove(&user);
        let _ = self.inner.store.remove(KeyKind::DeviceList, &user.to_string()).await;
        self.refresh_devices_later(vec![user]);
    }

    pub(crate) async fn remember_lid(&self, pn: &Jid, lid: &Jid) {
        if !pn.is_pn() || !lid.is_lid() {
            return;
        }
        let store = &self.inner.store;
        let pn_key = format!("pn:{}", pn.user);
        if store.get(KeyKind::LidMapping, &pn_key).await.ok().flatten().as_deref() == Some(lid.user.as_bytes()) {
            return;
        }
        if let Err(err) = store.set(KeyKind::LidMapping, &pn_key, lid.user.as_bytes().to_vec()).await {
            tracing::debug!(%err, "failed to store lid mapping");
        }
        let _ = store.set(KeyKind::LidMapping, &format!("lid:{}", lid.user), pn.user.as_bytes().to_vec()).await;
    }

    pub async fn lid_for_phone(&self, pn: &Jid) -> Result<Option<Jid>> {
        let raw = self.inner.store.get(KeyKind::LidMapping, &format!("pn:{}", pn.user)).await?;
        Ok(raw.and_then(|r| String::from_utf8(r).ok()).map(Jid::lid))
    }

    pub async fn phone_for_lid(&self, lid: &Jid) -> Result<Option<Jid>> {
        let raw = self.inner.store.get(KeyKind::LidMapping, &format!("lid:{}", lid.user)).await?;
        Ok(raw.and_then(|r| String::from_utf8(r).ok()).map(|user| Jid::new(user, crate::jid::USER_SERVER)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_lists_roundtrip() {
        let devices = vec![
            Jid::phone("5215512345678"),
            Jid::phone("5215512345678").with_device(7),
            Jid::lid("99").with_device(2),
        ];
        let (fetched_at, decoded) = decode_device_list(&encode_device_list(1_700_000_000, &devices)).unwrap();
        assert_eq!(fetched_at, 1_700_000_000);
        assert_eq!(decoded, devices);
        assert_eq!(decode_device_list(&encode_device_list(5, &[])).unwrap().1, Vec::<Jid>::new());
    }
}
