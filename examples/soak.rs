use std::collections::{BTreeMap, HashMap};
use std::fs::OpenOptions;
use std::io::Write as _;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use qrcode::render::unicode::Dense1x2;
use qrcode::QrCode;
use serde_json::{json, Value};
use soblendwa::{Client, Event, FileStore, Jid, Message, MessageEvent};
use tracing_subscriber::EnvFilter;

const COMMAND: &str = "!yowner";
const SNAPSHOT_EVERY: Duration = Duration::from_secs(10 * 60);

const USAGE: &str = "Uso: soak [carpeta de la sesión] [--pair 5215512345678]
       soak [carpeta de la sesión] --resumen

Se queda conectado días o semanas y anota en <carpeta>/soak.jsonl cuántos mensajes llegan, cuántos no se pudieron
descifrar, las reconexiones y la memoria. No guarda textos ni números.

Solo responde cuando tú escribes !yowner desde tu teléfono en un chat privado (mejor en el chat contigo mismo).
!yowner ayuda muestra las pruebas que puedes hacer con un solo número.";

const HELP: &str = "Pruebas con tu propio número (escríbelas aquí):
!yowner · resumen de la prueba larga
!yowner ping · cuánto tarda el bot y la red
!yowner fases · tiempos de los envíos por fase, con tu red
!yowner cita · te responde citándote y mencionándote
!yowner lid · tu número y tu LID, y la consulta en los dos sentidos
!yowner foto · reenvía la última foto que mandaste en este chat (la segunda vez no se vuelve a subir)
Además cuenta solo: crea un grupo, sal de uno o entra por enlace y aparece en el resumen.";

fn now_millis() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64)
}

// TODO: Windows. Only Linux has /proc, so on a PC this just shows "?".
fn rss_mb() -> Option<u64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    let line = status.lines().find(|line| line.starts_with("VmRSS:"))?;
    let kb: u64 = line.split_whitespace().nth(1)?.parse().ok()?;
    Some(kb / 1024)
}

fn error_kind(error: &str) -> &'static str {
    let error = error.to_lowercase();
    if error.contains("sender key") {
        "sender_key"
    } else if error.contains("mac") {
        "bad_mac"
    } else if error.contains("pre-key") || error.contains("prekey") {
        "prekey"
    } else if error.contains("session") {
        "no_session"
    } else {
        "other"
    }
}

fn is_private(chat: &Jid) -> bool {
    chat.is_pn() || chat.is_lid()
}

fn owner_command(message: &MessageEvent) -> Option<String> {
    let info = &message.info;
    if !info.from_me || !is_private(&info.chat) || info.is_older_than(Duration::from_secs(120)) {
        return None;
    }
    let mut words = message.text()?.split_whitespace();
    if !words.next()?.eq_ignore_ascii_case(COMMAND) {
        return None;
    }
    let rest: Vec<&str> = words.collect();
    (rest.len() <= 1).then(|| rest.first().map(|word| word.to_lowercase()).unwrap_or_default())
}

struct Report {
    path: PathBuf,
    run: u64,
}

impl Report {
    fn write(&self, event: &str, fields: Value) {
        let mut line = json!({ "at": now_millis(), "run": self.run, "event": event });
        if let (Value::Object(target), Value::Object(extra)) = (&mut line, fields) {
            target.extend(extra);
        }
        let written =
            OpenOptions::new().create(true).append(true).open(&self.path).and_then(|mut file| writeln!(file, "{line}"));
        if let Err(err) = written {
            eprintln!("no pude escribir el reporte: {err}");
        }
    }

    fn snapshot(&self, client: &Client) {
        self.write("stats", json!({ "stats": client.stats().to_json(), "rssMb": rss_mb() }));
    }

    fn history(&self) -> Vec<Value> {
        std::fs::read_to_string(&self.path)
            .unwrap_or_default()
            .lines()
            .filter_map(|line| serde_json::from_str(line).ok())
            .collect()
    }
}

#[derive(Default, Debug, PartialEq)]
struct Totals {
    since: Option<u64>,
    runs: usize,
    received: u64,
    sent: u64,
    undecryptable: u64,
    recovered: u64,
    lost: u64,
    reconnects: u64,
    panics: u64,
    disconnects: u64,
    max_rss: u64,
    joined: u64,
    left: u64,
    lid_pairs: u64,
    kinds: BTreeMap<String, [u64; 3]>,
    unresolved: u64,
}

