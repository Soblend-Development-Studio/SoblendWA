use std::collections::{HashMap, HashSet};

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use prost::Message as _;

use super::Client;
use crate::binary::Node;
use crate::crypto;
use crate::error::{Error, Result};
use crate::events::{ChatUpdate, Event};
use crate::jid::Jid;
use crate::message::now_millis;
use crate::proto::{
    AppStateSyncKeyShare, ArchiveChatAction, ContactAction, ExternalBlobReference, KeyId, LabelAssociationAction,
    LabelEditAction, MarkChatAsReadAction, MessageKey, MessageRangeAction, MuteAction, PinAction,
    PrivacySettingDisableLinkPreviewsAction, PushNameSetting, QuickReplyAction, StarAction, StoredHashEntry,
    StoredHashState, SyncActionData, SyncActionMessage, SyncActionMessageRange, SyncActionValue, SyncdBlob,
    SyncdMutation, SyncdMutations, SyncdPatch, SyncdRecord, SyncdSnapshot,
};
use crate::store::KeyKind;

pub const COLLECTIONS: [&str; 5] = ["critical_block", "critical_unblock_low", "regular_high", "regular_low", "regular"];
const MAX_ATTEMPTS: u32 = 2;
const SET: i32 = SyncdMutation::SET;
const REMOVE: i32 = SyncdMutation::REMOVE;

struct MutationKeys {
    index: [u8; 32],
    value_encryption: [u8; 32],
    value_mac: [u8; 32],
    snapshot_mac: [u8; 32],
    patch_mac: [u8; 32],
}

impl MutationKeys {
    fn derive(key_data: &[u8]) -> Self {
        let expanded: [u8; 160] = crypto::hkdf_array(key_data, None, b"WhatsApp Mutation Keys");
        let part = |n: usize| -> [u8; 32] { expanded[n * 32..(n + 1) * 32].try_into().expect("32 byte slice") };
        MutationKeys {
            index: part(0),
            value_encryption: part(1),
            value_mac: part(2),
            snapshot_mac: part(3),
            patch_mac: part(4),
        }
    }
}

fn lthash(hash: &mut [u8; 128], value_mac: &[u8], add: bool) {
    let expanded: [u8; 128] = crypto::hkdf_array(value_mac, None, b"WhatsApp Patch Integrity");
    for (lane, delta) in hash.as_chunks_mut::<2>().0.iter_mut().zip(expanded.as_chunks::<2>().0) {
        let current = u16::from_le_bytes(*lane);
        let delta = u16::from_le_bytes(*delta);
        let next = if add { current.wrapping_add(delta) } else { current.wrapping_sub(delta) };
        *lane = next.to_le_bytes();
    }
}

#[derive(Clone)]
struct HashState {
    version: u64,
    hash: [u8; 128],
    values: HashMap<Vec<u8>, Vec<u8>>,
}

impl Default for HashState {
    fn default() -> Self {
        HashState { version: 0, hash: [0; 128], values: HashMap::new() }
    }
}

impl HashState {
    fn mix(&mut self, index_mac: &[u8], value_mac: &[u8], operation: i32) {
        let previous = if operation == REMOVE {
            match self.values.remove(index_mac) {
                Some(previous) => Some(previous),
                None => return,
            }
        } else {
            self.values.insert(index_mac.to_vec(), value_mac.to_vec())
        };
        if let Some(previous) = previous {
            lthash(&mut self.hash, &previous, false);
        }
        if operation != REMOVE {
            lthash(&mut self.hash, value_mac, true);
        }
    }

    fn to_bytes(&self) -> Vec<u8> {
        StoredHashState {
            version: self.version,
            hash: self.hash.to_vec(),
            entries: self
                .values
                .iter()
                .map(|(index_mac, value_mac)| StoredHashEntry {
                    index_mac: index_mac.clone(),
                    value_mac: value_mac.clone(),
                })
                .collect(),
        }
        .encode_to_vec()
    }

    fn from_bytes(data: &[u8]) -> Result<Self> {
        let stored = StoredHashState::decode(data)?;
        Ok(HashState {
            version: stored.version,
            hash: stored.hash.try_into().map_err(|_| Error::other("stored app state hash is corrupt"))?,
            values: stored.entries.into_iter().map(|e| (e.index_mac, e.value_mac)).collect(),
        })
    }
}

fn snapshot_mac(hash: &[u8; 128], version: u64, name: &str, key: &[u8; 32]) -> [u8; 32] {
    crypto::hmac_sha256(key, &[hash, &version.to_be_bytes(), name.as_bytes()])
}

fn patch_mac(snapshot_mac: &[u8], value_macs: &[&[u8]], version: u64, name: &str, key: &[u8; 32]) -> [u8; 32] {
    let mut parts: Vec<&[u8]> = Vec::with_capacity(value_macs.len() + 3);
    parts.push(snapshot_mac);
    parts.extend_from_slice(value_macs);
    let version = version.to_be_bytes();
    parts.push(&version);
    parts.push(name.as_bytes());
    crypto::hmac_sha256(key, &parts)
}

