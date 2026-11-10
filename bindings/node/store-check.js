'use strict'

const assert = require('node:assert/strict')
const { FakeWhatsApp } = require('./index.js')

const usage = `Uso: node store-check.js '<store en JSON>'

  node store-check.js '{"type":"mongo","url":"mongodb://localhost:27017","database":"soblendwa_prueba"}'
  node store-check.js '{"type":"postgres","url":"postgres://usuario:clave@localhost/soblendwa_prueba"}'
  node store-check.js '{"type":"redis","url":"redis://127.0.0.1/"}'
  node store-check.js '{"type":"sqlite","path":"prueba.db"}'

Usa una base de pruebas, no la de tu bot. Las cuentas son del WhatsApp falso: no se conecta nada a WhatsApp real.`

const PIXEL = Buffer.from(
  '/9j/4AAQSkZJRgABAQEASABIAAD/2wBDAP//////////////////////////////////////////////////////////////////////////////////////wgALCAABAAEBAREA/8QAFBABAAAAAAAAAAAAAAAAAAAAAP/aAAgBAQABPxA=',
  'base64',
)

async function step(name, work) {
  const started = Date.now()
  const detail = await work()
  console.log(`ok  ${name} (${Date.now() - started} ms)${detail ? ` · ${detail}` : ''}`)
}

function collect(bot, prefix, total) {
  const seen = []
  return new Promise((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error(`llegaron ${seen.length} de ${total} mensajes "${prefix}"`)), 30000)
    const listener = (message) => {
      if (!message.text?.startsWith(prefix)) return
      seen.push(Number(message.text.slice(prefix.length)))
      if (seen.length === total) {
        clearTimeout(timer)
        bot.off('message', listener)
        resolve(seen)
      }
    }
    bot.on('message', listener)
  })
}

async function burst(from, to, chat, prefix, total) {
  const arrived = collect(to, prefix, total)
  for (let i = 0; i < total; i += 1) from.sendText(chat, `${prefix}${i}`)
  assert.deepEqual(await arrived, Array.from({ length: total }, (_, i) => i), 'los mensajes llegaron fuera de orden')
}

async function main() {
  let store
  try {
    store = JSON.parse(process.argv[2])
  } catch {
    console.log(usage)
    process.exit(2)
  }
  if (!['mongo', 'postgres', 'redis', 'sqlite'].includes(store.type)) {
    console.log(usage)
    process.exit(2)
  }

  const tag = `prueba_${Date.now().toString(36)}`
  const options = (who) => ({ store: { ...store, account: `${tag}_${who}` }, sendLimit: false })
  const BOT = '5215511111111'
  const ANA = '5215522222222'
  const CARLOS = '5215533333333'
  console.log(`store ${store.type}, cuentas ${tag}_bot y ${tag}_ana`)

  const fake = await FakeWhatsApp.start()
  let bot
  let ana
  let me
  const carlos = await fake.connect(CARLOS)
  const group = fake.createGroup('Prueba de store', ANA, [BOT, CARLOS])

  await step('vincular las dos cuentas guardando en la base', async () => {
    bot = await fake.connect(BOT, options('bot'))
    ana = await fake.connect(ANA, options('ana'))
    me = bot.me
    assert.ok(me)
  })

  await step('privados en orden y respuesta', async () => {
    await burst(ana, bot, BOT, 'privado ', 50)
    const answer = ana.waitFor('message', (m) => m.text === 'recibido', 15000)
    await bot.sendText(ANA, 'recibido')
    await answer
  })

  await step('grupo con sender keys', async () => {
    await burst(ana, bot, group, 'grupo ', 20)
    const heard = carlos.waitFor('message', (m) => m.text === 'el bot en el grupo', 15000)
    await bot.sendText(group, 'el bot en el grupo')
    await heard
  })

  await step('multimedia', async () => {
    const arrived = bot.waitFor('message', (m) => m.media?.type === 'image', 15000)
    await ana.sendImage(BOT, PIXEL, { caption: 'foto' })
    assert.deepEqual(await bot.download(await arrived), PIXEL)
  })

  await step('reiniciar leyendo todo desde la base', async () => {
    await bot.close()
    await ana.close()
    bot = await fake.connect(BOT, options('bot'))
    ana = await fake.connect(ANA, options('ana'))
    assert.equal(bot.me, me, 'la cuenta debe seguir siendo la misma, sin volver a vincular')
    await burst(ana, bot, BOT, 'después ', 50)
    await burst(ana, bot, group, 'grupo después ', 20)
    const heard = carlos.waitFor('message', (m) => m.text === 'sigo en el grupo', 15000)
    await bot.sendText(group, 'sigo en el grupo')
    await heard
  })

  await step('ráfaga grande y otro reinicio', async () => {
    const started = Date.now()
    await burst(ana, bot, BOT, 'ráfaga ', 500)
    const seconds = (Date.now() - started) / 1000
    await bot.close()
    bot = await fake.connect(BOT, options('bot'))
    await burst(ana, bot, BOT, 'final ', 20)
    return `${Math.round(500 / seconds)} mensajes/s con escrituras a la base`
  })

  await step('nada sin descifrar', async () => {
    for (const account of [bot, ana]) {
      const stats = account.stats()
      assert.equal(stats.undecryptable, 0, 'hubo mensajes sin descifrar')
      assert.equal(stats.lost, 0, 'hubo mensajes perdidos')
    }
  })

  for (const account of [bot, ana]) {
    try {
      await account.logout()
    } catch (error) {
      console.log(`no se pudo cerrar la sesión de prueba: ${error.message}`)
    }
    await account.close()
  }
  await carlos.close()
  console.log(`store-check: ${store.type} funciona`)
  console.log(`En la base quedan solo las credenciales sin vincular de ${tag}_bot y ${tag}_ana; puedes borrar la base de prueba.`)
  process.exit(0)
}

main().catch((error) => {
  console.error(`FALLA: ${error.message}`)
  process.exit(1)
})
