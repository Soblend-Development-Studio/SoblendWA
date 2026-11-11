=== enviar | Enviar mensajes | Mensajes
lead: Texto, respuestas, menciones, reacciones, ediciones y todo lo que no sea multimedia.

Todos los envíos devuelven el `id` del mensaje. Los ejemplos usan `client` en Rust y `bot` en los demás lenguajes.

## Texto

:::tabs
```rust Rust
use soblendwa::Jid;

let chat = Jid::phone("5215512345678");
let id = client.send_text(&chat, "Hola desde SoblendWA").await?;
```
```js Node.js
const id = await bot.sendText('5215512345678', 'Hola desde SoblendWA')
```
```python Python
id = await bot.send_text("5215512345678", "Hola desde SoblendWA")
```
```js Cliente JS
const id = await bot.sendText('5215512345678', 'Hola desde SoblendWA')
```
```bash HTTP
curl -X POST http://127.0.0.1:8787/api/bot/sendText \
  -H "Authorization: Bearer mi-clave" \
  -d '{"chat":"5215512345678","text":"Hola desde SoblendWA"}'
```
:::

Si el texto trae un enlace, la [vista previa](#vista-previa) se genera sola.

## Responder

`reply` cita el mensaje original y responde en el mismo chat.

:::tabs
```rust Rust
client.reply(&msg, "Respondiendo a tu mensaje").await?;
```
```js Node.js
await bot.reply(message, 'Respondiendo a tu mensaje')
```
```python Python
await bot.reply(message, "Respondiendo a tu mensaje")
```
```js Cliente JS
await bot.reply(message, 'Respondiendo a tu mensaje')
```
:::

También puedes citar en cualquier otro mensaje: `Message::text("hola").reply_to(&msg)` en Rust, o `{ quoted: message }` en las opciones de JavaScript.

## Reaccionar

:::tabs
```rust Rust
client.react(&msg, "🔥").await?;
```
```js Node.js
await bot.react(message, '🔥')
```
```python Python
await bot.react(message, "🔥")
```
```js Cliente JS
await bot.react(message, '🔥')
```
:::

Para quitar tu reacción, envía un emoji vacío.

## Menciones

Una mención necesita dos cosas: el `@número` en el texto y el JID en la lista de mencionados.

:::tabs
```rust Rust
use soblendwa::Message;

let usuario = Jid::phone("5215511111111");
client.send_message(&chat, Message::text("hola @5215511111111").mention([&usuario])).await?;
client.send_message(&grupo, Message::text("Atención a todos").mention_all()).await?;
```
```js Node.js
await bot.sendText(chat, 'hola @5215511111111', { mentions: ['5215511111111'] })
await bot.sendMentionAll(grupo, 'Atención a todos')
```
```python Python
await bot.send_text(chat, "hola @5215511111111", mentions=["5215511111111"])
await bot.send_mention_all(grupo, "Atención a todos")
```
```js Cliente JS
await bot.sendText(chat, 'hola @5215511111111', { mentions: ['5215511111111'] })
await bot.sendMentionAll(grupo, 'Atención a todos')
```
:::

Las menciones también funcionan en el texto de una imagen o un video: pasa `mentions` en las opciones de la multimedia.

## Editar y eliminar

:::tabs
```rust Rust
let id = client.send_text(&chat, "versión 1").await?;
client.edit_message(&chat, &id, "versión 2").await?;
client.revoke_message(&chat, msg.key()).await?;
```
```js Node.js
const id = await bot.sendText(chat, 'versión 1')
await bot.edit(chat, id, 'versión 2')
await bot.revoke(chat, id)
```
```python Python
id = await bot.send_text(chat, "versión 1")
await bot.edit(chat, id, "versión 2")
await bot.revoke(chat, id)
```
```js Cliente JS
const id = await bot.sendText(chat, 'versión 1')
await bot.edit(chat, id, 'versión 2')
await bot.revoke(chat, id)
```
:::

## Fijar y desfijar

```rust
client.pin_message(&chat, msg.key(), 7 * 24 * 60 * 60).await?;
client.unpin_message(&chat, msg.key()).await?;
```

## Reenviar

`.forwarded()` marca el mensaje como reenviado.

```rust
client.send_message(&chat, Message::text("reenviado").forwarded()).await?;
```

## Ubicación

```rust
client.send_message(&chat, Message::location(19.4326, -99.1332)).await?;
```

## Contactos

:::tabs
```rust Rust
let tarjeta = Message::vcard("Ana", "+52 1 55 1111 1111");
client.send_message(&chat, Message::contacts("Ana", [("Ana", tarjeta)])).await?;
```
```js Node.js
await bot.sendContacts(chat, [{ name: 'Ana', phone: '5215511111111' }])
```
```python Python
await bot.send_contacts(chat, [("Ana", "5215511111111")])
```
```js Cliente JS
await bot.sendContacts(chat, [{ name: 'Ana', phone: '5215511111111' }])
```
:::

## Invitar a un grupo

Envía una tarjeta de invitación. SoblendWA pide el código del grupo por ti, así que debes ser administrador.

:::tabs
```rust Rust
client.send_group_invite(&chat, &grupo, Some("Únete al equipo")).await?;
```
```js Node.js
await bot.sendGroupInvite(chat, grupo, 'Únete al equipo')
```
```python Python
await bot.send_group_invite(chat, grupo, "Únete al equipo")
```
```js Cliente JS
await bot.sendGroupInvite(chat, grupo, 'Únete al equipo')
```
:::

## Compartir o pedir un número

```rust
client.send_message(&chat, Message::share_phone_number()).await?;
client.send_message(&chat, Message::request_phone_number()).await?;
```

## Limitar el reenvío

Activa o desactiva el límite de reenvío de una conversación.

```rust
client.send_message(&chat, Message::limit_sharing(true)).await?;
```

## Estados

Publica una historia para una lista de contactos.

```rust
client.send_status(Message::text("Mi primer estado"), &[Jid::phone("5215511111111")]).await?;
```

## Etiqueta de miembro

Pone tu etiqueta dentro de un grupo, de hasta 30 caracteres. {exp}

:::tabs
```rust Rust
client.set_member_label(&grupo, "Soporte").await?;
```
```js Node.js
await bot.setMemberLabel(grupo, 'Soporte')
```
```python Python
await bot.set_member_label(grupo, "Soporte")
```
```js Cliente JS
await bot.setMemberLabel(grupo, 'Soporte')
```
:::

## Cualquier otro mensaje

`Message` es el protobuf de WhatsApp con todos sus campos públicos, así que puedes construir cualquier mensaje. En otros lenguajes, `sendProto` acepta los bytes del protobuf.

:::tabs
```rust Rust
let mensaje = Message { conversation: Some("Hecho a mano".into()), ..Default::default() };
client.send_message(&chat, mensaje).await?;
```
```js Node.js
await bot.sendProto(chat, bytes)
```
```python Python
await bot.send_proto(chat, data)
```
```js Cliente JS
await bot.sendProto(chat, bytes)
```
:::

## Opciones de envío {#enviar-opciones}

`send_message_with` da control total sobre un envío.

```rust
use soblendwa::{PreviewChoice, SendOptions};

let options = SendOptions { id: Some("3EB0MIID".into()), link_preview: PreviewChoice::Off, ..Default::default() };
client.send_message_with(&chat, Message::text("hola"), options).await?;
```

| Campo | Para qué |
|---|---|
| `id` | Usa este id en lugar de generar uno |
| `edit` | Marca de edición de bajo nivel. `edit_message` ya la usa por ti |
| `status_recipients` | Destinatarios de un estado |
| `link_preview` | `Auto`, `On`, `Off` o `Custom`. Ver [Vista previa](#vista-previa) |

## Qué puede fallar

| Error | Qué significa | Qué hacer |
|---|---|---|
| `Error::RateLimited` | Pasaste el [límite de envíos](#confiabilidad-limite) | Espera `retry_after` |
| `Error::NotLoggedIn` | La cuenta aún no está vinculada | Vincúlala primero |
| `Error::AccountBlocked` | WhatsApp bloqueó la cuenta | Revisa [Confiabilidad](#confiabilidad-bloqueo) |
| `Error::InvalidJid` | El destino no es válido | Revisa el chat |

Si no hay conexión, el mensaje se guarda en el [outbox](#confiabilidad-outbox) y sale al reconectar. La confirmación de WhatsApp llega después en `Event::ServerAck`.

=== recibir | Recibir mensajes | Mensajes
lead: Qué trae cada mensaje, cómo leerlo y cómo evitar responder a lo viejo.

## El bucle de eventos

:::tabs
```rust Rust
while let Some(event) = events.recv().await {
    if let Event::Message(msg) = event {
        if msg.info.from_me {
            continue;
        }
        println!("{}: {:?}", msg.info.sender, msg.text());
    }
}
```
```js Node.js
bot.on('message', (message) => {
  if (message.info.fromMe) return
  console.log(message.info.sender, message.text)
})
```
```python Python
@bot.on("message")
def show(message):
    if not message["info"]["fromMe"]:
        print(message["info"]["sender"], message["text"])
```
```js Cliente JS
bot.on('message', (message) => {
  if (message.info.fromMe) return
  console.log(message.info.sender, message.text)
})
```
:::

También llegan tus propios mensajes, enviados desde el teléfono o desde el bot: revisa siempre `from_me`.

## Datos del mensaje

`msg.info` trae el contexto y `msg.message` el contenido, ya desenvuelto.

| Rust | JSON | Qué es |
|---|---|---|
| `info.id` | `info.id` | Identificador del mensaje |
| `info.chat` | `info.chat` | Chat donde llegó |
| `info.sender` | `info.sender` | Quién lo envió, tal como llegó |
| `info.sender_alt` | `info.senderAlt` | La otra identidad, si WhatsApp la envió |
| `info.sender_phone` | `info.senderPhone` | Su JID de número, si se conoce |
| `info.sender_lid` | `info.senderLid` | Su LID, si se conoce |
| `info.from_me` | `info.fromMe` | Si lo enviaste tú |
| `info.is_group` | `info.isGroup` | Si viene de un grupo |
| `info.push_name` | `info.pushName` | El nombre que eligió esa persona |
| `info.timestamp` | `info.timestamp` | Hora de envío, en segundos Unix |
| `info.category` | `info.category` | Categoría de WhatsApp, como `peer` |
| `info.offline` | `info.offline` | Si llegó mientras estabas desconectado |
| `info.edit` | `info.edit` | Marca de edición, si la hay |
| `info.server_id` | `info.serverId` | Id de servidor, en canales |
| `info.elapsed()` | `info.elapsedMicros` | Tiempo dentro del bot desde que llegó del socket |

## Leer el contenido

| Rust | JSON | Qué devuelve |
|---|---|---|
| `msg.text()` | `text` | El texto o el caption, aunque venga envuelto |
| `msg.message.media()` | `media` | La multimedia. En JSON: tipo, mimetype, tamaño, caption, duración y dimensiones |
| `msg.message.media_type()` | `mediaType` | El tipo de multimedia |
| `msg.message.stanza_type()` | `kind` | El tipo de mensaje |
| `msg.quoted()` | `quoted` | Lo citado. En JSON: id, remitente, chat, texto y multimedia |
| `msg.mentions()` | `mentions` | Los JID mencionados |
| `msg.is_reaction()` | `reaction` | Si es una reacción. En JSON: `targetId` y `emoji` |
| `msg.message.is_view_once()` | `viewOnce` | Si es de una sola vista |
| `msg.message.interactive_reply()` | `reply` | La opción elegida en un [botón o lista](#interactivos) |
| `msg.message.link_thumbnail()` | `linkPreview` | Datos de la [vista previa](#vista-previa) recibida |
| `msg.raw` | `proto` | El protobuf original. En JSON, en base64 |
| — | `ageSecs` | Antigüedad en segundos |

## Evitar comandos viejos {#recibir-viejos}

Los mensajes que llegaron con el bot apagado traen `info.offline = true`. `is_stale` combina eso con la antigüedad.

:::tabs
```rust Rust
use std::time::Duration;

if msg.info.is_stale(Duration::from_secs(120)) {
    return Ok(());
}
```
```js Node.js
if (message.info.offline || message.ageSecs > 120) return
```
```python Python
if message["info"]["offline"] or message["ageSecs"] > 120:
    return
```
```js Cliente JS
if (message.info.offline || message.ageSecs > 120) return
```
:::

Cuando terminan de llegar los mensajes pendientes se emite `Event::OfflineSyncComplete { count }`.

## Duplicados

SoblendWA descarta por id los duplicados que llegan, y los cuenta en `stats().duplicates`, así que no tienes que hacerlo tú.

## Mensajes que no se pudieron descifrar

Si un mensaje no se puede descifrar llega `Event::UndecryptableMessage { info, error }`, y SoblendWA pide un reintento al remitente y una copia a tu teléfono. Si después llega bien, recibes `Event::Message` normal y `Event::MessageRecovered { id, chat, sender }`.

Si pasado el plazo sigue sin llegar, recibes `Event::MessageLost { id, chat, sender, error, waited }`, y se cuenta en `stats().lost`. Así sabes cuántos mensajes tu bot nunca leyó, en lugar de suponerlo. Si llega más tarde, igual recibes `MessageRecovered` y `lost` baja.

| Métrica | Qué cuenta |
|---|---|
| `stats().undecryptable` | Los que no se pudieron descifrar al llegar |
| `stats().awaiting_recovery` | Los que siguen esperando el reintento |
| `stats().recovered` | Los que llegaron bien tras el reintento |
| `stats().lost` | Los que no llegaron dentro del plazo |

El plazo es de 10 minutos y se cambia con `.retry_deadline(...)` en Rust o `retryDeadlineSecs` en Node.js y Python. El caso más común en grupos es `missing sender key`: el remitente no le envió al bot su clave de grupo, y el reintento se la pide.

La espera se guarda en el store, así que sobrevive a un reinicio: al volver, el bot sigue esperando lo que quedaba del plazo, y si el plazo ya pasó emite `MessageLost` al arrancar. Un mensaje que llega después del reinicio sigue dando `MessageRecovered`.

## Marcar como leído

:::tabs
```rust Rust
client.read_message(&msg).await?;
client.mark_read(&chat, None, &[msg.info.id.clone()]).await?;
client.mark_played(&msg).await?;
```
```js Cliente JS
await bot.read(message)
await bot.markPlayed(message)
```
```js Node.js
await bot.markPlayed(message)
```
```python Python
await bot.mark_played(message)
```
:::

`mark_played` es para notas de voz y videos circulares. Para otros tipos de recibo está `send_receipt`.

## Recibos de los demás

`Event::Receipt` avisa de lo que pasa con lo que enviaste: `Delivered`, `Read`, `Played`, `Retry` y otros tipos, que están en [Eventos](#ref-eventos).

## Responder por id en otros lenguajes

En Node.js, Python y el servidor, `reply`, `react`, `download` y otros aceptan el mensaje o su id. El id se busca entre los últimos 4.096 mensajes recibidos y, si activaste el store de chats, también ahí. Si es más viejo, recibes el error `message … is not among the recent messages`.

=== multimedia | Multimedia | Mensajes
lead: Imágenes, video, audio, documentos y stickers, desde unos kilobytes hasta archivos enormes.

## Enviar

:::tabs
```rust Rust
client.send_image(&chat, &std::fs::read("foto.jpg")?, Some("mi foto")).await?;
client.send_video(&chat, &std::fs::read("clip.mp4")?, Some("mi video")).await?;
client.send_audio(&chat, &std::fs::read("nota.ogg")?, true).await?;
client.send_document(&chat, &std::fs::read("reporte.pdf")?, "reporte.pdf", "application/pdf").await?;
client.send_sticker(&chat, &std::fs::read("sticker.webp")?).await?;
```
```js Node.js
const fs = require('node:fs')

await bot.sendImage(chat, fs.readFileSync('foto.jpg'), { caption: 'mi foto' })
await bot.sendVideo(chat, fs.readFileSync('clip.mp4'), { caption: 'mi video' })
await bot.sendAudio(chat, fs.readFileSync('nota.ogg'), { voiceNote: true })
await bot.sendDocument(chat, fs.readFileSync('reporte.pdf'), { fileName: 'reporte.pdf', mimetype: 'application/pdf' })
await bot.sendSticker(chat, fs.readFileSync('sticker.webp'))
```
```python Python
with open("foto.jpg", "rb") as photo:
    await bot.send_image(chat, photo.read(), caption="mi foto")

with open("nota.ogg", "rb") as audio:
    await bot.send_audio(chat, audio.read(), voice_note=True)

with open("reporte.pdf", "rb") as document:
    await bot.send_document(chat, document.read(), file_name="reporte.pdf", mimetype="application/pdf")
```
```js Cliente JS
await bot.sendImage(chat, bytes, { caption: 'mi foto' })
await bot.sendAudio(chat, bytes, { voiceNote: true })
await bot.sendDocument(chat, bytes, { fileName: 'reporte.pdf', mimetype: 'application/pdf' })
```
:::

Las dimensiones y las miniaturas se calculan solas. Para notas de voz usa audio Opus en un contenedor OGG, que es lo que espera WhatsApp.

## Opciones

| Rust (`MediaOptions`) | JavaScript | Python | Para qué |
|---|---|---|---|
| `caption` | `caption` | `caption` | Texto que acompaña |
| `mimetype` | `mimetype` | `mimetype` | Tipo MIME, si no se deduce solo |
| `file_name` | `fileName` | `file_name` | Nombre del documento |
| `voice_note` | `voiceNote` | `voice_note` | Enviar el audio como nota de voz |
| `gif` | `gif` | `gif` | Reproducir el video como GIF |
| `view_once` | `viewOnce` | `view_once` | De una sola vista |
| `seconds` | `seconds` | `seconds` | Duración |
| `width`, `height` | — | — | Dimensiones, si no quieres las calculadas |
| `thumbnail` | — | — | Miniatura JPEG propia |
| — | `quoted` | `quoted` | Citar un mensaje |

## Preparar antes de enviar

`prepare_media` sube el archivo y te devuelve un `Message` al que puedes añadir lo que quieras antes de enviarlo.

```rust
use soblendwa::{MediaKind, MediaOptions};

let options = MediaOptions { caption: Some("una sola vez".into()), view_once: true, ..Default::default() };
let mensaje = client.prepare_media(&std::fs::read("foto.jpg")?, MediaKind::Image, options).await?;
client.send_message(&chat, mensaje.reply_to(&msg)).await?;
```

## Varias fotos seguidas

Los archivos para el mismo chat suben a la vez y se entregan en el orden en que los pediste. Solo la entrega espera turno; la subida no. Seis fotos que tardan 250 ms en subir cada una salen en unos 0,4 s en lugar de 1,5 s.

:::tabs
```rust Rust
let (uno, dos, tres) = (std::fs::read("1.jpg")?, std::fs::read("2.jpg")?, std::fs::read("3.jpg")?);
let (a, b, c) = tokio::join!(
    client.send_image(&chat, &uno, None),
    client.send_image(&chat, &dos, None),
    client.send_image(&chat, &tres, None),
);
(a?, b?, c?);
```
```js Node.js
await Promise.all(fotos.map((foto) => bot.sendImage(chat, foto)))
```
```python Python
await asyncio.gather(*(bot.send_image(chat, foto) for foto in fotos))
```
:::

Si necesitas armar el mensaje tú mismo, `send_prepared(chat, futuro)` hace lo mismo con cualquier future que devuelva un `Message`: el lugar en el chat se toma al llamarla y el future sigue corriendo mientras salen los anteriores.

## GIF y video circular

```rust
let gif = MediaOptions { gif: true, ..Default::default() };
client.send_media(&chat, &std::fs::read("anim.mp4")?, MediaKind::Video, gif).await?;

let video = client.prepare_media(&std::fs::read("clip.mp4")?, MediaKind::Video, MediaOptions::default()).await?;
client.send_message(&chat, Message::video_note(video)).await?;
```

## Álbumes

```rust
let a = client.prepare_media(&std::fs::read("a.jpg")?, MediaKind::Image, MediaOptions::default()).await?;
let b = client.prepare_media(&std::fs::read("b.jpg")?, MediaKind::Image, MediaOptions::default()).await?;
let ids = client.send_album(&chat, vec![a, b]).await?;
```

El primer id de la lista es el del álbum y los siguientes, los de cada foto o video.

## Descargar

:::tabs
```rust Rust
if msg.message.media().is_some() {
    let bytes = client.download_message(&msg.message).await?;
    std::fs::write(format!("{}.bin", msg.info.id), bytes)?;
}
```
```js Node.js
if (message.media) {
  const bytes = await bot.download(message)
  require('node:fs').writeFileSync(`${message.info.id}.bin`, bytes)
}
```
```python Python
if message["media"]:
    data = await bot.download(message)
    with open(message["info"]["id"] + ".bin", "wb") as output:
        output.write(data)
```
```js Cliente JS
if (message.media) {
  const bytes = await bot.download(message)
}
```
:::

La descarga verifica el HMAC y los dos SHA-256 antes de entregarte los bytes. Si el archivo ya caducó en el servidor, `download_event` y `download_event_to_file` le piden al remitente que lo vuelva a subir, y lo descargan de nuevo. {exp}

## Archivos grandes

Con estos métodos la memoria no depende del tamaño del archivo.

:::tabs
```rust Rust
client.send_media_file(&chat, "pelicula.mp4", MediaKind::Video, MediaOptions::default()).await?;
client.send_media_file(&chat, "reporte.pdf", MediaKind::Document, MediaOptions::default()).await?;
let bytes = client.download_event_to_file(&msg, "descargas/video.mp4").await?;
```
```js Node.js
await bot.sendFile(chat, 'video', 'pelicula.mp4')
const bytes = await bot.downloadToFile(message, 'descargas/video.mp4')
```
```python Python
await bot.send_file(chat, "video", "pelicula.mp4")
size = await bot.download_to_file(message, "descargas/video.mp4")
```
```js Cliente JS
await bot.sendFile(chat, 'video', '/ruta/en/el/servidor/pelicula.mp4')
const bytes = await bot.downloadToFile(message, '/ruta/en/el/servidor/video.mp4')
```
:::

`send_media_file` lee el archivo dos veces por bloques de 256 KB: una para calcular los hashes que WhatsApp pide antes de subir y otra para cifrar mientras sube. No crea archivos temporales. El tipo MIME y el nombre del documento se deducen de la extensión.

`download_to_file` escribe en `archivo.part` y solo lo renombra cuando la verificación pasa, así que un archivo corrupto nunca aparece con su nombre final.

| Método | Cuándo usarlo |
|---|---|
| `upload_file` | Subir un archivo del disco sin enviarlo todavía |
| `upload_reader` | Fuentes que solo se leen una vez, como un socket. Cifra hacia un archivo temporal y lo borra al terminar |
| `prepare_media_file` | Como `prepare_media`, desde una ruta |
| `download_to_writer` | Descargar hacia cualquier escritor |

:::note Rutas en el servidor
Con el cliente JS y por HTTP, `sendFile` y `downloadToFile` usan rutas **de la máquina del servidor**. Las demás funciones de multimedia envían los bytes en base64.
:::

## Reutilización automática

Si envías el mismo archivo otra vez, como un sticker o la imagen de un comando, no se vuelve a cifrar ni a subir: sale tan rápido como un texto. En una cuenta real, una imagen repetida pasó de 1.420 ms a 4 ms.

- El archivo se recuerda 7 días. Lo cambias con `.media_cache(Some(duración))` o lo desactivas con `.media_cache(None)`.
- Pasada una hora se comprueba en el CDN que sigue disponible. {exp}
- `client.forget_upload(kind, sha256)` olvida un archivo concreto.
- `stats().media_uploaded` y `stats().media_reused` muestran cuánto ahorras.

## Herramientas de imagen

El módulo `soblendwa::imaging` tiene las funciones que SoblendWA usa por dentro.

```rust
use soblendwa::imaging::{image_dimensions, profile_picture, thumbnail};

let datos = std::fs::read("foto.jpg")?;
let tamano = image_dimensions(&datos);
let miniatura = thumbnail(&datos, 100)?;
let perfil = profile_picture(&datos)?;
```

=== vista-previa | Vista previa de enlaces | Mensajes
lead: Si un texto trae un enlace, la vista previa se genera sola, como en la app oficial.

Incluye título, descripción, miniatura y, si la página tiene video, la marca de video. No tienes que llamar a nada distinto.

## Por mensaje

:::tabs
```rust Rust
use soblendwa::{LinkPreview, PreviewChoice, SendOptions};

client.send_text(&chat, "Mira esto: https://www.rust-lang.org").await?;

let sin_vista = SendOptions { link_preview: PreviewChoice::Off, ..Default::default() };
client.send_message_with(&chat, Message::text("https://www.rust-lang.org"), sin_vista).await?;

let propia = LinkPreview { title: Some("Mi título".into()), description: Some("Escrito a mano".into()), ..Default::default() };
let opciones = SendOptions { link_preview: PreviewChoice::Custom(Box::new(propia)), ..Default::default() };
client.send_message_with(&chat, Message::text("https://www.rust-lang.org"), opciones).await?;
```
```js Node.js
await bot.sendText(chat, 'Mira esto: https://www.rust-lang.org')
await bot.sendText(chat, 'https://www.rust-lang.org', { preview: false })
await bot.sendText(chat, 'https://www.rust-lang.org', { preview: { title: 'Mi título', description: 'Escrito a mano' } })
```
```python Python
await bot.send_text(chat, "Mira esto: https://www.rust-lang.org")
await bot.send_text(chat, "https://www.rust-lang.org", preview=False)
await bot.send_text(chat, "https://www.rust-lang.org", preview={"title": "Mi título", "description": "Escrito a mano"})
```
```js Cliente JS
await bot.sendText(chat, 'Mira esto: https://www.rust-lang.org')
await bot.sendText(chat, 'https://www.rust-lang.org', { preview: false })
await bot.sendText(chat, 'https://www.rust-lang.org', { preview: { title: 'Mi título', description: 'Escrito a mano' } })
```
:::

| `PreviewChoice` | JavaScript y Python | Qué hace |
|---|---|---|
| `Auto` | sin opción | Genera la vista previa si hay un enlace. Es lo normal |
| `On` | `preview: true` | La pide de forma explícita |
| `Off` | `preview: false` | Sin vista previa |
| `Custom(..)` | `preview: { … }` | Usa tus datos: `url`, `title`, `description`, `thumbnail`, `image`, `width`, `height` y `video` |

`thumbnail` es la miniatura pequeña que va dentro del mensaje, en JPEG, e `image` es la imagen nítida que se sube cifrada. En Node.js y el cliente JS puedes pasarlas como `Buffer` o `Uint8Array`; en Python, como `bytes`. Así una bienvenida o un menú llevan siempre tu propia imagen, sin consultar ninguna página.

```js
const fs = require('node:fs')

await bot.sendText(chat, 'Bienvenido https://example.com/reglas', {
  preview: { url: 'https://example.com/reglas', title: 'Reglas del grupo', description: 'Léelas antes de escribir', thumbnail: fs.readFileSync('miniatura.jpg') },
})
```

## Para todo el cliente

:::tabs
```rust Rust
use std::time::Duration;
use soblendwa::LinkPreviewConfig;

let config = LinkPreviewConfig { timeout: Duration::from_secs(2), enabled: true, ..Default::default() };
let (client, events) = Client::builder().store(store).link_previews(config).build().await?;
```
```js Node.js
const bot = await SoblendWA.create({ linkPreviews: { enabled: true, timeoutMs: 2000 } })
```
```python Python
bot = await SoblendWA.create(link_previews={"enabled": True, "timeout_ms": 2000})
```
```bash Servidor
soblendwa-server --no-link-previews
```
:::

| Rust | JavaScript | Por defecto | Para qué |
|---|---|---|---|
| `enabled` | `enabled` | `true` | Activa las vistas previas |
| `timeout` | `timeoutMs` | 3 s | Tiempo máximo por petición |
| `upload_hq_thumbnail` | `uploadHqThumbnail` | `true` | Sube la miniatura nítida |
| `allow_private_hosts` | `allowPrivateHosts` | `false` | Permite consultar hosts privados |
| `max_html_bytes` | `maxHtmlBytes` | 512 KiB | Máximo de HTML leído |
| `max_image_bytes` | `maxImageBytes` | 4 MiB | Máximo de imagen descargada |
| `user_agent` | `userAgent` | — | Agente de usuario de la consulta |
| `cache_ttl` | `cacheTtlSecs` | 1 hora | Cuánto se recuerda un enlace |
| `fetcher` | — | Ninguno | Tu propia fuente de datos, en Rust |

## Cómo funciona

- La consulta sale de tu servidor. Para generar la vista previa, el bot abre la página, como hace el teléfono en la app oficial. Si repite enlaces de desconocidos y no quieres eso, usa `enabled: false`.
- Protegido contra enlaces maliciosos. Nunca consulta hosts privados: `127.0.0.1`, la red interna, el servicio de metadatos de la nube (`169.254.169.254`), `localhost`, `*.local` ni `*.internal`. También revisa cada redirección y a qué IP resuelve el dominio, así un dominio público que apunte a tu red interna tampoco pasa.
- Dos miniaturas. Una pequeña dentro del mensaje y otra nítida de hasta 1200 px, subida cifrada. Si la subida falla, el mensaje sale igual con la pequeña. La nítida aún es {exp}
- Reutiliza lo que ya hizo. Un enlace repetido no vuelve a consultar la página, por 1 hora, ni a subir la miniatura.
- Nunca frena un envío. Si la página falla o no responde, sale una tarjeta mínima con el dominio.
- Imágenes hostiles. La imagen se decodifica con límites de tamaño y memoria.

## Tu propia fuente de datos

Implementa `LinkPreviewFetcher` para traer los datos de otro servicio. Si devuelves `None`, el enlace se envía sin tus datos.

```rust
use std::sync::Arc;
use async_trait::async_trait;
use soblendwa::{LinkPreview, LinkPreviewConfig, LinkPreviewFetcher};

struct MiFuente;

#[async_trait]
impl LinkPreviewFetcher for MiFuente {
    async fn fetch(&self, url: &str) -> soblendwa::Result<Option<LinkPreview>> {
        Ok(Some(LinkPreview { url: url.to_owned(), title: Some("Desde mi API".into()), ..Default::default() }))
    }
}

let config = LinkPreviewConfig { fetcher: Some(Arc::new(MiFuente)), ..Default::default() };
```

## Otras funciones

| Método | Qué hace |
|---|---|
| `client.link_preview(url)` | Obtiene los datos de un enlace sin enviar nada |
| `client.with_link_preview(message)` | Devuelve el mensaje con su vista previa añadida |
| `client.send_text_with_preview(chat, text)` | Envía el texto pidiendo la vista previa |
| `client.set_link_previews_disabled(true)` | Cambia el ajuste de privacidad de vistas previas de tu cuenta |
| `message.link_thumbnail()` | Datos para descargar la miniatura nítida de un mensaje recibido |

Los mensajes recibidos traen en JSON `linkPreview` con `url`, `title`, `description`, `video`, `hasThumbnail`, `hasHighQualityThumbnail`, `thumbnailWidth` y `thumbnailHeight`.

=== encuestas | Encuestas y eventos | Mensajes
lead: Crea encuestas y eventos de calendario, y lee los votos y las respuestas ya descifrados.

## Crear una encuesta

El último número es cuántas opciones se pueden elegir. Con `1` se usa el formato de opción única; con `0` o más de una, el de varias opciones, igual que la app oficial.

:::tabs
```rust Rust
let encuesta = client.send_message(&chat, Message::poll("¿Pizza o tacos?", ["Pizza", "Tacos"], 1)).await?;
client.send_message(&chat, Message::poll("¿Qué te gusta?", ["Rust", "Go", "Zig"], 0)).await?;
```
```js Node.js
const encuesta = await bot.sendPoll(chat, '¿Pizza o tacos?', ['Pizza', 'Tacos'], 1)
await bot.sendPoll(chat, '¿Qué te gusta?', ['Rust', 'Go', 'Zig'], 0)
```
```python Python
encuesta = await bot.send_poll(chat, "¿Pizza o tacos?", ["Pizza", "Tacos"], 1)
await bot.send_poll(chat, "¿Qué te gusta?", ["Rust", "Go", "Zig"], 0)
```
```js Cliente JS
const encuesta = await bot.sendPoll(chat, '¿Pizza o tacos?', ['Pizza', 'Tacos'], 1)
await bot.sendPoll(chat, '¿Qué te gusta?', ['Rust', 'Go', 'Zig'], 0)
```
:::

`Message::announcement_poll` es para grupos de anuncios de comunidades.

## Votar

:::tabs
```rust Rust
client.vote_poll(&chat, &encuesta, &["Tacos"]).await?;
```
```js Node.js
await bot.votePoll(chat, encuesta, ['Tacos'])
```
```python Python
await bot.vote_poll(chat, encuesta, ["Tacos"])
```
```js Cliente JS
await bot.votePoll(chat, encuesta, ['Tacos'])
```
:::

## Leer los votos

:::tabs
```rust Rust
if let Event::PollVote(voto) = event {
    println!("{} votó {:?} en {}", voto.voter, voto.selected, voto.poll_id);
}
```
```js Node.js
bot.on('pollVote', ({ voter, selected, pollId }) => console.log(voter, selected, pollId))
```
```python Python
@bot.on("pollVote")
def voted(event):
    print(event["voter"], event["selected"], event["pollId"])
```
```js Cliente JS
bot.on('pollVote', ({ voter, selected, pollId }) => console.log(voter, selected, pollId))
```
:::

Las opciones llegan por nombre. Si activas el [store de chats](#store-chats), los votos también se guardan en `poll_votes`.

## Eventos de calendario

```rust
use soblendwa::events::EventResponseKind;

let evento = Message::event("Lanzamiento SoblendWA", 1_800_000_000)
    .event_details(Some("En vivo"), Some(1_800_003_600), Some((19.4326, -99.1332, "CDMX")), None, true);
let id = client.send_message(&chat, evento).await?;
client.respond_event(&chat, &id, EventResponseKind::Going, 0).await?;
```

Los parámetros de `event_details` son: descripción, hora de fin en segundos Unix, lugar como `(latitud, longitud, nombre)`, enlace para unirse y si se permiten invitados extra. `.cancel_event()` marca el evento como cancelado.

## Leer las respuestas

`Event::EventResponse` trae `chat`, `event_id`, `responder`, `kind`, `extra_guests` y `timestamp`.

| `kind` en Rust | En JSON | Qué significa |
|---|---|---|
| `Going` | `going` | Va |
| `NotGoing` | `notGoing` | No va |
| `Maybe` | `maybe` | Tal vez |
| `Unknown` | `unknown` | Respuesta no reconocida |

=== interactivos | Botones y listas | Mensajes
lead: Los mensajes interactivos con el formato que WhatsApp sí muestra.

:::note Qué formato usar
`Interactive`, `Message::buttons` y `Message::list` usan el formato *native flow*, y se probaron en un teléfono real. `legacy_buttons` y `legacy_list` generan los formatos viejos, que WhatsApp ya no muestra en cuentas normales, y quedan solo por compatibilidad.
:::

## Menú interactivo

:::tabs
```rust Rust
use soblendwa::interactive::{Interactive, Section};

let menu = Interactive::new("¿Qué quieres hacer?")
    .title("Soblend")
    .footer("SoblendWA")
    .quick_reply("ping", "Medir latencia")
    .url("Documentación", "https://github.com/Soblend-Development-Studio/SoblendWA")
    .copy("Copiar cupón", "SOBLEND-2026")
    .call("Llamar", "+5215512345678")
    .select("Ver comandos", vec![Section::new("Comandos").described_row("menu", "Menú", "Todos los comandos")]);
client.send_message(&chat, menu.build()).await?;
```
```js Node.js
await bot.sendInteractive(chat, {
  body: '¿Qué quieres hacer?',
  title: 'Soblend',
  footer: 'SoblendWA',
  buttons: [
    { type: 'reply', id: 'ping', text: 'Medir latencia' },
    { type: 'url', text: 'Documentación', url: 'https://github.com/Soblend-Development-Studio/SoblendWA' },
    { type: 'copy', text: 'Copiar cupón', code: 'SOBLEND-2026' },
    { type: 'call', text: 'Llamar', phone: '+5215512345678' },
    { type: 'select', title: 'Ver comandos', sections: [{ title: 'Comandos', rows: [{ id: 'menu', title: 'Menú', description: 'Todos los comandos' }] }] },
  ],
})
```
```python Python
await bot.send_interactive(
    chat,
    "¿Qué quieres hacer?",
    [
        {"type": "reply", "id": "ping", "text": "Medir latencia"},
        {"type": "url", "text": "Documentación", "url": "https://github.com/Soblend-Development-Studio/SoblendWA"},
        {"type": "copy", "text": "Copiar cupón", "code": "SOBLEND-2026"},
    ],
    title="Soblend",
    footer="SoblendWA",
)
```
```js Cliente JS
await bot.sendInteractive(chat, {
  body: '¿Qué quieres hacer?',
  footer: 'SoblendWA',
  buttons: [
    { type: 'reply', id: 'ping', text: 'Medir latencia' },
    { type: 'url', text: 'Documentación', url: 'https://github.com/Soblend-Development-Studio/SoblendWA' },
  ],
})
```
:::

| Botón | Rust | JSON | Qué hace |
|---|---|---|---|
| Respuesta rápida | `.quick_reply(id, texto)` | `{ type: 'reply', id, text }` | Envía una respuesta con ese id |
| Abrir enlace | `.url(texto, url)` | `{ type: 'url', text, url }` | Abre la página |
| Copiar | `.copy(texto, código)` | `{ type: 'copy', text, code }` | Copia un código |
| Llamar | `.call(texto, teléfono)` | `{ type: 'call', text, phone }` | Inicia una llamada |
| Menú | `.select(título, secciones)` | `{ type: 'select', title, sections }` | Abre una lista de opciones |

La cabecera puede llevar texto (`.title`, `.subtitle`) o multimedia ya preparada (`.image`, `.video`, `.document`).

## Botones simples

:::tabs
```rust Rust
client.send_message(&chat, Message::buttons("Elige", Some("pie"), [("si", "Sí"), ("no", "No")])).await?;
```
```js Node.js
await bot.sendButtons(chat, 'Elige', [{ id: 'si', text: 'Sí' }, { id: 'no', text: 'No' }], 'pie')
```
```python Python
await bot.send_buttons(chat, "Elige", [("si", "Sí"), ("no", "No")], "pie")
```
```js Cliente JS
await bot.sendButtons(chat, 'Elige', [{ id: 'si', text: 'Sí' }, { id: 'no', text: 'No' }], 'pie')
```
:::

## Listas

:::tabs
```rust Rust
use soblendwa::interactive::Section;

client.send_message(&chat, Message::list("Menú", "Elige un plato", "Ver menú", None, &[
    Section::new("Tacos").row("pastor", "Al pastor").row("suadero", "Suadero"),
])).await?;
```
```js Node.js
await bot.sendList(chat, {
  title: 'Menú',
  text: 'Elige un plato',
  buttonText: 'Ver menú',
  sections: [{ title: 'Tacos', rows: [{ id: 'pastor', title: 'Al pastor' }, { id: 'suadero', title: 'Suadero' }] }],
})
```
```python Python
await bot.send_list(chat, "Menú", "Elige un plato", "Ver menú", [
    {"title": "Tacos", "rows": [{"id": "pastor", "title": "Al pastor"}, {"id": "suadero", "title": "Suadero"}]},
])
```
```js Cliente JS
await bot.sendList(chat, {
  title: 'Menú',
  text: 'Elige un plato',
  buttonText: 'Ver menú',
  sections: [{ title: 'Tacos', rows: [{ id: 'pastor', title: 'Al pastor' }, { id: 'suadero', title: 'Suadero' }] }],
})
```
:::

## Leer la opción elegida

:::tabs
```rust Rust
if let Some(reply) = msg.message.interactive_reply() {
    println!("eligió {} ({:?})", reply.id, reply.source);
}
```
```js Node.js
bot.on('message', (message) => {
  if (message.reply?.id === 'ping') bot.reply(message, 'Pong')
})
```
```python Python
@bot.on("message")
async def chosen(message):
    if message["reply"] and message["reply"]["id"] == "ping":
        await bot.reply(message, "Pong")
```
```js Cliente JS
bot.on('message', (message) => {
  if (message.reply?.id === 'ping') bot.reply(message, 'Pong')
})
```
:::

`reply` trae `id`, `text`, `source` (`buttons`, `list`, `template` o `nativeFlow`), `flow` y `params`.

## Plantillas y respuestas

| Método | Qué hace |
|---|---|
| `Message::template(texto, pie, botones)` | Mensaje de plantilla con `TemplateButton` |
| `Message::button_reply(id, texto)` | Responde a un botón |
| `Message::template_reply(id, texto, índice)` | Responde a una plantilla |
| `Message::list_reply(título, id_fila)` | Responde a una lista |