fn value_mac(operation: i32, encrypted: &[u8], key_id: &[u8], key: &[u8; 32]) -> [u8; 32] {
    let op = [if operation == REMOVE { 2u8 } else { 1u8 }];
    let mut length = [0u8; 8];
    length[7] = (1 + key_id.len()) as u8;
    let mac = crypto::hmac_sha512(key, &[&op, key_id, encrypted, &length]);
    mac[..32].try_into().expect("64 byte mac")
}

struct Mutation {
    operation: i32,
    index: Vec<String>,
    value: SyncActionValue,
}

type KeyRing = HashMap<Vec<u8>, MutationKeys>;

fn key_id_of(key: &Option<KeyId>) -> Vec<u8> {
    key.as_ref().and_then(|k| k.id.clone()).unwrap_or_default()
}

fn decode_records<'a>(
    records: impl Iterator<Item = (i32, &'a SyncdRecord)>,
    state: &mut HashState,
    ring: &KeyRing,
    out: &mut Vec<Mutation>,
) -> Result<()> {
    for (operation, record) in records {
        let key_id = key_id_of(&record.key_id);
        let Some(keys) = ring.get(&key_id) else {
            return Err(Error::MissingAppStateKey(STANDARD.encode(&key_id)));
        };
        let blob = record.value.as_ref().and_then(|v| v.blob.as_deref()).unwrap_or_default();
        let index_mac = record.index.as_ref().and_then(|i| i.blob.as_deref()).unwrap_or_default();
        if blob.len() < 32 + 16 + 16 {
            continue;
        }
        let (encrypted, mac) = blob.split_at(blob.len() - 32);
        if value_mac(operation, encrypted, &key_id, &keys.value_mac) != mac {
            tracing::debug!("skipping app state record with a bad value mac");
            continue;
        }
        let Ok(plain) = crypto::aes_cbc_decrypt(&keys.value_encryption, &encrypted[..16], &encrypted[16..]) else {
            continue;
        };
        let Ok(data) = SyncActionData::decode(plain.as_slice()) else { continue };
        let index_bytes = data.index.unwrap_or_default();
        if crypto::hmac_sha256(&keys.index, &[&index_bytes]) != index_mac {
            return Err(Error::other("app state index mac does not match"));
        }
        out.push(Mutation {
            operation,
            index: serde_json::from_slice(&index_bytes).unwrap_or_default(),
            value: data.value.unwrap_or_default(),
        });
        state.mix(index_mac, mac, operation);
    }
    Ok(())
}

#[derive(Clone, Debug)]
pub enum ChatModification {
    Archive(bool),
    Pin(bool),
    Mute(Option<i64>),
    MarkRead(bool),
    Delete,
    Clear,
    Star { message_id: String, from_me: bool, starred: bool },
    DeleteForMe { message_id: String, from_me: bool, timestamp: i64 },
    SaveContact { full_name: String, first_name: Option<String> },
    RemoveContact,
    Label { label_id: String, labeled: bool },
    MessageLabel { label_id: String, message_id: String, labeled: bool },
}

struct PatchSpec {
    collection: &'static str,
    index: Vec<String>,
    value: SyncActionValue,
    version: i32,
    operation: i32,
}

impl Client {
    async fn app_state_keys(&self, ids: &HashSet<Vec<u8>>) -> Result<KeyRing> {
        let mut ring = KeyRing::new();
        for id in ids {
            let stored = self.inner.store.get(KeyKind::AppStateKey, &STANDARD.encode(id)).await?;
            match stored {
                Some(data) => {
                    ring.insert(id.clone(), MutationKeys::derive(&data));
                }
                None => return Err(Error::MissingAppStateKey(STANDARD.encode(id))),
            }
        }
        Ok(ring)
    }

    async fn load_hash_state(&self, name: &str) -> Result<HashState> {
        match self.inner.store.get(KeyKind::AppStateVersion, name).await? {
            Some(bytes) => HashState::from_bytes(&bytes).or_else(|_| Ok(HashState::default())),
            None => Ok(HashState::default()),
        }
    }

    async fn save_hash_state(&self, name: &str, state: &HashState) -> Result<()> {
        self.inner.store.set(KeyKind::AppStateVersion, name, state.to_bytes()).await
    }

    pub(crate) async fn store_app_state_keys(&self, share: &AppStateSyncKeyShare) -> Result<bool> {
        let mut newest = None;
        for key in &share.keys {
            let (Some(id), Some(data)) = (
                key.key_id.as_ref().and_then(|k| k.key_id.clone()),
                key.key_data.as_ref().and_then(|k| k.key_data.clone()),
            ) else {
                continue;
            };
            let encoded = STANDARD.encode(&id);
            self.inner.store.set(KeyKind::AppStateKey, &encoded, data).await?;
            newest = Some(encoded);
        }
        let Some(newest) = newest else { return Ok(false) };
        let first = self.creds().app_state_key_id.is_none();
        self.update_creds(|creds| creds.app_state_key_id = Some(newest)).await?;
        Ok(first)
    }

