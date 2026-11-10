<p align="center">
  <a href="https://github.com/Soblend-Development-Studio/SoblendWA">
    <picture>
      <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/Soblend-Development-Studio/SoblendWA/main/assets/banner-dark.svg">
      <img alt="SoblendWA: WhatsApp Web en Rust, para bots en cualquier lenguaje" src="https://raw.githubusercontent.com/Soblend-Development-Studio/SoblendWA/main/assets/banner-light.svg" width="100%">
    </picture>
  </a>
</p>

<p align="center">
  <a href="https://crates.io/crates/soblendwa"><img alt="crates.io" src="https://img.shields.io/crates/v/soblendwa?color=d0021b&label=crates.io"></a>
  <a href="https://www.npmjs.com/package/soblendwa-node"><img alt="npm" src="https://img.shields.io/npm/v/soblendwa-node?color=d0021b&label=npm"></a>
  <a href="https://pypi.org/project/soblendwa/"><img alt="PyPI" src="https://img.shields.io/pypi/v/soblendwa?color=d0021b&label=PyPI"></a>
  <a href="https://github.com/Soblend-Development-Studio/SoblendWA/actions/workflows/ci.yml"><img alt="CI" src="https://img.shields.io/github/actions/workflow/status/Soblend-Development-Studio/SoblendWA/ci.yml?branch=main&label=CI"></a>
  <a href="https://github.com/Soblend-Development-Studio/SoblendWA/blob/main/LICENSE"><img alt="Licencia MIT" src="https://img.shields.io/badge/licencia-MIT-18181b"></a>
</p>

<p align="center">
  <a href="https://soblend-development-studio.github.io/SoblendWA/"><b>Documentación</b></a> ·
  <a href="#instalar">Instalar</a> ·
  <a href="#un-bot-en-unas-líneas">Ejemplo</a> ·
  <a href="https://github.com/Soblend-Development-Studio/SoblendWA/blob/main/CHANGELOG.md">Cambios</a>
</p>

Cliente de WhatsApp Web multi-dispositivo escrito en Rust, para hacer bots en Rust, Node.js, Python o lo que hable
WebSocket. Lo hacemos en Soblend Development Studio.

Empezó porque nuestro bot en zapo ya pasaba de 600 MB de RAM con unas seis sesiones, y cada semana aparecían mensajes
que no se podían descifrar sin forma de saber cuántos eran. SoblendWA habla el mismo protocolo que Baileys, whatsmeow o
zapo (WebSocket, Noise, nodos binarios y Signal), sin navegador. La diferencia es que todo eso corre en Rust: desde Node
o Python solo te llegan eventos y llamas métodos.

> No es oficial ni tiene nada que ver con Meta. Usar una cuenta como bot puede terminar en bloqueo, así que antes de
> meterlo a producción mira qué está probado de verdad en [Estado](#estado).

## Instalar

| Tu bot está en | Instala | Notas |
|---|---|---|
| Node.js 18+ | `npm install soblendwa-node` | Windows x64, macOS (Intel y ARM), Linux x64/ARM64 con glibc 2.35+ |
| Python 3.9+ | `pip install soblendwa` | Windows x64, macOS y Linux x64/ARM64 |
| Rust 1.89+ | `cargo add soblendwa` | Features: `sqlite`, `postgres`, `redis`, `mongo` (`stores` = las cuatro) y `testing` |
| Otro lenguaje, Alpine o sin módulos nativos | [`soblendwa-server`](https://github.com/Soblend-Development-Studio/SoblendWA/releases) | Para JS/TS hay un cliente sin dependencias: `npm install soblendwa-client` |

El paquete de Node pesa unos 40 MB porque trae los binarios de las cinco plataformas. No hace falta tener Rust.

## Un bot en unas líneas

```js
const { SoblendWA } = require('soblendwa-node')

async function main() {
  const bot = await SoblendWA.create({ store: { type: 'sqlite', path: 'session/auth.db' } })

  bot.on('qr', ({ code }) => console.log('Escanea este QR:', code))
  bot.on('message', async (message) => {
    if (message.info.fromMe) return
    if (message.text === '!ping') await bot.reply(message, 'Pong')
  })

  await bot.connect()
}

main()
```

<details>
<summary>Lo mismo en Rust</summary>

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
            Event::LoggedOut { reason, .. } => {
                println!("Sesión cerrada: {reason}");
                break;
            }
            _ => {}
        }
    }
    Ok(())
}
```

</details>

<details>
<summary>Lo mismo en Python</summary>

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
        if not message["info"]["fromMe"] and message["text"] == "!ping":
            await bot.reply(message, "Pong")

    await bot.run()


asyncio.run(main())
```

