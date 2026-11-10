use std::io::Cursor;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use qrcode::render::unicode::Dense1x2;
use qrcode::QrCode;
use soblendwa::events::{DisconnectReason, EventResponseKind};
use soblendwa::interactive::{Interactive, Section};
use soblendwa::{ChatStore, Client, Event, FileStore, MediaKind, MediaOptions, Message, MessageEvent, Stats};
use tracing_subscriber::EnvFilter;

const HELP: &str = "*SoblendWA · comandos de prueba*
!prueba · prueba completa (todo sale en la consola)
!stats · métricas de la sesión
!quien · tu número y tu LID resueltos
!outbox · envíos esperando confirmación
!ping · latencia del bot
!info · créditos
!poll · encuesta de una opción
!encuesta · encuesta de varias opciones
!botones · mensaje interactivo
!clasicos · botones clásicos
!lista · lista de opciones
!evento · evento de calendario
!album · álbum de dos fotos
!archivo [MB] · documento grande en streaming
!descargar · responde a una foto, video o documento
!vista · enlace con vista previa
!contacto · tarjeta de contacto
!editar · envía y edita
!borrar · envía y elimina para todos
!fijar · fija tu mensaje 24 h
!reaccion · reacciona con 🔥
!archivar · archiva y desarchiva este chat
!chats · resumen del store de chats
!buscar <texto> · busca en el historial guardado
!bots · bots de Meta AI disponibles
!temporal · tus mensajes temporales
!limite · bloqueo de contacto nuevo
!canal <enlace> · datos de un canal
!group · datos del grupo
!comunidad · subgrupos de la comunidad
!invitacion · te mando la invitación del grupo
!todos · menciona a todos";

#[tokio::main]
async fn main() -> soblendwa::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("soblendwa=info")))
        .init();
    println!("{}", soblendwa::credits());

    let folder = std::env::args().nth(1).unwrap_or_else(|| "session".into());
    println!("Sesión: {folder}");
    let store = FileStore::open(&folder).await?;
    let chats = ChatStore::open(format!("{folder}/chats.json")).await?;
    let (client, mut events) = Client::builder().store(store).chat_store(chats).push_name("SoblendWA").build().await?;
    client.connect().await?;
    let watcher = client.clone();
    tokio::spawn(async move {
        let mut every = tokio::time::interval(Duration::from_secs(60));
        every.tick().await;
        loop {
            every.tick().await;
            println!("[stats] {}", stats_line(&watcher.stats()));
        }
    });

    while let Some(event) = events.recv().await {
        match event {
            Event::Qr(code) => show_qr(&code),
            Event::PairSuccess { id, platform, .. } => {
                println!("Linked to {id} ({})", platform.unwrap_or_default());
            }
            Event::Connected => println!(
                "[conectado] {} · ida y vuelta {} ms · manda !prueba o !ayuda",
                client.me().map(|j| j.to_string()).unwrap_or_default(),
                client.latency().map_or(0, |d| d.as_millis())
            ),
            Event::Disconnected { reason: DisconnectReason::RestartRequired, .. } => {}
            Event::Disconnected { reason, reconnecting } => {
                println!("[desconectado] {reason:?} · reconectando: {}", if reconnecting { "sí" } else { "no" });
            }
            Event::LoggedOut { reason, .. } => {
                println!("Logged out: {reason}");
                break;
            }
            Event::AccountBlocked { code, reason, expires } => {
                println!("[BLOQUEADA] WhatsApp bloqueó la cuenta ({code}): {reason} · expira: {expires:?}");
                println!("[BLOQUEADA] el bot ya no reintenta; la sesión se conserva");
            }
            Event::UndecryptableMessage { info, error } => {
                println!("[sin descifrar] {} de {} · {error} · ya se pidió un reintento", info.id, info.sender);
            }
            Event::MessageRecovered { id, sender, .. } => {
                println!("[recuperado] {id} de {sender} · se pudo leer tras el reintento");
            }
            Event::PictureChanged { jid, removed, .. } => {
                println!("[foto] {jid} {}", if removed { "quitó su foto" } else { "cambió su foto" });
            }
            Event::AboutChanged { jid, about, .. } => println!("[info] {jid} ahora dice: {}", one_line(&about, 60)),
            Event::BlocklistChanged { items } => println!("[bloqueos] {} cambios", items.len()),
            Event::PrivacyChanged { settings } => println!("[privacidad] {settings:?}"),
            Event::Message(message) if message.info.is_older_than(Duration::from_secs(120)) => {
                println!("{} · (viejo, no se ejecuta)", describe(&message));
            }
            Event::Message(message) if !message.info.from_me => {
                println!("{}", describe(&message));
                let client = client.clone();
                tokio::spawn(async move {
                    if let Err(err) = answer(&client, &message).await {
                        eprintln!("could not answer {}: {err}", message.info.id);
                        let _ = client.reply(&message, format!("⚠️ {err}")).await;
                    }
                });
            }
            Event::PollVote(vote) => println!("Vote from {} on {}: {:?}", vote.voter, vote.poll_id, vote.selected),
            Event::EventResponse(response) => {
                println!("{} answered {:?} to event {}", response.responder, response.kind, response.event_id)
            }
            Event::ServerAck(ack) => match (ack.error, ack.timings) {
                (Some(code), _) => println!("[rechazado] {} en {} · error {code}", ack.id, ack.chat),
                (None, Some(t)) => println!(
                    "[enviado] {} · cola {} · subida {} · preparar {} · cifrar {} · WhatsApp {} · total {}",
                    ack.id,
                    format_ms(t.queued),
                    t.upload.map_or_else(|| "-".into(), format_ms),
                    format_ms(t.prepare),
                    format_ms(t.encrypt),
                    t.server.map_or_else(|| "-".into(), format_ms),
                    format_ms(t.total()),
                ),
                (None, None) => println!("[interno] {} · confirmado por WhatsApp, sin tiempos por fase", ack.id),
            },
            Event::ChatUpdate { update, initial_sync: false, .. } => println!("Chat update: {update:?}"),
            Event::HistorySync(history) => println!(
                "History received: {} chats, {} contact names",
                history.conversations.len(),
                history.pushnames.len()
            ),
            _ => {}
        }
    }
    client.disconnect().await;
    Ok(())
}