    pub(crate) fn sync_app_state_later(&self, collections: Vec<String>, initial: bool) {
        let client = self.clone();
        tokio::spawn(async move {
            let names: Vec<&str> = collections.iter().map(String::as_str).collect();
            if let Err(err) = client.sync_app_state(&names, initial).await {
                tracing::warn!(%err, "app state sync failed");
            }
        });
    }

    pub async fn sync_app_state(&self, collections: &[&str], initial: bool) -> Result<()> {
        let _guard = self.inner.app_state_lock.lock().await;
        let (mutations, missing) = self.sync_collections(collections).await?;
        for mutation in mutations {
            self.apply_mutation(mutation, initial).await;
        }
        if !missing.is_empty() {
            self.request_app_state_keys(missing).await;
        }
        Ok(())
    }

    async fn sync_collections(&self, collections: &[&str]) -> Result<(Vec<Mutation>, Vec<String>)> {
        let mut pending: Vec<String> = collections.iter().map(|c| (*c).to_owned()).collect();
        let mut missing = Vec::new();
        let mut attempts: HashMap<String, u32> = HashMap::new();
        let mut gathered = Vec::new();
        while !pending.is_empty() {
            let mut states = HashMap::new();
            let mut nodes = Vec::with_capacity(pending.len());
            for name in &pending {
                let state = self.load_hash_state(name).await?;
                nodes.push(
                    Node::new("collection")
                        .attr("name", name.clone())
                        .attr("version", state.version)
                        .attr("return_snapshot", if state.version == 0 { "true" } else { "false" }),
                );
                states.insert(name.clone(), state);
            }
            let request = Node::new("iq")
                .attr("to", Jid::server())
                .attr("type", "set")
                .attr("xmlns", "w:sync:app:state")
                .child_node(Node::new("sync").children(nodes));
            let response = self.query(request).await?;

            let mut finished = HashSet::new();
            for collection in response.child("sync").map(Node::nodes).unwrap_or_default() {
                let Some(name) = collection.attr_string("name") else { continue };
                let state = states.remove(&name).unwrap_or_default();
                match self.apply_collection(&name, collection, state).await {
                    Ok(mutations) => {
                        gathered.extend(mutations);
                        if !collection.attr_is("has_more_patches", "true") {
                            finished.insert(name);
                        }
                    }
                    Err(Error::MissingAppStateKey(key)) => {
                        tracing::info!(%name, %key, "app state collection is waiting for its key");
                        missing.push(key);
                        finished.insert(name);
                    }
                    Err(err) => {
                        let tries = attempts.entry(name.clone()).or_insert(0);
                        *tries += 1;
                        tracing::warn!(%name, %err, attempt = *tries, "app state collection failed, restarting from a snapshot");
                        self.inner.store.remove(KeyKind::AppStateVersion, &name).await?;
                        if *tries >= MAX_ATTEMPTS {
                            finished.insert(name);
                        }
                    }
                }
            }
            if finished.is_empty() && states.len() == pending.len() {
                break;
            }
            pending.retain(|name| !finished.contains(name));
        }

        let mut seen = HashSet::new();
        let mut latest: Vec<Mutation> =
            gathered.into_iter().rev().filter(|m| seen.insert(m.index.join("\u{1}"))).collect();
        latest.reverse();
        Ok((latest, missing))
    }

