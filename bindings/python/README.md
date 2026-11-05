# SoblendWA para Python

Cliente de WhatsApp Web multi-dispositivo con el motor de
[SoblendWA](https://github.com/Soblend-Development-Studio/SoblendWA) en Rust, como módulo nativo para `asyncio`. Hecho
por **Soblend Development Studio**.

Todo el trabajo pesado (cifrado Signal, socket, códec binario, almacenamiento) corre en Rust, fuera del GIL. Los
métodos son corrutinas normales de Python: se usan con `await`, sin hilos ni callbacks.

## Instalar

```bash
pip install soblendwa
```

Funciona con Python 3.9 o más reciente (ABI estable) en Windows x64, macOS (Intel y ARM) y Linux x64 o ARM64. No hace falta
tener Rust.

### Compilar desde el repositorio

```text
pip install maturin
maturin develop --release
python test_bindings.py
```

Para generar una rueda instalable: `maturin build --release`.

## Uso

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
        if message["text"] == "!foto":
            with open("foto.jpg", "rb") as photo:
                await bot.send_image(message["info"]["chat"], photo.read(), caption="Hola", quoted=message)

    await bot.run()


asyncio.run(main())
```

Si prefieres manejar el bucle tú mismo: `await bot.connect()` y luego `async for event in bot.events(): ...`.

Opciones extra de `SoblendWA.create`: `proxy="socks5://usuario:clave@host:1080"`,
`chat_store={"path": "session/chats.json"}` y `link_previews={"enabled": False}`.

Los enlaces de un texto llevan vista previa sola. Para quitarla o escribirla a mano:
`await bot.send_text(chat, texto, preview=False)` o `preview={"title": "Mi título"}`.

Para migrar un bot de Baileys sin volver a escanear el QR:

```python
from soblendwa import import_baileys

report = await import_baileys("./auth_info", type="sqlite", path="session/auth.db")
```

Para vincular con código en lugar de QR: `await bot.request_pair_code("5215512345678")` después del primer evento `qr`.

## Almacenamiento

```python
store={"type": "file", "path": "session"}
store={"type": "sqlite", "path": "auth.db", "account": "ventas"}
store={"type": "postgres", "url": "postgres://bot:clave@localhost/wa", "account": "ventas"}
store={"type": "redis", "url": "redis://127.0.0.1/", "account": "ventas"}
store={"type": "mongo", "url": "mongodb://localhost:27017", "database": "wa", "account": "ventas"}
store={"type": "memory"}
```

MongoDB es experimental: falta probarlo contra un servidor real.

## Métodos

`send_text`, `reply`, `react`, `send_image`, `send_video`, `send_audio`, `send_document`, `send_sticker`, `send_file`,
`download`, `download_to_file`, `send_interactive`, `send_buttons`, `send_list`, `chats`, `contacts`, `messages`, `search`,
`send_poll`, `vote_poll`, `edit`, `revoke`, `typing`, `set_online`, `group_info`, `joined_groups`, `on_whatsapp`,
`profile_picture`, `block`, `unblock`, `send_proto`, `request_pair_code`, `stats`, `pending_sends`, `ping`, `elapsed`,
`disconnect` y `logout`.

Los eventos son diccionarios con una clave `type` (`qr`, `connected`, `message`, `pollVote`, `chatUpdate`, …), con
los mismos campos que en el binding de Node.js.

## Aviso

SoblendWA es independiente y no está afiliado a WhatsApp ni a Meta. Automatizar una cuenta puede ir contra sus Términos
de Servicio y provocar un bloqueo. Lee
[qué está probado](https://github.com/Soblend-Development-Studio/SoblendWA#estado).

MIT © 2026 Soblend Development Studio
