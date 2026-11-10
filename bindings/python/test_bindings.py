import asyncio
import shutil
import tempfile
import time
from pathlib import Path

from soblendwa import SoblendError, SoblendWA, credits, import_baileys, __author__


async def migration() -> None:
    scratch = Path(tempfile.mkdtemp(prefix="soblendwa-python-"))
    database = str(scratch / "auth.db")
    fixture = Path(__file__).resolve().parents[2] / "tests" / "fixtures" / "baileys-auth"
    report = await import_baileys(fixture, type="sqlite", path=database)
    assert report["sessions"] == 1 and report["senderKeys"] == 1 and report["failed"] == []

    imported = await SoblendWA.create(store={"type": "sqlite", "path": database}, chat_store={})
    assert imported.me == "5215511111111:7@s.whatsapp.net"
    assert imported.chats() == [] and imported.search("hola") == []
    print("baileys session imported through the native module")
    shutil.rmtree(scratch, ignore_errors=True)


async def main() -> None:
    assert "Soblend Development Studio" in credits()
    assert __author__ == "Soblend Development Studio"
    await migration()
    try:
        await SoblendWA.create(store={"type": "memory"}, proxy="ftp://nope")
    except RuntimeError as error:
        assert "unsupported scheme" in str(error)
    else:
        raise AssertionError("a bad proxy url should be rejected")

    bot = await SoblendWA.create(store={"type": "memory"}, push_name="SoblendWA test")
    assert bot.me is None
    assert bot.elapsed("unknown") is None
    try:
        await bot.reply("unknown", "hola")
    except SoblendError as error:
        assert error.code == "MESSAGE_NOT_FOUND" and "not among the recent messages" in str(error)
    else:
        raise AssertionError("reply to an unknown message should fail")
    try:
        await bot.call("nope")
    except SoblendError as error:
        assert error.code == "INVALID_ARGUMENT"
    else:
        raise AssertionError("an unknown method should fail")
    stats = await bot.call("stats")
    assert stats["lost"] == 0 and stats["sendTimings"]["samples"] == 0

    started = time.perf_counter()
    await bot.connect()

    async def first_qr() -> str:
        async for event in bot.events():
            if event["type"] == "qr":
                return event["code"]
        raise AssertionError("event stream ended before a QR arrived")

    code = await asyncio.wait_for(first_qr(), timeout=30)
    assert code.startswith("https://wa.me/settings/linked_devices#")
    print(f"QR received from WhatsApp in {(time.perf_counter() - started) * 1000:.0f} ms through the native module")
    await bot.disconnect()
    print("python bindings: ok")


if __name__ == "__main__":
    asyncio.run(main())