    async fn apply_collection(&self, name: &str, collection: &Node, mut state: HashState) -> Result<Vec<Mutation>> {
        if let Some((code, text)) = collection.error_code() {
            return Err(Error::Server { code, text });
        }
        let mut mutations = Vec::new();

        if let Some(raw) = collection.child("snapshot").and_then(Node::content_bytes) {
            let reference = ExternalBlobReference::decode(raw)?;
            let snapshot = SyncdSnapshot::decode(self.download(&reference).await?.as_slice())?;
            let mut ids: HashSet<Vec<u8>> = snapshot.records.iter().map(|r| key_id_of(&r.key_id)).collect();
            ids.insert(key_id_of(&snapshot.key_id));
            let ring = self.app_state_keys(&ids).await?;
            state = HashState {
                version: snapshot.version.as_ref().and_then(|v| v.version).unwrap_or(0),
                ..Default::default()
            };
            decode_records(snapshot.records.iter().map(|r| (SET, r)), &mut state, &ring, &mut mutations)?;
            if let Some(keys) = ring.get(&key_id_of(&snapshot.key_id)) {
                if snapshot.mac.as_deref()
                    != Some(&snapshot_mac(&state.hash, state.version, name, &keys.snapshot_mac)[..])
                {
                    tracing::warn!(%name, version = state.version, "app state snapshot mac mismatch");
                }
            }
            self.save_hash_state(name, &state).await?;
        }

        let base_version = collection.attr_u64("version").unwrap_or(state.version);
        let raw_patches = collection.child("patches").unwrap_or(collection);
        for raw in raw_patches.children_by("patch").filter_map(Node::content_bytes) {
            let mut patch = SyncdPatch::decode(raw)?;
            if let Some(reference) = patch.external_mutations.take() {
                let external = SyncdMutations::decode(self.download(&reference).await?.as_slice())?;
                patch.mutations.extend(external.mutations);
            }
            let version = patch.version.as_ref().and_then(|v| v.version).unwrap_or(base_version + 1);
            let mut ids: HashSet<Vec<u8>> =
                patch.mutations.iter().filter_map(|m| m.record.as_ref()).map(|r| key_id_of(&r.key_id)).collect();
            ids.insert(key_id_of(&patch.key_id));
            let ring = self.app_state_keys(&ids).await?;
            let main = ring.get(&key_id_of(&patch.key_id)).ok_or_else(|| Error::other("patch key vanished"))?;

            let macs: Vec<&[u8]> = patch
                .mutations
                .iter()
                .filter_map(|m| m.record.as_ref()?.value.as_ref()?.blob.as_deref())
                .map(|blob| &blob[blob.len().saturating_sub(32)..])
                .collect();
            let expected =
                patch_mac(patch.snapshot_mac.as_deref().unwrap_or_default(), &macs, version, name, &main.patch_mac);
            if patch.patch_mac.as_deref() != Some(&expected[..]) {
                return Err(Error::other(format!("invalid patch mac for {name} v{version}")));
            }

            state.version = version;
            let records = patch.mutations.iter().filter_map(|m| Some((m.operation.unwrap_or(SET), m.record.as_ref()?)));
            decode_records(records, &mut state, &ring, &mut mutations)?;
            if patch.snapshot_mac.as_deref() != Some(&snapshot_mac(&state.hash, version, name, &main.snapshot_mac)[..])
            {
                tracing::warn!(%name, version, "app state hash diverged after a patch");
            }
        }
        self.save_hash_state(name, &state).await?;
        Ok(mutations)
    }

    async fn apply_mutation(&self, mutation: Mutation, initial: bool) {
        let Some(update) = self.chat_update_of(&mutation).await else { return };
        if let ChatUpdate::PushName { name } = &update {
            let name = name.clone();
            if let Err(err) = self.update_creds(|creds| creds.push_name = Some(name)).await {
                tracing::debug!(%err, "could not store our push name");
            }
        }
        let timestamp = mutation.value.timestamp.unwrap_or_else(now_millis);
        self.emit(Event::ChatUpdate { update, timestamp, initial_sync: initial });
    }

    async fn chat_update_of(&self, mutation: &Mutation) -> Option<ChatUpdate> {
        let value = &mutation.value;
        let index = &mutation.index;
        let kind = index.first().map(String::as_str).unwrap_or_default();
        let jid_at = |n: usize| index.get(n).and_then(|j| j.parse::<Jid>().ok());
        let text_at = |n: usize| index.get(n).cloned().unwrap_or_default();
        let update = if let Some(mute) = &value.mute_action {
            let muted = mute.muted == Some(true);
            ChatUpdate::Muted { chat: jid_at(1)?, until: muted.then(|| mute.mute_end_timestamp.unwrap_or(-1)) }
        } else if let Some(archive) = &value.archive_chat_action {
            ChatUpdate::Archived { chat: jid_at(1)?, archived: archive.archived.unwrap_or(false) }
        } else if let Some(pin) = &value.pin_action {
            ChatUpdate::Pinned { chat: jid_at(1)?, pinned: pin.pinned.unwrap_or(false) }
        } else if let Some(read) = &value.mark_chat_as_read_action {
            ChatUpdate::MarkedRead { chat: jid_at(1)?, read: read.read.unwrap_or(false) }
        } else if let Some(lock) = &value.lock_chat_action {
            ChatUpdate::Locked { chat: jid_at(1)?, locked: lock.locked.unwrap_or(false) }
        } else if value.delete_chat_action.is_some() || kind == "deleteChat" {
            ChatUpdate::Deleted { chat: jid_at(1)? }
        } else if value.clear_chat_action.is_some() || kind == "clearChat" {
            ChatUpdate::Cleared { chat: jid_at(1)? }
        } else if let Some(star) = &value.star_action {
            ChatUpdate::Starred {
                chat: jid_at(1)?,
                message_id: text_at(2),
                from_me: text_at(3) == "1",
                starred: star.starred.unwrap_or(false),
            }
        } else if value.delete_message_for_me_action.is_some() || kind == "deleteMessageForMe" {
            ChatUpdate::DeletedForMe { chat: jid_at(1)?, message_id: text_at(2), from_me: text_at(3) == "1" }
        } else if kind == "contact" && mutation.operation == REMOVE {
            ChatUpdate::ContactRemoved { jid: jid_at(1)? }
        } else if let Some(contact) = &value.contact_action {
            let jid = jid_at(1)?;
            let lid = contact.lid_jid.as_deref().and_then(|j| j.parse::<Jid>().ok());
            let phone = contact
                .pn_jid
                .as_deref()
                .and_then(|j| j.parse::<Jid>().ok())
                .or_else(|| jid.is_pn().then(|| jid.clone()));
            if let (Some(pn), Some(lid)) = (&phone, &lid) {
                self.remember_lid(pn, lid).await;
            }
            ChatUpdate::Contact {
                jid,
                full_name: contact.full_name.clone(),
                first_name: contact.first_name.clone(),
                lid,
                phone,
            }
        } else if let Some(setting) = &value.push_name_setting {
            ChatUpdate::PushName { name: setting.name.clone()? }
        } else if let Some(label) = &value.label_edit_action {
            ChatUpdate::Label {
                id: text_at(1),
                name: label.name.clone(),
                color: label.color,
                deleted: label.deleted.unwrap_or(false),
            }
        } else if let Some(association) = &value.label_association_action {
            let labeled = association.labeled.unwrap_or(false);
            match kind {
                "label_message" => {
                    ChatUpdate::MessageLabel { label: text_at(1), chat: jid_at(2)?, message_id: text_at(3), labeled }
                }
                _ => ChatUpdate::ChatLabel { label: text_at(1), chat: jid_at(2)?, labeled },
            }
        } else {
            ChatUpdate::Other { index: index.clone() }
        };
        Some(update)
    }

