<img src="https://raw.githubusercontent.com/Soblend-Development-Studio/SoblendWA/main/assets/icon.svg" width="64" alt="" align="right">

# soblendwa para Python

WhatsApp Web multi-dispositivo para `asyncio`, con el motor de
[SoblendWA](https://github.com/Soblend-Development-Studio/SoblendWA) en Rust. De Soblend Development Studio.

Signal, el socket y el almacenamiento corren en Rust y fuera del GIL. Desde Python todo son corrutinas normales: `await`
y listo, sin hilos ni callbacks raros.

```bash
pip install soblendwa
```

Python 3.9 o más nuevo, en Windows x64, macOS (Intel y ARM) y Linux x64/ARM64. No necesitas Rust.

## Ejemplo

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
        if message["info"]["fromMe"] or message["info"]["offline"]:
            return
        if message["text"] == "!ping":
            await bot.reply(message, f"Pong {bot.elapsed(message):.2f} ms")
        if message["text"] == "!foto":
            with open("foto.jpg", "rb") as photo:
                await bot.send_image(message["info"]["chat"], photo.read(), caption="Hola", quoted=message)

    @bot.on("joinedGroup")
    async def greet(event):
        await bot.send_text(event["group"], "Hola a todos")

    await bot.run()


asyncio.run(main())
```

Si una función tuya lanza una excepción, se registra con `logging` y el bot sigue. Si prefieres llevar el bucle tú:
`await bot.connect()` y después `async for event in bot.events(): ...`.

Para vincular con número: `await bot.request_pair_code("5215512345678")` después del primer `qr`.

## Opciones

Además de `store` y `push_name`: `proxy="socks5://usuario:clave@host:1080"`, `chat_store={"path": "session/chats.json"}`,
`link_previews={"enabled": False}`, `media_store={"type": "sqlite", "path": "media.db"}` para compartir multimedia
subida entre cuentas, y `retry_deadline_secs=600`, que es cuánto se espera un mensaje que no se pudo descifrar antes
de darlo por perdido. `send_limit=False` apaga el límite de envíos, o pásale un dict con `perMinute`, `burst`,
`perChatPerMinute` y `perChatBurst`.

Si mandas varios mensajes al mismo chat con `asyncio.gather`, salen en el orden en que creaste las corrutinas.

Stores disponibles:

```python
store={"type": "file", "path": "session"}
store={"type": "sqlite", "path": "auth.db", "account": "ventas"}
store={"type": "postgres", "url": "postgres://bot:clave@localhost/wa", "account": "ventas"}
store={"type": "redis", "url": "redis://127.0.0.1/", "account": "ventas"}
store={"type": "mongo", "url": "mongodb://localhost:27017", "database": "wa", "account": "ventas"}
store={"type": "memory"}
```

## Métodos

Mensajes: `send_text(chat, text, quoted=, preview=, mentions=)`, `reply`, `react`, `edit`, `revoke`, `send_image`,
`send_video`, `send_audio`, `send_document`, `send_sticker`, `send_file`, `download`, `download_to_file`, `send_poll`,
`vote_poll`, `send_contacts`, `send_mention_all`, `send_group_invite`, `send_interactive`, `send_buttons`, `send_list`,
`send_proto`, `typing`, `set_online`, `mark_played`.

Con `preview=False` se quita la vista previa de un enlace y con `preview={"title": "…", "thumbnail": bytes}` la pones
tú.

Grupos: `group_info`, `group_metadata`, `joined_groups`, `group_create`, `group_add`, `group_kick`, `group_promote`,
`group_demote`, `group_subject`, `group_description`, `group_setting`, `group_picture`, `group_leave`,
`group_invite_code`, `group_revoke_invite`, `group_invite_info`, `group_join`, `group_requests`,
`group_respond_requests`.

Perfil y contactos: `profile_picture`, `set_profile_picture`, `remove_profile_picture`, `set_about`, `fetch_about`,
`set_push_name`, `business_profile`, `block`, `unblock`, `blocklist`, `privacy_settings`, `set_privacy`,
`on_whatsapp`, `lid_for_phone`, `phone_for_lid`.

Chats: `chats`, `contacts`, `messages`, `search`, `modify_chat`, `label_chat`, `label_message`, `edit_label`,
`delete_label`, `save_quick_reply`.

Sesión: `connect`, `run`, `restart`, `disconnect`, `logout`, `ping`, `stats`, `pending_sends`, `elapsed`,
`request_pair_code`. Y `call("metodo", **params)` para cualquier método del despachador por nombre.

Los eventos son diccionarios con `type` y los mismos campos que en Node: `message`, `loggedOut` (con `cause`),
`joinedGroup`, `leftGroup`, `messageRecovered`, `messageLost`, `lidMapping`, `chatUpdate`, etc.

## Errores

Lo que falla lanza `SoblendError`, con `code`:

```python
from soblendwa import SoblendError

try:
    url = await bot.profile_picture(jid)
except SoblendError as error:
    if error.code == "PRIVATE_PHOTO":
        url = None
    else:
        raise
```

Códigos que vas a ver seguido: `APPROVAL_REQUIRED`, `PRIVATE_PHOTO`, `MESSAGE_NOT_FOUND`, `NO_DEVICES`, `INVALID_ARGUMENT`,
`NOT_AUTHORIZED`, `FORBIDDEN`, `NOT_FOUND`, `RATE_LIMITED`, `NOT_CONNECTED`, `TIMEOUT`.

## Migrar desde Baileys

```python
from soblendwa import import_baileys

report = await import_baileys("./auth_info", type="sqlite", path="session/auth.db")
```

No hay que volver a escanear. Para zapo usa el ejemplo de Rust o el módulo de Node, que tienen `importZapo`.

## Compilar desde el repo

```bash
pip install maturin
maturin develop --release
python test_bindings.py
```

`maturin build --release` genera la rueda.

---

No estamos afiliados a WhatsApp ni a Meta, y automatizar una cuenta puede acabar en bloqueo. Lo que está probado y lo
que no: [README principal](https://github.com/Soblend-Development-Studio/SoblendWA#estado).

MIT © 2026 Soblend Development Studio