fn totals(history: &[Value], run: u64, live: &Value) -> Totals {
    let mut last = BTreeMap::new();
    let mut out = Totals::default();
    let mut open: BTreeMap<u64, i64> = BTreeMap::new();
    for line in history {
        out.since = out.since.or(line["at"].as_u64());
        let line_run = line["run"].as_u64().unwrap_or_default();
        let kind = line["kind"].as_str().unwrap_or("desconocido").to_owned();
        match line["event"].as_str() {
            Some("start") => out.runs += 1,
            Some("disconnected") => out.disconnects += 1,
            Some("joinedGroup") => out.joined += 1,
            Some("leftGroup") => out.left += 1,
            Some("lidMapping") => out.lid_pairs += 1,
            Some("undecryptable") => {
                out.kinds.entry(kind).or_default()[0] += 1;
                *open.entry(line_run).or_default() += 1;
            }
            Some("recovered") => {
                out.kinds.entry(kind).or_default()[1] += 1;
                *open.entry(line_run).or_default() -= 1;
            }
            Some("lost") => {
                out.kinds.entry(kind).or_default()[2] += 1;
                *open.entry(line_run).or_default() -= 1;
            }
            Some("stats") => {
                out.max_rss = out.max_rss.max(line["rssMb"].as_u64().unwrap_or_default());
                if line_run != run {
                    last.insert(line_run, line["stats"].clone());
                }
            }
            _ => {}
        }
    }
    let field = |name: &str| last.values().chain([live]).map(|stats| stats[name].as_u64().unwrap_or_default()).sum();
    out.received = field("received");
    out.sent = field("sent");
    out.undecryptable = field("undecryptable");
    out.recovered = field("recovered");
    out.lost = field("lost");
    out.reconnects = field("reconnects");
    out.panics = field("panics");
    out.unresolved =
        open.iter().filter(|(line_run, _)| **line_run != run).map(|(_, count)| (*count).max(0) as u64).sum();
    out
}

fn describe(all: &Totals) -> String {
    let days = all.since.map_or(0.0, |since| now_millis().saturating_sub(since) as f64 / 86_400_000.0);
    let percent = if all.received == 0 { 0.0 } else { all.undecryptable as f64 * 100.0 / all.received as f64 };
    let per_day = if days >= 1.0 {
        format!("{:.1} por día", all.undecryptable as f64 / days)
    } else {
        "el promedio por día sale después del primer día".to_owned()
    };
    let kinds = all
        .kinds
        .iter()
        .filter(|(_, [failed, _, _])| *failed > 0)
        .map(|(kind, [failed, recovered, lost])| format!("{kind} {failed} (recuperados {recovered}, perdidos {lost})"))
        .collect::<Vec<_>>()
        .join(", ");
    let memory = if all.max_rss == 0 { "?".to_owned() } else { format!("{} MB", all.max_rss) };
    let mut lines = vec![
        format!("Prueba: {days:.1} días, {} arranques", all.runs.max(1)),
        format!("Recibidos {} · enviados {}", all.received, all.sent),
        format!("Sin descifrar {} ({percent:.2}%) · {per_day}", all.undecryptable),
    ];
    if !kinds.is_empty() {
        lines.push(format!("Tipos: {kinds}"));
    }
    lines.extend([
        format!(
            "Recuperados {} · perdidos {} · sin resolver por reinicios {}",
            all.recovered, all.lost, all.unresolved
        ),
        format!("Reconexiones {} · desconexiones {} · pánicos {}", all.reconnects, all.disconnects, all.panics),
        format!("Grupos: entró a {} · salió de {} · pares número/LID nuevos {}", all.joined, all.left, all.lid_pairs),
        format!("Memoria máxima {memory}"),
    ]);
    lines.join("\n")
}

fn summary(client: &Client, report: &Report) -> String {
    let stats = client.stats();
    let mut all = totals(&report.history(), report.run, &stats.to_json());
    let rss = rss_mb();
    all.max_rss = all.max_rss.max(rss.unwrap_or_default());
    let up = stats.uptime.as_secs();
    let memory = rss.map_or_else(|| "?".to_owned(), |mb| format!("{mb} MB"));
    let header = format!(
        "SoblendWA {} · prendido hace {}d {}h {}m · memoria {memory} · esperando reintento {}",
        soblendwa::VERSION,
        up / 86_400,
        up % 86_400 / 3_600,
        up % 3_600 / 60,
        stats.awaiting_recovery
    );
    let ms = |spread: &soblendwa::Spread| {
        format!("{:.0}/{:.0} ms", spread.p50.as_secs_f64() * 1e3, spread.p95.as_secs_f64() * 1e3)
    };
    let send = &stats.send_timings;
    let sends = if send.samples == 0 {
        "Envíos: todavía no hay ninguno medido".to_owned()
    } else {
        format!(
            "Envíos p50/p95: total {} · WhatsApp {}",
            ms(&send.total),
            send.server.as_ref().map_or_else(|| "-".to_owned(), ms)
        )
    };
    [header, describe(&all), sends].join("\n")
}