    fn message_range(&self, chat: &Jid) -> SyncActionMessageRange {
        match self.inner.last_messages.get(chat) {
            Some(entry) => {
                let (key, timestamp) = entry.value().clone();
                SyncActionMessageRange {
                    last_message_timestamp: Some(timestamp),
                    last_system_message_timestamp: None,
                    messages: vec![SyncActionMessage { key: Some(key), timestamp: Some(timestamp) }],
                }
            }
            None => SyncActionMessageRange::default(),
        }
    }

    pub(crate) fn remember_last_message(&self, chat: &Jid, key: MessageKey, timestamp: i64) {
        self.inner.last_messages.insert(chat.to_non_device(), (key, timestamp));
    }

    fn patch_for(&self, chat: &Jid, modification: ChatModification) -> PatchSpec {
        let jid = chat.to_non_device().to_string();
        let value = |build: &dyn Fn(&mut SyncActionValue)| {
            let mut value = SyncActionValue::default();
            build(&mut value);
            value
        };
        let spec = |collection, index: Vec<String>, value, version| PatchSpec {
            collection,
            index,
            value,
            version,
            operation: SET,
        };
        match modification {
            ChatModification::Mute(until) => spec(
                "regular_high",
                vec!["mute".into(), jid],
                value(&|v| {
                    v.mute_action =
                        Some(MuteAction { muted: Some(until.is_some()), mute_end_timestamp: until, auto_muted: None })
                }),
                2,
            ),
            ChatModification::Archive(archived) => {
                let range = self.message_range(chat);
                spec(
                    "regular_low",
                    vec!["archive".into(), jid],
                    value(&|v| {
                        v.archive_chat_action =
                            Some(ArchiveChatAction { archived: Some(archived), message_range: Some(range.clone()) })
                    }),
                    3,
                )
            }
            ChatModification::MarkRead(read) => {
                let range = self.message_range(chat);
                spec(
                    "regular_low",
                    vec!["markChatAsRead".into(), jid],
                    value(&|v| {
                        v.mark_chat_as_read_action =
                            Some(MarkChatAsReadAction { read: Some(read), message_range: Some(range.clone()) })
                    }),
                    3,
                )
            }
            ChatModification::Pin(pinned) => spec(
                "regular_low",
                vec!["pin_v1".into(), jid],
                value(&|v| v.pin_action = Some(PinAction { pinned: Some(pinned) })),
                5,
            ),
            ChatModification::Delete => {
                let range = self.message_range(chat);
                spec(
                    "regular_high",
                    vec!["deleteChat".into(), jid, "1".into()],
                    value(&|v| v.delete_chat_action = Some(MessageRangeAction { message_range: Some(range.clone()) })),
                    6,
                )
            }
            ChatModification::Clear => {
                let range = self.message_range(chat);
                spec(
                    "regular_high",
                    vec!["clearChat".into(), jid, "1".into(), "0".into()],
                    value(&|v| v.clear_chat_action = Some(MessageRangeAction { message_range: Some(range.clone()) })),
                    6,
                )
            }
            ChatModification::Star { message_id, from_me, starred } => spec(
                "regular_low",
                vec!["star".into(), jid, message_id, if from_me { "1" } else { "0" }.into(), "0".into()],
                value(&|v| v.star_action = Some(StarAction { starred: Some(starred) })),
                2,
            ),
            ChatModification::DeleteForMe { message_id, from_me, timestamp } => spec(
                "regular_high",
                vec!["deleteMessageForMe".into(), jid, message_id, if from_me { "1" } else { "0" }.into(), "0".into()],
                value(&|v| {
                    v.delete_message_for_me_action = Some(crate::proto::DeleteMessageForMeAction {
                        delete_media: Some(true),
                        message_timestamp: Some(timestamp),
                    })
                }),
                3,
            ),
            ChatModification::SaveContact { full_name, first_name } => spec(
                "critical_unblock_low",
                vec!["contact".into(), jid],
                value(&|v| {
                    v.contact_action = Some(ContactAction {
                        full_name: Some(full_name.clone()),
                        first_name: first_name.clone(),
                        save_on_primary_addressbook: Some(true),
                        ..Default::default()
                    })
                }),
                2,
            ),
            ChatModification::RemoveContact => PatchSpec {
                collection: "critical_unblock_low",
                index: vec!["contact".into(), jid],
                value: SyncActionValue { contact_action: Some(ContactAction::default()), ..Default::default() },
                version: 2,
                operation: REMOVE,
            },
            ChatModification::Label { label_id, labeled } => spec(
                "regular",
                vec!["label_jid".into(), label_id, jid],
                value(&|v| v.label_association_action = Some(LabelAssociationAction { labeled: Some(labeled) })),
                3,
            ),
            ChatModification::MessageLabel { label_id, message_id, labeled } => spec(
                "regular",
                vec!["label_message".into(), label_id, jid, message_id, "0".into(), "0".into()],
                value(&|v| v.label_association_action = Some(LabelAssociationAction { labeled: Some(labeled) })),
                3,
            ),
        }
    }

