import asyncio
import base64
import os
import tempfile
import time
from pathlib import Path

from soblendwa import FakeWhatsApp, SoblendError

BOT = "5215511111111"
ANA = "5215522222222"
PIXEL = base64.b64decode(
    "/9j/4AAQSkZJRgABAQEASABIAAD/2wBDAP//////////////////////////////////////////////////////////////////////////////////////wgALCAABAAEBAREA/8QAFBABAAAAAAAAAAAAAAAAAAAAAP/aAAgBAQABPxA="
)


async def rejects(awaitable, code: str) -> SoblendError:
    try:
        await awaitable
    except SoblendError as error:
        assert error.code == code, f"expected {code}, got {error.code}: {error.message}"
        return error
    raise AssertionError(f"expected {code}")


def text_is(value: str):
    return lambda message: message.get("text") == value


async def step(name: str, work) -> None:
    started = time.perf_counter()
    await work()
    print(f"ok  {name} ({(time.perf_counter() - started) * 1000:.0f} ms)")


async def main() -> None:
    fake = await FakeWhatsApp.start()
    assert fake.url.startswith("ws://127.0.0.1:")
    bot = await fake.connect(BOT)
    ana = await fake.connect(ANA)
    assert fake.is_online(BOT) and fake.is_online(ANA)

    async def quotes_and_mentions() -> None:
        await ana.send_text(BOT, "hola bot")
        message = await bot.wait_for("message", text_is("hola bot"))
        await bot.send_text(message["info"]["chat"], f"hola @{ANA}", quoted=message, mentions=[ANA])
        reply = await ana.wait_for("message", lambda m: (m.get("text") or "").startswith("hola @"))
        assert reply["quotedId"] == message["info"]["id"]
        assert reply["mentions"] == [FakeWhatsApp.jid(ANA)]

    async def own_preview() -> None:
        await bot.send_text(
            ANA,
            "bienvenida https://example.com/reglas",
            preview={"url": "https://example.com/reglas", "title": "Reglas", "description": "Lee esto", "thumbnail": PIXEL},
        )
        message = await ana.wait_for("message", lambda m: "bienvenida" in (m.get("text") or ""))
        assert message["linkPreview"]["title"] == "Reglas"
        assert message["linkPreview"]["hasThumbnail"] is True

    async def groups() -> None:
        group = await ana.group_create("Equipo", [BOT])
        created = await bot.wait_for("joinedGroup")
        assert created["via"] == "created" and created["group"] == group["jid"]

        await rejects(bot.group_promote(group["jid"], BOT), "FORBIDDEN")
        results = await ana.group_promote(group["jid"], BOT)
        assert results[0]["ok"] is True
        await bot.group_subject(group["jid"], "Equipo Soblend")

        await ana.group_kick(group["jid"], BOT)
        assert (await bot.wait_for("leftGroup"))["removed"] is True

        open_group = await ana.group_create("Con enlace")
        code = await ana.group_invite_code(open_group["jid"])
        assert (await bot.group_invite_info(code))["subject"] == "Con enlace"
        assert await bot.group_join(f"https://chat.whatsapp.com/{code}") == open_group["jid"]
        assert (await bot.wait_for("joinedGroup"))["via"] == "invite"

        gated = fake.create_group("Con aprobación", ANA)
        fake.set_join_approval(gated)
        await rejects(bot.group_join(fake.invite_code(gated)), "APPROVAL_REQUIRED")
        assert any(g["jid"] == open_group["jid"] for g in await bot.joined_groups())

    async def named_errors() -> None:
        await rejects(bot.reply("no-existe", "hola"), "MESSAGE_NOT_FOUND")
        await rejects(bot.group_participants("120363000000000999@g.us", [ANA], "ban"), "INVALID_ARGUMENT")
        assert await bot.lid_for_phone("5215599999999") is None

    async def timeout_keeps_events() -> None:
        await rejects(bot.wait_for("message", text_is("nunca"), timeout=0.2), "TIMEOUT")
        await ana.send_text(BOT, "después del timeout")
        await bot.wait_for("message", text_is("después del timeout"))

    async def shared_uploads() -> None:
        shared = {"media_store": {"type": "sqlite", "path": os.path.join(tempfile.gettempdir(), f"soblendwa-media-{os.getpid()}.db")}}
        first = await fake.connect("5215533333333", **shared)
        second = await fake.connect("5215544444444", **shared)
        picture = (Path(__file__).resolve().parents[2] / "docs" / "favicon.svg").read_bytes()
        before = fake.media_uploads()
        await first.send_document(ANA, picture, file_name="logo.svg", mimetype="image/svg+xml")
        await second.send_document(ANA, picture, file_name="logo.svg", mimetype="image/svg+xml")
        assert fake.media_uploads() - before == 1, "the second bot should reuse the first upload"
        assert second.stats()["mediaReused"] == 1
        await first.disconnect()
        await second.disconnect()

    async def uploads_overlap() -> None:
        names = [f"orden-{i}.bin" for i in range(6)]
        closing = "después de los archivos"
        fake.slow_uploads(0.25)
        started = time.perf_counter()
        await asyncio.gather(
            *(bot.send_document(ANA, os.urandom(32 * 1024), file_name=n, mimetype="application/octet-stream") for n in names),
            bot.send_text(ANA, closing),
        )
        elapsed = time.perf_counter() - started
        fake.slow_uploads(0)
        arrived = []
        async for event in ana.events():
            label = (event.get("media") or {}).get("fileName") or event.get("text")
            if event["type"] == "message" and (label in names or label == closing):
                arrived.append(label)
                if len(arrived) == len(names) + 1:
                    break
        assert arrived == [*names, closing], arrived
        one_by_one = 0.25 * len(names)
        assert elapsed < one_by_one, f"one after another takes at least {one_by_one} s, this took {elapsed:.2f} s"

    async def send_limit() -> None:
        strict = await fake.connect("5215555555555", send_limit={"per_chat_burst": 2, "per_chat_per_minute": 2})
        await strict.send_text(ANA, "uno")
        await strict.send_text(ANA, "dos")
        await rejects(strict.send_text(ANA, "tres"), "RATE_LIMITED")
        await strict.disconnect()

    async def burst_in_order() -> None:
        total = 300
        sender = await fake.connect("5215566666666", send_limit=False)
        sends = asyncio.gather(*(sender.send_text(BOT, f"ráfaga {i}") for i in range(total)))
        seen = []
        async for event in bot.events():
            text = event.get("text") or ""
            if event["type"] == "message" and text.startswith("ráfaga "):
                seen.append(int(text[7:]))
                if len(seen) == total:
                    break
        await sends
        assert seen == list(range(total)), seen[:20]
        await sender.disconnect()

    async def restart_one() -> None:
        await bot.restart()
        await bot.wait_for("connected")
        await ana.send_text(BOT, "tras reiniciar")
        await bot.wait_for("message", text_is("tras reiniciar"))

    async def phase_timings() -> None:
        stats = bot.stats()
        assert stats["sendTimings"]["samples"] >= 1
        assert stats["sendTimings"]["totalMicros"]["p95"] >= stats["sendTimings"]["totalMicros"]["p50"]

    await step("mensajes con cita y menciones", quotes_and_mentions)
    await step("vista previa con miniatura propia", own_preview)
    await step("grupos: crear, unirse, expulsar, invitaciones y aprobación", groups)
    await step("errores con nombre", named_errors)
    await step("un timeout no se come los eventos que siguen", timeout_keeps_events)
    await step("imágenes subidas compartidas entre bots", shared_uploads)
    await step("archivos seguidos suben juntos y llegan en orden", uploads_overlap)
    await step("límite de envíos configurable", send_limit)
    await step("ráfaga de mensajes llega completa y en orden", burst_in_order)
    await step("reiniciar una sola cuenta", restart_one)
    await step("métricas con tiempos por fase", phase_timings)

    await bot.disconnect()
    await ana.disconnect()
    print("test_fake: todo bien")


if __name__ == "__main__":
    asyncio.run(main())
