use std::path::{Path, PathBuf};

use rusqlite::{params, Connection, OpenFlags, OptionalExtension, Row};

use super::{apply, collect, failed, key_epoch, Outgoing, ZapoData, ZapoExport, ZapoImport};
use crate::error::Result;
use crate::jid::Jid;
use crate::store::{Creds, Store};

fn has_table(db: &Connection, name: &str) -> rusqlite::Result<bool> {
    db.query_row("SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1", [name], |_| Ok(()))
        .optional()
        .map(|found| found.is_some())
}

fn rows<T>(
    db: &Connection,
    sql: &str,
    session: &str,
    map: impl FnMut(&Row<'_>) -> rusqlite::Result<T>,
) -> rusqlite::Result<Vec<T>> {
    db.prepare(sql)?.query_map([session], map)?.collect()
}

fn read_data(db: &Connection, session: &str) -> rusqlite::Result<Option<ZapoData>> {
    let found = db
        .query_row(
            "SELECT noise_pub_key, noise_priv_key, registration_id, identity_pub_key, identity_priv_key,
                signed_prekey_id, signed_prekey_pub_key, signed_prekey_priv_key, signed_prekey_signature,
                adv_secret_key, signed_identity, me_jid, me_lid, me_display_name, platform, routing_info
             FROM auth_credentials WHERE session_id = ?1",
            [session],
            |row| {
                Ok(ZapoData {
                    noise: (row.get(0)?, row.get(1)?),
                    registration: (row.get(2)?, row.get(3)?, row.get(4)?),
                    signed: (row.get(5)?, row.get(6)?, row.get(7)?, row.get(8)?),
                    adv_secret: row.get(9)?,
                    account: row.get(10)?,
                    me: row.get(11)?,
                    lid: row.get(12)?,
                    name: row.get(13)?,
                    platform: row.get(14)?,
                    routing_info: row.get(15)?,
                    ..Default::default()
                })
            },
        )
        .optional()?;
    let Some(mut data) = found else { return Ok(None) };
    if has_table(db, "signal_registration")? {
        let row = db
            .query_row(
                "SELECT registration_id, identity_pub_key, identity_priv_key FROM signal_registration WHERE session_id = ?1",
                [session],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?;
        data.registration = row.unwrap_or(data.registration);
    }
    if has_table(db, "signal_signed_prekey")? {
        let row = db
            .query_row(
                "SELECT key_id, pub_key, priv_key, signature FROM signal_signed_prekey WHERE session_id = ?1",
                [session],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .optional()?;
        data.signed = row.unwrap_or(data.signed);
    }
    if has_table(db, "signal_session")? {
        data.sessions = rows(
            db,
            "SELECT user, server, device, record FROM signal_session WHERE session_id = ?1",
            session,
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )?;
    }
    if has_table(db, "signal_prekey")? {
        data.pre_keys = rows(db, "SELECT key_id, priv_key FROM signal_prekey WHERE session_id = ?1", session, |row| {
            Ok((row.get(0)?, row.get(1)?))
        })?;
    }
    if has_table(db, "signal_meta")? {
        data.next_pre_key = db
            .query_row("SELECT next_prekey_id FROM signal_meta WHERE session_id = ?1", [session], |row| row.get(0))
            .optional()?;
    }
    if has_table(db, "sender_keys")? {
        data.sender_keys = rows(
            db,
            "SELECT group_id, sender_user, sender_server, sender_device, record FROM sender_keys WHERE session_id = ?1",
            session,
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
        )?;
    }
    if has_table(db, "appstate_sync_keys")? {
        data.app_state_keys =
            rows(db, "SELECT key_id, key_data FROM appstate_sync_keys WHERE session_id = ?1", session, |row| {
                Ok((row.get(0)?, row.get(1)?))
            })?;
    }
    if has_table(db, "appstate_collection_versions")? {
        data.versions = rows(
            db,
            "SELECT collection, version, hash FROM appstate_collection_versions WHERE session_id = ?1",
            session,
            |row| Ok((row.get(0)?, row.get::<_, i64>(1)?.max(0) as u64, row.get(2)?)),
        )?;
    }
    if has_table(db, "appstate_collection_index_values")? {
        data.index_values = rows(
            db,
            "SELECT collection, index_mac_hex, value_mac FROM appstate_collection_index_values WHERE session_id = ?1",
            session,
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?;
    }
    if has_table(db, "mailbox_contacts")? {
        data.lid_pairs.extend(rows(
            db,
            "SELECT phone_number, lid FROM mailbox_contacts WHERE session_id = ?1",
            session,
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?);
    }
    if has_table(db, "device_list_cache")? && db.prepare("SELECT alt_user_jid FROM device_list_cache LIMIT 0").is_ok() {
        data.lid_pairs.extend(rows(
            db,
            "SELECT user_jid, alt_user_jid FROM device_list_cache WHERE session_id = ?1",
            session,
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?);
    }
    Ok(Some(data))
}

fn open_read_only(path: &Path) -> Result<Connection> {
    Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX)
        .map_err(|err| failed(format!("cannot open {}: {err}", path.display())))
}

pub async fn import_zapo_sqlite(path: impl AsRef<Path>, session: &str, store: &dyn Store) -> Result<ZapoImport> {
    let path: PathBuf = path.as_ref().to_path_buf();
    let session = session.to_owned();
    let snapshot = tokio::task::spawn_blocking(move || {
        read_data(&open_read_only(&path)?, &session)
            .map_err(failed)?
            .ok_or_else(|| failed(format!("there are no credentials for the session {session:?}")))?
            .convert()
    })
    .await
    .map_err(failed)??;
    apply(snapshot, store).await
}

pub fn zapo_sessions(path: impl AsRef<Path>) -> Result<Vec<String>> {
    let db = open_read_only(path.as_ref())?;
    let mut statement = db.prepare("SELECT session_id FROM auth_credentials ORDER BY session_id").map_err(failed)?;
    let sessions = statement.query_map([], |row| row.get::<_, String>(0)).map_err(failed)?;
    sessions.collect::<rusqlite::Result<_>>().map_err(failed)
}
fn write_back(
    db: &mut Connection,
    session: &str,
    creds: &Creds,
    out: &Outgoing,
    report: &mut ZapoExport,
) -> Result<()> {
    for table in ["auth_credentials", "signal_session", "signal_identity", "signal_prekey", "sender_keys"] {
        if !has_table(db, table).map_err(failed)? {
            return Err(failed(format!("{table} is missing, open the database with zapo once before rolling back")));
        }
    }
    let exists = db
        .query_row("SELECT 1 FROM auth_credentials WHERE session_id = ?1", [session], |_| Ok(()))
        .optional()
        .map_err(failed)?
        .is_some();
    if !exists {
        return Err(failed(format!("the zapo database has no session {session:?} to roll back into")));
    }
    let optional: Vec<&str> = [
        "sender_key_distribution",
        "signal_registration",
        "signal_signed_prekey",
        "signal_meta",
        "appstate_sync_keys",
        "appstate_collection_versions",
        "appstate_collection_index_values",
    ]
    .into_iter()
    .filter(|table| has_table(db, table).unwrap_or(false))
    .collect();
    let has = |table: &str| optional.contains(&table);
    let now_ms = crate::message::now_secs() * 1000;

    let tx = db.transaction().map_err(failed)?;
    tx.execute(
        "UPDATE auth_credentials SET registration_id = ?2, identity_pub_key = ?3, identity_priv_key = ?4,
            signed_prekey_id = ?5, signed_prekey_pub_key = ?6, signed_prekey_priv_key = ?7, signed_prekey_signature = ?8,
            signed_identity = COALESCE(?9, signed_identity), me_jid = COALESCE(?10, me_jid), me_lid = COALESCE(?11, me_lid),
            me_display_name = COALESCE(?12, me_display_name), routing_info = COALESCE(?13, routing_info)
         WHERE session_id = ?1",
        params![
            session,
            creds.registration_id,
            creds.identity_key.public.to_vec(),
            creds.identity_key.private.to_vec(),
            creds.signed_pre_key.id,
            creds.signed_pre_key.key_pair.public.to_vec(),
            creds.signed_pre_key.key_pair.private.to_vec(),
            creds.signed_pre_key.signature.to_vec(),
            creds.account,
            creds.me.as_ref().map(Jid::to_string),
            creds.lid.as_ref().map(Jid::to_string),
            creds.push_name,
            creds.routing_info,
        ],
    )
    .map_err(failed)?;
    if has("signal_registration") {
        tx.execute(
            "INSERT INTO signal_registration (session_id, registration_id, identity_pub_key, identity_priv_key)
                VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(session_id) DO UPDATE SET registration_id = excluded.registration_id,
                identity_pub_key = excluded.identity_pub_key, identity_priv_key = excluded.identity_priv_key",
            params![
                session,
                creds.registration_id,
                creds.identity_key.public.to_vec(),
                creds.identity_key.private.to_vec()
            ],
        )
        .map_err(failed)?;
    }
    if has("signal_signed_prekey") {
        tx.execute(
            "INSERT INTO signal_signed_prekey (session_id, key_id, pub_key, priv_key, signature, uploaded)
                VALUES (?1, ?2, ?3, ?4, ?5, 1)
             ON CONFLICT(session_id) DO UPDATE SET key_id = excluded.key_id, pub_key = excluded.pub_key,
                priv_key = excluded.priv_key, signature = excluded.signature, uploaded = 1",
            params![
                session,
                creds.signed_pre_key.id,
                creds.signed_pre_key.key_pair.public.to_vec(),
                creds.signed_pre_key.key_pair.private.to_vec(),
                creds.signed_pre_key.signature.to_vec(),
            ],
        )
        .map_err(failed)?;
    }
    if has("signal_meta") {
        tx.execute(
            "INSERT INTO signal_meta (session_id, server_has_prekeys, next_prekey_id) VALUES (?1, 1, ?2)
             ON CONFLICT(session_id) DO UPDATE SET next_prekey_id = excluded.next_prekey_id",
            params![session, creds.next_pre_key_id],
        )
        .map_err(failed)?;
    }

    let mut wipe = vec!["signal_session", "signal_identity", "signal_prekey", "sender_keys"];
    wipe.extend(optional.iter().copied().filter(|t| {
        matches!(*t, "sender_key_distribution" | "appstate_collection_versions" | "appstate_collection_index_values")
    }));
    for table in wipe {
        tx.execute(&format!("DELETE FROM {table} WHERE session_id = ?1"), [session]).map_err(failed)?;
    }
    {
        let mut insert = tx
            .prepare(
                "INSERT INTO signal_session (session_id, user, server, device, record) VALUES (?1, ?2, ?3, ?4, ?5)",
            )
            .map_err(failed)?;
        let mut identity = tx
            .prepare(
                "INSERT INTO signal_identity (session_id, user, server, device, identity_key) VALUES (?1, ?2, ?3, ?4, ?5)",
            )
            .map_err(failed)?;
        for (user, server, device, record, remote) in &out.sessions {
            insert.execute(params![session, user, server, device, record]).map_err(failed)?;
            if let Some(remote) = remote {
                identity.execute(params![session, user, server, device, remote]).map_err(failed)?;
            }
            report.sessions += 1;
        }
        let mut pre_key = tx
            .prepare("INSERT INTO signal_prekey (session_id, key_id, pub_key, priv_key, uploaded) VALUES (?1, ?2, ?3, ?4, ?5)")
            .map_err(failed)?;
        for (id, public, private, uploaded) in &out.pre_keys {
            pre_key.execute(params![session, id, public.to_vec(), private.to_vec(), *uploaded]).map_err(failed)?;
            report.pre_keys += 1;
        }
        let mut sender_key = tx
            .prepare(
                "INSERT INTO sender_keys (session_id, group_id, sender_user, sender_server, sender_device, record)
                    VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            )
            .map_err(failed)?;
        for (group, user, server, device, record) in &out.sender_keys {
            sender_key.execute(params![session, group, user, server, device, record]).map_err(failed)?;
            report.sender_keys += 1;
        }
    }
    if has("appstate_sync_keys") {
        let mut key = tx
            .prepare(
                "INSERT INTO appstate_sync_keys (session_id, key_id, key_data, timestamp, key_epoch) VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(session_id, key_id) DO UPDATE SET key_data = excluded.key_data,
                    key_epoch = excluded.key_epoch,
                    timestamp = MAX(appstate_sync_keys.timestamp, excluded.timestamp)",
            )
            .map_err(failed)?;
        for (id, data, newest) in &out.app_state_keys {
            key.execute(params![session, id, data, if *newest { now_ms } else { 0 }, key_epoch(id)]).map_err(failed)?;
            report.app_state_keys += 1;
        }
    }
    if has("appstate_collection_versions") {
        let mut version = tx
            .prepare(
                "INSERT INTO appstate_collection_versions (session_id, collection, version, hash) VALUES (?1, ?2, ?3, ?4)",
            )
            .map_err(failed)?;
        for (name, state) in &out.versions {
            version.execute(params![session, name, state.version as i64, state.hash]).map_err(failed)?;
            report.app_state_versions += 1;
        }
        if has("appstate_collection_index_values") {
            let mut value = tx
                .prepare(
                    "INSERT OR REPLACE INTO appstate_collection_index_values
                        (session_id, collection, index_mac_hex, value_mac) VALUES (?1, ?2, ?3, ?4)",
                )
                .map_err(failed)?;
            for (name, state) in &out.versions {
                for entry in &state.entries {
                    value
                        .execute(params![session, name, hex::encode(&entry.index_mac), entry.value_mac])
                        .map_err(failed)?;
                }
            }
        }
    }
    tx.commit().map_err(failed)
}

pub async fn export_zapo_sqlite(store: &dyn Store, path: impl AsRef<Path>, session: &str) -> Result<ZapoExport> {
    store.flush().await?;
    let creds = store.load_creds().await?.ok_or_else(|| failed("the store has no credentials to roll back"))?;
    let mut report = ZapoExport::default();
    let out = collect(store, &creds, &mut report).await?;
    let path = path.as_ref().to_path_buf();
    let session = session.to_owned();
    tokio::task::spawn_blocking(move || {
        let mut db =
            Connection::open_with_flags(&path, OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX)
                .map_err(|err| failed(format!("cannot open {}: {err}", path.display())))?;
        db.busy_timeout(std::time::Duration::from_secs(10)).map_err(failed)?;
        write_back(&mut db, &session, &creds, &out, &mut report)?;
        Ok(report)
    })
    .await
    .map_err(failed)?
}
