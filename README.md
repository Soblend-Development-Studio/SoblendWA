<p align="center">
  <a href="https://github.com/Soblend-Development-Studio/SoblendWA">
    <picture>
      <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/Soblend-Development-Studio/SoblendWA/main/assets/banner-dark.svg">
      <img alt="SoblendWA: WhatsApp Web en Rust, para bots en cualquier lenguaje" src="https://raw.githubusercontent.com/Soblend-Development-Studio/SoblendWA/main/assets/banner-light.svg" width="100%">
    </picture>
  </a>
</p>

<p align="center">
  <strong>Cliente de WhatsApp Web multi-dispositivo, escrito en Rust, para bots en cualquier lenguaje.</strong><br>
  Hecho por <strong>Soblend Development Studio</strong>.
</p>

<p align="center">
  <a href="https://github.com/Soblend-Development-Studio/SoblendWA/actions/workflows/ci.yml"><img alt="CI" src="https://img.shields.io/github/actions/workflow/status/Soblend-Development-Studio/SoblendWA/ci.yml?branch=main&label=CI"></a>
  <a href="https://crates.io/crates/soblendwa"><img alt="crates.io" src="https://img.shields.io/crates/v/soblendwa?color=e6732e"></a>
  <a href="https://docs.rs/soblendwa"><img alt="docs.rs" src="https://img.shields.io/docsrs/soblendwa"></a>
  <a href="https://www.npmjs.com/package/soblendwa"><img alt="npm" src="https://img.shields.io/npm/v/soblendwa?color=cb3837"></a>
  <a href="https://pypi.org/project/soblendwa/"><img alt="PyPI" src="https://img.shields.io/pypi/v/soblendwa?color=3775a9"></a>
  <img alt="Rust 1.89 o más reciente" src="https://img.shields.io/badge/rust-1.89%2B-e6732e">
  <img alt="Sin código inseguro" src="https://img.shields.io/badge/unsafe-prohibido-2ea44f">
  <a href="https://github.com/Soblend-Development-Studio/SoblendWA/blob/main/LICENSE"><img alt="Licencia MIT" src="https://img.shields.io/badge/licencia-MIT-4f46e5"></a>
</p>

<p align="center">
  <a href="#instalación">Instalación</a> ·
  <a href="#inicio-rápido">Inicio rápido</a> ·
  <a href="#rendimiento">Rendimiento</a> ·
  <a href="#comparación">Comparación</a> ·
  <a href="#funciones">Funciones</a> ·
  <a href="#guía-de-rust">Guía</a> ·
  <a href="#servidor-cualquier-lenguaje">Servidor</a> ·
  <a href="#migrar-desde-otra-librería">Migrar</a> ·
  <a href="#estado">Estado</a>
</p>

---

SoblendWA habla el protocolo de WhatsApp Web directamente: WebSocket, handshake Noise, nodos binarios y cifrado de
extremo a extremo con el protocolo Signal. No usa navegador ni Puppeteer. Es la misma idea que Baileys, whatsmeow o zapo,
pero compilada a código nativo: sin recolector de basura, sin pausas y con un consumo de memoria mínimo.

