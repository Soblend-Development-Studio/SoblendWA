import asyncio

from soblendwa import SoblendWA, credits


def format_ms(ms: float) -> str:
    decimals = 3 if ms < 0.1 else 2 if ms < 1 else 1
    return f"{round(ms, decimals):g} ms"


async def main() -> None:
    print(credits())
    bot = await SoblendWA.create(store={"type": "sqlite", "path": "session/auth.db"}, push_name="SoblendWA")

    @bot.on("qr")
    def show_qr(event):
        print("Escanea este QR con WhatsApp > Dispositivos vinculados:\n" + event["code"])

    @bot.on("connected")
    def online(_):
        print(f"En línea como {bot.me}")

    @bot.on("message")
    async def answer(message):
        text = (message.get("text") or "").strip()
        if message["info"]["fromMe"] or not text:
            return
        if text.split()[0] in ("!ping", "#ping"):
            await bot.reply(message, f"Pong\n{format_ms(bot.elapsed(message))}")

    await bot.run()


if __name__ == "__main__":
    asyncio.run(main())