fn phases(client: &Client) -> String {
    let timings = client.stats().send_timings;
    if timings.samples == 0 {
        return "Todavía no hay envíos medidos. Prueba con !yowner ping y vuelve a pedirlo.".to_owned();
    }
    let row = |name: &str, spread: Option<&soblendwa::Spread>| match spread {
        Some(s) => format!(
            "{name}: {:.1} / {:.1} / {:.1} ms",
            s.p50.as_secs_f64() * 1e3,
            s.p95.as_secs_f64() * 1e3,
            s.max.as_secs_f64() * 1e3
        ),
        None => format!("{name}: -"),
    };
    [
        format!("Últimos {} envíos, p50 / p95 / máximo", timings.samples),
        row("cola", Some(&timings.queued)),
        row("subida", timings.upload.as_ref()),
        row("preparar", Some(&timings.prepare)),
        row("cifrar", Some(&timings.encrypt)),
        row("WhatsApp", timings.server.as_ref()),
        row("total", Some(&timings.total)),
    ]
    .join("\n")
}

async fn answer(
    client: &Client,
    report: &Report,
    message: &MessageEvent,
    command: &str,
    last_image: Option<&MessageEvent>,
) -> soblendwa::Result<()> {
    let chat = &message.info.chat;
    let text = match command {
        "" => summary(client, report),
        "ayuda" => HELP.to_owned(),
        "ping" => format!(
            "Pong · {:.2} ms dentro del bot · red {} ms",
            message.info.elapsed().as_secs_f64() * 1e3,
            client.latency().map_or_else(|| "?".to_owned(), |d| d.as_millis().to_string())
        ),
        "fases" => phases(client),
        "cita" => {
            let me = client.me().map(|j| j.to_non_device()).unwrap_or_else(|| chat.clone());
            let quoted =
                Message::text(format!("@{} esto es una cita con mención", me.user)).mention([&me]).reply_to(message);
            client.send_message(chat, quoted).await?;
            return Ok(());
        }
        "lid" => {
            let phone = client.me().map(|j| j.to_non_device());
            let lid = client.lid().map(|j| j.to_non_device());
            let lid_found = match &phone {
                Some(phone) => client.lid_for_phone(phone).await?,
                None => None,
            };
            let phone_found = match &lid {
                Some(lid) => client.phone_for_lid(lid).await?,
                None => None,
            };
            let show = |jid: &Option<Jid>| jid.as_ref().map_or_else(|| "-".to_owned(), Jid::to_string);
            format!(
                "Número: {}\nLID: {}\nlid_for_phone: {}\nphone_for_lid: {}",
                show(&phone),
                show(&lid),
                show(&lid_found),
                show(&phone_found)
            )
        }
        "foto" => {
            let Some(image) = last_image else {
                client.send_text(chat, "Manda una foto en este chat y luego !yowner foto.").await?;
                return Ok(());
            };
            let data = client.download_message(&image.message).await?;
            client.send_image(chat, &data, Some(&format!("La misma foto, {} bytes", data.len()))).await?;
            let stats = client.stats();
            format!("Subidas: {} · reutilizadas sin volver a subir: {}", stats.media_uploaded, stats.media_reused)
        }
        other => format!("No conozco \"{other}\". Escribe !yowner ayuda."),
    };
    println!("{text}");
    client.send_text(chat, text).await?;
    Ok(())
}

fn show_qr(code: &str) {
    match QrCode::new(code.as_bytes()) {
        Ok(qr) => println!(
            "\nEscanea este código en WhatsApp > Dispositivos vinculados:\n{}",
            qr.render::<Dense1x2>().quiet_zone(true).build()
        ),
        Err(_) => println!("QR: {code}"),
    }
}

async fn stop_signal() {
    #[cfg(unix)]
    {
        let mut term =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()).expect("SIGTERM handler");
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {}
            _ = term.recv() => {}
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}