async fn who(client: &Client, message: &MessageEvent) -> String {
    let info = &message.info;
    let phone = info.sender_phone.as_ref().map_or_else(|| "oculto por WhatsApp".into(), |j| format!("+{}", j.user));
    let lid = info.sender_lid.as_ref().map_or_else(|| "desconocido".into(), |j| j.user.clone());
    let admin = match info.is_group {
        true => match client.group_info(&info.chat).await {
            Ok(group) => format!(" · admin del grupo: {}", if group.is_admin(&info.sender) { "sí" } else { "no" }),
            Err(err) => format!(" · admin del grupo: no se pudo saber ({err})"),
        },
        false => String::new(),
    };
    format!("Tú: número {phone} · lid {lid}{admin}")
}

fn stats_line(stats: &Stats) -> String {
    format!(
        "recibidos {} · enviados {} · sin descifrar {} · recuperados {} · sin recuperar {} · duplicados {} · reconexiones {} · frenados {} · media subida {} · reutilizada {} · reenviados del outbox {} · panics {} · latencia {}",
        stats.received,
        stats.sent,
        stats.undecryptable,
        stats.recovered,
        stats.awaiting_recovery,
        stats.duplicates,
        stats.reconnects,
        stats.rate_limited,
        stats.media_uploaded,
        stats.media_reused,
        stats.outbox_resent,
        stats.panics,
        stats.latency.map_or_else(|| "-".into(), format_ms),
    )
}

fn one_line(text: &str, limit: usize) -> String {
    let mut lines = text.lines().map(str::trim).filter(|line| !line.is_empty());
    let first = lines.next().unwrap_or_default();
    let more = lines.count();
    let mut short: String = first.chars().take(limit).collect();
    if first.chars().count() > limit {
        short.push('…');
    }
    if more > 0 {
        short.push_str(&format!(" (+{more} líneas)"));
    }
    short
}

