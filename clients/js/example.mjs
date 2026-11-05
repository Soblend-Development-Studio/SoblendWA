import { connect } from './index.js'

const bot = await connect(process.env.SOBLENDWA_URL ?? 'ws://127.0.0.1:8787', {
  session: process.env.SOBLENDWA_SESSION ?? 'default',
  token: process.env.SOBLENDWA_TOKEN,
})

console.log(bot.me ? `Online como ${bot.me}` : 'Escanea el QR que muestra el servidor')
bot.on('connected', () => console.log(`Online como ${bot.me ?? 'tu cuenta'}`))

bot.on('message', async (message) => {
  if (message.info.fromMe || !message.text) return
  const [command] = message.text.trim().split(/\s+/)
  if (command === '!ping' || command === '#ping') {
    const ms = await bot.elapsed(message)
    await bot.reply(message, `Pong\n${Number(ms.toFixed(ms < 1 ? 2 : 1))} ms`)
  }
  if (command === '!menu') {
    await bot.sendInteractive(message.info.chat, {
      body: '¿Qué quieres hacer?',
      footer: 'SoblendWA',
      buttons: [
        { type: 'reply', id: 'ping', text: 'Medir latencia' },
        { type: 'url', text: 'Rust', url: 'https://www.rust-lang.org' },
      ],
    })
  }
  if (message.reply?.id === 'ping') {
    await bot.reply(message, 'Pong')
  }
})
