# SoblendWA para Node.js

Cliente de WhatsApp Web multi-dispositivo con el motor de
[SoblendWA](https://github.com/Soblend-Development-Studio/SoblendWA) en Rust, cargado como módulo nativo. Hecho por
**Soblend Development Studio**.

El cifrado Signal, el códec binario, el socket y el almacenamiento corren en Rust. JavaScript solo recibe eventos y
llama métodos, así que el bot no se frena por el recolector de basura ni por el hilo de eventos de Node.

## Instalar

```bash
npm install soblendwa
```

Funciona con Node.js 18 o más reciente en Windows x64, macOS (Intel y ARM) y Linux x64 o ARM64 con glibc 2.35 o más
reciente. No hace falta tener Rust. En Alpine (musl) usa
[`soblendwa-server`](https://github.com/Soblend-Development-Studio/SoblendWA#servidor-cualquier-lenguaje) con
[`soblendwa-client`](https://www.npmjs.com/package/soblendwa-client).

### Compilar desde el repositorio

```text
npm install
npm run build
npm test
```

`npm run build` genera `soblendwa.<plataforma>-<arquitectura>.node` con `@napi-rs/cli`, sin tocar `index.js` ni
`index.d.ts`. También puedes compilar con `cargo build -p soblendwa-node --release` y copiar la biblioteca resultante como
`soblendwa.node`.

## Uso

```js
const fs = require('node:fs')
const { SoblendWA } = require('soblendwa')

async function main() {
  const bot = await SoblendWA.create({
    store: { type: 'sqlite', path: 'session/auth.db' },
    pushName: 'Mi Bot',
  })

  bot.on('qr', ({ code }) => console.log('Escanea:', code))
  bot.on('connected', () => console.log('Conectado como', bot.me))
  bot.on('message', async (message) => {
    if (message.info.fromMe) return
    if (message.text === '!ping') await bot.reply(message, `Pong\n${bot.elapsed(message).toFixed(2)} ms`)
    if (message.text === '!foto') await bot.sendImage(message.info.chat, fs.readFileSync('foto.jpg'), { caption: 'Hola', quoted: message })
  })

  await bot.connect()
}

main()
```

Para migrar un bot de Baileys sin volver a escanear el QR:

```js
const { importBaileys } = require('soblendwa')
const report = await importBaileys('./auth_info', { type: 'sqlite', path: 'session/auth.db' })
```

Para vincular con código en lugar de QR: `await bot.requestPairCode('5215512345678')` después del primer evento `qr`.

También puedes recorrer los eventos con `for await (const event of bot.events())`.

## Opciones de `SoblendWA.create`

| Opción | Para qué |
|---|---|
| `store` | Dónde se guarda la sesión. Ver [Almacenamiento](#almacenamiento) |
| `pushName`, `deviceName`, `platform`, `countryCode` | Nombre visible, nombre en Dispositivos vinculados, plataforma y país |
| `onlineOnConnect`, `autoReconnect` | Aparecer en línea al conectar y reconectar solo |
| `proxy` | `'socks5://usuario:clave@host:1080'` o `http://` |
| `chatStore` | `{ path: 'session/chats.json', limit: 500 }` |
| `linkPreviews` | `{ enabled, timeoutMs, uploadHqThumbnail, allowPrivateHosts, maxHtmlBytes, maxImageBytes, userAgent, cacheTtlSecs }` |
| `server` | `{ wsUrl, noiseRootHex, mediaEndpoint }`, para conectar a un servidor falso en tus pruebas |

## Almacenamiento

```js
{ type: 'file', path: 'session' }
{ type: 'sqlite', path: 'auth.db', account: 'ventas' }
{ type: 'postgres', url: 'postgres://bot:clave@localhost/wa', account: 'ventas' }
{ type: 'redis', url: 'redis://127.0.0.1/', account: 'ventas' }
{ type: 'mongo', url: 'mongodb://localhost:27017', database: 'wa', account: 'ventas' }
{ type: 'memory' }
```

MongoDB es experimental: falta probarlo contra un servidor real.

También puedes pasar tu propio store con cualquier base de datos. Las lecturas ya pasan por caché y las escrituras llegan
agrupadas:

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

`apply` recibe una lista de `{ op: 'set' | 'remove' | 'clear', kind, key, value }`. En lugar de `apply` puedes dar `set`,
`remove` y `clear`.

## Métodos

| Método | Qué hace |
|---|---|
| `sendText(chat, text, { quoted, preview })` | Texto o respuesta. Los enlaces llevan vista previa sola; `preview: false` la quita y `preview: { title, description }` la escribe a mano |
| `stats()` / `pendingSends()` | Métricas de la sesión y envíos esperando confirmación |
| `reply(message, text)` / `react(message, emoji)` | Responder o reaccionar a un mensaje recibido |
| `sendImage` `sendVideo` `sendAudio` `sendDocument` `sendSticker` | Multimedia (`caption`, `fileName`, `mimetype`, `voiceNote`, `gif`, `viewOnce`, `quoted`) |
| `download(message)` | Descarga y descifra la multimedia de un mensaje recibido |
| `sendFile(chat, kind, path, options)` / `downloadToFile(message, path)` | Archivos de cualquier tamaño, por bloques y con memoria constante |
| `sendInteractive(chat, { body, buttons, title, footer })` | Botones `reply`, `url`, `copy`, `call` y `select` |
| `sendButtons(chat, text, [{ id, text }])` / `sendList(chat, { title, text, buttonText, sections })` | Botones y listas clásicos |
| `chats()` / `contacts()` / `messages(chat)` / `search(text)` | Store de chats (opción `chatStore`) |
| `sendPoll(chat, name, options, selectable)` / `votePoll` | Encuestas |
| `edit(chat, id, text)` / `revoke(chat, id)` | Editar o eliminar para todos |
| `typing(chat, state)` / `setOnline(bool)` | Escribiendo, grabando, en línea |
| `groupInfo(chat)` / `joinedGroups()` | Grupos |
| `onWhatsApp(...phones)` / `profilePicture(jid)` | Contactos |
| `block(jid)` / `unblock(jid)` | Bloqueo |
| `sendProto(chat, bytes)` | Cualquier mensaje de WhatsApp codificado en protobuf |
| `ping()` / `elapsed(message)` | Latencia de red y tiempo del mensaje dentro del bot |

`chat` acepta un JID completo (`120363...@g.us`) o solo el número de teléfono.

Los eventos son objetos con un campo `type`: `qr`, `pairSuccess`, `connected`, `disconnected`, `loggedOut`,
`accountBlocked`, `message`, `messageRecovered`, `receipt`, `presence`, `chatState`, `groupUpdate`, `call`, `pollVote`,
`eventResponse`, `historySync`, `serverAck`, `chatUpdate`, `notification` y `unhandled`. Los tipos completos están en
`index.d.ts`.

## Aviso

SoblendWA es independiente y no está afiliado a WhatsApp ni a Meta. Automatizar una cuenta puede ir contra sus Términos
de Servicio y provocar un bloqueo. Lee
[qué está probado](https://github.com/Soblend-Development-Studio/SoblendWA#estado).

MIT © 2026 Soblend Development Studio