fn kind_of(message: &Message) -> String {
    if let Some(reaction) = &message.reaction_message {
        return format!("[reacción {}]", reaction.text.as_deref().unwrap_or_default());
    }
    if message.poll_update_message.is_some() {
        return "[voto]".into();
    }
    if message.protocol_message.is_some() {
        return "[protocolo]".into();
    }
    format!("[{}]", message.stanza_type())
}

fn describe(message: &MessageEvent) -> String {
    let info = &message.info;
    let place = if info.is_group {
        format!("grupo {}", info.chat.user)
    } else if info.chat.is_newsletter() {
        format!("canal {}", info.chat.user)
    } else if info.chat.is_status_broadcast() {
        "estado".into()
    } else {
        "privado".into()
    };
    let phone = info.sender_phone.as_ref().map_or_else(|| "oculto".into(), |j| format!("+{}", j.user));
    let lid = info.sender_lid.as_ref().map_or_else(|| "-".into(), |j| j.user.clone());
    let body = match (message.text(), message.message.media_type()) {
        (Some(text), Some(kind)) => format!("[{kind}] {}", one_line(text, 80)),
        (Some(text), None) => one_line(text, 80),
        (None, Some(kind)) => format!("[{kind}]"),
        (None, None) => kind_of(&message.message),
    };
    let quoted = message
        .quoted()
        .map(|q| format!(" · citando: {}", one_line(q.get_text().or(q.media_type()).unwrap_or("mensaje"), 40)))
        .unwrap_or_default();
    let preview = message
        .message
        .extended_text_message
        .as_ref()
        .and_then(|text| text.title.as_deref())
        .map(|title| format!(" · vista previa: {}", one_line(title, 40)))
        .unwrap_or_default();
    let once = if message.message.is_view_once() { " · ver una vez" } else { "" };
    let offline = if info.offline { " · llegó estando desconectado" } else { "" };
    format!("[recibido] {place} · número {phone} · lid {lid} · {body}{preview}{quoted}{once}{offline}")
}

fn millis(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1000.0
}

