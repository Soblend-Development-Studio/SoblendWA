=== grupos | Grupos | Cuentas y chats
lead: Crear grupos, gestionar participantes, cambiar ajustes y usar invitaciones.

## Consultar

:::tabs
```rust Rust
let info = client.group_info(&grupo).await?;
let actualizado = client.group_metadata(&grupo).await?;
let todos = client.joined_groups().await?;
```
```js Node.js
const info = await bot.groupInfo(grupo)
const todos = await bot.joinedGroups()
```
```python Python
info = await bot.group_info(grupo)
todos = await bot.joined_groups()
```
```js Cliente JS
const info = await bot.groupInfo(grupo)
const todos = await bot.joinedGroups()
```
:::

`group_info` responde desde la caché al instante, y `group_metadata` siempre consulta al servidor. La caché se actualiza sola cuando WhatsApp avisa de un cambio. En Node.js y Python, `groupMetadata` y `group_metadata` hacen lo mismo.

## Cuando el bot entra o sale de un grupo {#grupos-entrar-salir}

`JoinedGroup` llega cuando el bot entra a un grupo, sin importar cómo: lo crearon con el bot dentro, lo añadieron después o entró con un enlace. `via` dice cuál fue. Es el evento para enviar un saludo.

:::tabs
```rust Rust
use soblendwa::events::JoinVia;

match event {
    Event::JoinedGroup { group, author, via } => {
        client.send_text(&group, "¡Hola! Escribe !menu para ver lo que hago").await?;
        println!("entré a {group} ({via:?}), por {author:?}");
    }
    Event::LeftGroup { group, removed, .. } => println!("salí de {group}, me sacaron: {removed}"),
    _ => {}
}
```
```js Node.js
bot.on('joinedGroup', async ({ group, via }) => {
  await bot.sendText(group, '¡Hola! Escribe !menu para ver lo que hago')
})
bot.on('leftGroup', ({ group, removed }) => console.log('salí de', group, removed ? '(me sacaron)' : ''))
```
```python Python
@bot.on("joinedGroup")
async def greet(event):
    await bot.send_text(event["group"], "¡Hola! Escribe !menu para ver lo que hago")
```
```js Cliente JS
bot.on('joinedGroup', async ({ group }) => {
  await bot.sendText(group, '¡Hola! Escribe !menu para ver lo que hago')
})
```
:::

| `via` | Cómo entró |
|---|---|
| `created` | Alguien creó el grupo con el bot dentro, o lo creó el propio bot |
| `added` | Un administrador lo añadió |
| `invite` | Entró con un enlace de invitación |

El aviso llega una sola vez por grupo, aunque WhatsApp envíe varios. `LeftGroup` trae `removed: true` si un administrador lo sacó, y `false` si salió el propio bot.

## Revisar si alguien es admin

`is_admin` acepta el número o el LID de la persona, así que funciona en grupos con LID.

```rust
let grupo_info = client.group_info(msg.chat()).await?;
if grupo_info.is_admin(msg.sender()) {
    client.reply(&msg, "Eres administrador").await?;
}
```

| Método de `GroupInfo` | Qué devuelve |
|---|---|
| `is_admin(jid)` | Si esa persona es administradora |
| `participant(jid)` | El participante, buscando por número o por LID |
| `admins()` | Los administradores |
| `is_lid_addressed()` | Si el grupo usa LID |

## Crear y gestionar

Las mismas acciones están en todos los lenguajes.

:::tabs
```rust Rust
use soblendwa::ParticipantAction;

let grupo = Jid::group("120363000000000001");
client.update_participants(&grupo, &[Jid::phone("5215522222222")], ParticipantAction::Remove).await?;
client.update_participants(&grupo, &[Jid::phone("5215533333333")], ParticipantAction::Promote).await?;
client.set_group_subject(&grupo, "Equipo").await?;
client.set_group_announce(&grupo, true).await?;
```
```js Node.js
const grupo = '120363000000000001@g.us'
await bot.groupKick(grupo, '5215522222222')
await bot.groupPromote(grupo, '5215533333333')
await bot.groupDemote(grupo, '5215533333333')
await bot.groupAdd(grupo, ['5215544444444', '5215555555555'])
await bot.groupSubject(grupo, 'Equipo')
await bot.groupDescription(grupo, 'Reglas en el mensaje fijado')
await bot.groupSetting(grupo, 'announce', true)
```
```python Python
grupo = "120363000000000001@g.us"
await bot.group_kick(grupo, "5215522222222")
await bot.group_promote(grupo, "5215533333333")
await bot.group_add(grupo, ["5215544444444", "5215555555555"])
await bot.group_subject(grupo, "Equipo")
await bot.group_setting(grupo, "announce", True)
```
```js Cliente JS
const grupo = '120363000000000001@g.us'
await bot.groupKick(grupo, '5215522222222')
await bot.groupPromote(grupo, '5215533333333')
await bot.groupSubject(grupo, 'Equipo')
await bot.groupSetting(grupo, 'announce', true)
```
:::