</details>

El evento `qr` trae el texto del código; el bot de ejemplo y el servidor ya lo dibujan en la terminal. Si prefieres
vincular con el número, llama `requestPairCode('5215512345678')` (`request_pair_code` en Rust y Python) después del
primer `qr` y escribe el código en Dispositivos vinculados.

Hay un bot de pruebas con unos 30 comandos en `examples/bot.rs`: `cargo run --release --example bot`, y desde otro
número le escribes `!ayuda`.

## Rendimiento

Un bot de eco contra el mismo `@zapo-js/fake-server`, cada uno en su proceso y con almacenamiento en memoria, zapo
usando su acelerador nativo. Es la mediana de 3 corridas alternadas, 500 idas y vueltas más 500 en ráfaga:

| | SoblendWA | zapo-js 1.11 |
|---|---|---|
| Ida y vuelta p50 / p95 / p99 | **4,2** / 10,1 / 16,3 ms | 10,5 / 18,4 / 23,3 ms |
| Mensajes por segundo en ráfaga | **474** | 228 |
| CPU del bot por mensaje | **0,7 ms** | 4,5 ms |
| Memoria al terminar (pico) | **12 MB** (24 MB) | 130 MB (131 MB) |
| Conectar y vincular | **100 ms** | 1.565 ms |

Ojo con estos números: es una sola máquina Windows con otras cosas abiertas (la ráfaga nuestra varió entre 394 y 786
msg/s), el servidor falso corre en Node y pone un piso para los dos, y no hay red real. En producción casi todo el
tiempo se va en la red hasta WhatsApp, y eso no lo arregla ninguna librería. Para ver cuánto se va en cada fase con tu
red, usa `stats().sendTimings`.

<details>
<summary>Más números: pieza por pieza, desde Node y con muchas cuentas</summary>

Contra el código real de cada librería en la misma máquina:

| | SoblendWA | whatsmeow | Baileys 7 |
|---|---|---|---|
| Codificar y decodificar un stanza de 21 nodos | 63.242/s | 12.810/s | 5.320/s |
| Cifrar y descifrar un mensaje 1:1 (Signal) | 135.504/s | 12.223/s | 3.720/s |
| Lo mismo en grupo (sender keys) | 9.117/s | 3.565/s | 58/s |

whatsmeow es el del 2 de noviembre de 2026 con libsignal v0.2.2, y Baileys es 7.0.0-rc14. A esos dos se les midió
con las sesiones en memoria, que es su caso más rápido; nosotros serializamos la sesión en cada mensaje. Lo nuestro se
repite con `cargo test --release -- --ignored throughput`.