    pub async fn chat_modify(&self, chat: &Jid, modification: ChatModification) -> Result<()> {
        let spec = self.patch_for(chat, modification);
        self.push_patch(spec).await
    }

    pub async fn set_push_name(&self, name: &str) -> Result<()> {
        let spec = PatchSpec {
            collection: "critical_block",
            index: vec!["setting_pushName".into()],
            value: SyncActionValue {
                push_name_setting: Some(PushNameSetting { name: Some(name.to_owned()) }),
                ..Default::default()
            },
            version: 1,
            operation: SET,
        };
        self.push_patch(spec).await?;
        let name = name.to_owned();
        self.update_creds(|creds| creds.push_name = Some(name)).await
    }

    pub async fn archive_chat(&self, chat: &Jid, archived: bool) -> Result<()> {
        self.chat_modify(chat, ChatModification::Archive(archived)).await
    }

    pub async fn pin_chat(&self, chat: &Jid, pinned: bool) -> Result<()> {
        self.chat_modify(chat, ChatModification::Pin(pinned)).await
    }

    pub async fn mute_chat(&self, chat: &Jid, duration: Option<std::time::Duration>) -> Result<()> {
        let until = duration.map(|d| now_millis() + d.as_millis() as i64);
        self.chat_modify(chat, ChatModification::Mute(until)).await
    }

    pub async fn mark_chat_read(&self, chat: &Jid, read: bool) -> Result<()> {
        self.chat_modify(chat, ChatModification::MarkRead(read)).await
    }

    pub async fn delete_chat(&self, chat: &Jid) -> Result<()> {
        self.chat_modify(chat, ChatModification::Delete).await
    }

    pub async fn clear_chat(&self, chat: &Jid) -> Result<()> {
        self.chat_modify(chat, ChatModification::Clear).await
    }

    pub async fn star_message(&self, chat: &Jid, message_id: &str, from_me: bool, starred: bool) -> Result<()> {
        let modification = ChatModification::Star { message_id: message_id.to_owned(), from_me, starred };
        self.chat_modify(chat, modification).await
    }

    pub async fn save_contact(&self, jid: &Jid, full_name: &str) -> Result<()> {
        let first_name = full_name.split_whitespace().next().map(str::to_owned);
        let modification = ChatModification::SaveContact { full_name: full_name.to_owned(), first_name };
        self.chat_modify(jid, modification).await
    }

    pub async fn label_chat(&self, chat: &Jid, label_id: &str, labeled: bool) -> Result<()> {
        self.chat_modify(chat, ChatModification::Label { label_id: label_id.to_owned(), labeled }).await
    }

    pub async fn label_message(&self, chat: &Jid, message_id: &str, label_id: &str, labeled: bool) -> Result<()> {
        let modification = ChatModification::MessageLabel {
            label_id: label_id.to_owned(),
            message_id: message_id.to_owned(),
            labeled,
        };
        self.chat_modify(chat, modification).await
    }

    fn setting_patch(index: Vec<String>, value: SyncActionValue, version: i32) -> PatchSpec {
        PatchSpec { collection: "regular", index, value, version, operation: SET }
    }

