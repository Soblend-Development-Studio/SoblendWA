'use strict'

const { SoblendWA, credits } = require('./index.js')

function formatMs(ms) {
  const decimals = ms < 0.1 ? 3 : ms < 1 ? 2 : 1
  return `${Number(ms.toFixed(decimals))} ms`
}

async function main() {
  console.log(credits())
  const bot = await SoblendWA.create({ store: { type: 'sqlite', path: 'session/auth.db' }, pushName: 'SoblendWA' })

  bot.on('qr', ({ code }) => console.log('Escanea este QR con WhatsApp > Dispositivos vinculados:\n' + code))
  bot.on('connected', () => console.log(`En línea como ${bot.me}`))
  bot.on('loggedOut', ({ reason }) => {
    console.log(`Sesión cerrada: ${reason}`)
    process.exit(0)
  })
  bot.on('message', async (message) => {
    if (message.info.fromMe || !message.text) return
    const [command] = message.text.trim().split(/\s+/)
    if (command === '!ping' || command === '#ping') {
      await bot.reply(message, `Pong\n${formatMs(bot.elapsed(message))}`)
    }
  })

  await bot.connect()
}

main().catch((error) => {
  console.error(error)
  process.exit(1)
})
