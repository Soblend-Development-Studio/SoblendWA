<img src="https://raw.githubusercontent.com/Soblend-Development-Studio/SoblendWA/main/assets/icon.svg" width="64" alt="" align="right">

# soblendwa-client

Cliente de JavaScript y TypeScript para el servidor de
[SoblendWA](https://github.com/Soblend-Development-Studio/SoblendWA). De Soblend Development Studio.

El motor corre en `soblendwa-server` y tu bot solo le habla por WebSocket. Nada nativo que compilar, cero dependencias.
Funciona en Node 18+, Bun, Deno y el navegador. En Node 22+, Bun y Deno usa el WebSocket que ya traen; en Node 18 a 21
instala también `ws`.

```bash
npm install soblendwa-client
```

## Primero, el servidor

Baja el binario de tu plataforma de [Releases](https://github.com/Soblend-Development-Studio/SoblendWA/releases):

```bash
./soblendwa-server --token mi-clave
```

O con Rust y el repo: `cargo run --release -p soblendwa-server -- --token mi-clave`. La primera vez sale un QR en la
terminal para escanear desde Dispositivos vinculados.

Para probar sin cuenta: `./soblendwa-server --fake --session bot --session ana`. Usa un WhatsApp falso con el protocolo
real y nada sale de tu máquina.

## Después, el bot

```js
import { connect } from 'soblendwa-client'

const bot = await connect('ws://127.0.0.1:8787', { token: 'mi-clave' })

bot.on('message', async (message) => {
  if (message.info.fromMe) return
  if (message.text === '!ping') {
    const ms = await bot.elapsed(message)
    await bot.reply(message, `Pong ${ms.toFixed(2)} ms`)
  }
  if (message.text === '!menu') {
    await bot.sendInteractive(message.info.chat, {
      body: '¿Qué quieres hacer?',
      buttons: [
        { type: 'reply', id: 'ping', text: 'Medir latencia' },
        { type: 'url', text: 'Rust', url: 'https://www.rust-lang.org' },
      ],
    })
  }
  if (message.reply?.id === 'ping') await bot.reply(message, 'Pong')
})
```

Los tipos de TypeScript vienen incluidos.

Si se cae la conexión con el servidor, el cliente reconecta solo. WhatsApp sigue conectado del lado del servidor, así
que la sesión no se pierde.

Cada sesión es una cuenta distinta y el servidor las lleva todas a la vez:

```js
const ventas = await connect(url, { session: 'ventas', token })
const soporte = await connect(url, { session: 'soporte', token })
```

## Métodos

Son los mismos que en `soblendwa-node`.

- Mensajes: `sendText(chat, text, { quoted, mentions, preview })`, `reply`, `react`, `read`, `edit`, `revoke`,
  `sendImage`, `sendVideo`, `sendAudio`, `sendDocument`, `sendSticker`, `sendMedia`, `sendPoll`, `votePoll`,
  `sendContacts`, `sendMentionAll`, `sendGroupInvite`, `sendInteractive`, `sendButtons`, `sendList`, `sendProto`,
  `typing`, `setOnline`, `markPlayed`
- Multimedia: `download`, y `sendFile` / `downloadToFile`, que usan rutas del servidor y son lo mejor para archivos
  grandes
- Grupos: `groupInfo`, `groupMetadata`, `joinedGroups`, `groupCreate`, `groupAdd`, `groupKick`, `groupPromote`,
  `groupDemote`, `groupParticipants`, `groupSubject`, `groupDescription`, `groupSetting`, `groupPicture`, `groupLeave`,
  `groupInviteCode`, `groupRevokeInvite`, `groupInviteInfo`, `groupJoin`, `groupRequests`, `groupRespondRequests`,
  `setMemberLabel`
- Perfil: `profilePicture`, `setProfilePicture`, `removeProfilePicture`, `setAbout`, `fetchAbout`, `setPushName`,
  `businessProfile`, `block`, `unblock`, `blocklist`, `privacySettings`, `setPrivacy`
- Contactos: `onWhatsApp`, `lidForPhone`, `phoneForLid`
- Chats: `chats`, `contacts`, `messages`, `search`, `modifyChat`, `labelChat`, `labelMessage`, `editLabel`,
  `deleteLabel`, `saveQuickReply`
- Sesión: `status`, `restart`, `logout`, `ping`, `elapsed`, `stats`, `pendingSends`, `requestPairCode`, `close`

Para lo que no esté en la lista: `bot.call(método, parámetros)`.

Con `preview: false` el enlace sale sin vista previa, y con `preview: { title, description }` la escribes tú.

## Errores

Lo que falla se rechaza con un `SoblendError` que trae `code` y, si vino de WhatsApp, `status`:

```js
import { SoblendError } from 'soblendwa-client'

try {
  await bot.groupJoin(link)
} catch (error) {
  if (error instanceof SoblendError && error.code === 'APPROVAL_REQUIRED') console.log('pide aprobación')
  else throw error
}
```

`NOT_CONNECTED` y `TIMEOUT` salen del propio cliente; los demás (`APPROVAL_REQUIRED`, `PRIVATE_PHOTO`, `FORBIDDEN`,
`RATE_LIMITED`, …) vienen del servidor.

---

No estamos afiliados a WhatsApp ni a Meta, y automatizar una cuenta puede acabar en bloqueo. Lo que está probado y lo
que no: [README principal](https://github.com/Soblend-Development-Studio/SoblendWA#estado).

MIT © 2026 Soblend Development Studio
