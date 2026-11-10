use soblendwa::{Client, Event, FileStore};

#[tokio::main]
async fn main() -> soblendwa::Result<()> {
    let Some(phone) = std::env::args().nth(1) else {
        eprintln!("usage: cargo run --example pair_code -- <phone number with country code>");
        return Ok(());
    };
    println!("{}", soblendwa::credits());

    let store = FileStore::open("session").await?;
    let (client, mut events) = Client::builder().store(store).build().await?;
    client.connect().await?;

    let mut requested = false;
    while let Some(event) = events.recv().await {
        match event {
            Event::Qr(_) if !requested && !client.is_logged_in() => {
                requested = true;
                let code = client.request_pair_code(&phone).await?;
                println!("WhatsApp > Linked devices > Link with phone number");
                println!("Code: {}-{}", &code[..4], &code[4..]);
            }
            Event::PairSuccess { id, .. } => println!("Linked as {id}"),
            Event::Connected => println!("Connected, the session is stored in ./session"),
            Event::LoggedOut { reason, .. } => {
                println!("Logged out: {reason}");
                break;
            }
            _ => {}
        }
    }
    Ok(())
}
