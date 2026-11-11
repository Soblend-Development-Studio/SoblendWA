# Run after `pip install soblendwa==X`: connects to WhatsApp until the first QR and stops. Nothing gets linked.

import asyncio
import platform
import sys
import time

import soblendwa


async def main() -> None:
    started = time.monotonic()
    bot = await soblendwa.SoblendWA.create(store={"type": "memory"})
    await bot.connect()

    async def first_qr() -> None:
        async for event in bot.events():
            if event["type"] == "qr":
                return
        raise RuntimeError("the event stream ended before a QR")

    await asyncio.wait_for(first_qr(), 60)
    elapsed = (time.monotonic() - started) * 1000
    print(f"soblendwa {soblendwa.__version__} on {platform.system()}-{platform.machine()}, Python {sys.version.split()[0]}: QR after {elapsed:.0f} ms")
    await bot.disconnect()


asyncio.run(main())