    pub async fn edit_label(&self, id: &str, name: &str, color: i32) -> Result<()> {
        let value = SyncActionValue {
            label_edit_action: Some(LabelEditAction {
                name: Some(name.to_owned()),
                color: Some(color),
                predefined_id: None,
                deleted: Some(false),
            }),
            ..Default::default()
        };
        self.push_patch(Self::setting_patch(vec!["label_edit".into(), id.to_owned()], value, 3)).await
    }

    pub async fn delete_label(&self, id: &str) -> Result<()> {
        let value = SyncActionValue {
            label_edit_action: Some(LabelEditAction { deleted: Some(true), ..Default::default() }),
            ..Default::default()
        };
        self.push_patch(Self::setting_patch(vec!["label_edit".into(), id.to_owned()], value, 3)).await
    }

    pub async fn save_quick_reply(&self, id: Option<&str>, shortcut: &str, message: &str) -> Result<String> {
        let id = id.map_or_else(|| crate::message::now_secs().to_string(), str::to_owned);
        let value = SyncActionValue {
            quick_reply_action: Some(QuickReplyAction {
                shortcut: Some(shortcut.to_owned()),
                message: Some(message.to_owned()),
                keywords: Vec::new(),
                count: Some(0),
                deleted: Some(false),
            }),
            ..Default::default()
        };
        self.push_patch(Self::setting_patch(vec!["quick_reply".into(), id.clone()], value, 2)).await?;
        Ok(id)
    }

    pub async fn delete_quick_reply(&self, id: &str) -> Result<()> {
        let value = SyncActionValue {
            quick_reply_action: Some(QuickReplyAction {
                deleted: Some(true),
                count: Some(0),
                message: Some(String::new()),
                shortcut: Some(String::new()),
                keywords: Vec::new(),
            }),
            ..Default::default()
        };
        self.push_patch(Self::setting_patch(vec!["quick_reply".into(), id.to_owned()], value, 2)).await
    }

    pub async fn set_link_previews_disabled(&self, disabled: bool) -> Result<()> {
        let value = SyncActionValue {
            privacy_setting_disable_link_previews_action: Some(PrivacySettingDisableLinkPreviewsAction {
                is_previews_disabled: Some(disabled),
            }),
            ..Default::default()
        };
        self.push_patch(Self::setting_patch(vec!["setting_disableLinkPreviews".into()], value, 8)).await
    }

    async fn push_patch(&self, spec: PatchSpec) -> Result<()> {
        let _guard = self.inner.app_state_lock.lock().await;
        let key_id_text = self
            .creds()
            .app_state_key_id
            .ok_or_else(|| Error::MissingAppStateKey("the phone has not shared an app state key yet".into()))?;
        let key_id = STANDARD.decode(&key_id_text).map_err(|_| Error::other("stored app state key id is corrupt"))?;
        let ring = self.app_state_keys(&HashSet::from([key_id.clone()])).await?;
        let keys = ring.get(&key_id).expect("ring holds the requested key");
        let index = serde_json::to_vec(&spec.index)?;

        for attempt in 0..2 {
            let (refreshed, _) = self.sync_collections(&[spec.collection]).await?;
            for mutation in refreshed {
                self.apply_mutation(mutation, false).await;
            }
            let mut state = self.load_hash_state(spec.collection).await?;

            let mut value = spec.value.clone();
            value.timestamp = Some(now_millis());
            let data = SyncActionData {
                index: Some(index.clone()),
                value: Some(value),
                padding: Some(Vec::new()),
                version: Some(spec.version),
            }
            .encode_to_vec();
            let iv = crypto::random_array::<16>();
            let encrypted = [&iv[..], &crypto::aes_cbc_encrypt(&keys.value_encryption, &iv, &data)?].concat();
            let mac = value_mac(spec.operation, &encrypted, &key_id, &keys.value_mac);
            let index_mac = crypto::hmac_sha256(&keys.index, &[&index]);

            state.mix(&index_mac, &mac, spec.operation);
            state.version += 1;
            let snapshot = snapshot_mac(&state.hash, state.version, spec.collection, &keys.snapshot_mac);
            let patch = SyncdPatch {
                version: None,
                mutations: vec![SyncdMutation {
                    operation: Some(spec.operation),
                    record: Some(SyncdRecord {
                        index: Some(SyncdBlob { blob: Some(index_mac.to_vec()) }),
                        value: Some(SyncdBlob { blob: Some([&encrypted[..], &mac].concat()) }),
                        key_id: Some(KeyId { id: Some(key_id.clone()) }),
                    }),
                }],
                external_mutations: None,
                snapshot_mac: Some(snapshot.to_vec()),
                patch_mac: Some(
                    patch_mac(&snapshot, &[&mac], state.version, spec.collection, &keys.patch_mac).to_vec(),
                ),
                key_id: Some(KeyId { id: Some(key_id.clone()) }),
            };

            let request = Node::new("iq")
                .attr("to", Jid::server())
                .attr("type", "set")
                .attr("xmlns", "w:sync:app:state")
                .child_node(
                    Node::new("sync").child_node(
                        Node::new("collection")
                            .attr("name", spec.collection)
                            .attr("version", state.version - 1)
                            .attr("return_snapshot", "false")
                            .child_node(Node::new("patch").bytes(patch.encode_to_vec())),
                    ),
                );
            let response = self.query(request).await?;
            let conflict = response
                .path(&["sync", "collection"])
                .and_then(Node::error_code)
                .filter(|(code, _)| *code == 409 || *code == 400);
            match conflict {
                Some(_) if attempt == 0 => continue,
                Some((code, text)) => return Err(Error::Server { code, text }),
                None => return self.save_hash_state(spec.collection, &state).await,
            }
        }
        Err(Error::other("app state patch kept conflicting"))
    }
}

