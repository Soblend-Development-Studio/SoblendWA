=== node | Node.js y TypeScript | Lenguajes
lead: El motor en Rust, cargado como módulo nativo dentro del proceso de tu bot.

El cifrado Signal, el códec binario, el socket y el almacenamiento corren en Rust. JavaScript solo recibe eventos y llama métodos, así que el bot no se frena por el recolector de basura ni por el hilo de eventos de Node.js.

## Crear el cliente

```js
const { SoblendWA } = require('soblendwa-node')

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
  })

  await bot.connect()
}

main()
```

Con TypeScript es igual, y los tipos vienen en `index.d.ts`. Las opciones de `create` están en [Configuración](#configuracion-en-node-js-python-y-el-servidor).

## Eventos

`SoblendWA` es un `EventEmitter`. Cada evento se emite por su `type`, y también como `event`, y los errores de los handlers como `error`.

```js
bot.on('event', (event) => console.log(event.type))
bot.on('error', (error) => console.error(error))

async function recorrer() {
  for await (const event of bot.events()) {
    console.log(event.type)
  }
}
```

## Errores {#node-errores}

Todos los métodos fallan con un `SoblendError`, que trae `code` y, si el error viene de WhatsApp, `status`. Compara siempre `code`, nunca el texto: el texto puede cambiar entre versiones y el código no.

```js
const { SoblendError } = require('soblendwa-node')

try {
  await bot.groupJoin(enlace)
} catch (error) {
  if (!(error instanceof SoblendError)) throw error
  if (error.code === 'APPROVAL_REQUIRED') console.log('Falta que un admin apruebe')
  else if (error.code === 'NOT_FOUND') console.log('El enlace no existe')
  else console.log(error.code, error.status, error.message)
}
```

La lista completa de códigos está en [Errores](#ref-errores).

## Métodos

`chat` acepta un JID completo o solo el número. `message` acepta el objeto recibido o su id.

@m connect :: connect() :: Conecta. Devuelve el propio cliente.
@m restart :: restart() :: Desconecta y vuelve a conectar solo esta cuenta.
@m close :: close() :: Desconecta y quita los handlers, para descartar la cuenta.
@m waitFor :: waitFor(type, filter, timeout) :: Espera el siguiente evento de ese tipo que cumpla el filtro. Falla con `TIMEOUT`.
@m call :: call(method, params) :: Llama a cualquier [método del servidor](#servidor-metodos) por su nombre.
@m disconnect :: disconnect() :: Cierra la conexión y conserva la sesión.
@m logout :: logout() :: Desvincula y borra la sesión.
@m me :: bot.me :: Tu JID, o `null`.
@m connected :: bot.connected :: Si está conectado.
@m ping :: ping() :: Latencia de red en milisegundos, medida ahora.
@m stats :: stats() :: [Métricas](#confiabilidad-metricas) de la sesión. No es asíncrono.
@m pendingSends :: pendingSends() :: Envíos esperando confirmación.
@m elapsed :: elapsed(message) :: Milisegundos que lleva el mensaje dentro del bot. No es asíncrono.
@m requestPairCode :: requestPairCode(phone) :: Pide un [código de vinculación](#vincular-codigo).
@m sendText :: sendText(chat, text, { quoted, preview, mentions }) :: Envía texto, con cita, menciones y [vista previa](#vista-previa) propia.
@m reply :: reply(message, text) :: Responde citando.
@m react :: react(message, emoji) :: Reacciona.
@m edit :: edit(chat, messageId, text) :: Edita un mensaje tuyo.
@m revoke :: revoke(chat, messageId) :: Elimina para todos.
@m markPlayed :: markPlayed(message) :: Marca una nota de voz como reproducida.
@m sendImage :: sendImage(chat, data, options) :: Envía una imagen.
@m sendVideo :: sendVideo(chat, data, options) :: Envía un video.
@m sendAudio :: sendAudio(chat, data, options) :: Envía audio. Con `voiceNote: true`, como nota de voz.
@m sendDocument :: sendDocument(chat, data, options) :: Envía un documento.
@m sendSticker :: sendSticker(chat, data, options) :: Envía un sticker WebP.
@m sendMedia :: sendMedia(chat, kind, data, options) :: Envío genérico. `kind` es `image`, `video`, `audio`, `document` o `sticker`.
@m sendFile :: sendFile(chat, kind, path, options) :: Envía un archivo del disco, por bloques y con memoria constante.
@m download :: download(message) :: Descarga y descifra. Devuelve un `Buffer`.
@m downloadToFile :: downloadToFile(message, path) :: Descarga a disco por bloques y devuelve los bytes escritos.
@m sendInteractive :: sendInteractive(chat, spec) :: [Mensaje interactivo](#interactivos).
@m sendButtons :: sendButtons(chat, text, buttons, footer) :: Botones simples.
@m sendList :: sendList(chat, list) :: Lista de opciones.
@m sendPoll :: sendPoll(chat, name, options, selectable) :: Crea una encuesta.
@m votePoll :: votePoll(chat, pollId, options) :: Vota.
@m sendContacts :: sendContacts(chat, cards) :: Envía contactos, con `{ name, phone }`.
@m sendMentionAll :: sendMentionAll(chat, text) :: Menciona a todo el grupo.
@m sendGroupInvite :: sendGroupInvite(to, group, caption) :: Envía una invitación.
@m setMemberLabel :: setMemberLabel(group, label) :: Pone tu etiqueta en un grupo.
@m sendProto :: sendProto(chat, bytes) :: Envía cualquier mensaje protobuf.
@m typing :: typing(chat, state) :: `composing`, `recording` o `paused`.
@m setOnline :: setOnline(available) :: Aparece en línea o no.
@m groupInfo :: groupInfo(chat) :: Datos de un grupo.
@m joinedGroups :: joinedGroups() :: Tus grupos.
@m onWhatsApp :: onWhatsApp(...phones) :: Comprueba números.
@m profilePicture :: profilePicture(jid, fullSize) :: URL de la foto de perfil.
@m block :: block(jid) :: Bloquea.
@m unblock :: unblock(jid) :: Desbloquea.
@m modifyChat :: modifyChat(chat, action, seconds) :: [Archiva, fija, silencia y más](#chats).
@m labelChat :: labelChat(chat, labelId, labeled) :: Etiqueta un chat.
@m labelMessage :: labelMessage(chat, message, labelId, labeled) :: Etiqueta un mensaje.
@m editLabel :: editLabel(id, name, color) :: Crea o edita una etiqueta.
@m deleteLabel :: deleteLabel(id) :: Borra una etiqueta.
@m saveQuickReply :: saveQuickReply(shortcut, text) :: Guarda una respuesta rápida.
@m chats :: chats() :: Tus chats, del store de chats. No es asíncrono.
@m contacts :: contacts() :: Tus contactos. No es asíncrono.
@m messages :: messages(chat, { limit, before }) :: Mensajes de un chat. No es asíncrono.
@m search :: search(text, limit) :: Busca en los mensajes guardados. No es asíncrono.
@m botList :: botList() :: Bots de Meta AI.
@m reachoutTimelock :: reachoutTimelock() :: Si WhatsApp limitó el contacto con gente nueva.
@m newChatMessageCap :: newChatMessageCap() :: Cupo de chats nuevos.
@m disappearingModes :: disappearingModes(...jids) :: Mensajes temporales de cada chat.
@m groupMetadata :: groupMetadata(chat) :: Datos de un grupo, siempre desde el servidor.
@m groupCreate :: groupCreate(subject, participants) :: Crea un grupo.
@m groupParticipants :: groupParticipants(chat, participants, action) :: `add`, `remove`, `promote` o `demote`. Devuelve `{ jid, status, ok }` por persona.
@m groupAdd :: groupAdd(chat, participants) :: Añade gente.
@m groupKick :: groupKick(chat, participants) :: Saca gente.
@m groupPromote :: groupPromote(chat, participants) :: Da admin.
@m groupDemote :: groupDemote(chat, participants) :: Quita admin.
@m groupSubject :: groupSubject(chat, subject) :: Cambia el nombre.
@m groupDescription :: groupDescription(chat, description) :: Cambia la descripción. Con `null`, la quita.
@m groupSetting :: groupSetting(chat, setting, value) :: `announce`, `locked`, `joinApproval`, `memberAdd` o `ephemeral`.
@m groupPicture :: groupPicture(chat, data) :: Cambia la foto del grupo.
@m groupLeave :: groupLeave(chat) :: Sale del grupo.
@m groupInviteCode :: groupInviteCode(chat) :: El código de invitación.
@m groupRevokeInvite :: groupRevokeInvite(chat) :: Invalida el código y devuelve uno nuevo.
@m groupJoin :: groupJoin(invite) :: Entra con un enlace o código. Falla con `APPROVAL_REQUIRED` si pide aprobación.
@m groupInviteInfo :: groupInviteInfo(invite) :: Datos del grupo de un enlace, sin entrar.
@m groupRequests :: groupRequests(chat) :: Las solicitudes pendientes.
@m groupRespondRequests :: groupRespondRequests(chat, participants, approve) :: Aprueba o rechaza solicitudes.
@m setProfilePicture :: setProfilePicture(data, target) :: Cambia tu foto, o la de un grupo.
@m removeProfilePicture :: removeProfilePicture(target) :: Quita esa foto.
@m setAbout :: setAbout(text) :: Cambia tu información.
@m fetchAbout :: fetchAbout(...jids) :: Lee la información de varias cuentas.
@m setPushName :: setPushName(name) :: Cambia tu nombre visible.
@m businessProfile :: businessProfile(jid) :: El perfil de empresa de una cuenta, o `null`.
@m blocklist :: blocklist() :: Tus bloqueados.
@m privacySettings :: privacySettings() :: Tus ajustes de privacidad.
@m setPrivacy :: setPrivacy(setting, value) :: Cambia un ajuste de privacidad.
@m lidForPhone :: lidForPhone(phone) :: El LID de un número, o `null`.
@m phoneForLid :: phoneForLid(lid) :: El número de un LID, o `null`.

## Funciones del paquete

| Función | Qué hace |
|---|---|
| `importBaileys(source, target)` | [Migra una sesión de Baileys](#migrar-desde-baileys) |
| `importZapo(source, target, { check })` | [Migra una sesión de zapo](#migrar-desde-node), o solo la revisa con `check` |
| `rollbackZapo(source, target)` | Devuelve la sesión a zapo |
| `FakeWhatsApp` | El [WhatsApp falso](#pruebas-node) para tus pruebas |
| `SoblendError` | La clase de los errores, con `code` y `status` |
| `credits()`, `version()` | Créditos y versión |
| `NativeClient` | El cliente de bajo nivel que usa `SoblendWA` |

## Stores

Además de los [stores incluidos](#almacenamiento), puedes [escribir el tuyo en JavaScript](#almacenamiento-store-escrito-en-javascript).

=== python | Python | Lenguajes
lead: El motor en Rust como módulo nativo para asyncio, con el trabajo pesado fuera del GIL.

Los métodos son corrutinas normales de Python: se usan con `await`, sin hilos ni callbacks.

## Crear el cliente

```python
import asyncio
from soblendwa import SoblendWA


async def main():
    bot = await SoblendWA.create(store={"type": "sqlite", "path": "session/auth.db"}, push_name="Mi Bot")

    @bot.on("qr")
    def qr(event):
        print("Escanea:", event["code"])

    @bot.on("message")
    async def answer(message):
        if message["info"]["fromMe"]:
            return
        if message["text"] == "!ping":
            await bot.reply(message, f"Pong\n{bot.elapsed(message):.2f} ms")

    await bot.run()


asyncio.run(main())
```

Las opciones de `create` usan snake_case y se convierten solas. Están en [Configuración](#configuracion-en-node-js-python-y-el-servidor). Las opciones anidadas también, como `link_previews={"timeout_ms": 2000}`.

## Eventos

`@bot.on("tipo")` registra un handler, que puede ser normal o `async`. `"event"` recibe todos. `bot.run()` conecta y atiende los eventos hasta que la sesión termine.

Si prefieres manejar el bucle tú mismo:

```python
await bot.connect()
async for event in bot.events():
    print(event["type"])
```

Los eventos son diccionarios con los mismos campos que en [JSON](#ref-eventos), en camelCase, como `message["info"]["fromMe"]`. Un handler que lanza una excepción se registra con `logging` y no detiene a los demás.

Para esperar uno solo, `await bot.wait_for("connected")` o `await bot.wait_for("message", lambda m: m["text"] == "hola", timeout=30)`. Si sales de `events()` a la mitad, lo que quedaba del lote no se pierde: lo recibe la siguiente lectura.

## Errores

Los métodos fallan con `SoblendError`, que trae `code`. Compara el código, no el texto.

```python
from soblendwa import SoblendError

try:
    await bot.group_join(enlace)
except SoblendError as error:
    if error.code == "APPROVAL_REQUIRED":
        print("Falta que un admin apruebe")
```

## Métodos

`chat` acepta un JID completo o solo el número. `message` acepta el diccionario recibido o su id.

@m connect :: connect() :: Conecta.
@m run :: run() :: Conecta y atiende los eventos.
@m disconnect :: disconnect() :: Cierra la conexión y conserva la sesión.
@m logout :: logout() :: Desvincula y borra la sesión.
@m me :: bot.me :: Tu JID, o `None`.
@m connected :: bot.connected :: Si está conectado.
@m ping :: ping() :: Latencia de red en milisegundos.
@m stats :: stats() :: [Métricas](#confiabilidad-metricas). No es corrutina.
@m pending_sends :: pending_sends() :: Envíos esperando confirmación.
@m elapsed :: elapsed(message) :: Milisegundos que lleva el mensaje dentro del bot. No es corrutina.
@m request_pair_code :: request_pair_code(phone) :: Pide un [código de vinculación](#vincular-codigo).
@m send_text :: send_text(chat, text, quoted=None, preview=None) :: Envía texto.
@m reply :: reply(message, text) :: Responde citando.
@m react :: react(message, emoji) :: Reacciona.
@m edit :: edit(chat, message_id, text) :: Edita un mensaje tuyo.
@m revoke :: revoke(chat, message_id) :: Elimina para todos.
@m mark_played :: mark_played(message) :: Marca una nota de voz como reproducida.
@m send_image :: send_image(chat, data, **options) :: Envía una imagen.
@m send_video :: send_video(chat, data, **options) :: Envía un video.
@m send_audio :: send_audio(chat, data, **options) :: Envía audio. Con `voice_note=True`, como nota de voz.
@m send_document :: send_document(chat, data, **options) :: Envía un documento.
@m send_sticker :: send_sticker(chat, data, **options) :: Envía un sticker WebP.
@m send_media :: send_media(chat, kind, data, quoted=None, **options) :: Envío genérico.
@m send_file :: send_file(chat, kind, path, quoted=None, **options) :: Envía un archivo del disco, por bloques.
@m download :: download(message) :: Descarga y descifra. Devuelve `bytes`.
@m download_to_file :: download_to_file(message, path) :: Descarga a disco y devuelve los bytes escritos.
@m send_interactive :: send_interactive(chat, body, buttons, quoted=None, **header) :: [Mensaje interactivo](#interactivos). `header` admite `title`, `subtitle` y `footer`.
@m send_buttons :: send_buttons(chat, text, buttons, footer=None) :: Botones simples, como pares `(id, texto)`.
@m send_list :: send_list(chat, title, text, button_text, sections, footer=None) :: Lista de opciones.
@m send_poll :: send_poll(chat, name, options, selectable=1) :: Crea una encuesta.
@m vote_poll :: vote_poll(chat, poll_id, options) :: Vota.
@m send_contacts :: send_contacts(chat, cards) :: Envía contactos, como pares `(nombre, teléfono)`.
@m send_mention_all :: send_mention_all(chat, text) :: Menciona a todo el grupo.
@m send_group_invite :: send_group_invite(to, group, caption=None) :: Envía una invitación.
@m set_member_label :: set_member_label(group, label) :: Pone tu etiqueta en un grupo.
@m send_proto :: send_proto(chat, proto) :: Envía cualquier mensaje protobuf.
@m typing :: typing(chat, state="composing") :: `composing`, `recording` o `paused`.
@m set_online :: set_online(available=True) :: Aparece en línea o no.
@m group_info :: group_info(chat) :: Datos de un grupo.
@m joined_groups :: joined_groups() :: Tus grupos.
@m on_whatsapp :: on_whatsapp(*phones) :: Comprueba números.
@m profile_picture :: profile_picture(jid, full_size=False) :: URL de la foto de perfil.
@m block :: block(jid) :: Bloquea.
@m unblock :: unblock(jid) :: Desbloquea.
@m modify_chat :: modify_chat(chat, action, seconds=None) :: [Archiva, fija, silencia y más](#chats).
@m label_chat :: label_chat(chat, label_id, labeled=True) :: Etiqueta un chat.
@m label_message :: label_message(chat, message, label_id, labeled=True) :: Etiqueta un mensaje.
@m edit_label :: edit_label(id, name, color=0) :: Crea o edita una etiqueta.
@m delete_label :: delete_label(id) :: Borra una etiqueta.
@m save_quick_reply :: save_quick_reply(shortcut, text) :: Guarda una respuesta rápida.
@m chats :: chats() :: Tus chats, del store de chats. No es corrutina.
@m contacts :: contacts() :: Tus contactos. No es corrutina.
@m messages :: messages(chat, limit=50, before=None) :: Mensajes de un chat. No es corrutina.
@m search :: search(text, limit=50) :: Busca en los mensajes guardados. No es corrutina.
@m bot_list :: bot_list() :: Bots de Meta AI.
@m reachout_timelock :: reachout_timelock() :: Si WhatsApp limitó el contacto con gente nueva.
@m new_chat_message_cap :: new_chat_message_cap() :: Cupo de chats nuevos.
@m disappearing_modes :: disappearing_modes(*jids) :: Mensajes temporales de cada chat.
@m restart :: restart() :: Desconecta y vuelve a conectar solo esta cuenta.
@m call :: call(method, **params) :: Llama a cualquier [método del servidor](#servidor-metodos) por su nombre.
@m group_metadata :: group_metadata(chat) :: Datos de un grupo, siempre desde el servidor.
@m group_create :: group_create(subject, participants) :: Crea un grupo.
@m group_participants :: group_participants(chat, participants, action) :: `add`, `remove`, `promote` o `demote`.
@m group_add :: group_add(chat, participants) :: Añade gente.
@m group_kick :: group_kick(chat, participants) :: Saca gente.
@m group_promote :: group_promote(chat, participants) :: Da admin.
@m group_demote :: group_demote(chat, participants) :: Quita admin.
@m group_subject :: group_subject(chat, subject) :: Cambia el nombre.
@m group_description :: group_description(chat, description=None) :: Cambia la descripción.
@m group_setting :: group_setting(chat, setting, value) :: `announce`, `locked`, `joinApproval`, `memberAdd` o `ephemeral`.
@m group_picture :: group_picture(chat, data) :: Cambia la foto del grupo.
@m group_leave :: group_leave(chat) :: Sale del grupo.
@m group_invite_code :: group_invite_code(chat) :: El código de invitación.
@m group_revoke_invite :: group_revoke_invite(chat) :: Invalida el código y devuelve uno nuevo.
@m group_join :: group_join(invite) :: Entra con un enlace. Falla con `APPROVAL_REQUIRED` si pide aprobación.
@m group_invite_info :: group_invite_info(invite) :: Datos del grupo de un enlace, sin entrar.
@m group_requests :: group_requests(chat) :: Las solicitudes pendientes.
@m group_respond_requests :: group_respond_requests(chat, participants, approve=True) :: Aprueba o rechaza solicitudes.
@m set_profile_picture :: set_profile_picture(data, target=None) :: Cambia tu foto, o la de un grupo.
@m remove_profile_picture :: remove_profile_picture(target=None) :: Quita esa foto.
@m set_about :: set_about(text) :: Cambia tu información.
@m fetch_about :: fetch_about(*jids) :: Lee la información de varias cuentas.
@m set_push_name :: set_push_name(name) :: Cambia tu nombre visible.
@m business_profile :: business_profile(jid) :: El perfil de empresa de una cuenta, o `None`.
@m blocklist :: blocklist() :: Tus bloqueados.
@m privacy_settings :: privacy_settings() :: Tus ajustes de privacidad.
@m set_privacy :: set_privacy(setting, value) :: Cambia un ajuste de privacidad.
@m lid_for_phone :: lid_for_phone(phone) :: El LID de un número, o `None`.
@m phone_for_lid :: phone_for_lid(lid) :: El número de un LID, o `None`.

## Funciones del paquete

| Función | Qué hace |
|---|---|
| `import_baileys(source, **target)` | [Migra una sesión de Baileys](#migrar-desde-baileys). Por defecto, a la carpeta `session` |
| `SoblendError` | La clase de los errores, con `code` |
| `credits()` | Créditos |
| `__version__`, `__author__` | Versión y autor |
| `NativeClient` | El cliente de bajo nivel que usa `SoblendWA` |

=== cliente-js | Cliente JavaScript | Lenguajes
lead: `soblendwa-client` habla con el servidor por WebSocket. Sin nada nativo, sin dependencias y con tipos.

Funciona en Node.js 18+, Bun, Deno y el navegador. En Node.js 22+, Bun y Deno usa el WebSocket incluido; en Node.js 18 a 21 instala `ws`.

## Conectar

```js
import { connect } from 'soblendwa-client'

const bot = await connect('ws://127.0.0.1:8787', { session: 'ventas', token: 'mi-clave' })

bot.on('message', async (message) => {
  if (message.info.fromMe) return
  if (message.text === '!ping') {
    const ms = await bot.elapsed(message)
    await bot.reply(message, `Pong\n${ms.toFixed(2)} ms`)
  }
})
```

| Opción | Por defecto | Para qué |
|---|---|---|
| `session` | `default` | La cuenta. El servidor la abre si no existe |
| `token` | Ninguno | La clave del servidor |
| `reconnect` | `true` | Reconectar solo |
| `timeout` | 120.000 ms | Tiempo máximo de cada llamada |

Si te desconectas del servidor, el cliente se reconecta solo, con una espera creciente de hasta 30 segundos. WhatsApp sigue conectado en el servidor mientras tanto, así que no se pierde la sesión.

## Eventos

```js
bot.on('qr', ({ code }) => console.log(code))
bot.on('connected', () => console.log('En línea'))
bot.on('lagged', (missed) => console.warn('Perdí', missed, 'eventos'))
bot.on('error', (error) => console.error(error))

async function recorrer() {
  for await (const event of bot.events()) {
    console.log(event.type)
  }
}
```

Además de los [eventos de WhatsApp](#ref-eventos), el cliente emite `open`, `close`, `lagged` y `error`. `lagged` avisa de que tu bot iba tan lento que el servidor descartó eventos.

## Varias cuentas

Cada sesión es una cuenta distinta, y el servidor las maneja todas a la vez.

```js
const ventas = await connect(url, { session: 'ventas', token })
const soporte = await connect(url, { session: 'soporte', token })
```

## Métodos

Los mismos que el [módulo de Node.js](#node), incluidas las acciones de grupo, el perfil y `lidForPhone`, con todo asíncrono porque cada llamada viaja al servidor. También tiene `status()`, `read(message)`, `restart()`, `waitFor()`, `close()` y `call(método, parámetros)` para cualquier otro.

Los errores son `SoblendError` con `code` y `status`, igual que en Node.js. Si la conexión al servidor se corta, las llamadas pendientes fallan con `NOT_CONNECTED`, y las que tardan más de `timeout` con `TIMEOUT`.

```js
const estado = await bot.status()
await bot.read(message)
const resultado = await bot.call('sendText', { chat: '5215512345678', text: 'hola' })
bot.close()
```

`sendMedia` y compañía mandan los bytes por el WebSocket, en base64. `sendFile` y `downloadToFile` usan rutas **del servidor** y son la opción para archivos grandes. `bytes` puede ser un `Uint8Array`, un `ArrayBuffer` o un texto en base64.

=== servidor | Servidor | Lenguajes
lead: `soblendwa-server` corre el motor y expone WhatsApp por WebSocket, HTTP y webhooks, para cualquier lenguaje.

Un solo servidor maneja varias cuentas a la vez, y usó unos 14 MB de RAM con dos cuentas contra el WhatsApp falso.

## Arrancarlo

```bash Terminal
soblendwa-server --token mi-clave --session ventas --session soporte
```

La primera vez muestra el QR de cada cuenta en la terminal. Para reutilizar una sesión que ya vinculaste con el bot de ejemplo, usa `--dir . --session session`.

:::danger Protege el servidor
Quien pueda hablar con él controla tu WhatsApp. Por defecto solo acepta conexiones locales. Si usas `--host 0.0.0.0`, pon siempre `--token` y un proxy con HTTPS delante.
:::

## Opciones

| Opción | Variable de entorno | Por defecto | Para qué |
|---|---|---|---|
| `--host` | `SOBLENDWA_HOST` | `127.0.0.1` | Dónde escucha |
| `--port` | `SOBLENDWA_PORT` | `8787` | Puerto |
| `--token` | `SOBLENDWA_TOKEN` | Ninguno | Clave obligatoria para WebSocket, HTTP y webhooks |
| `--dir` | `SOBLENDWA_DIR` | `sessions` | Una carpeta por cuenta |
| `--sqlite` | `SOBLENDWA_SQLITE` | — | Una base SQLite para todas las cuentas |
| `--mongo` | `SOBLENDWA_MONGO` | — | MongoDB |
| `--mongo-db` | `SOBLENDWA_MONGO_DB` | `soblendwa` | Nombre de la base de MongoDB |
| `--memory` | — | — | Todo en memoria |
| `--session` | — | `default` | Cuentas que se abren al arrancar. Se repite. Las demás se abren al primer uso |
| `--webhook` | `SOBLENDWA_WEBHOOK` | Ninguno | Envía cada evento por POST a tu URL |
| `--proxy` | `SOBLENDWA_PROXY` | Ninguno | Proxy HTTP o SOCKS5 |
| `--push-name` | `SOBLENDWA_PUSH_NAME` | Ninguno | Nombre visible |
| `--no-chat-store` | — | — | Desactiva el store de chats |
| `--no-link-previews` | — | — | Desactiva las vistas previas |
| `--fake` | — | — | Usa el [WhatsApp falso](#pruebas-falso) |
| `--options` | `SOBLENDWA_OPTIONS` | Ninguno | Opciones del cliente para todas las cuentas, como JSON o `@archivo.json`. Son las mismas de [Node.js y Python](#configuracion-en-node-js-python-y-el-servidor); el almacenamiento lo sigue eligiendo el servidor |

El nombre de una sesión tiene de 1 a 64 caracteres: letras, números, `-` y `_`. Los registros se ajustan con `RUST_LOG`.

## Autenticación

Con `--token`, cada petición debe llevar la clave de una de estas formas:

- La cabecera `Authorization: Bearer mi-clave`.
- El parámetro `?token=mi-clave`.

Sin clave o con una incorrecta, responde `401` con `{"error":{"message":"missing or wrong token"}}`.

## Rutas HTTP

| Ruta | Qué hace |
|---|---|
| `GET /` | Nombre, versión, sesiones y métodos disponibles. No pide clave |
| `GET /sessions` | Las cuentas abiertas |
| `GET /metrics` | Las métricas de todas las cuentas para [Prometheus](#confiabilidad-prometheus) |
| `GET /api/<cuenta>` | El estado de esa cuenta, con el QR pendiente si lo hay. La abre si no existe |
| `POST /api/<cuenta>/<método>` | Llama a un método, con los parámetros como JSON en el cuerpo |
| `GET /ws/<cuenta>` | WebSocket de esa cuenta |
| `GET /ws` | WebSocket de la cuenta `default` |

```bash Enviar un texto
curl -X POST http://127.0.0.1:8787/api/ventas/sendText \
  -H "Authorization: Bearer mi-clave" \
  -d '{"chat":"5215512345678","text":"Hola desde curl"}'
```

| Respuesta | Cuándo |
|---|---|
| `200` con `{"result": …}` | Funcionó |
| `400` con `{"error":{"message":…}}` | El cuerpo no es JSON o el nombre de la cuenta no es válido |
| `401` | Falta la clave o está mal |
| `422` con `{"error":{"message":…,"code":…,"status":…}}` | El método falló. `code` es el [código del error](#ref-errores) y `status` el de WhatsApp, si lo hay |

## Protocolo WebSocket

Al conectar a `ws://host:8787/ws/<cuenta>?token=…` llega primero el saludo y, si hay un QR pendiente, el evento `qr`.

```json Saludo
{ "type": "hello", "session": "ventas", "me": "5215512345678:3@s.whatsapp.net", "connected": true, "methods": ["status", "sendText"] }
```

Después llegan los eventos:

```json Evento
{ "session": "ventas", "event": { "type": "message", "text": "hola" } }
```

Para llamar un método, envía un `id` tuyo, y la respuesta trae el mismo. Las llamadas se atienden en paralelo, así que las respuestas pueden llegar en otro orden.

```json Llamada
{ "id": 1, "method": "sendText", "params": { "chat": "5215512345678", "text": "hola" } }
```

```json Respuesta correcta
{ "id": 1, "result": "3EB0ABC123" }
```

```json Respuesta con error
{ "id": 1, "error": { "message": "invalid argument: missing parameter \"text\"", "code": "INVALID_ARGUMENT", "status": null } }
```

Si un cliente va tan lento que el servidor descarta eventos, recibe `{"type":"lagged","missed":N}`. El búfer guarda hasta 4.096 eventos por cuenta.

## Webhooks

Con `--webhook`, cada evento se envía por POST a tu URL, con el mismo JSON que en el WebSocket y un límite de 10 segundos. Si hay `--token`, va en la cabecera `Authorization`, y conviene que lo revises para saber que el aviso viene de tu servidor. Si la entrega falla, se registra en los logs y no se reintenta.

```js Recibir webhooks en Node.js
const http = require('node:http')

http.createServer((request, response) => {
  if (request.headers.authorization !== 'Bearer mi-clave') {
    response.writeHead(401).end()
    return
  }
  let body = ''
  request.on('data', (chunk) => (body += chunk))
  request.on('end', () => {
    const { session, event } = JSON.parse(body)
    console.log(session, event.type)
    response.writeHead(200).end()
  })
}).listen(3000)
```

## Métodos disponibles {#servidor-metodos}

Son los mismos por WebSocket y por HTTP. `chat` acepta un JID o solo el número. `messageId` es el id de un mensaje reciente o guardado en el store de chats. Los bytes viajan en base64.

| Método | Parámetros | Devuelve |
|---|---|---|
| `status` | — | `{ me, connected }` |
| `connect`, `disconnect`, `logout` | — | — |
| `ping` | — | Milisegundos |
| `stats` | — | [Métricas](#confiabilidad-metricas) |
| `pendingSends` | — | `[{ id, chat, queuedAt, attempts }]` |
| `elapsed` | `messageId` | Milisegundos |
| `requestPairCode` | `phone` | El código |
| `sendText` | `chat`, `text`, `quotedId?`, `preview?`, `mentions?` | id |
| `reply` | `messageId`, `text` | id |
| `react` | `messageId`, `emoji` | id |
| `read`, `markPlayed` | `messageId` | — |
| `sendMedia` | `chat`, `kind`, `data`, y las [opciones de multimedia](#multimedia-opciones), con `quotedId?` | id |
| `sendFile` | `chat`, `kind`, `path`, y las mismas opciones | id |
| `download` | `messageId` | `{ base64, size }` |
| `downloadToFile` | `messageId`, `path` | Bytes escritos |
| `sendPoll` | `chat`, `name`, `options`, `selectable?` | id |
| `votePoll` | `chat`, `pollId`, `options` | id |
| `edit` | `chat`, `messageId`, `text` | id |
| `revoke` | `chat`, `messageId` | id |
| `typing` | `chat`, `state?` | — |
| `setOnline` | `available?` | — |
| `sendInteractive` | `chat`, `body`, `buttons`, `title?`, `subtitle?`, `footer?`, `quotedId?` | id |
| `sendButtons` | `chat`, `text`, `buttons`, `footer?` | id |
| `sendList` | `chat`, `title`, `text`, `buttonText`, `sections`, `footer?` | id |
| `sendContacts` | `chat`, `cards` | id |
| `sendMentionAll` | `chat`, `text` | id |
| `sendGroupInvite` | `to`, `group`, `caption?` | id |
| `setMemberLabel` | `group`, `label` | id |
| `sendProto` | `chat`, `proto` | id |
| `groupInfo` | `chat` | Datos del grupo |
| `joinedGroups` | — | Lista de grupos |
| `onWhatsApp` | `phones` | `[{ query, jid, exists, lid }]` |
| `profilePicture` | `jid`, `fullSize?` | URL o `null` |
| `block`, `unblock` | `jid` | — |
| `modifyChat` | `chat`, `action`, `seconds?` | — |
| `labelChat` | `chat`, `labelId`, `labeled?` | — |
| `labelMessage` | `chat`, `messageId`, `labelId`, `labeled?` | — |
| `editLabel` | `id`, `name`, `color?` | — |
| `deleteLabel` | `id` | — |
| `saveQuickReply` | `shortcut`, `text` | id |
| `chats`, `contacts` | — | Lista |
| `messages` | `chat`, `limit?`, `before?` | Lista |
| `search` | `text`, `limit?` | Lista |
| `botList`, `reachoutTimelock`, `newChatMessageCap` | — | Datos de la cuenta |
| `disappearingModes` | `jids` | `[{ jid, seconds, setAt }]` |
| `restart` | — | — |
| `groupMetadata` | `chat` | Datos del grupo |
| `groupCreate` | `subject`, `participants?` | Datos del grupo |
| `groupParticipants` | `chat`, `participants`, `action` | `[{ jid, status, ok }]` |
| `groupSubject` | `chat`, `subject` | — |
| `groupDescription` | `chat`, `description?` | — |
| `groupSetting` | `chat`, `setting`, `value` | — |
| `groupPicture` | `chat`, `data` | — |
| `groupLeave` | `chat` | — |
| `groupInviteCode`, `groupRevokeInvite` | `chat` | El código |
| `groupJoin` | `invite` | JID del grupo |
| `groupInviteInfo` | `invite` | Datos del grupo |
| `groupRequests` | `chat` | `[{ jid, method, requestedAt }]` |
| `groupRespondRequests` | `chat`, `participants`, `approve?` | `[{ jid, status, ok }]` |
| `setProfilePicture` | `data`, `target?` | — |
| `removeProfilePicture` | `target?` | — |
| `setAbout` | `text` | — |
| `fetchAbout` | `jids` | `[{ jid, text, setAt }]` |
| `setPushName` | `name` | — |
| `businessProfile` | `jid` | Perfil o `null` |
| `blocklist` | — | Lista de JID |
| `privacySettings` | — | Ajustes |
| `setPrivacy` | `setting`, `value` | — |
| `lidForPhone` | `phone` | LID o `null` |
| `phoneForLid` | `lid` | Número o `null` |

## Ejemplos en otros lenguajes

:::tabs
```python Python
import requests

requests.post(
    "http://127.0.0.1:8787/api/ventas/sendText",
    headers={"Authorization": "Bearer mi-clave"},
    json={"chat": "5215512345678", "text": "Hola"},
)
```
```php PHP
<?php
$context = stream_context_create(["http" => [
    "method" => "POST",
    "header" => "Authorization: Bearer mi-clave\r\nContent-Type: application/json",
    "content" => json_encode(["chat" => "5215512345678", "text" => "Hola"]),
]]);
echo file_get_contents("http://127.0.0.1:8787/api/ventas/sendText", false, $context);
```
```go Go
body := strings.NewReader(`{"chat":"5215512345678","text":"Hola"}`)
request, _ := http.NewRequest("POST", "http://127.0.0.1:8787/api/ventas/sendText", body)
request.Header.Set("Authorization", "Bearer mi-clave")
response, err := http.DefaultClient.Do(request)
```
```csharp C#
var http = new HttpClient();
http.DefaultRequestHeaders.Add("Authorization", "Bearer mi-clave");
var body = new StringContent("{\"chat\":\"5215512345678\",\"text\":\"Hola\"}");
var response = await http.PostAsync("http://127.0.0.1:8787/api/ventas/sendText", body);
```
:::
