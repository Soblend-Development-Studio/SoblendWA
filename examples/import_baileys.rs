use soblendwa::store::import_baileys;
use soblendwa::{FileStore, Store};

#[tokio::main]
async fn main() -> soblendwa::Result<()> {
    let mut args = std::env::args().skip(1);
    let (Some(source), target) = (args.next(), args.next().unwrap_or_else(|| "session".into())) else {
        eprintln!("usage: cargo run --example import_baileys -- <baileys auth_info folder> [soblendwa session folder]");
        return Ok(());
    };
    println!("{}", soblendwa::credits());

    let store = FileStore::open(&target).await?;
    if store.load_creds().await?.is_some_and(|creds| creds.is_logged_in()) {
        eprintln!("{target} already holds a linked session; pick an empty folder");
        return Ok(());
    }
    let report = import_baileys(&source, store.as_ref()).await?;

    println!("Imported {source} into {target}");
    println!("  account:          {}", report.me.map(|j| j.to_string()).unwrap_or_else(|| "not linked".into()));
    println!("  signal sessions:  {}", report.sessions);
    println!("  pre-keys:         {}", report.pre_keys);
    println!("  group keys:       {}", report.sender_keys);
    println!("  app state keys:   {} ({} collections)", report.app_state_keys, report.app_state_versions);
    println!("  lid mappings:     {}", report.lid_mappings);
    if !report.skipped.is_empty() {
        println!("  rebuilt on demand: {} files (device lists, sender key memory, tokens)", report.skipped.len());
    }
    for (file, err) in &report.failed {
        println!("  could not import {file}: {err}");
    }
    println!("Stop the Baileys bot before starting SoblendWA: both cannot use the same session at once.");
    Ok(())
}