impl crate::client::Downloadable for ExternalBlobReference {
    fn media_kind(&self) -> crate::client::MediaKind {
        crate::client::MediaKind::AppState
    }
    fn media_key(&self) -> Option<&[u8]> {
        self.media_key.as_deref()
    }
    fn direct_path(&self) -> Option<&str> {
        self.direct_path.as_deref()
    }
    fn url(&self) -> Option<&str> {
        None
    }
    fn file_enc_sha256(&self) -> Option<&[u8]> {
        self.file_enc_sha256.as_deref()
    }
    fn file_sha256(&self) -> Option<&[u8]> {
        self.file_sha256.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(
        keys: &MutationKeys,
        key_id: &[u8],
        index: &[&str],
        value: SyncActionValue,
        operation: i32,
    ) -> SyncdRecord {
        let index = serde_json::to_vec(index).unwrap();
        let data = SyncActionData { index: Some(index.clone()), value: Some(value), padding: None, version: Some(2) };
        let iv = [3u8; 16];
        let encrypted =
            [&iv[..], &crypto::aes_cbc_encrypt(&keys.value_encryption, &iv, &data.encode_to_vec()).unwrap()].concat();
        let mac = value_mac(operation, &encrypted, key_id, &keys.value_mac);
        SyncdRecord {
            index: Some(SyncdBlob { blob: Some(crypto::hmac_sha256(&keys.index, &[&index]).to_vec()) }),
            value: Some(SyncdBlob { blob: Some([&encrypted[..], &mac].concat()) }),
            key_id: Some(KeyId { id: Some(key_id.to_vec()) }),
        }
    }

    #[test]
    fn lthash_add_remove() {
        let mut state = HashState::default();
        state.mix(b"index-a", &[1; 32], SET);
        state.mix(b"index-b", &[2; 32], SET);
        assert_ne!(state.hash, [0; 128]);
        state.mix(b"index-a", &[9; 32], SET);
        state.mix(b"index-a", &[9; 32], REMOVE);
        state.mix(b"index-b", &[2; 32], REMOVE);
        assert_eq!(state.hash, [0; 128]);
        assert!(state.values.is_empty());
    }

    #[test]
    fn records_and_state() {
        let key_id = b"key-1".to_vec();
        let keys = MutationKeys::derive(&[7; 32]);
        let mute = SyncActionValue {
            mute_action: Some(MuteAction { muted: Some(true), mute_end_timestamp: Some(99), auto_muted: None }),
            ..Default::default()
        };
        let archived = SyncActionValue {
            archive_chat_action: Some(ArchiveChatAction { archived: Some(true), message_range: None }),
            ..Default::default()
        };
        let records = [
            record(&keys, &key_id, &["mute", "1@s.whatsapp.net"], mute, SET),
            record(&keys, &key_id, &["archive", "2@s.whatsapp.net"], archived, SET),
        ];
        let ring = KeyRing::from([(key_id.clone(), MutationKeys::derive(&[7; 32]))]);
        let mut state = HashState::default();
        let mut out = Vec::new();
        decode_records(records.iter().map(|r| (SET, r)), &mut state, &ring, &mut out).unwrap();
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].index, vec!["mute", "1@s.whatsapp.net"]);
        assert_eq!(out[1].value.archive_chat_action.as_ref().unwrap().archived, Some(true));
        assert_eq!(state.values.len(), 2);

        let restored = HashState::from_bytes(&state.to_bytes()).unwrap();
        assert_eq!(restored.hash, state.hash);
        assert_eq!(restored.values, state.values);
    }

    #[test]
    fn missing_keys() {
        let keys = MutationKeys::derive(&[1; 32]);
        let records = [record(&keys, b"other", &["pin_v1", "1@s.whatsapp.net"], SyncActionValue::default(), SET)];
        let mut out = Vec::new();
        let result =
            decode_records(records.iter().map(|r| (SET, r)), &mut HashState::default(), &KeyRing::new(), &mut out);
        assert!(matches!(result, Err(Error::MissingAppStateKey(_))));
    }
}