> **Aviso:** SoblendWA es independiente y no está afiliado a WhatsApp ni a Meta. Automatizar una cuenta puede ir contra sus
> Términos de Servicio y provocar un bloqueo. Lee el [aviso completo](#aviso) y qué está probado en [Estado](#estado).

## Por qué SoblendWA

<table>
  <tr>
    <td width="33%" valign="top"><strong>Rápido</strong><br>En un bot de eco contra el mismo servidor: 4,2 ms de ida y vuelta frente a 10,5 ms de zapo, con seis veces menos CPU por mensaje.</td>
    <td width="33%" valign="top"><strong>Ligero</strong><br>12 MB de memoria tras la prueba, frente a 130 MB. Conecta y vincula en 100 ms, no en 1,5 s.</td>
    <td width="33%" valign="top"><strong>En tu lenguaje</strong><br>Rust, Node.js, TypeScript, Python y cualquier otro por WebSocket, HTTP o webhooks. El mismo motor en todos.</td>
  </tr>
  <tr>
    <td valign="top"><strong>Hecho para correr meses</strong><br>Outbox persistente, reconexión escalonada, límite de envíos y pánicos contenidos. Si bloquean la cuenta, se detiene sin borrar la sesión.</td>
    <td valign="top"><strong>Migra sin escanear</strong><br>Importa sesiones de Baileys y de zapo sin volver a vincular. Si algo no te convence, vuelves a zapo con rollback.</td>
    <td valign="top"><strong>Prueba sin arriesgar tu número</strong><br>WhatsApp falso con el protocolo real: cifrado, grupos, multimedia y fallos provocados, también desde JavaScript.</td>
  </tr>
</table>

## Instalación

| Si tu bot está en… | Instala | Sigue en |
|---|---|---|
| Rust | `cargo add soblendwa` | [Rust](#rust) |
| Node.js o TypeScript, en el mismo proceso | `npm install soblendwa` | [Node.js](#nodejs-y-typescript) |
| Python (asyncio) | `pip install soblendwa` | [Python](#python) |
| JavaScript o TypeScript sin módulos nativos (Node, Bun, Deno, navegador) | `npm install soblendwa-client` y `soblendwa-server` | [Sin compilar nada](#sin-compilar-nada) |
| Cualquier otro lenguaje (Go, PHP, Java, C#…) o en otra máquina | `soblendwa-server` | [Servidor](#servidor-cualquier-lenguaje) |

Todas las opciones usan el mismo motor en Rust y tienen las mismas funciones principales.

| Paquete | Dónde funciona |
|---|---|
| `soblendwa` en crates.io | Rust 1.89 o más reciente |
| `soblendwa` en npm | Node.js 18 o más reciente en Windows x64, macOS (Intel y ARM) y Linux x64 o ARM64 con glibc 2.35 o más reciente (Ubuntu 22.04, Debian 12). Trae los binarios de las cinco plataformas, así que la descarga ronda los 40 MB. En Alpine usa el servidor |
| `soblendwa` en PyPI | Python 3.9 o más reciente en Windows x64, macOS (Intel y ARM) y Linux x64 o ARM64 (manylinux 2.28) |
| `soblendwa-client` en npm | Node.js 18 o más reciente, Bun, Deno y el navegador. Sin dependencias |
| `soblendwa-server` | Binarios listos para Linux x64 y ARM64 (estáticos), Windows x64 y macOS (Intel y ARM) |

En Rust, estas son las features opcionales: `sqlite`, `postgres`, `redis`, `mongo` o `stores` (las cuatro), y `testing`
(el WhatsApp falso).

```toml
[dependencies]
soblendwa = { version = "0.1", features = ["sqlite"] }
tokio = { version = "1", features = ["full"] }
```

### Sin compilar nada

Cada versión publica binarios de `soblendwa-server` en
[Releases](https://github.com/Soblend-Development-Studio/SoblendWA/releases). Los de Linux son estáticos: funcionan en
cualquier VPS sin instalar Rust ni librerías.

```bash
curl -L -o soblendwa.tar.gz https://github.com/Soblend-Development-Studio/SoblendWA/releases/latest/download/soblendwa-server-x86_64-unknown-linux-musl.tar.gz
tar -xzf soblendwa.tar.gz && ./soblendwa-server --token "una-clave-larga" --session bot
```

Para compilarlo tú: `cargo install --git https://github.com/Soblend-Development-Studio/SoblendWA soblendwa-server`.

Usó ~14 MB de RAM con dos cuentas contra el WhatsApp falso, así que un VPS de 2 núcleos le queda de sobra.

## Inicio rápido

### Rust

```rust,no_run
use soblendwa::{Client, Event, FileStore};

#[tokio::main]
async fn main() -> soblendwa::Result<()> {
    let store = FileStore::open("session").await?;
    let (client, mut events) = Client::builder().store(store).push_name("Mi Bot").build().await?;
    client.connect().await?;

    while let Some(event) = events.recv().await {
        match event {
            Event::Qr(code) => println!("Escanea este QR: {code}"),
            Event::Connected => println!("Conectado como {:?}", client.me()),
            Event::Message(msg) if !msg.info.from_me => {
                if msg.text() == Some("!ping") {
                    let ms = msg.info.elapsed().as_secs_f64() * 1000.0;
                    client.reply(&msg, format!("Pong\n{ms:.2} ms")).await?;
                }
            }
            Event::LoggedOut { reason } => {
                println!("Sesión cerrada: {reason}");
                break;
            }
            _ => {}
        }
    }
    Ok(())
}
```

### Node.js y TypeScript

```js
const { SoblendWA } = require('soblendwa')

async function main() {
  const bot = await SoblendWA.create({ store: { type: 'sqlite', path: 'session/auth.db' } })

  bot.on('qr', ({ code }) => console.log('Escanea este QR:', code))
  bot.on('message', async (message) => {
    if (message.info.fromMe) return
    if (message.text === '!ping') await bot.reply(message, `Pong\n${bot.elapsed(message).toFixed(2)} ms`)
  })

  await bot.connect()
}

main()
```

Los tipos de TypeScript vienen incluidos. Más detalle en
[`bindings/node`](https://github.com/Soblend-Development-Studio/SoblendWA/tree/main/bindings/node).

### Python

```python
import asyncio
from soblendwa import SoblendWA


async def main():
    bot = await SoblendWA.create(store={"type": "sqlite", "path": "session/auth.db"})

    @bot.on("qr")
    def show_qr(event):
        print("Escanea este QR:", event["code"])

    @bot.on("message")
    async def answer(message):
        if message["info"]["fromMe"]:
            return
        if message["text"] == "!ping":
            await bot.reply(message, f"Pong\n{bot.elapsed(message):.2f} ms")

    await bot.run()


asyncio.run(main())
```

Más detalle en [`bindings/python`](https://github.com/Soblend-Development-Studio/SoblendWA/tree/main/bindings/python).

### JavaScript con el servidor

Con `soblendwa-server` corriendo, tu bot no compila nada y funciona en Node, Bun, Deno o el navegador:

```js
import { connect } from 'soblendwa-client'

const bot = await connect('ws://127.0.0.1:8787', { session: 'bot', token: 'una-clave-larga' })

bot.on('message', async (message) => {
  if (message.info.fromMe) return
  if (message.text === '!ping') await bot.reply(message, `Pong\n${(await bot.elapsed(message)).toFixed(2)} ms`)
})
```

### Vincular tu teléfono

El texto del evento `qr` es el contenido del código QR: dibújalo con cualquier librería de QR y escanéalo en
**WhatsApp > Dispositivos vinculados**. El servidor y el bot de ejemplo ya lo dibujan en la terminal.

Para usar un código en lugar de QR, llama a `client.request_pair_code("5215512345678")` (en JavaScript
`bot.requestPairCode`, en Python `bot.request_pair_code`) después del primer `qr` y escríbelo en
**Dispositivos vinculados > Vincular con el número de teléfono**.

## Rendimiento

### Un bot completo contra zapo

Un bot de eco de punta a punta. Los dos se conectan al mismo `@zapo-js/fake-server` con el protocolo real (Noise,
vinculación por QR, Signal), cada uno en su propio proceso y con almacenamiento en memoria. zapo corre con su acelerador
`@zapo-js/native`.

| Mediana de 3 corridas alternadas (500 idas y vueltas + 500 en ráfaga) | SoblendWA | zapo-js 1.11 |
|---|---|---|
| Ida y vuelta, p50 / p95 / p99 | **4,2 / 10,1 / 16,3 ms** | 10,5 / 18,4 / 23,3 ms |
| Mensajes por segundo en ráfaga | **474** | 228 |
| CPU del bot por mensaje en ráfaga | **0,7 ms** | 4,5 ms |
| Memoria tras la prueba (pico) | **12 MB (24 MB)** | 130 MB (131 MB) |
| Conectar y vincular | **100 ms** | 1.565 ms |

Medido en una sola máquina Windows con otros programas abiertos, así que los valores absolutos varían entre corridas: la
ráfaga de SoblendWA fue de 394 a 786 msg/s. Los tiempos incluyen al servidor falso, que corre en Node, así que marcan un
piso para los dos. La memoria y la conexión de zapo incluyen el arranque de Node. Es un bot mínimo, sin multimedia, base
de datos ni red real; en producción la mayor parte de la espera es la red hasta WhatsApp, igual para cualquier librería.

### Las piezas, una por una

| | SoblendWA (Rust) | whatsmeow (Go) | Baileys 7 (JavaScript) |
|---|---|---|---|
| Codificar + decodificar un stanza de 21 nodos (3.375 bytes) | **63.242 stanzas/s** | 12.810 (4,9× menos) | 5.320 (11,9× menos) |
| Cifrar + descifrar un mensaje 1:1 (Signal) | **135.504 msg/s** | 12.223 (11× menos) | 3.720 (36× menos) |
| Cifrar + descifrar un mensaje de grupo (Sender Keys) | **9.117 msg/s** | 3.565 (2,6× menos) | 58 (157× menos) |

Se midió en la misma máquina contra el código real de cada librería: whatsmeow del 2 de noviembre de 2026 con su
`libsignal` v0.2.2 (Go 1.27.2) y Baileys 7.0.0-rc14 con su `libsignal` y su `GroupCipher`. Mismo stanza, mismo mensaje
de 120 bytes y mismas operaciones; las tres librerías producen exactamente los mismos 3.375 bytes al codificar. SoblendWA
serializa la sesión en cada mensaje; a whatsmeow y Baileys se les midió con los registros como objetos en memoria, que
es su caso más rápido (whatsmeow serializando como su almacén SQL baja a 6.773 msg/s). Puedes repetir lo nuestro con
`cargo test --release -- --ignored throughput`.

### Por qué es rápido

Desde que llega un mensaje hasta que sale la respuesta, el bot tarda **~0,05 ms** en un chat conocido. El resto del
tiempo que ves en el teléfono es la red hasta los servidores de WhatsApp: ninguna librería puede bajarla.

- **Sin esperas de red en chats conocidos.** Las sesiones, los dispositivos y los datos de los grupos se guardan en
  disco y se usan al instante, aunque reinicies el bot. Se actualizan en segundo plano y cuando WhatsApp avisa de un
  cambio. Al conectar se precargan tus grupos y los dispositivos de sus miembros.
- **Grupos grandes rápidos desde el primer mensaje.** Las consultas de claves y dispositivos salen en paralelo (un solo
  viaje de red) y las sesiones Signal se crean usando todos los núcleos: 1000 dispositivos en ~0,1 s en lugar de ~0,4 s.
- **Socket sin demoras.** `TCP_NODELAY` evita que el sistema retenga mensajes pequeños. El lector y el escritor tienen
  cada uno su propio cifrador AES-GCM y no comparten locks.
- **Arranque rápido.** La versión de WhatsApp Web se recuerda y se actualiza en segundo plano.
- **Almacenamiento sin bloqueos.** `FileStore` escribe en un log de solo-escritura únicamente lo que cambió y compacta
  de vez en cuando. Los backends SQL y Redis agrupan las escrituras en segundo plano.
- **Colas por chat, al recibir y al enviar.** Los chats distintos van en paralelo y cada chat conserva su orden: un texto
  nunca espera detrás de un video pesado de otro chat, y en el mismo chat sale en el orden en que lo pediste.
- **Multimedia reutilizada.** Si envías el mismo archivo otra vez (un sticker, la imagen de un comando), no se vuelve a
  cifrar ni a subir: sale tan rápido como un texto. Se guarda 7 días y se verifica en el CDN pasada una hora.
- **Sin código inseguro.** `#![forbid(unsafe_code)]`, y el decodificador binario está probado contra cientos de miles de
  entradas hostiles.

## Comparación

| | SoblendWA | Baileys 7 | whatsmeow | zapo-js 1.11 |
|---|---|---|---|---|
| Lenguaje | Rust | JavaScript | Go | TypeScript |
| Se usa desde | Rust, Node.js, Python y cualquier lenguaje por WebSocket, HTTP o webhooks | JavaScript, TypeScript | Go | JavaScript, TypeScript |
| Almacenamiento incluido | Archivos, memoria, SQLite, PostgreSQL, Redis y MongoDB | Archivos | SQLite y PostgreSQL | SQLite, PostgreSQL, MySQL, Redis y MongoDB (paquetes aparte) |
| Vista previa automática de enlaces | Sí, con bloqueo de hosts privados | Con `link-preview-js` | A mano | Sí |
| Outbox persistente y límite de envíos | Sí | — | — | — |
| Importar la sesión de otra librería | Baileys y zapo, con rollback a zapo | — | — | — |
| WhatsApp falso para pruebas | Incluido | — | — | `@zapo-js/fake-server` |
| Llamadas con audio | — | — | — | `@zapo-js/voip` |
| Actuar como teléfono principal | — | — | — | Sí |
| Madurez | Versión 0.1, faltan pruebas largas | Años de uso | Años de uso | Estable desde la 1.0 |

`—` significa que no viene incluido o que no lo encontramos en esa versión. Si algo cambió o nos equivocamos,
[abre un issue](https://github.com/Soblend-Development-Studio/SoblendWA/issues) y lo corregimos.

## Confiabilidad

Pensado para bots que corren meses sin que nadie los mire, con varias cuentas en el mismo proceso.

- **Si WhatsApp bloquea la cuenta, se detiene y te dice por qué.** Un `403` (cuenta bloqueada), `402` (baneo temporal)
  o `406` (baneo) emite `Event::AccountBlocked { code, reason, expires }` y deja de reintentar. La sesión **no se
  borra**: si desbloqueas la cuenta en el teléfono, vuelves a conectar sin escanear otro QR.
- **Reconexión con espera creciente y aleatoria.** De 0 a 0,75 s la primera vez, luego 1 s, 2 s, 4 s… hasta 60 s, con
  un margen aleatorio para que cien sesiones no se reconecten en el mismo milisegundo. El servidor además escalona el
  arranque de las sesiones.
- **Límite de envíos.** Un bug que manda mensajes en bucle no se convierte en spam: por defecto 240 mensajes por minuto
  en total (ráfagas de 60) y 40 por chat (ráfagas de 15). Lo que pase el límite devuelve `Error::RateLimited` con el
  tiempo de espera. Se ajusta con `.send_limit(Some(SendLimit { .. }))` o se desactiva con `.send_limit(None)`.
- **Un fallo no tumba todo.** Si un handler entra en pánico, se registra, se cuenta en `stats().panics` y la sesión
  sigue. Cada sesión del servidor es independiente: si una se cuelga abriendo su almacenamiento, las demás siguen.
- **Reinicios sin pérdidas.** Cada mensaje se guarda en un outbox hasta que WhatsApp confirma que lo recibió. Si se
  cae la conexión o reinicias el proceso, se reenvía al reconectar **con el mismo ID**, y un mensaje que sigue en vuelo no
  se reenvía. Al recibir, SoblendWA descarta los duplicados por ID (`stats().duplicates`); los bots con otras librerías
  deben hacer lo mismo. `client.pending_sends()` muestra lo que está esperando.
- **Reintentos después de reiniciar.** Lo enviado se conserva hasta dos días, así un contacto que no pudo descifrar un
  mensaje puede pedirlo de nuevo aunque el bot se haya reiniciado.
- **Mensajes viejos marcados.** Los que llegaron mientras el bot estaba apagado traen `info.offline = true` (y en JSON
  `ageSecs`), para no ejecutar comandos de hace horas: `if msg.info.is_stale(Duration::from_secs(120)) { return }`.
- **Número y LID ya resueltos.** Cada mensaje trae `info.sender_phone` y `info.sender_lid`, sin importar con cuál de
  los dos llegó. En grupos con LID ya no hace falta buscar el número en ocho lugares para revisar permisos:
  `group.is_admin(&sender)` funciona con cualquiera de los dos. Si WhatsApp no comparte el número de alguien,
  `sender_phone` queda vacío; el LID sigue sirviendo para identificarlo.
- **Mensajes normalizados.** `msg.text()`, `msg.message.media()` y `msg.quoted()` funcionan igual aunque el mensaje
  venga envuelto en `ephemeral`, `viewOnce`, `documentWithCaption`, hijos de álbum, Meta AI, menciones de grupo o estados
  de grupo. En JSON: `text`, `media` (tipo, mimetype, tamaño, caption, duración, dimensiones), `quoted` (id, remitente,
  texto y media de lo citado) y `viewOnce`.
- **Tiempos por fase.** Cada `ServerAck` trae `timings`: cuánto esperó en la cola de su chat, cuánto tardó la subida,
  preparar (dispositivos y sesiones), cifrar y la respuesta de WhatsApp.
- **Métricas.** `client.stats()` devuelve mensajes recibidos y enviados, los que no se pudieron descifrar, los que se
  recuperaron tras el reintento (`Event::MessageRecovered`) y los que siguen sin recuperar, reconexiones, envíos
  frenados, multimedia subida y reutilizada, reenvíos del outbox, pánicos, cachés y latencia. Desde el servidor o el
  cliente JS: `await bot.stats()`.

## Funciones

### Conexión y seguridad

- Vinculación con código QR o con un código de 8 caracteres usando tu número.
- Handshake Noise `XX_25519_AESGCM_SHA256` que verifica la cadena de certificados de WhatsApp y su vigencia.
- Protocolo Signal completo: X3DH, Double Ratchet, Sender Keys, rotación de pre-keys y de la signed pre-key (cada 30
  días), y reintentos automáticos en ambos sentidos. Probado contra `libsignal` en las dos direcciones.
- Reconexión automática, keepalive, latencia medida en todo momento y detección de la última versión de WhatsApp Web.
- Proxy HTTP o SOCKS5, con usuario y contraseña, para el socket y la multimedia.
- Tokens de privacidad (`tctoken`) y reporting tokens emitidos y adjuntados como en la app oficial.

### Mensajes

- Texto, respuestas, menciones (también `@todos`), reacciones, ubicación, uno o varios contactos, editar, eliminar para
  todos, reenviar y fijar.
- Vista previa de enlaces automática, con miniatura nítida y bloqueo de hosts privados.
- Encuestas de una o varias opciones y para grupos de anuncios, votar y leer los votos descifrados.
- Eventos de calendario (con descripción, lugar, fin y enlace), responderlos y leer las respuestas descifradas.
- Botones, listas y mensajes interactivos (respuesta rápida, abrir enlace, copiar código, llamar, menú de opciones), y
  la opción elegida en `interactive_reply()`.
- Invitaciones a grupo, mensajes de producto, compartir o pedir el número, limitar el reenvío y etiquetas de miembro.
- Estados (historias) para una lista de contactos.
- Recibos de leído y de reproducido.

### Multimedia

- Imagen, video, GIF, video circular, audio, nota de voz, documento, sticker y álbumes.
- Dimensiones y miniaturas automáticas, y foto de perfil recortada y redimensionada sola.
- Archivos de cualquier tamaño con memoria constante: se cifran y descifran por bloques mientras se leen o escriben.
- Descarga verificada (HMAC + SHA-256) antes de que el archivo aparezca en disco, y re-subida automática de multimedia
  caducada.
- Reutilización de lo que ya subiste: el mismo archivo sale sin volver a cifrarse ni subirse.

### Chats y cuenta

- App state en vivo y modificable: archivar, fijar, silenciar, marcar como leído, borrar, vaciar, destacar mensajes,
  contactos, etiquetas (de chats y de mensajes), respuestas rápidas, nombre visible y vistas previas.
- Historial al vincular y bajo demanda; si un mensaje no se puede descifrar, se pide un reintento al remitente y una
  copia a tu teléfono a la vez.
- Store de chats, contactos y mensajes con reacciones, votos, ediciones, borrados, estado de entrega y búsqueda.
- Perfil: foto, info, bloqueos, todos los ajustes de privacidad, mensajes temporales, bots de Meta AI, límite de
  contacto nuevo y cupo de mensajes.
- Eventos tipados cuando alguien cambia su foto o su info, y cuando cambian tu lista de bloqueados o tu privacidad.

### Grupos, comunidades, canales y empresa

- Grupos: crear, participantes, admins, asunto, descripción, ajustes, invitaciones, solicitudes de ingreso, foto,
  unirse y salir. La sender key se rota cuando alguien sale.
- Comunidades: crear, vincular y desvincular grupos, listar subgrupos, salir y aprobación de miembros.
- Canales: crear, consultar por JID o enlace, seguir, silenciar, editar, borrar, reaccionar, leer publicaciones,
  suscriptores y administradores.
- Empresa: catálogo, productos con imágenes, colecciones, pedidos, perfil (dirección, correo, sitios, horario) y foto de
  portada.
- Enlaces de llamada y rechazo de llamadas.

### Integración

- Almacenamiento en archivos, memoria, SQLite, PostgreSQL, Redis o MongoDB, o tu propia base de datos, también escrita en
  JavaScript. Varias cuentas pueden compartir la misma base.
- Módulos nativos para Node.js y Python, servidor WebSocket/HTTP/webhooks y cliente JS/TS.
- Eventos en JSON (`event.to_json()`) listos para webhooks, logs o colas.
- Migración de sesiones desde Baileys y zapo sin volver a vincular, con rollback a zapo.
- Un WhatsApp falso con el protocolo real para probar tu bot sin ninguna cuenta.

## Vista previa de enlaces

Si un texto trae un enlace, la vista previa se genera sola al enviarlo, como en la app oficial: título, descripción,
miniatura y, si la página tiene video, la marca de video. No hace falta llamar a nada distinto.

```rust,ignore
use std::time::Duration;
use soblendwa::{LinkPreview, LinkPreviewConfig, PreviewChoice, SendOptions};

client.send_text(&chat, "Mira esto: https://www.rust-lang.org").await?;

let sin_vista = SendOptions { link_preview: PreviewChoice::Off, ..Default::default() };
client.send_message_with(&chat, Message::text("https://www.rust-lang.org"), sin_vista).await?;

let propia = LinkPreview { title: Some("Mi título".into()), description: Some("Escrito a mano".into()), ..Default::default() };
let opciones = SendOptions { link_preview: PreviewChoice::Custom(Box::new(propia)), ..Default::default() };
client.send_message_with(&chat, Message::text("https://www.rust-lang.org"), opciones).await?;

let (client, events) = Client::builder()
    .link_previews(LinkPreviewConfig { timeout: Duration::from_secs(2), enabled: false, ..Default::default() })
    .build()
    .await?;
```

En JavaScript: `bot.sendText(chat, texto, { preview: false })`, `{ preview: true }` u `{ preview: { title, description } }`.
En Python: `await bot.send_text(chat, texto, preview=False)`. En el servidor se apaga con `--no-link-previews`.

- **La consulta sale de tu servidor.** Para generar la vista previa, el bot abre la página, como hace el teléfono en la
  app oficial. Si repite enlaces de desconocidos y no quieres eso, usa `enabled: false`.
- **Protegido contra enlaces maliciosos.** Nunca consulta hosts privados: `127.0.0.1`, la red interna, el servicio de
  metadatos de la nube (`169.254.169.254`), `localhost`, `*.local` ni `*.internal`. También revisa cada redirección y a
  qué IP resuelve el dominio, así un dominio público que apunte a tu red interna tampoco pasa. Para permitirlos:
  `allow_private_hosts: true`.
- **Dos miniaturas.** Una pequeña dentro del mensaje y otra nítida de hasta 1200 px, subida cifrada, para que se vea bien
  en el teléfono. Si la subida falla, el mensaje sale igual con la pequeña. `message.link_thumbnail()` permite descargarla.
- **Reutiliza lo que ya hizo.** Un enlace repetido no vuelve a consultar la página (se recuerda 1 hora) ni a subir la
  miniatura.
- **Nunca frena un envío.** Tiene 3 s por petición por defecto. Si la página falla o no responde, sale una tarjeta mínima con
  el dominio.
- **Imágenes hostiles.** La imagen se decodifica con límites de tamaño y memoria.
- **Extensible.** Implementa `LinkPreviewFetcher` para traer los datos de tu propia fuente.

Los mensajes recibidos traen en JSON `linkPreview` con la URL, el título, la descripción y si tiene miniatura.

## Guía de Rust

Cada tema se abre al hacer clic. Los ejemplos son para pegar dentro de tu bot, después de crear el `client`.

<details>
<summary><strong>Ejemplos listos para correr</strong></summary>

| Comando | Qué hace |
|---|---|
| `cargo run --release --example bot` | Bot de pruebas: dibuja el QR en la terminal y responde unos 30 comandos, uno por función. Escribe `!ayuda` desde otro número |
| `cargo run --release --example pair_code -- 5215512345678` | Vincula con código de 8 caracteres en lugar de QR |
| `cargo run --release --example import_baileys -- ./auth_info ./session` | Trae una sesión de Baileys sin volver a vincular |
| `cargo run --release --example import_zapo --features sqlite -- ./auth/state.sqlite bot ./session` | Trae una sesión de zapo sin volver a vincular |
| `cargo run --release --example canary -- ./sesion-prueba-a ./sesion-prueba-b` | Prueba diaria entre dos cuentas de prueba, con alerta si algo falla |
| `cargo run --release --example echo_bot -- <url> <raíz hex>` | Bot de eco en memoria para servidores falsos, como el de las comparaciones |
| `node clients/js/example.mjs` | Bot en JavaScript que usa el servidor (`!ping` y `!menu`) |

El bot de pruebas guarda la sesión en `./session` (o en la carpeta que le pases: `--example bot -- mi-carpeta`) y el
historial en `chats.json` dentro de ella. Si un comando falla, el error te llega como respuesta en el mismo chat. `!ping`
responde solo `Pong` y el tiempo que tardó el bot. `!prueba` revisa de una vez la latencia, los tiempos por fase, la
reutilización de multimedia y el outbox; `!stats` muestra las métricas, `!quien` el número y el LID de quien escribe, y
`!outbox` lo que sigue pendiente.

</details>

<details>
<summary><strong>Enviar mensajes</strong></summary>

```rust,ignore
use soblendwa::{Jid, Message, MediaKind, MediaOptions};

let chat = Jid::phone("5215512345678");

client.send_text(&chat, "Hola desde SoblendWA").await?;
client.reply(&msg, "respondiendo a tu mensaje").await?;
client.react(&msg, "🔥").await?;

let usuario = Jid::phone("5215511111111");
client.send_message(&chat, Message::text("hola @5215511111111").mention([&usuario])).await?;
client.send_message(&grupo, Message::text("Atención a todos").mention_all()).await?;

let encuesta = client.send_message(&chat, Message::poll("¿Pizza o tacos?", ["Pizza", "Tacos"], 1)).await?;
client.vote_poll(&chat, &encuesta, &["Tacos"]).await?;
client.send_message(&chat, Message::poll("¿Qué te gusta?", ["Rust", "Go", "Zig"], 0)).await?;

client.send_message(&chat, Message::location(19.4326, -99.1332)).await?;
client.send_message(&chat, Message::contacts("Ana", [("Ana", Message::vcard("Ana", "+52 1 55 1111 1111"))])).await?;
client.send_group_invite(&chat, &grupo, Some("Únete al equipo")).await?;

let id = client.send_text(&chat, "versión 1").await?;
client.edit_message(&chat, &id, "versión 2").await?;

client.send_image(&chat, &std::fs::read("foto.jpg")?, Some("mi foto")).await?;
client.send_audio(&chat, &std::fs::read("nota.ogg")?, true).await?;
client.send_document(&chat, &std::fs::read("reporte.pdf")?, "reporte.pdf", "application/pdf").await?;

let options = MediaOptions { caption: Some("una sola vez".into()), view_once: true, ..Default::default() };
let mensaje = client.prepare_media(&std::fs::read("foto.jpg")?, MediaKind::Image, options).await?;
client.send_message(&chat, mensaje.reply_to(&msg)).await?;

let video = client.prepare_media(&std::fs::read("clip.mp4")?, MediaKind::Video, MediaOptions::default()).await?;
client.send_message(&chat, Message::video_note(video)).await?;
```

Una encuesta con `1` opción elegible usa el formato de opción única; con `0` o más de una, el de varias opciones, igual
que la app oficial. `Message::announcement_poll` es para grupos de anuncios de comunidades.

</details>

<details>
<summary><strong>Recibir y descargar multimedia, también archivos grandes</strong></summary>

```rust,ignore
if let Event::Message(msg) = event {
    if msg.message.media().is_some() {
        let bytes = client.download_message(&msg.message).await?;
        std::fs::write(format!("{}.bin", msg.info.id), bytes)?;
    }
}
```

Para archivos grandes:

```rust,ignore
use soblendwa::{MediaKind, MediaOptions};

client.send_media_file(&chat, "pelicula.mp4", MediaKind::Video, MediaOptions::default()).await?;
client.send_media_file(&chat, "reporte.pdf", MediaKind::Document, MediaOptions::default()).await?;

let bytes = client.download_event_to_file(&msg, "descargas/video.mp4").await?;
```

`send_media_file` lee el archivo dos veces por bloques de 256 KB: una para calcular los hashes que WhatsApp pide
antes de subir y otra para cifrar mientras sube. No crea archivos temporales y la memoria no depende del tamaño. Para
fuentes que solo se pueden leer una vez (un socket, otra descarga) usa `upload_reader`, que cifra hacia un archivo
temporal y lo borra al terminar. El tipo MIME y el nombre del documento se deducen de la extensión.

`download_to_file` escribe en `archivo.part` y solo lo renombra cuando el HMAC y los dos SHA-256 coinciden: un archivo
corrupto o alterado nunca aparece con su nombre final. Si el archivo ya caducó en el servidor,
`download_event_to_file` le pide al remitente que lo vuelva a subir y lo descarga de nuevo.

</details>

<details>
<summary><strong>Botones, listas y mensajes interactivos</strong></summary>

```rust,ignore
use soblendwa::interactive::{Interactive, Section};

let menu = Interactive::new("¿Qué quieres hacer?")
    .title("Soblend")
    .footer("SoblendWA")
    .quick_reply("ping", "Medir latencia")
    .url("Documentación", "https://soblend.dev")
    .copy("Copiar cupón", "SOBLEND-2026")
    .select("Ver comandos", vec![Section::new("Comandos").described_row("menu", "Menú", "Todos los comandos")]);
client.send_message(&chat, menu.build()).await?;

client.send_message(&chat, Message::buttons("Elige", Some("pie"), [("si", "Sí"), ("no", "No")])).await?;
client.send_message(&chat, Message::list("Menú", "Elige un plato", "Ver menú", None, &[
    Section::new("Tacos").row("pastor", "Al pastor").row("suadero", "Suadero"),
])).await?;

if let Some(reply) = msg.message.interactive_reply() {
    println!("eligió {} ({:?})", reply.id, reply.source);
}
```

`Interactive`, `Message::buttons` y `Message::list` usan el formato interactivo (*native flow*), el que WhatsApp sí
muestra; se probó en vivo con un teléfono. `legacy_buttons` y `legacy_list` generan los formatos viejos
(`buttonsMessage` y `listMessage`), que WhatsApp ya no muestra en cuentas normales; quedan solo por compatibilidad.

</details>

<details>
<summary><strong>Chats, contactos y etiquetas (app state)</strong></summary>

```rust,ignore
use std::time::Duration;
use soblendwa::events::ChatUpdate;

client.archive_chat(&chat, true).await?;
client.pin_chat(&chat, true).await?;
client.mute_chat(&chat, Some(Duration::from_secs(8 * 60 * 60))).await?;
client.mark_chat_read(&chat, true).await?;
client.star_message(&chat, &msg.info.id, false, true).await?;
client.save_contact(&Jid::phone("5215511111111"), "Ana Soblend").await?;

client.edit_label("1", "Clientes VIP", 3).await?;
client.label_chat(&chat, "1", true).await?;
client.label_message(&chat, &msg.info.id, "1", true).await?;
client.save_quick_reply(None, "hola", "¡Hola! ¿En qué te ayudo?").await?;

if let Event::ChatUpdate { update: ChatUpdate::Archived { chat, archived }, .. } = event {
    println!("{chat} archivado: {archived}");
}
```

Los cambios que hagas desde el teléfono llegan como `Event::ChatUpdate`. Cada parche se verifica con LTHash y MACs
antes de aplicarse; si falta una clave, se le pide al teléfono y la sincronización continúa sola cuando llega.

</details>

<details>
<summary><strong>Store de chats, contactos y mensajes</strong></summary>

```rust,ignore
use soblendwa::ChatStore;

let chats = ChatStore::open("session/chats.json").await?;
let (client, events) = Client::builder().store(store).chat_store(chats.clone()).build().await?;

for chat in chats.chats() {
    println!("{} · {} sin leer", chats.name_of(&chat.jid).unwrap_or(chat.jid.to_string()), chat.unread);
}
let ultimos = chats.messages(&chat, 20);
let encontrados = chats.search("factura", 10);
let viejo = chats.message(&chat, "3EB0ABC...").unwrap();
client.reply(&viejo.to_event(), "respondiendo a un mensaje de ayer").await?;
```

Se alimenta solo: mensajes entrantes y enviados, historial, recibos (enviado, entregado, leído, reproducido),
reacciones, votos de encuestas, ediciones, borrados, cambios de app state, nombres de contactos y asuntos de grupos.
Guarda hasta 500 mensajes por chat (configurable con `ChatStore::open_with_limit`) y se escribe a disco cada 10
segundos si hubo cambios, y al llamar `client.disconnect()`. La escritura es atómica: un corte de luz no deja el
archivo a medias.

</details>

<details>
<summary><strong>Grupos</strong></summary>

```rust,ignore
use soblendwa::ParticipantAction;

let info = client.group_info(&chat_del_grupo).await?;
let actualizado = client.group_metadata(&chat_del_grupo).await?;

let grupo = client.create_group("Equipo Soblend", &[Jid::phone("5215511111111")]).await?;
client.update_participants(&grupo.jid, &[Jid::phone("5215522222222")], ParticipantAction::Promote).await?;
client.set_group_description(&grupo.jid, Some("Hecho con SoblendWA")).await?;
let enlace = client.group_invite_code(&grupo.jid).await?;
println!("https://chat.whatsapp.com/{enlace}");
```

`group_info` responde desde la caché al instante. `group_metadata` siempre consulta al servidor.

Más funciones de grupo: `group_join_requests`, `respond_join_requests`, `set_group_join_approval`,
`set_group_member_add_mode`, `group_invite_info`, `accept_group_invite`, `set_group_picture` y `set_member_label`.

</details>

<details>
<summary><strong>Perfil, privacidad y estados</strong></summary>

```rust,ignore
use soblendwa::{PrivacySetting, PrivacyValue};

let foto = client.profile_picture_url(&chat, true).await?;
client.set_profile_picture(None, std::fs::read("yo.jpg")?).await?;
client.set_about("Hecho con SoblendWA").await?;
let infos = client.fetch_about(&[chat.clone()]).await?;

client.block(&chat).await?;
let bloqueados = client.blocklist().await?;

client.set_privacy(PrivacySetting::LastSeen, PrivacyValue::Contacts).await?;
let ajustes = client.privacy_settings().await?;

client.set_disappearing(&chat, 7 * 24 * 60 * 60).await?;
let temporales = client.disappearing_modes(&[chat.clone()]).await?;
client.pin_message(&chat, msg.key(), 7 * 24 * 60 * 60).await?;
client.send_status(Message::text("Mi primer estado"), &[Jid::phone("5215511111111")]).await?;

let negocio = client.business_profile(&chat).await?;
let limite = client.reachout_timelock().await?;
let bots = client.bot_list().await?;
```

La foto de perfil puede ser JPEG, PNG, WebP o GIF de cualquier tamaño: se recorta al centro y se deja en 640×640.

</details>

<details>
<summary><strong>Eventos de calendario y llamadas</strong></summary>

```rust,ignore
use soblendwa::events::EventResponseKind;

let evento = Message::event("Lanzamiento SoblendWA", 1_800_000_000)
    .event_details(Some("En vivo"), Some(1_800_003_600), Some((19.4326, -99.1332, "CDMX")), None, true);
let id = client.send_message(&chat, evento).await?;
client.respond_event(&chat, &id, EventResponseKind::Going, 0).await?;

let enlace = client.create_call_link(true, None).await?;
```

</details>

<details>
<summary><strong>Canales y comunidades</strong></summary>

```rust,ignore
let canal = client.newsletter_create("Soblend News", Some("Novedades del estudio")).await?;
let otro = client.newsletter_info_by_invite("https://whatsapp.com/channel/0029Va...").await?;
client.newsletter_follow(&otro.jid).await?;
for publicacion in client.newsletter_messages(&otro.jid, 20, None).await? {
    client.newsletter_react(&otro.jid, &publicacion.server_id, Some("🔥")).await?;
}

let comunidad = client.create_community("Soblend", "Comunidad oficial").await?;
let grupo = client.create_community_group(&comunidad.jid, "Anuncios técnicos", &[]).await?;
let subgrupos = client.community_groups(&comunidad.jid).await?;
```

Los mensajes de canal que recibes traen `msg.info.server_id`, que es lo que usa `newsletter_react`.

</details>

<details>
<summary><strong>Catálogo de empresa</strong></summary>

```rust,ignore
use soblendwa::business::{BusinessHours, BusinessProfileUpdate, CatalogImage, OpeningHours, ProductDraft, Weekday};

let pagina = client.catalog(None, 10, None).await?;
let producto = client.product_create(&ProductDraft {
    name: Some("Taza Soblend".into()),
    price: Some(149_000),
    currency: Some("MXN".into()),
    images: vec![CatalogImage::Data(std::fs::read("taza.jpg")?)],
    ..Default::default()
}).await?;
client.send_product(&chat, &mi_numero, &producto, &std::fs::read("taza.jpg")?, Some("¡Nuevo!")).await?;
client.product_delete(&[&producto.id]).await?;

client.update_business_profile(&BusinessProfileUpdate {
    email: Some("ventas@soblend.dev".into()),
    hours: Some(BusinessHours {
        timezone: "America/Mexico_City".into(),
        days: vec![(Weekday::Mon, OpeningHours::Between { open_minute: 9 * 60, close_minute: 18 * 60 })],
    }),
    ..Default::default()
}).await?;
```

Los precios van en milésimas, como en la app: `149_000` son 149,00.

</details>

<details>
<summary><strong>Proxy</strong></summary>

```rust,ignore
use soblendwa::Proxy;

let (client, events) = Client::builder()
    .store(store)
    .proxy(Proxy::parse("socks5://usuario:clave@127.0.0.1:1080")?)
    .build()
    .await?;
```

Acepta `http://`, `socks5://` y `socks5h://`, con usuario y contraseña opcionales. Pasan por el proxy el WebSocket, las
subidas y descargas de multimedia, las vistas previas de enlaces y la consulta de la versión de WhatsApp Web.

</details>

<details>
<summary><strong>Latencia</strong></summary>

```rust,ignore
let bot = msg.info.elapsed();
let red = client.latency();
let medido_ahora = client.ping().await?;
```

- `msg.info.elapsed()` mide cuánto lleva el mensaje dentro del bot, desde que llegó del socket. Incluye descifrarlo.
- `client.latency()` devuelve al instante la latencia de red medida en segundo plano (handshake y keepalive).
- `client.ping()` hace una medición nueva; tarda un viaje de ida y vuelta.

La primera vez que el bot le escribe a un chat nuevo, tiene que pedirle al servidor los dispositivos del destinatario y sus
claves: uno o dos viajes de ida y vuelta, igual que WhatsApp Web. Después, todo queda guardado y los envíos solo cuestan
el cifrado local, que tarda microsegundos.

El tiempo que ves en el teléfono es: viaje de tu teléfono a WhatsApp + **~0,05 ms del bot** + viaje de WhatsApp al bot
y de vuelta. Esos viajes dependen de la distancia física y de tu conexión. Ningún código puede bajarlos de lo que
permita la red.

</details>

<details>
<summary><strong>Lista de eventos</strong></summary>

| Evento | Cuándo ocurre |
|---|---|
| `Qr(String)` | Hay un QR nuevo para escanear (el primero dura 60 s, los siguientes 20 s) |
| `PairSuccess { .. }` | El teléfono aprobó la vinculación |
| `Connected` | La sesión está lista para enviar y recibir |
| `Disconnected { reason, reconnecting }` | Se perdió la conexión |
| `LoggedOut { reason }` | Se cerró la sesión; los datos se borran y el siguiente `connect()` muestra un QR nuevo |
| `Message(Box<MessageEvent>)` | Llegó un mensaje ya descifrado |
| `UndecryptableMessage { .. }` | No se pudo descifrar; ya se pidió un reintento automáticamente |
| `MessageRecovered { .. }` | Un mensaje que no se pudo descifrar llegó bien tras el reintento |
| `AccountBlocked { code, reason, expires }` | WhatsApp bloqueó la cuenta; el cliente deja de reintentar y conserva la sesión |
| `PictureChanged { .. }` | Alguien cambió o quitó su foto de perfil |
| `AboutChanged { .. }` | Alguien cambió su info |
| `BlocklistChanged { items }` | Cambió tu lista de bloqueados |
| `PrivacyChanged { settings }` | Cambió un ajuste de privacidad |
| `Receipt(Receipt)` | Entregado, leído, reproducido o reintento |
| `Presence` / `ChatState` | En línea o desconectado, "escribiendo…" o "grabando audio…" |
| `GroupUpdate(GroupUpdate)` | Altas, bajas, admins, asunto, descripción y ajustes del grupo |
| `Call(CallEvent)` | Llamada entrante, aceptada, rechazada o terminada |
| `PollVote(PollVote)` | Alguien votó en una encuesta; trae las opciones elegidas por nombre |
| `EventResponse(EventResponse)` | Alguien respondió a un evento: va, no va o tal vez, y cuántos invitados lleva |
| `ChatUpdate { update, .. }` | Cambió un chat o contacto: archivado, fijado, silenciado, leído, borrado, destacado, etiquetas… |
| `ServerAck(ServerAck)` | WhatsApp confirmó o rechazó un mensaje enviado; trae la latencia real de esa confirmación |
| `HistorySync(Box<HistorySync>)` | Llegó un bloque del historial: chats, mensajes y nombres de contactos |
| `OfflineSyncComplete { count }` | Ya se entregaron los mensajes que llegaron mientras estabas desconectado |
| `Notification(Node)` / `Unhandled(Node)` | Stanzas crudos para casos avanzados |

En Node.js, Python y el servidor llegan los mismos eventos como JSON, con `type` en camelCase (`message`, `pollVote`,
`chatUpdate`…).

</details>

<details>
<summary><strong>Configuración del cliente</strong></summary>

```rust,ignore
use std::time::Duration;
use soblendwa::{Client, Platform, SendLimit};

let (client, events) = Client::builder()
    .store(store)
    .device_name("SoblendWA")
    .platform(Platform::Desktop)
    .push_name("Mi Bot")
    .online_on_connect(false)
    .auto_reconnect(true)
    .query_timeout(Duration::from_secs(30))
    .send_limit(Some(SendLimit { per_minute: 120, burst: 30, per_chat_per_minute: 20, per_chat_burst: 8 }))
    .outbox(true)
    .media_cache(Some(Duration::from_secs(7 * 24 * 60 * 60)))
    .workers(16)
    .build()
    .await?;
```

`device_name` es el nombre que aparece en **Dispositivos vinculados** del teléfono. Por defecto es `SoblendWA`.

</details>

## Almacenamiento

| Store | Feature | Para qué |
|---|---|---|
| `FileStore` | incluido | Un bot en una máquina. Log de solo-escritura con compactación |
| `MemoryStore` | incluido | Pruebas |
| `SqliteStore` | `sqlite` | Un archivo, varias cuentas, sin servidor. SQLite va incluido en el binario |
| `PostgresStore` | `postgres` | Muchos bots o servidores compartiendo una base |
| `RedisStore` | `redis` | Despliegues efímeros (contenedores) con estado en Redis |
| `MongoStore` | `mongo` | Bots que ya usan MongoDB. **Experimental:** falta probarlo contra un servidor real |
| El tuyo en JavaScript | módulo de Node.js | Cualquier base de datos, escribiendo el store en JS |

```rust,ignore
use std::sync::Arc;
use soblendwa::store::{MongoStore, PostgresStore, RedisStore, SqliteStore};

let store = SqliteStore::open_account("datos/bots.db", "ventas").await?;
let store = PostgresStore::connect_account("postgres://bot:clave@localhost/wa", "ventas").await?;
let store = RedisStore::connect_account("redis://127.0.0.1/", "ventas").await?;
let store = MongoStore::connect_account("mongodb://localhost:27017", "wa", "ventas").await?;

let (client, events) = Client::builder().store(Arc::new(store)).build().await?;
```

Para el servidor: `soblendwa-server --mongo mongodb://localhost:27017 --mongo-db wa --session ventas`. Para el módulo
de Node.js: `{ store: { type: 'mongo', url, database, account } }`. Si tienes un MongoDB donde puedas escribir, la prueba
completa se corre con `SOBLEND_MONGO_URL=mongodb://... cargo test --features "sqlite mongo" --lib mongo -- --ignored`.

**Stores escritos en JavaScript.** Con el módulo de Node.js pasas un objeto con tus funciones y usas la base de datos que
quieras. Las lecturas ya pasan por caché y las escrituras llegan agrupadas, así que no hace falta optimizar nada:

```js
const store = {
  loadCreds: () => db.creds.findOne(),
  saveCreds: (json) => db.creds.save(json),
  get: (kind, key) => db.keys.get(`${kind}:${key}`),
  keys: (kind) => db.keys.list(kind),
  apply: (writes) => db.keys.bulk(writes),
}
const bot = await SoblendWA.create({ store })
```

`apply` recibe una lista de `{ op: 'set' | 'remove' | 'clear', kind, key, value }`. Si prefieres, en lugar de `apply` puedes
dar `set`, `remove` y `clear`. Probado de punta a punta: vinculación, mensajes y reinicio con el mismo store.

Todos los backends de Rust comparten el mismo núcleo, `BatchedStore`: las lecturas salen de memoria después de la primera vez, y las
escrituras se agrupan en una sola transacción (o un solo pipeline en Redis) en segundo plano. Si una clave cambia mil
veces antes de escribirse, solo se escribe el último valor. Enviar un mensaje nunca espera a la base de datos.
`client.disconnect()` vacía lo pendiente antes de salir.

Para otra base de datos implementa `Backend` (cuatro métodos, y `list` si quieres exportar a zapo) y envuélvelo en
`BatchedStore::new(tu_backend)`: la caché y el agrupado vienen gratis. Si prefieres controlarlo todo, implementa
directamente el trait `Store`.

```rust,ignore
#[async_trait::async_trait]
impl soblendwa::Backend for MiBaseDeDatos {
    async fn load_creds(&self) -> Result<Option<Creds>>;
    async fn save_creds(&self, creds: &Creds) -> Result<()>;
    async fn fetch(&self, kind: KeyKind, key: &str) -> Result<Option<Vec<u8>>>;
    async fn apply(&self, writes: &[KeyWrite]) -> Result<()>;
}
```

## Servidor: cualquier lenguaje

`soblendwa-server` corre el motor en Rust y expone WhatsApp por WebSocket, HTTP y webhooks. Tu bot puede estar en
JavaScript, TypeScript, Go, PHP, Java, C#… o en otra máquina. Un solo servidor maneja varias cuentas a la vez.

```text
soblendwa-server --token mi-clave
cargo run --release -p soblendwa-server -- --token mi-clave
```

La primera línea usa el [binario ya compilado](#sin-compilar-nada) y la segunda lo compila desde este repositorio. La
primera vez muestra el QR en la terminal. Para reutilizar una sesión que ya vinculaste con el bot de ejemplo, usa
`--dir . --session session`.

| Opción | Para qué |
|---|---|
| `--host 127.0.0.1` `--port 8787` | Dónde escucha. Por defecto solo acepta conexiones locales |
| `--token` | Clave obligatoria para WebSocket, HTTP y webhooks (`Authorization: Bearer …` o `?token=`) |
| `--dir sessions` / `--sqlite bots.db` / `--memory` | Dónde se guardan las sesiones: una carpeta por cuenta, una base SQLite o memoria |
| `--mongo <url>` `--mongo-db wa` | Guarda las sesiones en MongoDB (experimental) |
| `--session ventas --session soporte` | Cuentas que se abren al arrancar (las demás se abren al primer uso) |
| `--webhook https://…` | Envía cada evento por POST a tu URL |
| `--proxy`, `--push-name` | Proxy HTTP/SOCKS5 y nombre visible |
| `--no-chat-store`, `--no-link-previews` | Desactivan el store de chats y las vistas previas de enlaces |
| `--fake` | Usa el WhatsApp falso: nada sale de tu máquina y cada sesión recibe un número de prueba |

Las opciones con valor, salvo `--session`, también se pueden pasar como variables de entorno: `SOBLENDWA_HOST`,
`SOBLENDWA_PORT`, `SOBLENDWA_TOKEN`, `SOBLENDWA_DIR`, `SOBLENDWA_SQLITE`, `SOBLENDWA_MONGO`, `SOBLENDWA_MONGO_DB`,
`SOBLENDWA_WEBHOOK`, `SOBLENDWA_PROXY` y `SOBLENDWA_PUSH_NAME`.

**WebSocket** `ws://host:8787/ws/<cuenta>?token=…`: al conectar llega
`{"type":"hello","session":…,"me":…,"connected":…,"methods":[…]}`. Después llegan los eventos como
`{"session":"ventas","event":{"type":"message",…}}`. Para llamar una función envía
`{"id":1,"method":"sendText","params":{"chat":"5215512345678","text":"hola"}}` y recibes `{"id":1,"result":"3EB0…"}`
o `{"id":1,"error":{"message":"…"}}`. Las llamadas se atienden en paralelo.

**HTTP** para lenguajes sin WebSocket o scripts:

```bash
curl -X POST http://127.0.0.1:8787/api/ventas/sendText \
  -H "Authorization: Bearer mi-clave" \
  -d '{"chat":"5215512345678","text":"Hola desde curl"}'
```

`GET /` lista los métodos disponibles, `GET /sessions` las cuentas abiertas y `GET /api/<cuenta>` el estado (con el
QR pendiente, si lo hay). Los bytes de multimedia viajan en base64 (`data`, `proto`); para archivos grandes usa
`sendFile` y `downloadToFile` con rutas del servidor.

**Cliente JavaScript y TypeScript** listo en
[`clients/js`](https://github.com/Soblend-Development-Studio/SoblendWA/tree/main/clients/js): sin dependencias, con tipos y
reconexión automática. Funciona en Node, Bun, Deno y el navegador.

## Node.js y Python

El mismo motor en Rust, compilado como módulo nativo dentro del proceso de tu bot, sin servidor aparte. Para compilarlos
desde este repositorio, en lugar de instalarlos:

```text
cd bindings/node && npm install && npm run build
cd bindings/python && pip install maturin && maturin develop --release
```

Los dos tienen la misma API que el cliente JS del servidor: texto, respuestas, reacciones, multimedia (también archivos
grandes con `sendFile`/`send_file` y `downloadToFile`/`download_to_file`), botones, listas e interactivos, encuestas,
contactos, `@todos`, invitaciones, editar, eliminar, presencia, grupos, chats y etiquetas (`modifyChat`, `labelChat`,
`saveQuickReply`), store de chats (`chats`, `messages`, `search`), datos de la cuenta (`botList`,
`reachoutTimelock`, `disappearingModes`), métricas (`stats`, `pendingSends`/`pending_sends`), proxy, migración desde
Baileys (`importBaileys`) y `sendProto` para cualquier mensaje protobuf. Los eventos llegan como objetos JSON con un campo
`type`; los mensajes incluyen el protobuf original en base64 (`proto`) y, si son la respuesta a un botón o lista, la
opción elegida en `reply`. Más detalle en
[`bindings/node`](https://github.com/Soblend-Development-Studio/SoblendWA/blob/main/bindings/node/README.md) y
[`bindings/python`](https://github.com/Soblend-Development-Studio/SoblendWA/blob/main/bindings/python/README.md).

## Migrar desde otra librería

### Desde zapo

Si tu bot usa zapo con `@zapo-js/store-sqlite`, pásalo sin volver a vincular:

```text
cargo run --release --example import_zapo --features sqlite -- ./auth/state.sqlite bot ./session
```

Convierte las credenciales, las sesiones Signal (incluidas las claves de mensajes que llegaron fuera de orden), las
pre-keys, las sender keys de grupos, las claves y versiones de app state y los pares número ↔ LID de los contactos. La
base de zapo se abre **solo para leer**: no se modifica.

**Volver a zapo (rollback).** Si algo no te convence, escribe el estado actual de SoblendWA de vuelta en la base de zapo
y sigue con tu bot de antes, con los ratchets al día:

```text
cargo run --release --example import_zapo --features sqlite -- --rollback ./session ./auth/state.sqlite bot
```

Probado contra zapo-js 1.11 real, en los dos sentidos: zapo crea la base, SoblendWA la importa, descifra los mensajes
pendientes (uno que llegó fuera de orden y uno de una cuenta LID) y responde; los contactos de zapo descifran esas
respuestas; tras el rollback, zapo sigue mandando y recibiendo mensajes privados y de grupo. Desde código:
`soblendwa::store::import_zapo_sqlite(...)` y `export_zapo_sqlite(...)`.

**Si tu bot usa `@zapo-js/store-mongo`:**

```text
cargo run --release --example import_zapo --features "sqlite mongo" -- --mongo mongodb://localhost:27017 mi_base bot ./session
cargo run --release --example import_zapo --features "sqlite mongo" -- --rollback-mongo ./session mongodb://localhost:27017 mi_base bot
```

**Experimental:** la conversión se probó con documentos que siguen el esquema exacto de `@zapo-js/store-mongo`, y descifra
mensajes pendientes reales de zapo, pero todavía no contra un servidor MongoDB. Haz una copia de la base antes del
rollback y corre primero la prueba de [Almacenamiento](#almacenamiento).

### Desde Baileys

```text
cargo run --release --example import_baileys -- ./auth_info ./session
```

O desde tu código:

```rust,ignore
let store = FileStore::open("session").await?;
let reporte = soblendwa::store::import_baileys("auth_info", store.as_ref()).await?;
println!("{} sesiones, {} claves de grupo", reporte.sessions, reporte.sender_keys);
```

Convierte las credenciales, las sesiones Signal de libsignal-node, las pre-keys, las sender keys de grupos, las claves
y versiones de app state y los mapeos número ↔ LID. Las listas de dispositivos y los tokens se vuelven a pedir solos.
El bot sigue vinculado: no hace falta escanear otro QR y los chats en curso siguen cifrándose con las mismas
sesiones. Detén el bot de Baileys antes de arrancar SoblendWA; los dos no pueden usar la misma sesión a la vez.

<details>
<summary><strong>Equivalencias con Baileys: cada función pública de Baileys 7 y su equivalente</strong></summary>

| Baileys | SoblendWA |
|---|---|
| `useMultiFileAuthState` | `FileStore` (y `import_baileys` para traer tu sesión) |
| `makeInMemoryStore` | `ChatStore` |
| `sendMessage({ text })`, `{ image }`, `{ video }`… | `send_text`, `send_image`, `send_media_file`, `prepare_media` + `send_message` |
| `{ ptv: true }` | `Message::video_note` |
| `{ contacts }`, `{ location }` | `Message::contacts`, `Message::vcard`, `Message::location` |
| `{ react }`, `{ delete }`, `{ edit }`, `{ forward }` | `react`, `revoke_message`, `edit_message`, `.forwarded()` |
| `{ poll }`, `{ event }`, `{ album }`, `{ pin }` | `Message::poll` / `announcement_poll`, `Message::event(..).event_details(..)`, `send_album`, `pin_message` |
| `{ groupInvite }`, `{ product }` | `send_group_invite`, `send_product` |
| `{ buttonReply }`, `{ listReply }` | `Message::button_reply`, `template_reply`, `list_reply` |
| `{ sharePhoneNumber }`, `{ requestPhoneNumber }`, `{ limitSharing }` | `Message::share_phone_number`, `request_phone_number`, `limit_sharing` |
| `mentions`, `mentionAll` | `.mention(..)`, `.mention_all()` |
| `readMessages`, `sendReceipt(s)` | `read_message`, `mark_read`, `mark_played`, `send_receipt` |
| `chatModify`, `star`, `addOrEditContact`, `removeContact` | `archive_chat`, `pin_chat`, `mute_chat`, `mark_chat_read`, `delete_chat`, `clear_chat`, `star_message`, `save_contact`, `chat_modify` |
| `addLabel`, `addChatLabel`, `addMessageLabel` (y sus `remove`) | `edit_label`, `delete_label`, `label_chat`, `label_message` |
| `addOrEditQuickReply` | `save_quick_reply`, `delete_quick_reply` |
| `update*Privacy`, `updateDisableLinkPreviewsPrivacy` | `set_privacy`, `set_link_previews_disabled` |
| `updateDefaultDisappearingMode`, `disappearingMessagesInChat`, `fetchDisappearingDuration` | `set_default_disappearing`, `set_disappearing`, `disappearing_modes` |
| `fetchStatus`, `updateProfileStatus`, `updateProfileName` | `fetch_about`, `set_about`, `set_push_name` |
| `profilePictureUrl`, `updateProfilePicture`, `removeProfilePicture` | `profile_picture_url`, `set_profile_picture`, `remove_profile_picture` |
| `updateBlockStatus`, `fetchBlocklist`, `fetchPrivacySettings` | `block`, `unblock`, `blocklist`, `privacy_settings` |
| `onWhatsApp`, `getBusinessProfile` | `on_whatsapp`, `business_profile` |
| `group*` | `group_info`, `group_metadata`, `create_group`, `update_participants`, `set_group_*`, `group_invite_code`, `join_group`, `accept_group_invite`, `group_join_requests`… |
| `community*` | `create_community`, `create_community_group`, `link_group`, `unlink_group`, `community_groups`, `leave_community`, y las funciones de grupo |
| `newsletter*`, `subscribeNewsletterUpdates` | `newsletter_*`, `newsletter_live_updates` |
| `getCatalog`, `getCollections`, `getOrderDetails`, `product*`, `updateBussinesProfile`, `updateCoverPhoto` | `catalog`, `catalog_collections`, `order_details`, `product_*`, `update_business_profile`, `set_business_cover_photo` |
| `updateMemberLabel`, `getBotListV2` | `set_member_label`, `bot_list` |
| `fetchAccountReachoutTimelock`, `fetchNewChatMessageCap` | `reachout_timelock`, `new_chat_message_cap` |
| `fetchMessageHistory`, `requestPlaceholderResend`, `updateMediaMessage` | `request_history`, `request_placeholder_resend`, `reupload_media` |
| `rejectCall`, `createCallLink`, `sendPresenceUpdate`, `presenceSubscribe` | `reject_call`, `create_call_link`, `send_chat_presence` / `set_presence`, `subscribe_presence` |
| `requestPairingCode`, `logout`, `end`, `cleanDirtyBits`, `rotateSignedPreKey` | `request_pair_code`, `logout`, `disconnect`, `clean_dirty_bits`, `rotate_signed_pre_key` |
| `relayMessage`, `sendNode`, `query` | `send_message_with`, `send_node`, `query` |

Lo que SoblendWA agrega encima: SQLite, PostgreSQL, Redis y MongoDB integrados; el mismo motor en Rust, Node.js, Python y
por WebSocket para cualquier lenguaje; importar sesiones de Baileys; y la velocidad de la sección
[Rendimiento](#rendimiento), medida contra el código de Baileys y de whatsmeow.

</details>

## Arquitectura

<details>
<summary><strong>Cómo está organizado el código</strong></summary>

```text
src/
├── binary/          códec de nodos binarios de WhatsApp, diccionario de tokens y pruebas de robustez
├── crypto/          X25519, firmas XEdDSA, AES-GCM/CBC/CTR, HKDF, PBKDF2
├── noise.rs         handshake Noise XX, verificación de certificados y framing
├── proto/           mensajes protobuf de WhatsApp y de Signal
├── signal/          sesiones (X3DH + Double Ratchet) y Sender Keys
├── store/           credenciales, FileStore, MemoryStore, BatchedStore, SQLite/PostgreSQL/Redis/MongoDB, Baileys y zapo
├── chats.rs         store de chats, contactos y mensajes
├── interactive.rs   botones, listas, plantillas y mensajes interactivos
├── json.rs          eventos, mensajes normalizados, métricas y nodos a JSON
├── testing.rs       WhatsApp falso en memoria con el protocolo real (feature testing)
└── client/          conexión, vinculación, envío, recepción, app state, grupos, comunidades, canales, catálogo,
                     cuenta, multimedia (también en streaming y reutilizada), proxy, vistas previas, presencia,
                     tokens, USync, outbox, vista previa de enlaces, límite de envíos, colas por chat y métricas
bindings/
├── bridge/          sesión compartida y despachador de métodos por nombre
├── node/            módulo nativo para Node.js (napi-rs)
├── python/          módulo nativo para Python (PyO3, asyncio)
└── server/          servidor WebSocket + HTTP + webhooks para cualquier lenguaje (axum)
clients/
└── js/              cliente JavaScript/TypeScript del servidor, sin dependencias
examples/            bot de pruebas, bot de eco para servidores falsos, vinculación con código, canario diario e
                     importación de Baileys y de zapo
tests/fixtures/      sesión de Baileys y base de zapo con claves y números de prueba, para las pruebas de migración
.github/workflows/   CI en tres sistemas, y binarios y paquetes para cada versión
```

</details>

## Estado

Versión 0.1. Esto es lo que se probó de verdad, y contra qué.

**Probado en vivo con un teléfono real**, comando por comando con el bot de ejemplo: vinculación por QR, mensajes
privados y de grupo (incluidos grupos con LID), `!ping`, editar, eliminar para todos, reacciones, fijar, contactos,
vista previa de enlaces, encuestas de una y de varias opciones con votos descifrados, eventos de calendario, álbumes,
documentos grandes subidos en streaming, descarga verificada a disco, mensajes interactivos, archivar (escritura de app
state), store de chats y búsqueda, bots de Meta AI, mensajes temporales, límite de contacto nuevo, canales, grupos e
invitaciones, y confirmaciones del servidor.

**Probado en vivo con una cuenta real y `!prueba`:** los mensajes viejos no se ejecutan (más de 1.000 al arrancar),
número y LID resueltos, admin detectado con solo el LID, tiempos por fase, la imagen repetida sale sin volver a
subirse (de 1.420 ms a 4 ms) y el outbox se vacía cuando WhatsApp confirma.

**Probado contra los servidores reales de WhatsApp, sin teléfono:** conexión a través de un proxy SOCKS5 con usuario y
contraseña, los módulos de Node.js y Python, y el servidor WebSocket con el cliente JavaScript (QR recibido por
WebSocket y API HTTP).

**Probado contra libsignal-node** (la librería Signal de Baileys): la migración desde Baileys. Una sesión creada por
libsignal se importa, SoblendWA descifra mensajes pendientes (incluido uno que obliga a avanzar el ratchet y otro de
grupo) y libsignal descifra las respuestas de SoblendWA.

**Probado contra zapo-js 1.11 real:** importación de su base SQLite y rollback, en los dos sentidos (ver
[Desde zapo](#desde-zapo)).

**Probado contra `@zapo-js/fake-server`,** escrito por otros: handshake Noise y certificado, vinculación por QR con firma
ADV, mensajes privados y de grupo, y un store escrito en JavaScript que sobrevive a un reinicio.

**Probado de punta a punta contra el WhatsApp falso** (protocolo real, sin cuentas): mensajes privados y de grupo entre
cuentas, mensajes dañados o sin sender key que se recuperan con el reintento, reintentos después de reiniciar,
reconexión con reenvío del outbox sin duplicar, cuenta bloqueada que se detiene, límite de envíos, multimedia subida y
descargada (también en streaming), vista previa completa con bloqueo de hosts privados, eventos tipados, y el servidor con
el cliente JavaScript y el harness de pruebas largas.

**Experimental** (implementado y probado solo localmente, todavía no contra una cuenta real): crear comunidades y
canales, catálogo de empresa, re-subida de multimedia caducada, etiquetas, respuestas rápidas, etiquetas de miembro,
mensajes de producto, el reenvío del outbox tras una caída real, la recuperación tras reintento con WhatsApp real, el
evento de cuenta bloqueada con un bloqueo real, la reutilización de multimedia pasada la hora (verificación en el CDN),
la miniatura nítida de la vista previa, los eventos de foto, info, bloqueos y privacidad, y el canario. Sin probar contra
un servidor real: `MongoStore`, `PostgresStore`, `RedisStore` y la importación desde `@zapo-js/store-mongo`. Todavía no se
hace una prueba de varios días en producción. Si alguno falla con tu cuenta, abre un
[issue](https://github.com/Soblend-Development-Studio/SoblendWA/issues) con el error.

**No incluido, a diferencia de zapo:** actuar como teléfono principal (registro móvil y vincular otros dispositivos),
mensajes de Meta AI con su cifrado propio (`msmsg`), confirmación por passkey al vincular, llamadas con audio y video,
telemetría WAM y un store de MySQL incluido (puedes escribirlo tú como store propio). De las llamadas solo se reciben los
eventos, se pueden rechazar y se pueden crear enlaces.

## Pruebas

```text
cargo test --workspace
cargo test --release -- --ignored throughput
cargo test -p soblendwa-server -- --ignored
```

- `cargo test --workspace` corre las pruebas del núcleo, de los backends de almacenamiento, de los bindings y del
  servidor (HTTP y WebSocket).
- `throughput` mide el rendimiento de la tabla de [Las piezas, una por una](#las-piezas-una-por-una).
- La prueba ignorada del servidor se conecta a WhatsApp y espera el QR por WebSocket.
- Con el servidor corriendo, `node clients/js/test.mjs` prueba el cliente JS (usa `SOBLENDWA_URL` y
  `SOBLENDWA_TOKEN`). `node bindings/node/test.js` y `python bindings/python/test_bindings.py` prueban los módulos
  nativos.

Incluye pruebas de robustez del decodificador binario: cientos de miles de entradas aleatorias y mutadas, anidamiento
hostil y bombas de compresión. Ninguna entrada puede provocar un pánico, un desbordamiento de pila ni agotar la memoria.

### WhatsApp falso para tus pruebas

`soblendwa::testing::FakeWhatsApp` (feature `testing`) es un servidor de WhatsApp en memoria que habla el protocolo
real: handshake Noise con su propia cadena de certificados, nodos binarios, reparto de pre-keys, USync, grupos con
sender keys, acks, recibos y mensajes guardados para cuentas desconectadas. Las cuentas son clientes SoblendWA normales,
así que prueba tu bot completo, cifrado incluido, sin arriesgar ningún número:

```rust,ignore
let fake = FakeWhatsApp::start().await?;
let (bot, mut eventos) = fake.connect("5215511111111").await?;
let (ana, _) = fake.connect("5215522222222").await?;
ana.send_text(&FakeWhatsApp::jid("5215511111111"), "!ping").await?;
fake.drop_connections("5215511111111");
fake.block("5215511111111", 403);
```

También simula la subida y descarga de multimedia, y permite provocar fallos: `corrupt_next` daña mensajes para probar
reintentos, `hold_acks` retiene confirmaciones y `expire_media` caduca archivos.

Para bots en JavaScript, TypeScript u otros lenguajes: `soblendwa-server --fake --session bot --session ana` levanta
el servidor contra ese WhatsApp falso (cada sesión recibe un número de prueba) y tu bot se conecta igual que siempre.

SoblendWA también pasa contra `@zapo-js/fake-server`, un servidor falso escrito por otros: desde Node, con
`server: { wsUrl: servidor.url, noiseRootHex: raiz }` en las opciones, tus bots pueden usarlo también.

### Pruebas de días y canario

- `node clients/js/soak.mjs --sessions bot,ana --hours 72 --pid <pid del servidor>` manda texto y multimedia entre
  cuentas vinculadas durante días y escribe cada minuto en `soak.jsonl` la memoria (RSS) por proceso, los mensajes
  perdidos, los que no se pudieron descifrar, las reconexiones y las métricas de cada cuenta. Con `--webhook` avisa en
  cuanto aparece un mensaje sin descifrar o un bloqueo. Para medir la memoria por cuenta, corre un servidor por cuenta y
  pasa `--pid cuenta=pid`.
- `cargo run --release --example canary -- ./sesion-prueba-a ./sesion-prueba-b` conecta dos cuentas de prueba, manda
  un texto y una imagen de una a otra, comprueba que se descifran y que la imagen descargada es idéntica, y revisa ping,
  mensajes sin descifrar y pánicos. Con `SOBLENDWA_ALERT_WEBHOOK` (Discord, Slack o cualquier webhook JSON) avisa si algo
  falla. Prográmalo una vez al día (cron, systemd timer o el Programador de tareas) para enterarte el mismo día si
  WhatsApp cambió algo.
- `.github/workflows/ci.yml` corre formato, clippy y todas las pruebas en Linux, Windows y macOS, y además levanta el
  servidor con `--fake` y le pasa el harness desde JavaScript.

## Aviso

SoblendWA es un proyecto independiente. No está afiliado, asociado ni respaldado por WhatsApp ni por Meta.
Automatizar una cuenta puede ir contra los Términos de Servicio de WhatsApp y provocar que la bloqueen.
Úsalo con responsabilidad y no envíes spam.

## Licencia y agradecimientos

MIT © 2026 **Soblend Development Studio**. Errores y propuestas en
[issues](https://github.com/Soblend-Development-Studio/SoblendWA/issues).

SoblendWA es una implementación propia, y existe gracias a lo que documentaron y publicaron
[Baileys](https://github.com/WhiskeySockets/Baileys), [whatsmeow](https://github.com/tulir/whatsmeow), libsignal y zapo.