Cada acción sobre participantes devuelve una lista con `jid`, `status` y `ok`. Si una persona no se pudo añadir o sacar, su `status` lo dice (por ejemplo `409` si ya estaba) y las demás siguen. Si el bot no es administrador, la llamada entera falla con el error `FORBIDDEN`.

En Rust también puedes crear y gestionar todo el grupo:

```rust
use soblendwa::ParticipantAction;

let nuevo = client.create_group("Equipo Soblend", &[Jid::phone("5215511111111")]).await?;
let resultados = client
    .update_participants(&nuevo.jid, &[Jid::phone("5215522222222")], ParticipantAction::Promote)
    .await?;
client.set_group_subject(&nuevo.jid, "Equipo").await?;
client.set_group_description(&nuevo.jid, Some("Hecho con SoblendWA")).await?;
client.set_group_picture(&nuevo.jid, std::fs::read("grupo.jpg")?).await?;
client.leave_group(&nuevo.jid).await?;
```

`ParticipantAction` puede ser `Add`, `Remove`, `Promote` o `Demote`. Cada resultado trae el `jid` y un `status`, parecido a un código HTTP: `200` significa que funcionó.

## Ajustes

| Método | Qué cambia |
|---|---|
| `set_group_announce(grupo, true)` | Solo los administradores pueden escribir |
| `set_group_locked(grupo, true)` | Solo los administradores pueden cambiar los datos del grupo |
| `set_group_ephemeral(grupo, segundos)` | Mensajes temporales. `0` los desactiva |
| `set_group_join_approval(grupo, true)` | Un administrador aprueba cada solicitud para entrar |
| `set_group_member_add_mode(grupo, true)` | Todos los miembros pueden añadir gente. Con `false`, solo los administradores |

En Node.js, Python y el cliente JS es un solo método, `groupSetting(grupo, ajuste, valor)`:

| Ajuste | Valor | Equivale a |
|---|---|---|
| `announce` | `true` o `false` | `set_group_announce` |
| `locked` | `true` o `false` | `set_group_locked` |
| `joinApproval` | `true` o `false` | `set_group_join_approval` |
| `memberAdd` | `'all'`, `'admins'` o un booleano | `set_group_member_add_mode` |
| `ephemeral` | Segundos, o `0` para apagarlo | `set_group_ephemeral` |

## Invitaciones

```rust
let codigo = client.group_invite_code(&grupo).await?;
println!("https://chat.whatsapp.com/{codigo}");

let nuevo_codigo = client.revoke_group_invite(&grupo).await?;
let detalles = client.group_invite_info(&nuevo_codigo).await?;
let unido = client.join_group(&nuevo_codigo).await?;
```