#[tokio::main]
async fn main() -> soblendwa::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("soblendwa=error")))
        .init();
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!("{USAGE}");
        return Ok(());
    }
    let mut pair = args.iter().position(|arg| arg == "--pair").and_then(|at| args.get(at + 1)).cloned();
    let folder = args
        .iter()
        .enumerate()
        .find(|(at, arg)| !arg.starts_with("--") && (*at == 0 || args[at - 1] != "--pair"))
        .map_or_else(|| "sesion-soak".to_owned(), |(_, arg)| arg.clone());
    if args.iter().any(|arg| arg == "--resumen") {
        let report = Report { path: PathBuf::from(&folder).join("soak.jsonl"), run: 0 };
        println!("{}", describe(&totals(&report.history(), 0, &json!({}))));
        return Ok(());
    }

    let store = FileStore::open(&folder).await?;
    let report = Arc::new(Report { path: PathBuf::from(&folder).join("soak.jsonl"), run: now_millis() });
    let (client, mut events) =
        Client::builder().store(store).online_on_connect(false).device_name("SoblendWA").build().await?;
    report.write("start", json!({ "version": soblendwa::VERSION }));
    println!("{} · sesión en {folder} · reporte en {}", soblendwa::credits(), report.path.display());
    client.connect().await?;

    let mut every = tokio::time::interval(SNAPSHOT_EVERY);
    every.tick().await;
    let stop = stop_signal();
    tokio::pin!(stop);
    let mut asked_code = false;
    let mut last_image: HashMap<Jid, MessageEvent> = HashMap::new();
    let mut pending: HashMap<String, &'static str> = HashMap::new();

    loop {
        let event = tokio::select! {
            _ = &mut stop => {
                report.snapshot(&client);
                client.disconnect().await;
                println!("Detenido; la sesión quedó guardada.");
                return Ok(());
            }
            _ = every.tick() => {
                report.snapshot(&client);
                continue;
            }
            event = events.recv() => match event {
                Some(event) => event,
                None => return Ok(()),
            },
        };
        match event {
            Event::Qr(code) => match pair.clone() {
                Some(number) if !asked_code => {
                    asked_code = true;
                    match client.request_pair_code(&number).await {
                        Ok(pair_code) => {
                            println!();
                            println!("Código para vincular: {pair_code}");
                            println!("En el teléfono: Dispositivos vinculados > Vincular dispositivo > Vincular con número de teléfono");
                        }
                        Err(err) => {
                            eprintln!("no pude pedir el código ({err}); usa el QR");
                            pair = None;
                            show_qr(&code);
                        }
                    }
                }
                Some(_) => {}
                None => show_qr(&code),
            },
            Event::Connected => {
                println!(
                    "Conectado como {}. Escribe {COMMAND} en tu chat contigo mismo.",
                    client.me().map(|j| j.to_string()).unwrap_or_default()
                );
                report.write("connected", json!({}));
            }
            Event::Disconnected { reason, reconnecting } => {
                report.write("disconnected", json!({ "reason": format!("{reason:?}"), "reconnecting": reconnecting }));
            }
            Event::LoggedOut { cause, code, .. } => {
                println!("Se cerró la sesión: {} ({code:?}). Hay que volver a vincular.", cause.as_str());
                report.write("loggedOut", json!({ "cause": cause.as_str(), "code": code }));
                report.snapshot(&client);
                return Ok(());
            }
            Event::AccountBlocked { code, reason, .. } => {
                println!("WhatsApp bloqueó la cuenta ({code}): {reason}. La sesión se conserva.");
                report.write("accountBlocked", json!({ "code": code, "reason": reason }));
                report.snapshot(&client);
                return Ok(());
            }
            Event::UndecryptableMessage { info, error } => {
                let kind = error_kind(&error);
                pending.insert(info.id, kind);
                report.write("undecryptable", json!({ "kind": kind }));
            }
            Event::MessageRecovered { id, .. } => {
                let kind = pending.remove(&id).unwrap_or("desconocido");
                report.write("recovered", json!({ "kind": kind }));
            }
            Event::MessageLost { id, waited, .. } => {
                let kind = pending.remove(&id).unwrap_or("desconocido");
                report.write("lost", json!({ "kind": kind, "waitedSecs": waited.as_secs() }));
            }
            Event::JoinedGroup { group, via, .. } => {
                println!("entré al grupo {group} ({})", via.as_str());
                report.write("joinedGroup", json!({ "via": via.as_str() }));
            }
            Event::LeftGroup { group, removed, .. } => {
                println!("salí del grupo {group}{}", if removed { " (me sacaron)" } else { "" });
                report.write("leftGroup", json!({ "removed": removed }));
            }
            Event::LidMapping { .. } => report.write("lidMapping", json!({})),
            Event::Message(message) => {
                let message = *message;
                if message.info.from_me
                    && is_private(&message.info.chat)
                    && message.message.media_type() == Some("image")
                {
                    last_image.insert(message.info.chat.clone(), message.clone());
                }
                let Some(command) = owner_command(&message) else { continue };
                let image = last_image.get(&message.info.chat).cloned();
                let (client, report) = (client.clone(), report.clone());
                tokio::spawn(async move {
                    if let Err(err) = answer(&client, &report, &message, &command, image.as_ref()).await {
                        eprintln!("no pude responder {COMMAND} {command}: {err}");
                        report.write("replyFailed", json!({ "error": err.code() }));
                        let _ = client.send_text(&message.info.chat, format!("error {}: {err}", err.code())).await;
                    }
                });
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use soblendwa::proto::Message;
    use soblendwa::{Jid, MessageInfo};

    use super::*;

    fn message(chat: Jid, from_me: bool, text: &str) -> MessageEvent {
        let info = MessageInfo {
            id: "3EB0".into(),
            chat: chat.clone(),
            sender: chat,
            sender_alt: None,
            sender_phone: None,
            sender_lid: None,
            from_me,
            is_group: false,
            push_name: None,
            timestamp: (now_millis() / 1000) as i64,
            category: None,
            offline: false,
            edit: None,
            server_id: None,
            received_at: Instant::now(),
        };
        let message = Message::text(text);
        MessageEvent { info, raw: message.clone(), message }
    }

    #[test]
    fn owner_only() {
        let command = |chat: Jid, from_me: bool, text: &str| owner_command(&message(chat, from_me, text));
        assert_eq!(command(Jid::phone("5215511111111"), true, " !yowner "), Some(String::new()));
        assert_eq!(command(Jid::lid("174342050709710"), true, "!YOWNER Ping"), Some("ping".into()));
        assert_eq!(command(Jid::phone("5215511111111"), false, "!yowner"), None);
        assert_eq!(command(Jid::group("120363000000000001"), true, "!yowner"), None);
        assert_eq!(command(Jid::phone("5215511111111"), true, "!yowner por favor ya"), None);
        assert_eq!(command(Jid::phone("5215511111111"), true, "hola !yowner"), None);
    }

    #[test]
    fn totals_across_restarts() {
        let history = vec![
            json!({ "at": 1_000, "run": 1, "event": "start" }),
            json!({ "at": 2_000, "run": 1, "event": "stats", "rssMb": 11, "stats": { "received": 100, "undecryptable": 2 } }),
            json!({ "at": 3_000, "run": 1, "event": "stats", "rssMb": 14, "stats": { "received": 150, "undecryptable": 3 } }),
            json!({ "at": 4_000, "run": 1, "event": "disconnected" }),
            json!({ "at": 4_100, "run": 1, "event": "undecryptable", "kind": "sender_key" }),
            json!({ "at": 4_200, "run": 1, "event": "undecryptable", "kind": "sender_key" }),
            json!({ "at": 4_300, "run": 1, "event": "recovered", "kind": "sender_key" }),
            json!({ "at": 5_000, "run": 2, "event": "start" }),
            json!({ "at": 5_500, "run": 2, "event": "undecryptable", "kind": "bad_mac" }),
            json!({ "at": 6_000, "run": 2, "event": "stats", "rssMb": 12, "stats": { "received": 5 } }),
        ];
        let live = json!({ "received": 40, "undecryptable": 1, "lost": 1 });
        let all = totals(&history, 2, &live);
        assert_eq!((all.since, all.runs, all.disconnects, all.max_rss), (Some(1_000), 2, 1, 14));
        assert_eq!((all.received, all.undecryptable, all.lost), (190, 4, 1));
        assert_eq!(all.kinds.get("sender_key"), Some(&[2, 1, 0]));
        assert_eq!(all.kinds.get("bad_mac"), Some(&[1, 0, 0]));
        assert_eq!(all.unresolved, 1, "only the finished run can leave messages unresolved");
    }

    #[test]
    fn error_kinds() {
        assert_eq!(error_kind("no sender key for 120363000000000001@g.us"), "sender_key");
        assert_eq!(error_kind("bad message MAC"), "bad_mac");
        assert_eq!(error_kind("strange"), "other");
    }
}
