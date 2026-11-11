'use strict'

const assert = require('node:assert/strict')
const { FakeWhatsApp, SoblendError } = require('./index.js')

const BOT = '5215511111111'
const ANA = '5215522222222'
const PIXEL = Buffer.from(
  '/9j/4AAQSkZJRgABAQEASABIAAD/2wBDAP//////////////////////////////////////////////////////////////////////////////////////wgALCAABAAEBAREA/8QAFBABAAAAAAAAAAAAAAAAAAAAAP/aAAgBAQABPxA=',
  'base64',
)

async function rejects(promise, code) {
  try {
    await promise
  } catch (error) {
    assert.ok(error instanceof SoblendError, `expected a SoblendError, got ${error}`)
    assert.equal(error.code, code, error.message)
    return error
  }
  assert.fail(`expected ${code}`)
}

async function step(name, work) {
  const started = Date.now()
  await work()
  console.log(`ok  ${name} (${Date.now() - started} ms)`)
}

async function main() {
  const fake = await FakeWhatsApp.start()
  const bot = await fake.connect(BOT)
  const ana = await fake.connect(ANA)

  await step('mensajes con cita y menciones', async () => {
    const incoming = bot.waitFor('message', (m) => m.text === 'hola bot')
    await ana.sendText(BOT, 'hola bot')
    const message = await incoming
    const answer = ana.waitFor('message', (m) => m.text?.startsWith('hola @'))
    await bot.sendText(message.info.chat, `hola @${ANA}`, { quoted: message, mentions: [ANA] })
    const reply = await answer
    assert.equal(reply.quotedId, message.info.id)
    assert.deepEqual(reply.mentions, [FakeWhatsApp.jid(ANA)])
  })

  await step('vista previa con miniatura propia', async () => {
    const incoming = ana.waitFor('message', (m) => m.text?.includes('bienvenida'))
    await bot.sendText(ANA, 'bienvenida https://example.com/reglas', {
      preview: { url: 'https://example.com/reglas', title: 'Reglas', description: 'Lee esto', thumbnail: PIXEL },
    })
    const message = await incoming
    assert.equal(message.linkPreview.title, 'Reglas')
    assert.equal(message.linkPreview.hasThumbnail, true)
  })

  await step('grupos: crear, unirse, expulsar, invitaciones y aprobación', async () => {
    const joined = bot.waitFor('joinedGroup')
    const group = await ana.groupCreate('Equipo', [BOT])
    const created = await joined
    assert.equal(created.via, 'created')
    assert.equal(created.group, group.jid)

    await rejects(bot.groupPromote(group.jid, BOT), 'FORBIDDEN')
    const results = await ana.groupPromote(group.jid, BOT)
    assert.equal(results[0].ok, true)
    await bot.groupSubject(group.jid, 'Equipo Soblend')
    await bot.groupSetting(group.jid, 'announce', true)

    const left = bot.waitFor('leftGroup')
    await ana.groupKick(group.jid, BOT)
    assert.equal((await left).removed, true)

    const open = await ana.groupCreate('Con enlace')
    const code = await ana.groupInviteCode(open.jid)
    const info = await bot.groupInviteInfo(code)
    assert.equal(info.subject, 'Con enlace')
    const viaInvite = bot.waitFor('joinedGroup')
    assert.equal(await bot.groupJoin(`https://chat.whatsapp.com/${code}`), open.jid)
    assert.equal((await viaInvite).via, 'invite')

    const gated = await ana.groupCreate('Con aprobación')
    await ana.groupSetting(gated.jid, 'joinApproval', true)
    await rejects(bot.groupJoin(await ana.groupInviteCode(gated.jid)), 'APPROVAL_REQUIRED')

    const groups = await bot.joinedGroups()
    assert.ok(groups.some((g) => g.jid === open.jid))
  })

  await step('errores con nombre', async () => {
    await rejects(bot.reply('no-existe', 'hola'), 'MESSAGE_NOT_FOUND')
    await rejects(bot.groupParticipants('120363000000000999@g.us', [ANA], 'ban'), 'INVALID_ARGUMENT')
    assert.equal(await bot.lidForPhone('5215599999999'), null)
  })

  await step('un handler roto no detiene al bot ni al proceso', async () => {
    bot.on('message', (m) => {
      if (m.text === 'rompe') throw new Error('handler roto a propósito')
    })
    bot.on('message', async (m) => {
      if (m.text === 'rompe') throw new Error('handler async roto a propósito')
    })
    let reported = 0
    bot.on('error', () => (reported += 1))
    const after = bot.waitFor('message', (m) => m.text === 'sigues vivo')
    await ana.sendText(BOT, 'rompe')
    await ana.sendText(BOT, 'sigues vivo')
    await after
    await new Promise((resolve) => setTimeout(resolve, 50))
    assert.equal(reported, 2)
  })

  await step('reiniciar una sola cuenta', async () => {
    const back = bot.waitFor('connected', null, 15000)
    await bot.restart()
    await back
    const incoming = bot.waitFor('message', (m) => m.text === 'tras reiniciar')
    await ana.sendText(BOT, 'tras reiniciar')
    await incoming
  })

  await step('imágenes subidas compartidas entre bots', async () => {
    const shared = { mediaStore: { type: 'sqlite', path: require('node:path').join(require('node:os').tmpdir(), `soblendwa-media-${process.pid}.db`) } }
    const first = await fake.connect('5215533333333', shared)
    const second = await fake.connect('5215544444444', shared)
    const picture = require('node:fs').readFileSync(require('node:path').join(__dirname, '..', '..', 'docs', 'favicon.svg'))
    const before = fake.mediaUploads()
    await first.sendDocument(ANA, picture, { fileName: 'logo.svg', mimetype: 'image/svg+xml' })
    await second.sendDocument(ANA, picture, { fileName: 'logo.svg', mimetype: 'image/svg+xml' })
    assert.equal(fake.mediaUploads() - before, 1, 'el segundo bot debe reutilizar la subida del primero')
    assert.equal(second.stats().mediaReused, 1)
    await first.close()
    await second.close()
  })

  await step('archivos seguidos suben juntos y llegan en orden', async () => {
    const names = Array.from({ length: 6 }, (_, i) => `orden-${i}.bin`)
    const closing = 'después de los archivos'
    const arrived = []
    const done = new Promise((resolve) => {
      const listener = (m) => {
        const label = m.media?.fileName ?? m.text
        if (!names.includes(label) && label !== closing) return
        arrived.push(label)
        if (arrived.length === names.length + 1) {
          ana.off('message', listener)
          resolve()
        }
      }
      ana.on('message', listener)
    })
    fake.slowUploads(250)
    const started = Date.now()
    await Promise.all([
      ...names.map((fileName) =>
        bot.sendDocument(ANA, require('node:crypto').randomBytes(32 * 1024), { fileName, mimetype: 'application/octet-stream' }),
      ),
      bot.sendText(ANA, closing),
    ])
    const elapsed = Date.now() - started
    fake.slowUploads(0)
    await done
    assert.deepEqual(arrived, [...names, closing])
    const oneByOne = 250 * names.length
    assert.ok(elapsed < oneByOne, `one after another takes at least ${oneByOne} ms, this took ${elapsed} ms`)
  })

  await step('métricas con tiempos por fase', async () => {
    const stats = bot.stats()
    assert.equal(typeof stats.lost, 'number')
    assert.ok(stats.sendTimings.samples >= 1)
    assert.ok(stats.sendTimings.totalMicros.p95 >= stats.sendTimings.totalMicros.p50)
  })

  await step('límite de envíos: falla, espera en fila o se apaga', async () => {
    const strict = await fake.connect('5215544444444', {
      sendLimit: { perChatBurst: 2, perChatPerMinute: 2, maxWaitSecs: 0 },
    })
    await strict.sendText(ANA, 'uno')
    await strict.sendText(ANA, 'dos')
    const refused = await rejects(strict.sendText(ANA, 'tres'), 'RATE_LIMITED')
    assert.match(refused.message, /send limit/)

    const patient = await fake.connect('5215577777777', { sendLimit: { perChatBurst: 2, perChatPerMinute: 240 } })
    const names = Array.from({ length: 5 }, (_, i) => `en fila ${i}`)
    const arrived = []
    const done = new Promise((resolve) => {
      const listener = (m) => {
        if (!names.includes(m.text)) return
        arrived.push(m.text)
        if (arrived.length === names.length) {
          ana.off('message', listener)
          resolve()
        }
      }
      ana.on('message', listener)
    })
    await Promise.all(names.map((text) => patient.sendText(ANA, text)))
    await done
    assert.deepEqual(arrived, names)
    assert.equal(patient.stats().rateDelayed, 3)

    const free = await fake.connect('5215555555555', { sendLimit: false })
    await Promise.all(Array.from({ length: 100 }, (_, i) => free.sendText(ANA, `rápido ${i}`)))
    await strict.close()
    await patient.close()
    await free.close()
  })

  await step('una opción mal escrita se explica', async () => {
    const error = await rejects(fake.connect('5215588888888', { sendLimt: false }), 'INVALID_ARGUMENT')
    assert.match(error.message, /did you mean "sendLimit"/)
  })

  await step('ráfaga de mensajes llega completa y en orden', async () => {
    const total = 300
    const seen = []
    const done = new Promise((resolve) => {
      const listener = (message) => {
        if (!message.text?.startsWith('ráfaga ')) return
        seen.push(Number(message.text.slice(7)))
        if (seen.length === total) {
          bot.off('message', listener)
          resolve()
        }
      }
      bot.on('message', listener)
    })
    const sender = await fake.connect('5215566666666', { sendLimit: false })
    for (let i = 0; i < total; i += 1) sender.sendText(BOT, `ráfaga ${i}`)
    await done
    assert.deepEqual(seen, Array.from({ length: total }, (_, i) => i))
    await sender.close()
  })

  await bot.close()
  await ana.close()
  console.log('test-fake: todo bien')
  process.exit(0)
}

main().catch((error) => {
  console.error(error)
  process.exit(1)
})