`accept_group_invite(remitente, invitación)` acepta una tarjeta de invitación recibida por mensaje. Para enviar una, usa [`send_group_invite`](#enviar-invitar-a-un-grupo).

:::tabs
```js Node.js
const info = await bot.groupInviteInfo('https://chat.whatsapp.com/ABC123')
try {
  const grupo = await bot.groupJoin('https://chat.whatsapp.com/ABC123')
} catch (error) {
  if (error.code === 'APPROVAL_REQUIRED') console.log('Pedí entrar a', info.subject, '; falta que un admin apruebe')
  else if (error.code === 'NOT_FOUND') console.log('El enlace no existe')
  else throw error
}
```
```python Python
from soblendwa import SoblendError

try:
    grupo = await bot.group_join("https://chat.whatsapp.com/ABC123")
except SoblendError as error:
    if error.code == "APPROVAL_REQUIRED":
        print("Pedí entrar; falta que un admin apruebe")
```
```rust Rust
match client.join_group("https://chat.whatsapp.com/ABC123").await {
    Ok(grupo) => println!("entré a {grupo}"),
    Err(soblendwa::Error::ApprovalRequired(grupo)) => println!("pedí entrar a {grupo}"),
    Err(error) => return Err(error),
}
```
:::

Si el grupo pide aprobación, la solicitud ya quedó enviada y el error es `APPROVAL_REQUIRED`. Cuando un administrador la acepte, llega `joinedGroup`.

## Solicitudes para entrar

```rust
let solicitudes = client.group_join_requests(&grupo).await?;
let personas: Vec<Jid> = solicitudes.iter().map(|s| s.jid.clone()).collect();
client.respond_join_requests(&grupo, &personas, true).await?;
```

## Cambios en vivo

`Event::GroupUpdate` avisa de lo que cambia: `group`, `author`, `action`, `participants` y `timestamp`.

| `GroupAction` | En JSON | Qué significa |
|---|---|---|
| `Created` | `created` | Se creó el grupo |
| `Add`, `Remove`, `Leave` | `add`, `remove`, `leave` | Entró, sacaron o se fue alguien |
| `Promote`, `Demote` | `promote`, `demote` | Nombraron o quitaron un administrador |
| `Subject(texto)` | `subject` | Cambió el nombre |
| `Description(texto)` | `description` | Cambió la descripción |
| `Announce(bool)` | `announce` | Solo administradores escriben |
| `Locked(bool)` | `locked` | Solo administradores editan los datos |
| `Ephemeral(segundos)` | `ephemeral` | Cambiaron los mensajes temporales |
| `Other(nombre)` | `other` | Otro cambio |

Cuando alguien sale de un grupo, SoblendWA rota la sender key para que esa persona no pueda leer lo que venga después.

## Datos de `GroupInfo`

| Campo | Qué es |
|---|---|
| `jid`, `subject`, `description` | Identificador, nombre y descripción |
| `owner`, `creation` | Creador y fecha de creación |
| `subject_owner`, `subject_time`, `description_id` | Quién puso el nombre, cuándo, y el id de la descripción |
| `announce`, `locked`, `join_approval`, `members_can_add` | Ajustes |
| `ephemeral` | Segundos de los mensajes temporales |
| `addressing_mode` | Cómo se direcciona, por número o por LID |
| `linked_parent`, `is_community`, `is_community_announce` | Relación con comunidades |
| `participants` | Lista con `jid`, `phone_number`, `lid`, `is_admin` e `is_super_admin` |

=== comunidades | Comunidades | Cuentas y chats
lead: Crear comunidades, vincular grupos y gestionar miembros. {exp}

```rust
let comunidad = client.create_community("Soblend", "Comunidad oficial").await?;
let subgrupo = client.create_community_group(&comunidad.jid, "Anuncios técnicos", &[]).await?;
let subgrupos = client.community_groups(&comunidad.jid).await?;

client.link_group(&comunidad.jid, &grupo).await?;
client.unlink_group(&comunidad.jid, &grupo).await?;
client.set_community_join_approval(&comunidad.jid, true).await?;
client.leave_community(&comunidad.jid).await?;
```

Una comunidad es un tipo de grupo, así que también funcionan las [funciones de grupo](#grupos), como `group_info` y `update_participants`. En `GroupInfo`, `is_community` indica si lo es, `is_community_announce` si es su grupo de anuncios y `linked_parent` a qué comunidad pertenece.

`community_groups` devuelve una lista de `LinkedGroup` con `jid`, `subject`, `owner`, `created` y `size`.

Para encuestas en grupos de anuncios, usa `Message::announcement_poll`.

=== canales | Canales | Cuentas y chats
lead: Crear, seguir y publicar en canales de WhatsApp.

## Consultar y seguir

```rust
let otro = client.newsletter_info_by_invite("https://whatsapp.com/channel/0029Va...").await?;
let mismo = client.newsletter_info(&otro.jid).await?;
client.newsletter_follow(&otro.jid).await?;
client.newsletter_mute(&otro.jid, true).await?;
client.newsletter_unfollow(&otro.jid).await?;
```

## Leer y reaccionar

```rust
for publicacion in client.newsletter_messages(&otro.jid, 20, None).await? {
    client.newsletter_react(&otro.jid, &publicacion.server_id, Some("🔥")).await?;
}
```

Los mensajes de canal que recibes traen `msg.info.server_id`, que es lo que usa `newsletter_react`. `None` quita tu reacción. `newsletter_live_updates` pide recibir en vivo los cambios del canal, y devuelve por cuántos segundos si WhatsApp lo indica.

## Crear y administrar {exp}

```rust
let canal = client.newsletter_create("Soblend News", Some("Novedades del estudio")).await?;
client.newsletter_update(&canal.jid, Some("Soblend"), None, None).await?;
let suscriptores = client.newsletter_subscribers(&canal.jid).await?;
let admins = client.newsletter_admin_count(&canal.jid).await?;
client.newsletter_demote(&canal.jid, &Jid::phone("5215511111111")).await?;
client.newsletter_change_owner(&canal.jid, &Jid::phone("5215522222222")).await?;
client.newsletter_delete(&canal.jid).await?;
```

Para publicar, envía un mensaje normal a su JID con `send_message`.

## Datos

| `NewsletterInfo` | Qué es |
|---|---|
| `jid`, `name`, `description`, `invite` | Identificación |
| `subscribers`, `verified`, `muted` | Estado |
| `picture_path`, `role`, `created` | Foto, tu rol y fecha |
| `raw` | La respuesta completa en JSON |

| `NewsletterMessage` | Qué es |
|---|---|
| `server_id`, `id`, `timestamp` | Identificación |
| `views`, `reactions` | Vistas y reacciones |
| `message` | El contenido |

=== chats | Chats, contactos y etiquetas | Cuentas y chats
lead: Lo mismo que cambias en el teléfono: archivar, fijar, silenciar, etiquetas y contactos.

Estas funciones usan el *app state* de WhatsApp, la lista de cambios que se sincroniza entre tus dispositivos. Cada parche se verifica con LTHash y MACs antes de aplicarse. Si falta una clave, se le pide al teléfono y la sincronización continúa sola cuando llega.

## Organizar chats

:::tabs
```rust Rust
use std::time::Duration;

client.archive_chat(&chat, true).await?;
client.pin_chat(&chat, true).await?;
client.mute_chat(&chat, Some(Duration::from_secs(8 * 60 * 60))).await?;
client.mark_chat_read(&chat, true).await?;
client.clear_chat(&chat).await?;
client.delete_chat(&chat).await?;
```
```js Node.js
await bot.modifyChat(chat, 'archive')
await bot.modifyChat(chat, 'pin')
await bot.modifyChat(chat, 'mute', 8 * 60 * 60)
await bot.modifyChat(chat, 'read')
await bot.modifyChat(chat, 'clear')
await bot.modifyChat(chat, 'delete')
```
```python Python
await bot.modify_chat(chat, "archive")
await bot.modify_chat(chat, "pin")
await bot.modify_chat(chat, "mute", 8 * 60 * 60)
await bot.modify_chat(chat, "read")
await bot.modify_chat(chat, "clear")
await bot.modify_chat(chat, "delete")
```
```js Cliente JS
await bot.modifyChat(chat, 'archive')
await bot.modifyChat(chat, 'pin')
await bot.modifyChat(chat, 'mute', 8 * 60 * 60)
await bot.modifyChat(chat, 'read')
await bot.modifyChat(chat, 'clear')
await bot.modifyChat(chat, 'delete')
```
:::

Las acciones de `modifyChat` son `archive`, `unarchive`, `pin`, `unpin`, `mute`, `unmute`, `read`, `unread`, `delete` y `clear`. Si no indicas segundos, `mute` dura 8 horas.

## Mensajes destacados y contactos

```rust
client.star_message(&chat, &msg.info.id, false, true).await?;
client.save_contact(&Jid::phone("5215511111111"), "Ana Soblend").await?;
client.set_push_name("Mi Bot").await?;
```

## Etiquetas {exp}

:::tabs
```rust Rust
client.edit_label("1", "Clientes VIP", 3).await?;
client.label_chat(&chat, "1", true).await?;
client.label_message(&chat, &msg.info.id, "1", true).await?;
client.delete_label("1").await?;
```
```js Node.js
await bot.editLabel('1', 'Clientes VIP', 3)
await bot.labelChat(chat, '1')
await bot.labelMessage(chat, message, '1')
await bot.deleteLabel('1')
```
```python Python
await bot.edit_label("1", "Clientes VIP", 3)
await bot.label_chat(chat, "1")
await bot.label_message(chat, message, "1")
await bot.delete_label("1")
```
```js Cliente JS
await bot.editLabel('1', 'Clientes VIP', 3)
await bot.labelChat(chat, '1')
await bot.labelMessage(chat, message, '1')
await bot.deleteLabel('1')
```
:::

## Respuestas rápidas {exp}

:::tabs
```rust Rust
let id = client.save_quick_reply(None, "hola", "¡Hola! ¿En qué te ayudo?").await?;
client.delete_quick_reply(&id).await?;
```
```js Node.js
const id = await bot.saveQuickReply('hola', '¡Hola! ¿En qué te ayudo?')
```
```python Python
id = await bot.save_quick_reply("hola", "¡Hola! ¿En qué te ayudo?")
```
```js Cliente JS
const id = await bot.saveQuickReply('hola', '¡Hola! ¿En qué te ayudo?')
```
:::

## Cualquier otro cambio

`chat_modify` acepta todas las modificaciones, incluidas las que no tienen atajo.

```rust
use soblendwa::ChatModification;

client.chat_modify(&chat, ChatModification::RemoveContact).await?;
client.chat_modify(&chat, ChatModification::DeleteForMe {
    message_id: msg.info.id.clone(),
    from_me: msg.info.from_me,
    timestamp: msg.info.timestamp,
}).await?;
```

`ChatModification` puede ser `Archive`, `Pin`, `Mute`, `MarkRead`, `Delete`, `Clear`, `Star`, `DeleteForMe`, `SaveContact`, `RemoveContact`, `Label` o `MessageLabel`.

## Cambios desde el teléfono

Lo que cambies en el teléfono llega como `Event::ChatUpdate { update, timestamp, initial_sync }`. `initial_sync` indica si viene de la sincronización inicial.

```rust
use soblendwa::events::ChatUpdate;

if let Event::ChatUpdate { update: ChatUpdate::Archived { chat, archived }, .. } = event {
    println!("{chat} archivado: {archived}");
}
```

| `ChatUpdate` | En JSON | Campos |
|---|---|---|
| `Muted` | `muted` | `chat`, `until` |
| `Archived` | `archived` | `chat`, `archived` |
| `Pinned` | `pinned` | `chat`, `pinned` |
| `MarkedRead` | `markedRead` | `chat`, `read` |
| `Locked` | `locked` | `chat`, `locked` |
| `Deleted`, `Cleared` | `deleted`, `cleared` | `chat` |
| `Starred` | `starred` | `chat`, `message_id`, `from_me`, `starred` |
| `DeletedForMe` | `deletedForMe` | `chat`, `message_id`, `from_me` |
| `Contact` | `contact` | `jid`, `full_name`, `first_name`, `lid`, `phone` |
| `ContactRemoved` | `contactRemoved` | `jid` |
| `PushName` | `pushName` | `name` |
| `Label` | `label` | `id`, `name`, `color`, `deleted` |
| `ChatLabel` | `chatLabel` | `label`, `chat`, `labeled` |
| `MessageLabel` | `messageLabel` | `label`, `chat`, `message_id`, `labeled` |
| `Other` | `other` | `index` |

En JSON, los campos usan camelCase, como `messageId` o `fullName`.

## Sincronizar a mano

Normalmente no hace falta. `client.sync_app_state(&COLLECTIONS, false)` fuerza una sincronización, y `COLLECTIONS` contiene las colecciones que usa WhatsApp.

=== store-chats | Store de chats e historial | Cuentas y chats
lead: Una base local con tus chats, contactos y mensajes, que se alimenta sola.

## Activarlo

:::tabs
```rust Rust
use soblendwa::ChatStore;

let chats = ChatStore::open("session/chats.json").await?;
let (client, events) = Client::builder().store(store).chat_store(chats.clone()).build().await?;
```
```js Node.js
const bot = await SoblendWA.create({ chatStore: { path: 'session/chats.json', limit: 500 } })
```
```python Python
bot = await SoblendWA.create(chat_store={"path": "session/chats.json", "limit": 500})
```
```bash Servidor
soblendwa-server --session bot
```
:::

El servidor lo activa por defecto, en una carpeta por cuenta. Se desactiva con `--no-chat-store`.

## Consultar

:::tabs
```rust Rust
for chat in chats.chats() {
    println!("{} · {} sin leer", chats.name_of(&chat.jid).unwrap_or(chat.jid.to_string()), chat.unread);
}
let ultimos = chats.messages(&chat, 20);
let encontrados = chats.search("factura", 10);
let viejo = chats.message(&chat, "3EB0ABC...").unwrap();
client.reply(&viejo.to_event(), "respondiendo a un mensaje de ayer").await?;
```
```js Node.js
const lista = bot.chats()
const contactos = bot.contacts()
const ultimos = bot.messages(chat, { limit: 20 })
const encontrados = bot.search('factura', 10)
```
```python Python
lista = bot.chats()
contactos = bot.contacts()
ultimos = bot.messages(chat, 20)
encontrados = bot.search("factura", 10)
```
```js Cliente JS
const lista = await bot.chats()
const contactos = await bot.contacts()
const ultimos = await bot.messages(chat, { limit: 20 })
const encontrados = await bot.search('factura', 10)
```
:::

## Qué guarda

Se alimenta solo con mensajes entrantes y enviados, historial, recibos (enviado, entregado, leído, reproducido), reacciones, votos de encuestas, ediciones, borrados, cambios de app state, nombres de contactos y asuntos de grupos.

Guarda hasta 500 mensajes por chat, ajustable con `ChatStore::open_with_limit`. Se escribe a disco cada 10 segundos si hubo cambios, y al llamar `client.disconnect()`. La escritura es atómica, así que un corte de luz no deja el archivo a medias.

| Tipo | Campos |
|---|---|
| `Chat` | `jid`, `name`, `unread`, `last_message_at`, `archived`, `pinned`, `muted_until`, `locked`, `labels` |
| `Contact` | `jid`, `name`, `first_name`, `push_name`, `lid`, `phone`, y `display_name()` |
| `StoredMessage` | `id`, `chat`, `sender`, `from_me`, `timestamp`, `push_name`, `message`, `status`, `starred`, `edited`, `revoked`, `reactions`, `poll_votes` |

`status` puede ser `Error`, `Pending`, `Sent`, `Delivered`, `Read` o `Played`. Los métodos completos están en [la referencia de ChatStore](#ref-chatstore).

## Historial

Al vincular, el teléfono envía el historial en bloques, y cada uno llega como `Event::HistorySync`. También puedes pedir más.

```rust
let oldest = msg.key();
client.request_history(&chat, &oldest, msg.info.timestamp * 1000, 50).await?;
```

Pide entre 1 y 50 mensajes anteriores al indicado. El resultado llega como otro `Event::HistorySync`. `request_placeholder_resend` le pide al teléfono una copia de mensajes concretos que no se pudieron descifrar, y `download_history` descarga un bloque a partir de su notificación.

En JSON, `historySync` trae `syncType`, `progress`, `chunkOrder`, `conversations`, `pushNames` y `lidMappings`.

=== perfil | Perfil y privacidad | Cuentas y chats
lead: Foto, información, bloqueos, ajustes de privacidad y mensajes temporales.

## Foto de perfil

```rust
let foto = client.profile_picture_url(&chat, true).await?;
client.set_profile_picture(None, std::fs::read("yo.jpg")?).await?;
client.remove_profile_picture(None).await?;
```

Con `None` cambias la tuya; con `Some(&grupo)` la de un grupo. La imagen puede ser JPEG, PNG, WebP o GIF de cualquier tamaño: se recorta al centro y se deja en 640×640.

`profile_picture_url` devuelve `None` si la cuenta no tiene foto, y el error `PRIVATE_PHOTO` si la tiene pero su privacidad no te deja verla.

:::tabs
```js Node.js
try {
  const url = await bot.profilePicture(jid, true)
  await bot.reply(message, url ? url : 'No tiene foto de perfil')
} catch (error) {
  if (error.code === 'PRIVATE_PHOTO') await bot.reply(message, 'Su foto de perfil es privada')
  else throw error
}
await bot.setProfilePicture(fs.readFileSync('yo.jpg'))
await bot.setAbout('Hecho con SoblendWA')
await bot.setPushName('Mi Bot')
```
```python Python
from soblendwa import SoblendError

try:
    url = await bot.profile_picture(jid, True)
except SoblendError as error:
    if error.code == "PRIVATE_PHOTO":
        url = None
```
```js Cliente JS
try {
  const url = await bot.profilePicture(jid, true)
} catch (error) {
  if (error.code === 'PRIVATE_PHOTO') console.log('Foto privada')
}
```
:::

## Información y nombre

```rust
client.set_about("Hecho con SoblendWA").await?;
let infos = client.fetch_about(&[chat.clone()]).await?;
client.set_push_name("Mi Bot").await?;
```

## Bloquear

:::tabs
```rust Rust
client.block(&chat).await?;
client.unblock(&chat).await?;
let bloqueados = client.blocklist().await?;
```
```js Node.js
await bot.block(jid)
await bot.unblock(jid)
```
```python Python
await bot.block(jid)
await bot.unblock(jid)
```
```js Cliente JS
await bot.block(jid)
await bot.unblock(jid)
```
:::

## Privacidad

```rust
use soblendwa::{PrivacySetting, PrivacyValue};

client.set_privacy(PrivacySetting::LastSeen, PrivacyValue::Contacts).await?;
let ajustes = client.privacy_settings().await?;
```

| `PrivacySetting` | Qué controla |
|---|---|
| `LastSeen` | Última vez |
| `Online` | En línea |
| `ProfilePicture` | Foto de perfil |
| `About` | Información |
| `ReadReceipts` | Confirmaciones de lectura |
| `GroupsAdd` | Quién te añade a grupos |
| `Calls` | Quién te llama |
| `Messages` | Quién te escribe |

`PrivacyValue` puede ser `Everyone`, `Contacts`, `ContactsExcept`, `Nobody`, `MatchLastSeen` o `Known`. No todos los valores valen para todos los ajustes: WhatsApp rechaza las combinaciones que no admite.

Los cambios llegan como `Event::PrivacyChanged` y `Event::BlocklistChanged`, y los de otras personas como `Event::PictureChanged` y `Event::AboutChanged`. {exp}

## Mensajes temporales

:::tabs
```rust Rust
client.set_disappearing(&chat, 7 * 24 * 60 * 60).await?;
client.set_default_disappearing(0).await?;
let modos = client.disappearing_modes(&[chat.clone()]).await?;
```
```js Node.js
const modos = await bot.disappearingModes(chat)
```
```python Python
modos = await bot.disappearing_modes(chat)
```
```js Cliente JS
const modos = await bot.disappearingModes(chat)
```
:::

## Comprobar números

:::tabs
```rust Rust
let encontrados = client.on_whatsapp(&["5215511111111"]).await?;
let lid = client.lid_for_phone(&Jid::phone("5215511111111")).await?;
let telefono = client.phone_for_lid(&Jid::lid("100000000000001")).await?;
```
```js Node.js
const encontrados = await bot.onWhatsApp('5215511111111')
```
```python Python
encontrados = await bot.on_whatsapp("5215511111111")
```
```js Cliente JS
const encontrados = await bot.onWhatsApp('5215511111111')
```
:::

Cada resultado trae `query`, `jid`, `exists` y `lid`. `phone_for_lid` solo conoce los pares que WhatsApp ya compartió contigo.

## Límites de la cuenta

```rust
let limite = client.reachout_timelock().await?;
let cupo = client.new_chat_message_cap().await?;
let bots = client.bot_list().await?;
```

| Método | Qué devuelve |
|---|---|
| `reachout_timelock` | Si WhatsApp limitó el contacto con gente nueva: `active`, `ends_at` y `enforcement` |
| `new_chat_message_cap` | Cuántos chats nuevos puedes iniciar, como JSON |
| `bot_list` | Los bots de Meta AI disponibles: `jid` y `persona_id` |
| `business_profile(jid)` | El perfil de empresa de otra cuenta |

=== presencia | Presencia y llamadas | Cuentas y chats
lead: En línea, escribiendo, grabando, y lo que puedes hacer con las llamadas.

## En línea

:::tabs
```rust Rust
client.set_presence(true).await?;
client.subscribe_presence(&chat).await?;
```
```js Node.js
await bot.setOnline(true)
```
```python Python
await bot.set_online(True)
```
```js Cliente JS
await bot.setOnline(true)
```
:::

Para saber si alguien está en línea, suscríbete y atiende `Event::Presence { from, available, last_seen }`. Con `.online_on_connect(false)` el bot no aparece en línea al conectar.

## Escribiendo y grabando

:::tabs
```rust Rust
use soblendwa::ChatPresence;

client.send_chat_presence(&chat, ChatPresence::Composing).await?;
client.send_chat_presence(&chat, ChatPresence::Recording).await?;
client.send_chat_presence(&chat, ChatPresence::Paused).await?;
```
```js Node.js
await bot.typing(chat, 'composing')
await bot.typing(chat, 'recording')
await bot.typing(chat, 'paused')
```
```python Python
await bot.typing(chat, "composing")
await bot.typing(chat, "recording")
await bot.typing(chat, "paused")
```
```js Cliente JS
await bot.typing(chat, 'composing')
await bot.typing(chat, 'recording')
await bot.typing(chat, 'paused')
```
:::

Lo que hacen los demás llega como `Event::ChatState { chat, sender, state }`.

## Llamadas

:::warn No incluye audio ni video
SoblendWA recibe los eventos de llamada, puede rechazarlas y crear enlaces, pero no transmite audio ni video. {no}
:::

```rust
use soblendwa::events::CallKind;

if let Event::Call(llamada) = event {
    if llamada.kind == CallKind::Offer {
        client.reject_call(&llamada.id, &llamada.from).await?;
    }
}
let enlace = client.create_call_link(true, None).await?;
```

`Event::Call` trae `id`, `from`, `kind` (`Offer`, `Accept`, `Reject` o `Terminate`), `video`, `is_group` y `timestamp`. En `create_call_link`, `true` significa video, y el segundo parámetro es una hora de inicio opcional en segundos Unix.

=== empresa | Catálogo de empresa | Cuentas y chats
lead: Productos, colecciones, pedidos y el perfil de tu cuenta de empresa. {exp}

:::note Solo para WhatsApp Business
Estas funciones necesitan una cuenta de empresa y aún no se han probado contra una cuenta real.
:::

## Catálogo y productos

```rust
use soblendwa::business::{CatalogImage, ProductDraft};

let pagina = client.catalog(None, 10, None).await?;
let producto = client.product_create(&ProductDraft {
    name: Some("Taza Soblend".into()),
    price: Some(149_000),
    currency: Some("MXN".into()),
    images: vec![CatalogImage::Data(std::fs::read("taza.jpg")?)],
    ..Default::default()
}).await?;
client.product_update(&producto.id, &ProductDraft { hidden: Some(true), ..Default::default() }).await?;
client.product_delete(&[&producto.id]).await?;
```

Los precios van en milésimas, como en la app: `149_000` son 149,00. Las imágenes pueden ser bytes (`CatalogImage::Data`) o una URL (`CatalogImage::Url`). Para seguir paginando, pasa `next_cursor` como tercer parámetro de `catalog`.

## Enviar un producto

```rust
client.send_product(&chat, &mi_numero, &producto, &std::fs::read("taza.jpg")?, Some("¡Nuevo!")).await?;
```

## Colecciones y pedidos

```rust
let colecciones = client.catalog_collections(None, 10).await?;
let pedido = client.order_details("id-del-pedido", "token-del-pedido").await?;
```

## Perfil de empresa

```rust
use soblendwa::business::{BusinessHours, BusinessProfileUpdate, OpeningHours, Weekday};

client.update_business_profile(&BusinessProfileUpdate {
    email: Some("ventas@soblend.dev".into()),
    hours: Some(BusinessHours {
        timezone: "America/Mexico_City".into(),
        days: vec![(Weekday::Mon, OpeningHours::Between { open_minute: 9 * 60, close_minute: 18 * 60 })],
    }),
    ..Default::default()
}).await?;

let portada = client.set_business_cover_photo(std::fs::read("portada.jpg")?).await?;
client.remove_business_cover_photo(&portada).await?;
let perfil = client.business_profile(&chat).await?;
```

`OpeningHours` puede ser `AllDay`, `ByAppointment` o `Between { open_minute, close_minute }`, con los minutos contados desde la medianoche.

## Datos

| Tipo | Campos |
|---|---|
| `Product` | `id`, `name`, `retailer_id`, `url`, `description`, `price`, `currency`, `hidden`, `images`, `review_status`, `can_appeal` |
| `ProductDraft` | `name`, `description`, `retailer_id`, `price`, `currency`, `images`, `hidden`, `origin_country` |
| `Catalog` | `products`, `next_cursor` |
| `ProductCollection` | `id`, `name`, `products`, `status`, `can_appeal` |
| `Order` | `items`, `total`, `currency` |
| `OrderItem` | `id`, `name`, `image_url`, `price`, `currency`, `quantity` |
| `BusinessProfile` | `jid`, `description`, `address`, `email`, `website`, `category` |
| `BusinessProfileUpdate` | `description`, `address`, `email`, `websites`, `hours` |
