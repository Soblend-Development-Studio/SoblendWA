# soblendwa-client

Cliente de JavaScript y TypeScript para el servidor de
[SoblendWA](https://github.com/Soblend-Development-Studio/SoblendWA). Hecho por **Soblend Development Studio**.

El motor (Rust) corre en `soblendwa-server`; tu bot solo habla con él por WebSocket. No compila nada nativo, no tiene
dependencias y funciona en Node 18+, Bun, Deno y el navegador. En Node 22+, Bun y Deno usa el WebSocket incluido; en
Node 18–21 instala `ws`.

```bash
npm install soblendwa-client
```

## 1. Arranca el servidor

Descarga el binario de tu plataforma en
[Releases](https://github.com/Soblend-Development-Studio/SoblendWA/releases) y córrelo:

```text
./soblendwa-server --token mi-clave
```

O, si tienes Rust y el repositorio: `cargo run --release -p soblendwa-server -- --token mi-clave`.

La primera vez muestra un QR en la terminal: escanéalo en **WhatsApp > Dispositivos vinculados**.

Para probar sin ninguna cuenta: `./soblendwa-server --fake --session bot --session ana` usa un WhatsApp falso con el
protocolo real, y nada sale de tu máquina.

## 2. Escribe tu bot

```js
import { connect } from 'soblendwa-client'

const bot = await connect('ws://127.0.0.1:8787', { token: 'mi-clave' })

bot.on('message', async (message) => {
  if (message.info.fromMe) return
  if (message.text === '!ping') {
    const ms = await bot.elapsed(message)
    await bot.reply(message, `Pong\n${ms.toFixed(2)} ms`)
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

Con TypeScript es igual: los tipos vienen incluidos.

Si te desconectas del servidor, el cliente se reconecta solo. WhatsApp sigue conectado en el servidor mientras tanto,
así que no se pierde la sesión.

## Varias cuentas

Cada sesión es una cuenta de WhatsApp distinta y el servidor las maneja todas a la vez:

```js
const ventas = await connect(url, { session: 'ventas', token })
const soporte = await connect(url, { session: 'soporte', token })
```

## Vista previa de enlaces

Los enlaces de un texto llevan vista previa sola. Para quitarla o escribirla a mano:

```js
await bot.sendText(chat, texto, { preview: false })
await bot.sendText(chat, texto, { preview: { title: 'Mi título', description: 'Escrito a mano' } })
```

## Métodos

Los mismos que el módulo nativo `soblendwa`: `sendText`, `reply`, `react`, `read`, `sendImage`, `sendVideo`,
`sendAudio`, `sendDocument`, `sendSticker`, `sendFile`, `download`, `downloadToFile`, `sendPoll`, `votePoll`, `edit`,
`revoke`, `typing`, `setOnline`, `sendInteractive`, `sendButtons`, `sendList`, `sendContacts`, `sendMentionAll`,
`sendGroupInvite`, `setMemberLabel`, `sendProto`, `groupInfo`, `joinedGroups`, `onWhatsApp`, `profilePicture`, `block`,
`unblock`, `modifyChat`, `labelChat`, `labelMessage`, `editLabel`, `deleteLabel`, `saveQuickReply`, `chats`, `contacts`,
`messages`, `search`, `botList`, `reachoutTimelock`, `newChatMessageCap`, `disappearingModes`, `stats`, `pendingSends`,
`ping`, `elapsed`, `requestPairCode` y `logout`. Para cualquier otro, `bot.call(método, parámetros)`.

`sendMedia` y compañía mandan los bytes por el WebSocket. `sendFile` y `downloadToFile` usan rutas **del servidor** y
son la opción para archivos grandes.

## Aviso

SoblendWA es independiente y no está afiliado a WhatsApp ni a Meta. Automatizar una cuenta puede ir contra sus Términos
de Servicio y provocar un bloqueo. Lee
[qué está probado](https://github.com/Soblend-Development-Studio/SoblendWA#estado).

MIT © 2026 Soblend Development Studio
