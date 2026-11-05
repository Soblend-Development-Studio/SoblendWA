use soblendwa::store::{export_zapo_sqlite, import_zapo_sqlite, zapo_sessions};
use soblendwa::{FileStore, Store};

#[cfg(feature = "mongo")]
use soblendwa::store::{export_zapo_mongo, import_zapo_mongo, ZapoMongo};

const USAGE: &str = "usage:
  cargo run --example import_zapo --features \"sqlite mongo\" -- --mongo <url> <database> [zapo session id] [soblendwa session folder]
  cargo run --example import_zapo --features \"sqlite mongo\" -- --rollback-mongo <soblendwa session folder> <url> <database> [zapo session id]
  cargo run --example import_zapo --features sqlite -- <zapo.sqlite> [zapo session id] [soblendwa session folder]
  cargo run --example import_zapo --features sqlite -- --rollback <soblendwa session folder> <zapo.sqlite> [zapo session id]";

fn pick_session(db: &str, given: Option<String>) -> soblendwa::Result<Option<String>> {
    if given.is_some() {
        return Ok(given);
    }
    let sessions = zapo_sessions(db)?;
    match sessions.as_slice() {
        [only] => Ok(Some(only.clone())),
        [] => {
            eprintln!("{db} has no zapo session");
            Ok(None)
        }
        many => {
            eprintln!("{db} holds several sessions, name one: {}", many.join(", "));
            Ok(None)
        }
    }
}

async fn import(db: String, session: Option<String>, target: String) -> soblendwa::Result<()> {
    let Some(session) = pick_session(&db, session)? else { return Ok(()) };
    let store = FileStore::open(&target).await?;
    if store.load_creds().await?.is_some_and(|creds| creds.is_logged_in()) {
        eprintln!("{target} already holds a linked session; pick an empty folder");
        return Ok(());
    }
    let report = import_zapo_sqlite(&db, &session, store.as_ref()).await?;

    println!("Imported the zapo session {session:?} from {db} into {target}");
    println!("  account:          {}", report.me.map(|j| j.to_string()).unwrap_or_else(|| "not linked".into()));
    println!("  signal sessions:  {}", report.sessions);
    println!("  pre-keys:         {}", report.pre_keys);
    println!("  group keys:       {}", report.sender_keys);
    println!("  app state keys:   {} ({} collections)", report.app_state_keys, report.app_state_versions);
    println!("  lid mappings:     {}", report.lid_mappings);
    for (item, err) in &report.failed {
        println!("  could not import {item}: {err}");
    }
    println!("The zapo database was opened read-only and is untouched.");
    println!("Stop the zapo bot before starting SoblendWA: both cannot use the same session at once.");
    println!(
        "To go back later: cargo run --example import_zapo --features sqlite -- --rollback {target} {db} {session}"
    );
    Ok(())
}

async fn rollback(source: String, db: String, session: Option<String>) -> soblendwa::Result<()> {
    let Some(session) = pick_session(&db, session)? else { return Ok(()) };
    let store = FileStore::open(&source).await?;
    let report = export_zapo_sqlite(store.as_ref(), &db, &session).await?;

    println!("Wrote the SoblendWA state of {source} back into the zapo session {session:?} of {db}");
    println!("  signal sessions:  {}", report.sessions);
    println!("  pre-keys:         {}", report.pre_keys);
    println!("  group keys:       {}", report.sender_keys);
    println!("  app state keys:   {} ({} collections)", report.app_state_keys, report.app_state_versions);
    for (item, err) in &report.failed {
        println!("  could not export {item}: {err}");
    }
    println!("Stop SoblendWA before starting the zapo bot again.");
    Ok(())
}

#[cfg(feature = "mongo")]
fn pick_mongo_session(sessions: Vec<String>, given: Option<String>) -> Option<String> {
    if given.is_some() {
        return given;
    }
    match sessions.as_slice() {
        [only] => Some(only.clone()),
        [] => {
            eprintln!("the database has no zapo session");
            None
        }
        many => {
            eprintln!("the database holds several sessions, name one: {}", many.join(", "));
            None
        }
    }
}