Desde Node, con dos cuentas en el mismo proceso y una mandándole 10.000 mensajes seguidos a la otra, la 0.2 manda
~9.000 a 9.900 por segundo (la 0.1, ~7.500) y le entrega ~6.500 por segundo al handler (la 0.1, ~4.600), las dos
medidas con la misma compilación. Qué cambió por dentro está en el
[CHANGELOG](https://github.com/Soblend-Development-Studio/SoblendWA/blob/main/CHANGELOG.md).

Con muchas cuentas en un solo proceso de Node (`bindings/node/bench-accounts.js`, contra el WhatsApp falso, Node 22,
Windows):

| Cuentas | RSS antes | RSS con las cuentas | Por cuenta |
|---|---|---|---|
| 20 | 33 MB | 65 MB | 1,6 MB |
| 50 | 33 MB | 96 MB | 1,3 MB |

En las dos corridas las cuentas se mandan mensajes en anillo, luego se le corta la conexión a una y se cierra otra, y
las demás siguen funcionando. El RSS incluye al servidor falso, que corre en el mismo proceso. Son cuentas recién
vinculadas: con grupos grandes, store de chats y semanas de caché la memoria crece, así que mídelo en tu servidor con
`node --expose-gc bench-accounts.js 20,50`.

</details>

## Qué trae

- QR o código de 8 caracteres, Noise con verificación de certificados, Signal completo (X3DH, Double Ratchet, sender
  keys, rotación de pre-keys) y reintentos en los dos sentidos.
- Texto, respuestas, menciones (también a todos), reacciones, editar, borrar para todos, reenviar, fijar, ubicación,
  contactos, encuestas, eventos de calendario, estados, botones, listas y mensajes interactivos.
- Vista previa de enlaces automática, con miniatura nítida y sin consultar nunca hosts privados.
- Imagen, video, GIF, video circular, audio, notas de voz, documentos, stickers y álbumes. Archivos de cualquier tamaño
  con memoria constante, descargas verificadas antes de tocar el disco, y lo que ya subiste no se vuelve a subir.
- Grupos completos (crear, participantes, admins, asunto, descripción, ajustes, enlaces, solicitudes), comunidades,
  canales y catálogo de empresa.
- App state: archivar, fijar, silenciar, etiquetas, respuestas rápidas, contactos.
- Perfil, privacidad, bloqueos, mensajes temporales.
- Sesiones en archivos, memoria, SQLite, PostgreSQL, Redis, MongoDB o tu propia base, incluso escrita en JavaScript.
- Proxy HTTP o SOCKS5 para todo, incluida la multimedia.
- Un WhatsApp falso con el protocolo real para probar sin arriesgar números, usable desde Rust y desde Node.

Lo que no tiene y zapo sí: actuar como teléfono principal, llamadas con audio y el cifrado propio de los mensajes de Meta
AI.

## Para bots que corren meses

Esta parte es la que más nos importaba. Cosas que pasan solas:

- Si WhatsApp bloquea la cuenta (403, 402 o 406) llega `AccountBlocked` con el motivo y deja de reintentar, pero no borra
  la sesión. Desbloqueas en el teléfono y vuelve a conectar sin QR.
- Cuando se cierra la sesión, `LoggedOut` dice quién la cerró: `cause` es `removedFromPhone` si la quitaron desde
  Dispositivos vinculados (el famoso `device_removed`), `sessionInvalidated` si fue WhatsApp, o `userLogout` si fuiste tú.
- Un mensaje que no se pudo descifrar se pide otra vez al remitente y a tu teléfono. Si llega, sale
  `messageRecovered`; si pasan 10 minutos (`retryDeadlineSecs`) sin nada, sale `messageLost` y se cuenta en
  `stats().lost`. Así sabes cuántos se perdieron de verdad, no solo cuántos fallaron.
- Hay un evento `joinedGroup` para cuando el bot entra a un grupo, sin importar si lo crearon con él, lo agregaron o
  entró por enlace. Y `leftGroup` cuando sale o lo sacan. Nos pasó estar esperando `add` mientras llegaba `create`; ya
  no hace falta saber eso.
- Cada mensaje trae el número y el LID del remitente ya resueltos. Puedes preguntar `lidForPhone` / `phoneForLid` cuando
  quieras, y cada vez que se descubre un par nuevo llega `lidMapping`, que sirve para unir perfiles sin migraciones.
- Lo que envías queda en un outbox hasta que WhatsApp lo confirma. Si se cae la conexión o reinicias, se reenvía con el
  mismo ID. Los mensajes que llegaron con el bot apagado vienen con `offline` y `ageSecs`, para no ejecutar comandos
  de hace horas.
- Varias cuentas pueden compartir la caché de multimedia (`mediaStore`): si una ya subió una imagen, las demás la mandan
  sin volver a subirla, y sobrevive a los reinicios.
- `stats().sendTimings` da p50, p95 y máximo de cada fase del envío con tu red: cola del chat, subida, preparación,
  cifrado y respuesta del servidor.
- Lo que mandas al mismo chat sale en el orden en que lo llamaste, aunque no esperes cada envío. Chats distintos van
  en paralelo, y los archivos suben a la vez aunque vayan al mismo chat: solo la entrega espera turno.
- Si el bot se reinicia mientras espera el reintento de un mensaje que no pudo descifrar, la espera sigue donde estaba
  (se guarda en el store) y `messageLost` / `messageRecovered` llegan igual.
- Hay un límite de envíos por defecto (240 por minuto, 40 por chat) para que un bucle no termine en spam; se ajusta
  o se apaga con `sendLimit`. La reconexión espera cada vez más, con algo de azar para que cien sesiones no reconecten
  en el mismo milisegundo.
- Un handler que truena (pánico en Rust, excepción o promesa rechazada en Node, excepción en Python) se reporta y la
  sesión sigue. Una cuenta que falla no tumba a las demás del proceso, y cada una se puede reiniciar o cerrar sola.
- Los errores tienen código: `APPROVAL_REQUIRED` si el grupo pide aprobación, `PRIVATE_PHOTO` si la foto es privada,
  `NO_DEVICES` si el número no tiene WhatsApp, `RATE_LIMITED`, etc. En Node y Python llegan como `SoblendError` con
  `code` y `status`.

## Servidor: cualquier lenguaje

`soblendwa-server` corre el motor y lo expone por WebSocket, HTTP y webhooks. Un servidor maneja varias cuentas.

```bash
curl -L -o soblendwa.tar.gz https://github.com/Soblend-Development-Studio/SoblendWA/releases/latest/download/soblendwa-server-x86_64-unknown-linux-musl.tar.gz
tar -xzf soblendwa.tar.gz && ./soblendwa-server --token "una-clave-larga" --session bot
```

El de Linux es estático, así que corre en cualquier VPS. Con dos cuentas anduvo en unos 14 MB. Para compilarlo tú:
`cargo install --git https://github.com/Soblend-Development-Studio/SoblendWA soblendwa-server`.

Por WebSocket te conectas a `ws://host:8787/ws/<cuenta>?token=…`, llegan los eventos como
`{"session":"ventas","event":{"type":"message",…}}` y llamas métodos con
`{"id":1,"method":"sendText","params":{"chat":"5215512345678","text":"hola"}}`. La respuesta es `{"id":1,"result":…}`
o `{"id":1,"error":{"code":"NOT_FOUND","status":404,"message":"…"}}`. Por HTTP es lo mismo:

```bash
curl -X POST http://127.0.0.1:8787/api/ventas/sendText \
  -H "Authorization: Bearer mi-clave" \
  -d '{"chat":"5215512345678","text":"Hola desde curl"}'
```

`GET /` lista los métodos y `GET /metrics` da las métricas de todas las cuentas para Prometheus. El cliente de JS/TS
está en
[`clients/js`](https://github.com/Soblend-Development-Studio/SoblendWA/tree/main/clients/js) y funciona en Node, Bun,
Deno y el navegador.

<details>
<summary>Opciones del servidor</summary>

| Opción | Para qué |
|---|---|
| `--host`, `--port` | Dónde escucha (por defecto `127.0.0.1:8787`) |
| `--token` | Clave para todo, como `Authorization: Bearer …` o `?token=` |
| `--dir`, `--sqlite`, `--mongo` + `--mongo-db`, `--memory` | Dónde guardar las sesiones |
| `--session` | Cuentas que se abren al arrancar; las demás se abren al primer uso |
| `--webhook` | Manda cada evento por POST a tu URL |
| `--proxy`, `--push-name` | Proxy y nombre visible |
| `--no-chat-store`, `--no-link-previews` | Apagan el store de chats y las vistas previas |
| `--fake` | Usa el WhatsApp falso, nada sale de tu máquina |

Casi todas tienen variable de entorno (`SOBLENDWA_TOKEN`, `SOBLENDWA_PORT`, …).

</details>

## Migrar desde zapo o Baileys

Las dos migraciones traen credenciales, sesiones Signal, pre-keys, sender keys, app state y los pares número/LID, así que
nadie tiene que volver a vincular. Apaga el bot viejo antes; dos librerías no pueden usar la misma sesión a la vez.

Desde Node, con zapo en SQLite o en Mongo:

```js
const { importZapo, rollbackZapo } = require('soblendwa-node')

const zapo = { type: 'mongo', url: 'mongodb://localhost:27017', database: 'bot', session: 'principal' }
const prueba = await importZapo(zapo, null, { check: true })
const hecho = await importZapo(zapo, { type: 'sqlite', path: 'session/auth.db' })
await rollbackZapo({ type: 'sqlite', path: 'session/auth.db' }, zapo)
```

Con `check: true` lee todo y cuenta lo que pasaría sin escribir nada. Se niega a importar sobre un store que ya tiene una
cuenta vinculada, y `rollbackZapo` escribe el estado de vuelta en la base de zapo con los ratchets al día. Con SQLite se
probó contra zapo-js 1.11 real en los dos sentidos. Desde `@zapo-js/store-mongo` pasa contra un MongoDB real en el CI,
pero con documentos de prueba y no con una base que haya escrito zapo, así que hazlo primero sobre una copia, como explica
[la guía](https://soblend-development-studio.github.io/SoblendWA/#migrar-copia).

Desde Rust, sin Node:

```bash
cargo run --release --example import_zapo --features sqlite -- ./auth/state.sqlite bot ./session
cargo run --release --example import_zapo --features sqlite -- --rollback ./session ./auth/state.sqlite bot
cargo run --release --example import_baileys -- ./auth_info ./session
```

Desde Baileys en Node: `importBaileys('./auth_info', { type: 'sqlite', path: 'session/auth.db' })`. La tabla de
equivalencias función por función está en la documentación.

## Estado

Lo que se probó de verdad: vinculación con QR y con código, mensajes privados y de grupo, multimedia, encuestas,
mensajes interactivos, canales, grupos, la recuperación de mensajes de grupo sin sender key y los tiempos por fase con
red real, todo con teléfonos y números reales. Las migraciones se probaron contra libsignal-node y zapo-js 1.11 reales,
y todo lo demás contra nuestro WhatsApp falso, que habla el protocolo real.

Falta probar en real: comunidades y canales propios, catálogo, re-subida de multimedia caducada, etiquetas, el bloqueo
con WhatsApp de verdad, qué tanto se recuperan los mensajes con MAC mala, y `joinedGroup`, la causa de `loggedOut` y
`messageLost` con un número de verdad. MongoDB, PostgreSQL y Redis ya pasan contra servidores reales en el CI
(MongoDB 7, PostgreSQL 16 y Redis 7), pero falta usarlos con datos de producción. La corrida de semanas ya está en
marcha y falta que termine.

Si algo falla con tu cuenta, abre un [issue](https://github.com/Soblend-Development-Studio/SoblendWA/issues) con el
error. Si es una falla de seguridad, mejor en privado como dice
[SECURITY.md](https://github.com/Soblend-Development-Studio/SoblendWA/blob/main/SECURITY.md).

<details>
<summary>Qué se probó y contra qué, con detalle</summary>

Con un teléfono real, comando por comando con el bot de ejemplo: vinculación, mensajes privados y de grupo (también
grupos con LID), editar, borrar, reacciones, fijar, contactos, vistas previas, encuestas y votos, eventos de calendario,
álbumes, documentos grandes en streaming, descargas, mensajes interactivos, archivar, store de chats, bots de Meta AI,
mensajes temporales, canales, grupos e invitaciones. También que los mensajes viejos no se ejecutan, que número y LID
salen resueltos y que una imagen repetida pasa de 1.420 ms a 4 ms.

Con un número real y `examples/soak.rs`, en noviembre de 2026: vincular con código, la recuperación de mensajes de
grupo que llegan sin su sender key (el reintento los trae en menos de un segundo), los tiempos por fase con red real,
descargar y reenviar fotos reutilizando lo ya subido, y el aviso `lidMapping`.

Contra servidores reales sin teléfono: proxy SOCKS5, los módulos de Node y Python, y el servidor con el cliente JS.

Contra el código de otros: sesiones de libsignal-node (la migración de Baileys en los dos sentidos), zapo-js 1.11 real
(importar y volver), y `@zapo-js/fake-server`.

Contra nuestro WhatsApp falso: todo lo demás, incluidos reintentos y mensajes perdidos, outbox tras una caída, cuentas
bloqueadas o quitadas desde el teléfono, eventos de grupo, multimedia compartida entre cuentas y muchas cuentas en un
proceso.

</details>

Para probar lo que falta hay dos scripts en el paquete de Node: `store-check.js` revisa tu base de datos sin tocar
WhatsApp, y `live-check.js` es un bot para un número de prueba que anota todo en un reporte y luego te lo resume. Si
solo tienes un número, `examples/soak.rs` se queda conectado con el tuyo y solo te contesta a ti. Los pasos están en
[Probar en real](https://soblend-development-studio.github.io/SoblendWA/#probar-en-real).

## Pruebas

```bash
cargo test --workspace
cargo test --features "sqlite mongo testing" --lib
node bindings/node/test-fake.js
python bindings/python/test_fake.py
```

`test-fake.js` y `test_fake.py` levantan el WhatsApp falso desde Node y Python y prueban citas, menciones, miniaturas
propias, grupos, errores, reinicios, multimedia compartida, subidas lentas y el orden de una ráfaga. En tus propias
pruebas lo usas igual:

```js
const { FakeWhatsApp } = require('soblendwa-node')

const fake = await FakeWhatsApp.start()
const bot = await fake.connect('5215511111111')
const ana = await fake.connect('5215522222222')
await ana.sendText('5215511111111', '!ping')
```

En Python es `from soblendwa import FakeWhatsApp` con los mismos métodos en snake_case, y `bot.wait_for("message")`.

El decodificador binario se prueba con cientos de miles de entradas aleatorias y mutadas, y ninguna lo hace entrar en
pánico. El CI corre formato, clippy y todo lo anterior en Linux, Windows y macOS, y también contra MongoDB, PostgreSQL
y Redis de verdad.

Para corridas largas está `examples/soak.rs`: se queda conectado semanas con tu número, no responde a nadie y solo te
contesta a ti cuando escribes `!yowner` en un chat privado. `clients/js/soak.mjs` hace algo parecido entre dos cuentas,
y `examples/canary.rs` conviene correrlo una vez al día para enterarse rápido si WhatsApp cambió algo.

---

<p align="center">
  <img src="https://raw.githubusercontent.com/Soblend-Development-Studio/SoblendWA/main/assets/icon.svg" width="44" alt=""><br>
  MIT © 2026 Soblend Development Studio
</p>

<p align="center">
  SoblendWA es una implementación propia, pero no existiría sin lo que documentaron
  <a href="https://github.com/WhiskeySockets/Baileys">Baileys</a>, <a href="https://github.com/tulir/whatsmeow">whatsmeow</a>,
  libsignal y zapo.
</p>
