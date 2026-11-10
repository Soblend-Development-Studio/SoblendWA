<img src="https://raw.githubusercontent.com/Soblend-Development-Studio/SoblendWA/main/assets/icon.svg" width="64" alt="" align="right">

# soblendwa-node

WhatsApp Web multi-dispositivo para Node.js, con el motor de
[SoblendWA](https://github.com/Soblend-Development-Studio/SoblendWA) en Rust cargado como módulo nativo. De Soblend
Development Studio.

Signal, el socket, el códec y el almacenamiento corren en Rust; a JavaScript solo le llegan eventos. El recolector de
basura de Node no frena al bot y la memoria por cuenta es baja.

```bash
npm install soblendwa-node
```

Node 18 o más nuevo, en Windows x64, macOS (Intel y ARM) y Linux x64/ARM64 con glibc 2.35+. No hace falta Rust. En
Alpine (musl) no hay binario: usa
[`soblendwa-server`](https://github.com/Soblend-Development-Studio/SoblendWA#servidor-cualquier-lenguaje) con
[`soblendwa-client`](https://www.npmjs.com/package/soblendwa-client).

## Ejemplo

```js
const fs = require('node:fs')
const { SoblendWA } = require('soblendwa-node')

async function main() {
  const bot = await SoblendWA.create({
    store: { type: 'sqlite', path: 'session/auth.db' },
    pushName: 'Mi Bot',
  })

  bot.on('qr', ({ code }) => console.log('Escanea:', code))
  bot.on('connected', () => console.log('Conectado como', bot.me))
  bot.on('joinedGroup', ({ group }) => bot.sendText(group, 'Hola a todos'))
  bot.on('message', async (message) => {
    if (message.info.fromMe || message.info.offline) return
    if (message.text === '!ping') await bot.reply(message, `Pong ${bot.elapsed(message).toFixed(2)} ms`)
    if (message.text === '!foto') {
      await bot.sendImage(message.info.chat, fs.readFileSync('foto.jpg'), { caption: 'Hola', quoted: message })
    }
  })

  await bot.connect()
}

main()
```

Para vincular con número en vez de QR: `await bot.requestPairCode('5215512345678')` después del primer `qr`.

Si prefieres no usar `on`, también puedes hacer `for await (const event of bot.events())`.

## Opciones de `SoblendWA.create`

| Opción | |
|---|---|
| `store` | Dónde se guarda la sesión, ver abajo. Por defecto la carpeta `session` |
| `pushName`, `deviceName`, `platform`, `countryCode` | Nombre visible, nombre en Dispositivos vinculados, plataforma y país |
| `onlineOnConnect`, `autoReconnect` | Aparecer en línea al conectar, reconectar solo |
| `proxy` | `'socks5://usuario:clave@host:1080'` o `http://…` |
| `chatStore` | `{ path: 'session/chats.json', limit: 500 }` para tener `chats()`, `messages()` y `search()` |
| `linkPreviews` | `{ enabled, timeoutMs, uploadHqThumbnail, allowPrivateHosts, … }` |
| `mediaStore` | Caché de multimedia compartida entre cuentas, por ejemplo `{ type: 'sqlite', path: 'media.db' }` |
| `retryDeadlineSecs` | Cuánto esperar a que llegue un mensaje que no se pudo descifrar antes de darlo por perdido (600) |
| `sendLimit` | `false` lo apaga; `{ perMinute, burst, perChatPerMinute, perChatBurst }` lo ajusta (240, 60, 40 y 15) |
| `server` | `{ wsUrl, noiseRootHex, mediaEndpoint }`, para apuntar a un servidor falso |

## Dónde guardar la sesión

```js
{ type: 'file', path: 'session' }
{ type: 'sqlite', path: 'auth.db', account: 'ventas' }
{ type: 'postgres', url: 'postgres://bot:clave@localhost/wa', account: 'ventas' }
{ type: 'redis', url: 'redis://127.0.0.1/', account: 'ventas' }
{ type: 'mongo', url: 'mongodb://localhost:27017', database: 'wa', account: 'ventas' }
{ type: 'memory' }
```

Con `account` varias cuentas comparten la misma base. MongoDB, PostgreSQL y Redis pasan contra servidores reales en el
CI, pero nadie los ha usado semanas en producción todavía; si los usas, haz respaldo.

También puedes escribir el store tú con la base que quieras. Las lecturas ya pasan por caché y las escrituras llegan
juntas, así que no tienes que optimizar nada:

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

`apply` recibe `[{ op: 'set' | 'remove' | 'clear', kind, key, value }]`. Si no quieres `apply`, da `set`, `remove` y
`clear` por separado.

## Métodos

`chat` puede ser un JID (`120363...@g.us`) o solo el número. Los envíos al mismo chat salen en el orden en que los
llamas, aunque no esperes cada uno con `await`.

Mensajes:

- `sendText(chat, text, { quoted, mentions, preview })`. Los enlaces llevan vista previa sola. `preview: false` la
  quita y `preview: { title, description, thumbnail }` la pones tú, con la miniatura como `Buffer`.
- `reply(message, text)`, `react(message, emoji)`, `edit(chat, id, text)`, `revoke(chat, id)`
- `sendImage`, `sendVideo`, `sendAudio`, `sendDocument`, `sendSticker` con `{ caption, fileName, mimetype, voiceNote,
  gif, viewOnce, quoted, mentions }`
- `sendFile(chat, kind, path)` y `downloadToFile(message, path)` para archivos grandes, sin cargarlos en memoria
- `download(message)` devuelve un `Buffer`
- `sendPoll`, `votePoll`, `sendContacts`, `sendMentionAll`, `sendGroupInvite`, `sendInteractive`, `sendButtons`,
  `sendList`, `sendProto`
- `typing(chat, 'composing' | 'recording' | 'paused')`, `setOnline(bool)`, `markPlayed(message)`

Grupos:

- `groupInfo(chat)` sale de la caché; `groupMetadata(chat)` siempre pregunta al servidor; `joinedGroups()`
- `groupCreate(subject, participants)`, `groupLeave(chat)`
- `groupAdd`, `groupKick`, `groupPromote`, `groupDemote` (o `groupParticipants(chat, jids, action)`). Devuelven
  `[{ jid, status, ok }]` por cada participante
- `groupSubject`, `groupDescription`, `groupPicture`
- `groupSetting(chat, setting, value)` con `announce`, `locked`, `joinApproval`, `memberAdd` o `ephemeral`
- `groupInviteCode`, `groupRevokeInvite`, `groupInviteInfo(link)`, `groupJoin(link)`
- `groupRequests(chat)`, `groupRespondRequests(chat, jids, approve)`

Perfil y contactos:

- `profilePicture(jid)`, `setProfilePicture(buffer)`, `removeProfilePicture()`
- `setAbout`, `fetchAbout`, `setPushName`, `businessProfile`
- `block`, `unblock`, `blocklist`, `privacySettings`, `setPrivacy`
- `onWhatsApp(...phones)`, `lidForPhone(phone)`, `phoneForLid(lid)`

Chats (con `chatStore`): `chats()`, `contacts()`, `messages(chat)`, `search(text)`, `modifyChat`, `labelChat`,
`labelMessage`, `editLabel`, `deleteLabel`, `saveQuickReply`.

La sesión: `connect()`, `restart()`, `disconnect()`, `close()`, `logout()`, `ping()`, `stats()`, `pendingSends()`,
`elapsed(message)`, `waitFor(type, filter, timeoutMs)`.

Y si falta algo, `call(method, params)` llama cualquier método del despachador por nombre, igual que el servidor.

## Eventos

Cada evento es un objeto con `type`. Los de siempre: `qr`, `pairSuccess`, `connected`, `disconnected`, `message`,
`receipt`, `presence`, `chatState`, `groupUpdate`, `call`, `pollVote`, `eventResponse`, `historySync`, `serverAck`,
`chatUpdate`, `notification` y `unhandled`.

Los que más ayudan en un bot grande:

- `loggedOut` trae `cause`: `removedFromPhone`, `sessionInvalidated` o `userLogout`, y el `code` del servidor.
- `accountBlocked` cuando WhatsApp bloquea la cuenta. No se borra la sesión.
- `joinedGroup` (`via`: `created`, `added` o `invite`) y `leftGroup` (`removed` dice si te sacaron).
- `messageRecovered` cuando llega un mensaje que primero no se pudo descifrar, y `messageLost` si nunca llegó.
  `stats().lost` lleva la cuenta.
- `lidMapping` cada vez que se descubre que un número y un LID son la misma persona.

Los tipos completos están en `index.d.ts`.

## Errores

Todo lo que falla lanza un `SoblendError` con `code` y, si vino del servidor de WhatsApp, `status`:

```js
const { SoblendError } = require('soblendwa-node')

try {
  await bot.groupJoin(link)
} catch (error) {
  if (error.code === 'APPROVAL_REQUIRED') return bot.reply(message, 'Ese grupo pide aprobación, ya mandé la solicitud')
  throw error
}
```

Algunos códigos: `APPROVAL_REQUIRED`, `PRIVATE_PHOTO`, `MESSAGE_NOT_FOUND`, `NO_DEVICES` (el número no tiene WhatsApp),
`INVALID_ARGUMENT`, `NOT_AUTHORIZED`, `FORBIDDEN`, `NOT_FOUND`, `RATE_LIMITED`, `ACCOUNT_BLOCKED`, `NOT_CONNECTED`,
`TIMEOUT`. La lista entera está en `ErrorCode` dentro de `index.d.ts`.

Si un listener lanza una excepción o su promesa se rechaza, se emite `error` (o se imprime si no escuchas `error`) y los
demás listeners y la cuenta siguen funcionando.

## Varias cuentas

Cada `SoblendWA.create` es una cuenta independiente dentro del mismo proceso. Si una falla, las demás siguen, y puedes
hacer `restart()` o `close()` de una sola. `bench-accounts.js` conecta N cuentas contra el servidor falso y mide la
memoria:

```bash
node --expose-gc bench-accounts.js 20,50
```

Para que varias cuentas reutilicen lo que otra ya subió, dales el mismo `mediaStore`. Como vive en una base, también
sobrevive a los reinicios.

## Servidor falso para tus pruebas

`FakeWhatsApp` es un WhatsApp en memoria que habla el protocolo real, con cifrado y todo. Ningún número real se toca:

```js
const { FakeWhatsApp } = require('soblendwa-node')

const fake = await FakeWhatsApp.start()
const bot = await fake.connect('5215511111111')
const ana = await fake.connect('5215522222222')

const group = fake.createGroup('Pruebas', '5215522222222', ['5215511111111'])
await ana.sendText(group, '!menu')

fake.setJoinApproval(group, true)
fake.dropConnections('5215511111111')
fake.corruptNext('5215511111111')
```

También tiene `addToGroup`, `removeFromGroup`, `inviteCode`, `refuseConnections`, `block`, `holdAcks`, `expireMedia`,
`mediaUploads`, `routed` e `isOnline`. `test-fake.js` es un buen ejemplo de uso.

## Migrar desde zapo o Baileys

```js
const { importZapo, rollbackZapo, importBaileys } = require('soblendwa-node')

const zapo = { type: 'sqlite', path: 'auth/state.sqlite' }
const destino = { type: 'sqlite', path: 'session/auth.db' }

console.log(await importZapo(zapo, null, { check: true }))
await importZapo(zapo, destino)
await rollbackZapo(destino, zapo)

await importBaileys('./auth_info', destino)
```

Para zapo en Mongo: `{ type: 'mongo', url, database, session, collectionPrefix }`. `check: true` lo lee todo y te dice
qué se va a importar sin escribir nada. Ninguna de las dos pide volver a vincular. Apaga el bot viejo antes de arrancar
este.

## Probar en real

Vienen dos scripts para quitarle lo experimental a lo que uses:

```bash
node node_modules/soblendwa-node/store-check.js '{"type":"mongo","url":"mongodb://localhost:27017","database":"soblendwa_prueba"}'
node node_modules/soblendwa-node/live-check.js --pair 5215512345678
node node_modules/soblendwa-node/live-check.js --resumen prueba-en-vivo/live-check.jsonl
```

`store-check.js` prueba tu MongoDB, PostgreSQL, Redis o SQLite con el WhatsApp falso: guarda, reinicia y comprueba que
nada se perdió. `live-check.js` es un bot para un número de prueba, con comandos para cada función (`!ayuda`), que
anota en un reporte los mensajes sin descifrar, recuperados y perdidos, los grupos, las desconexiones y la memoria.
Con `--resumen` sale el total, también por día, para compararlo con lo que tenías antes. Los pasos completos están en
[Probar en real](https://soblend-development-studio.github.io/SoblendWA/#probar-en-real).

## Compilar desde el repo

```bash
npm install
npm run build
npm test
```

`npm run build` genera `soblendwa.<plataforma>-<arquitectura>.node` con `@napi-rs/cli`. También vale
`cargo build -p soblendwa-node --release` y copiar la biblioteca como `soblendwa.node`.

---

No estamos afiliados a WhatsApp ni a Meta, y automatizar una cuenta puede acabar en bloqueo. Lo que está probado y lo
que no está en el [README principal](https://github.com/Soblend-Development-Studio/SoblendWA#estado).

MIT © 2026 Soblend Development Studio