#[cfg(feature = "mongo")]
async fn import_mongo(url: String, database: String, session: Option<String>, target: String) -> soblendwa::Result<()> {
    let source = ZapoMongo::connect(&url, &database).await?;
    let Some(session) = pick_mongo_session(source.sessions().await?, session) else { return Ok(()) };
    let store = FileStore::open(&target).await?;
    if store.load_creds().await?.is_some_and(|creds| creds.is_logged_in()) {
        eprintln!("{target} already holds a linked session; pick an empty folder");
        return Ok(());
    }
    let report = import_zapo_mongo(&source, &session, store.as_ref()).await?;
    println!("Imported the zapo session {session:?} from the database {database} into {target}");
    println!("  account:          {}", report.me.map(|j| j.to_string()).unwrap_or_else(|| "not linked".into()));
    println!("  signal sessions:  {}", report.sessions);
    println!("  pre-keys:         {}", report.pre_keys);
    println!("  group keys:       {}", report.sender_keys);
    println!("  app state keys:   {} ({} collections)", report.app_state_keys, report.app_state_versions);
    println!("  lid mappings:     {}", report.lid_mappings);
    for (item, err) in &report.failed {
        println!("  could not import {item}: {err}");
    }
    println!("Nothing was written to MongoDB. Stop the zapo bot before starting SoblendWA.");
    Ok(())
}

#[cfg(feature = "mongo")]
async fn rollback_mongo(
    source: String,
    url: String,
    database: String,
    session: Option<String>,
) -> soblendwa::Result<()> {
    let target = ZapoMongo::connect(&url, &database).await?;
    let Some(session) = pick_mongo_session(target.sessions().await?, session) else { return Ok(()) };
    let store = FileStore::open(&source).await?;
    let report = export_zapo_mongo(store.as_ref(), &target, &session).await?;
    println!("Wrote the SoblendWA state of {source} back into the zapo session {session:?} of {database}");
    println!("  signal sessions:  {}", report.sessions);
    println!("  pre-keys:         {}", report.pre_keys);
    println!("  group keys:       {}", report.sender_keys);
    println!("  app state keys:   {} ({} collections)", report.app_state_keys, report.app_state_versions);
    for (item, err) in &report.failed {
        println!("  could not export {item}: {err}");
    }
    println!("Stop SoblendWA before starting the zapo bot again.");
    Ok(())
}

#[tokio::main]
async fn main() -> soblendwa::Result<()> {
    let mut args = std::env::args().skip(1);
    let first = args.next();
    println!("{}", soblendwa::credits());
    match first.as_deref() {
        #[cfg(feature = "mongo")]
        Some("--mongo") => match (args.next(), args.next()) {
            (Some(url), Some(database)) => {
                let session = args.next();
                let target = args.next().unwrap_or_else(|| "session".into());
                import_mongo(url, database, session, target).await
            }
            _ => {
                eprintln!("{USAGE}");
                Ok(())
            }
        },
        #[cfg(feature = "mongo")]
        Some("--rollback-mongo") => match (args.next(), args.next(), args.next()) {
            (Some(source), Some(url), Some(database)) => rollback_mongo(source, url, database, args.next()).await,
            _ => {
                eprintln!("{USAGE}");
                Ok(())
            }
        },
        Some("--rollback") => match (args.next(), args.next()) {
            (Some(source), Some(db)) => rollback(source, db, args.next()).await,
            _ => {
                eprintln!("{USAGE}");
                Ok(())
            }
        },
        Some(db) => {
            let session = args.next();
            let target = args.next().unwrap_or_else(|| "session".into());
            import(db.to_owned(), session, target).await
        }
        None => {
            eprintln!("{USAGE}");
            Ok(())
        }
    }
}