fn format_ms(duration: Duration) -> String {
    let ms = millis(duration);
    let decimals = if ms < 0.1 {
        3
    } else if ms < 1.0 {
        2
    } else {
        1
    };
    let text = format!("{ms:.decimals$}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    format!("{text} ms")
}

fn show_qr(code: &str) {
    match QrCode::new(code.as_bytes()) {
        Ok(qr) => println!(
            "\nScan this code with WhatsApp > Linked devices:\n{}",
            qr.render::<Dense1x2>().quiet_zone(true).build()
        ),
        Err(_) => println!("QR: {code}"),
    }
}

fn solid_png(rgb: [u8; 3]) -> Vec<u8> {
    let image = image::RgbImage::from_pixel(640, 480, image::Rgb(rgb));
    let mut out = Cursor::new(Vec::new());
    image::DynamicImage::ImageRgb8(image).write_to(&mut out, image::ImageFormat::Png).expect("png encoding");
    out.into_inner()
}

fn extension_for(message: &Message) -> String {
    let m = message.unwrapped();
    if let Some(name) = m.document_message.as_ref().and_then(|d| d.file_name.clone()) {
        return name;
    }
    match message.media_type() {
        Some("image") => "imagen.jpg",
        Some("video" | "gif" | "ptv") => "video.mp4",
        Some("ptt" | "audio") => "audio.ogg",
        Some("sticker") => "sticker.webp",
        _ => "archivo.bin",
    }
    .to_owned()
}

async fn answer(client: &Client, message: &MessageEvent) -> soblendwa::Result<()> {
    if let Some(reply) = message.message.interactive_reply() {
        if reply.id == "ping" {
            return client.reply(message, format!("Pong\n{}", format_ms(message.info.elapsed()))).await.map(drop);
        }
        let text = format!("Elegiste *{}* ({:?})", reply.text.unwrap_or(reply.id), reply.source);
        return client.reply(message, text).await.map(drop);
    }
    let Some(text) = message.text() else { return Ok(()) };
    let (command, rest) = text.split_once(' ').unwrap_or((text, ""));
    let rest = rest.trim();
    let chat = message.chat();
    match command {
        "!ping" | "#ping" => {
            let took = format_ms(message.info.elapsed());
            client.reply(message, format!("Pong\n{took}")).await?;
        }
        "!prueba" => {
            let mut report = Vec::new();
            println!("[prueba] empezando en {chat}");
            report.push(format!("Bot: {} desde que llegó tu mensaje", format_ms(message.info.elapsed())));
            report.push(format!("Red hasta WhatsApp: {}", format_ms(client.ping().await?)));
            report.push(who(client, message).await);
            let started = Instant::now();
            client.send_text(chat, "1/3 · texto").await?;
            report.push(format!("Texto listo para salir en {}", format_ms(started.elapsed())));
            let before = client.stats();
            let seed =
                std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.subsec_nanos());
            let picture = solid_png([(seed % 200) as u8 + 30, (seed / 7 % 200) as u8 + 30, 200]);
            let first = Instant::now();
            client.send_image(chat, &picture, Some("2/3 · imagen nueva (se sube)")).await?;
            let first = first.elapsed();
            let second = Instant::now();
            client.send_image(chat, &picture, Some("3/3 · la misma imagen (se reutiliza)")).await?;
            let second = second.elapsed();
            let after = client.stats();
            report.push(format!(
                "Imagen nueva {} · la misma otra vez {} (subidas {}, reutilizadas {})",
                format_ms(first),
                format_ms(second),
                after.media_uploaded - before.media_uploaded,
                after.media_reused - before.media_reused
            ));
            let pending = client.pending_sends().await.map_or(0, |p| p.len());
            report.push(format!("Esperando confirmación de WhatsApp: {pending}"));
            report.push(stats_line(&after));
            for line in &report {
                println!("[prueba] {line}");
            }
            client.reply(message, format!("*Prueba completa*\n{}", report.join("\n"))).await?;
        }
        "!stats" => {
            let line = stats_line(&client.stats());
            println!("[stats] {line}");
            client.reply(message, line).await?;
        }
        "!quien" => {
            let line = who(client, message).await;
            println!("[quien] {line}");
            client.reply(message, line).await?;
        }
        "!outbox" => {
            let pending = client.pending_sends().await?;
            let line = if pending.is_empty() {
                "Nada pendiente: WhatsApp confirmó todo".to_owned()
            } else {
                pending
                    .iter()
                    .map(|p| format!("{} → {} ({} intentos)", p.id, p.chat, p.attempts))
                    .collect::<Vec<_>>()
                    .join("\n")
            };
            println!("[outbox] {line}");
            client.reply(message, line).await?;
        }
        "!ayuda" | "!help" => {
            client.reply(message, HELP).await?;
        }
        "!info" => {
            client.reply(message, soblendwa::credits()).await?;
        }
        "!echo" if !rest.is_empty() => {
            client.reply(message, rest).await?;
        }
        "!poll" => {
            let poll = Message::poll("¿Cuál es el mejor lenguaje?", ["Rust", "Rust 🦀", "Siempre Rust"], 1);
            client.send_message(chat, poll).await?;
        }
        "!encuesta" => {
            let poll = Message::poll("¿Qué te gusta de SoblendWA?", ["Velocidad", "Rust", "Node", "Python"], 0);
            client.send_message(chat, poll).await?;
        }
        "!botones" => {
            let menu =
                Interactive::new("¿Qué quieres probar?")
                    .title("SoblendWA")
                    .footer("Soblend Development Studio")
                    .quick_reply("ping", "Medir latencia")
                    .url("Rust", "https://www.rust-lang.org")
                    .copy("Copiar cupón", "SOBLEND-2026")
                    .select(
                        "Más opciones",
                        vec![Section::new("Pruebas")
                            .described_row("album", "Álbum", "Dos fotos generadas")
                            .described_row("evento", "Evento", "Evento de calendario")],
                    );
            client.send_message(chat, menu.build()).await?;
        }
        "!clasicos" => {
            let buttons = Message::buttons("Botones clásicos", Some("SoblendWA"), [("si", "Sí"), ("no", "No")]);
            client.send_message(chat, buttons).await?;
        }
        "!lista" => {
            let sections = [Section::new("Tacos").row("pastor", "Al pastor").row("suadero", "Suadero")];
            let list = Message::list("Menú", "Elige tu taco", "Ver menú", Some("SoblendWA"), &sections);
            client.send_message(chat, list).await?;
        }
        "!evento" => {
            let start = unix_now() + 3600;
            let event = Message::event("Prueba de SoblendWA", start).event_details(
                Some("Evento creado desde Rust"),
                Some(start + 1800),
                Some((19.4326, -99.1332, "Ciudad de México")),
                None,
                true,
            );
            let id = client.send_message(chat, event).await?;
            client.respond_event(chat, &id, EventResponseKind::Going, 0).await?;
        }
        "!album" => {
            let mut items = Vec::new();
            for (rgb, caption) in [([220, 40, 40], "Rojo"), ([40, 90, 220], "Azul")] {
                let options = MediaOptions { caption: Some(caption.into()), ..Default::default() };
                items.push(client.prepare_media(&solid_png(rgb), MediaKind::Image, options).await?);
            }
            client.send_album(chat, items).await?;
        }
        "!archivo" => {
            let megabytes: usize = rest.parse().unwrap_or(20).clamp(1, 500);
            let path = std::env::temp_dir().join(format!("soblendwa-prueba-{megabytes}MB.bin"));
            tokio::fs::write(&path, soblendwa::crypto::random_vec(megabytes * 1024 * 1024)).await?;
            let started = Instant::now();
            let options = MediaOptions {
                file_name: Some(format!("prueba-{megabytes}MB.bin")),
                caption: Some(format!("{megabytes} MB subidos en streaming")),
                ..Default::default()
            };
            let sent = client.send_media_file(chat, &path, MediaKind::Document, options).await;
            let _ = tokio::fs::remove_file(&path).await;
            sent?;
            let seconds = started.elapsed().as_secs_f64();
            let speed = megabytes as f64 / seconds.max(0.001);
            client.reply(message, format!("Subido en {seconds:.1} s ({speed:.1} MB/s)")).await?;
        }
        "!descargar" => {
            let Some(quoted) = message.quoted() else {
                client.reply(message, "Responde con !descargar a una foto, video, audio o documento").await?;
                return Ok(());
            };
            let Some(media) = quoted.media() else {
                client.reply(message, "Ese mensaje no tiene multimedia").await?;
                return Ok(());
            };
            let path = PathBuf::from("descargas").join(format!("{}-{}", message.info.id, extension_for(quoted)));
            let started = Instant::now();
            let bytes = client.download_to_file(media, &path).await?;
            let text = format!(
                "Descargado y verificado: {:.2} MB en {:.1} s\n{}",
                bytes as f64 / 1_048_576.0,
                started.elapsed().as_secs_f64(),
                path.display()
            );
            client.reply(message, text).await?;
        }
        "!vista" => {
            client.send_text_with_preview(chat, "Mira esto: https://www.rust-lang.org").await?;
        }
        "!contacto" => {
            let phone = message.info.sender.user.clone();
            let name = message.info.push_name.clone().unwrap_or_else(|| "Contacto".into());
            let card = Message::contacts(&name, [(name.clone(), Message::vcard(&name, &phone))]);
            client.send_message(chat, card).await?;
        }
        "!editar" => {
            let id = client.send_text(chat, "Versión 1").await?;
            tokio::time::sleep(Duration::from_secs(2)).await;
            client.edit_message(chat, &id, "Versión 2 (editado)").await?;
        }
        "!borrar" => {
            let id = client.send_text(chat, "Este mensaje se borra en 3 segundos").await?;
            tokio::time::sleep(Duration::from_secs(3)).await;
            let key = soblendwa::proto::MessageKey {
                remote_jid: Some(chat.to_string()),
                from_me: Some(true),
                id: Some(id),
                participant: None,
            };
            client.revoke_message(chat, key).await?;
        }
        "!fijar" => {
            client.pin_message(chat, message.key(), 24 * 60 * 60).await?;
        }
        "!reaccion" => {
            client.react(message, "🔥").await?;
        }
        "!archivar" => {
            client.archive_chat(chat, true).await?;
            tokio::time::sleep(Duration::from_secs(3)).await;
            client.archive_chat(chat, false).await?;
            client.reply(message, "Archivado y desarchivado ✓").await?;
        }
        "!chats" => {
            let Some(store) = client.chats() else { return Ok(()) };
            let list = store.chats();
            let mut text = format!("{} chats · {} mensajes sin leer\n", list.len(), store.unread_total());
            for chat in list.iter().take(5) {
                let name = store.name_of(&chat.jid).unwrap_or_else(|| chat.jid.user.clone());
                text.push_str(&format!("\n• {name} ({} sin leer)", chat.unread));
            }
            client.reply(message, text).await?;
        }
        "!buscar" if !rest.is_empty() => {
            let Some(store) = client.chats() else { return Ok(()) };
            let found = store.search(rest, 5);
            let mut text = format!("{} resultados para \"{rest}\"", found.len());
            for hit in &found {
                text.push_str(&format!("\n• {}", hit.text().unwrap_or_default().chars().take(60).collect::<String>()));
            }
            client.reply(message, text).await?;
        }
        "!bots" => {
            let bots = client.bot_list().await?;
            client.reply(message, format!("{} bots de Meta AI disponibles", bots.len())).await?;
        }
        "!temporal" => {
            let modes = client.disappearing_modes(std::slice::from_ref(&message.info.sender)).await?;
            let text = match modes.first() {
                Some(mode) if !mode.duration.is_zero() => {
                    format!("Tus mensajes temporales duran {} días", mode.duration.as_secs() / 86_400)
                }
                _ => "No tienes mensajes temporales por defecto".into(),
            };
            client.reply(message, text).await?;
        }
        "!limite" => {
            let lock = client.reachout_timelock().await?;
            let text = if lock.active {
                format!("Cuenta limitada para contactos nuevos hasta {:?}", lock.ends_at)
            } else {
                "Sin límites para escribir a contactos nuevos ✓".into()
            };
            client.reply(message, text).await?;
        }
        "!canal" if !rest.is_empty() => {
            let info = client.newsletter_info_by_invite(rest).await?;
            let text = format!(
                "*{}*\n{} seguidores{}",
                info.name.unwrap_or_default(),
                info.subscribers.unwrap_or_default(),
                if info.verified { " · verificado" } else { "" }
            );
            client.reply(message, text).await?;
        }
        "!group" if message.info.is_group => {
            let info = client.group_info(chat).await?;
            let admins = info.admins().count();
            let summary = format!("{}\n{} miembros · {} admins", info.subject, info.participants.len(), admins);
            client.reply(message, summary).await?;
        }
        "!comunidad" if message.info.is_group => {
            let info = client.group_metadata(chat).await?;
            let community = if info.is_community { Some(chat.clone()) } else { info.linked_parent.clone() };
            let Some(community) = community else {
                client.reply(message, "Este grupo no pertenece a una comunidad").await?;
                return Ok(());
            };
            let groups = client.community_groups(&community).await?;
            let names: Vec<String> = groups.iter().map(|g| format!("• {}", g.subject)).collect();
            client.reply(message, format!("{} grupos en la comunidad\n{}", groups.len(), names.join("\n"))).await?;
        }
        "!invitacion" if message.info.is_group => {
            let to = message.info.sender.to_non_device();
            client.send_group_invite(&to, chat, Some("Invitación enviada por SoblendWA")).await?;
            client.reply(message, "Te mandé la invitación por privado ✓").await?;
        }
        "!todos" if message.info.is_group => {
            client.send_message(chat, Message::text("Atención a todos 📣").mention_all()).await?;
        }
        _ => {}
    }
    Ok(())
}

fn unix_now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}
