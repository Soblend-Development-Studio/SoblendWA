'use strict'

const fs = require('node:fs')
const path = require('node:path')
const { SoblendWA } = require('./index.js')

const usage = `Bot de prueba en vivo para un número de prueba, no para el de tu bot.

  node live-check.js [--store '<store en JSON>'] [--pair 5215512345678] [--report archivo.jsonl]
  node live-check.js --resumen archivo.jsonl

Por defecto guarda la sesión en prueba-en-vivo/auth.db y el reporte en prueba-en-vivo/live-check.jsonl.
Escríbele !ayuda desde otro número para ver los comandos.`

const HELP = [
  '!ping · latencia dentro del bot',
  '!estado · recibidos, sin descifrar, recuperados, perdidos, reconexiones',
  '!fases · tiempos por fase de los últimos envíos, con tu red',
  '!lid · tu número y tu LID, y la consulta en los dos sentidos',
  '!pfp · tu foto de perfil (avisa si es privada)',
  '!cita · responde citándote y mencionándote',
  '!foto · descarga y reenvía tu última imagen en este chat (o la que respondas)',
  '!rafaga · cinco mensajes seguidos sin esperar, deben llegar en orden',
  '!unir <enlace> · entra a un grupo (avisa si pide aprobación)',
  '!enlaceinfo <enlace> · datos de un grupo por su enlace',
  'En grupos: !enlace, !nombre <texto>, !desc <texto>, !cerrar, !abrir, !admin @, !quitaradmin @, !sacar @, !salir',
].join('\n')

const LOGO = Buffer.from(
  '/9j/4AAQSkZJRgABAQEASABIAAD/2wBDAP//////////////////////////////////////////////////////////////////////////////////////wgALCAABAAEBAREA/8QAFBABAAAAAAAAAAAAAAAAAAAAAP/aAAgBAQABPxA=',
  'base64',
)

function option(name, fallback) {
  const at = process.argv.indexOf(name)
  return at > 0 ? process.argv[at + 1] : fallback
}

const ms = (micros) => (micros / 1000).toFixed(1)
const phone = (jid) => (jid || '').split('@')[0].split(':')[0]

function summarize(file) {
  const lines = fs.readFileSync(file, 'utf8').split('\n').filter(Boolean).map((line) => JSON.parse(line))
  if (lines.length === 0) return console.log('El reporte está vacío.')
  const runs = new Map()
  const counts = {}
  let maxRss = 0
  for (const line of lines) {
    counts[line.event] = (counts[line.event] || 0) + 1
    if (line.event === 'stats') {
      runs.set(line.run, line.stats)
      maxRss = Math.max(maxRss, line.rssMb)
    }
  }
  const total = (field) => [...runs.values()].reduce((sum, stats) => sum + (stats[field] || 0), 0)
  const days = (lines.at(-1).at - lines[0].at) / 86400000
  const received = total('received')
  const undecryptable = total('undecryptable')
  const causes = lines.filter((l) => l.event === 'loggedOut').map((l) => `${l.cause} (${l.code ?? 'sin código'})`)
  console.log(`Del ${new Date(lines[0].at).toLocaleString()} al ${new Date(lines.at(-1).at).toLocaleString()} (${days.toFixed(1)} días, ${runs.size} arranques)`)
  console.log(`Mensajes recibidos: ${received} · enviados: ${total('sent')}`)
  console.log(`Sin descifrar: ${undecryptable} (${received ? ((undecryptable / received) * 100).toFixed(2) : 0}%, ${days > 0 ? (undecryptable / days).toFixed(1) : undecryptable} por día)`)
  console.log(`Recuperados después del reintento: ${total('recovered')} · perdidos de verdad: ${total('lost')}`)
  console.log(`Reconexiones: ${total('reconnects')} · pánicos: ${total('panics')} · duplicados descartados: ${total('duplicates')}`)
  console.log(`Memoria máxima: ${maxRss} MB`)
  console.log(`Bloqueos: ${counts.accountBlocked || 0} · sesiones cerradas: ${causes.length ? causes.join(', ') : 0} · caídas del proceso: ${counts.crash || 0}`)
  console.log(`Entradas a grupos: ${counts.joinedGroup || 0} · salidas: ${counts.leftGroup || 0} · pares número/LID nuevos: ${counts.lidMapping || 0}`)
}

function attach(bot, report) {
  const snapshot = () => report('stats', { stats: bot.stats(), rssMb: Math.round(process.memoryUsage().rss / 1048576) })

  let paired = false
  bot.on('qr', async ({ code }) => {
    const number = option('--pair')
    if (number && !paired) {
      paired = true
      console.log('Código para vincular:', await bot.requestPairCode(number))
    } else if (!number) {
      console.log('Escanea este QR (o usa --pair <número>):', code)
    }
  })
  bot.on('connected', () => {
    console.log('Conectado como', bot.me)
    report('connected')
  })
  for (const type of ['disconnected', 'loggedOut', 'accountBlocked', 'joinedGroup', 'leftGroup', 'messageLost', 'messageRecovered', 'undecryptableMessage', 'lidMapping']) {
    bot.on(type, (event) => {
      console.log(type, JSON.stringify(event))
      report(type, event)
    })
  }
  bot.on('error', (error) => {
    console.error('handler:', error)
    report('handlerError', { error: String(error?.message || error) })
  })

  bot.on('joinedGroup', ({ group, via }) =>
    bot.sendText(group, `Hola, entré al grupo (${via}). Prueba de vista previa: https://github.com/Soblend-Development-Studio/SoblendWA`, {
      preview: { title: 'SoblendWA', description: 'Saludo con miniatura propia', thumbnail: LOGO },
    }),
  )

  const lastImage = new Map()
  bot.on('message', async (message) => {
    if (message.media?.type === 'image' && !message.info.fromMe) lastImage.set(message.info.chat, message.info.id)
    if (message.info.fromMe || message.info.offline || !message.text?.startsWith('!')) return
    const chat = message.info.chat
    const [command, ...words] = message.text.trim().split(/\s+/)
    const rest = words.join(' ')
    const say = (text, options = {}) => bot.sendText(chat, text, { quoted: message, ...options })
    try {
      switch (command) {
        case '!ayuda':
          return await say(HELP)
        case '!ping':
          return await say(`Pong ${bot.elapsed(message)?.toFixed(2)} ms`)
        case '!estado': {
          const s = bot.stats()
          return await say(
            `Activo ${Math.round(s.uptimeSecs / 60)} min\nRecibidos ${s.received} · enviados ${s.sent}\nSin descifrar ${s.undecryptable} · recuperados ${s.recovered} · perdidos ${s.lost} · esperando ${s.awaitingRecovery}\nReconexiones ${s.reconnects} · pánicos ${s.panics}`,
          )
        }
        case '!fases': {
          const t = bot.stats().sendTimings
          if (!t.samples) return await say('Todavía no hay envíos medidos.')
          const row = (name, spread) => (spread ? `${name}: ${ms(spread.p50)} / ${ms(spread.p95)} / ${ms(spread.max)} ms` : `${name}: -`)
          return await say(
            [`Últimos ${t.samples} envíos, p50 / p95 / máximo`, row('cola', t.queuedMicros), row('subida', t.uploadMicros), row('preparar', t.prepareMicros), row('cifrar', t.encryptMicros), row('WhatsApp', t.serverMicros), row('total', t.totalMicros)].join('\n'),
          )
        }
        case '!lid': {
          const { senderPhone, senderLid } = message.info
          const lid = senderPhone ? await bot.lidForPhone(phone(senderPhone)) : null
          const number = senderLid ? await bot.phoneForLid(senderLid) : null
          return await say(`Número: ${senderPhone || 'oculto'}\nLID: ${senderLid || '-'}\nlidForPhone: ${lid || '-'}\nphoneForLid: ${number || '-'}`)
        }
        case '!pfp': {
          try {
            const url = await bot.profilePicture(message.info.sender)
            return await say(url ? `Tu foto: ${url}` : 'No tienes foto de perfil.')
          } catch (error) {
            if (error.code === 'PRIVATE_PHOTO') return await say('Tu foto es privada (PRIVATE_PHOTO).')
            throw error
          }
        }
        case '!cita':
          return await say(`@${phone(message.info.sender)} esto es una cita con mención`, { mentions: [message.info.sender] })
        case '!foto': {
          const target = message.quoted?.media?.type === 'image' ? message.quotedId : lastImage.get(chat)
          if (!target) return await say('Manda una imagen y luego !foto, o responde a una con !foto.')
          const data = await bot.download(target)
          await bot.sendImage(chat, data, { caption: `La misma imagen, ${data.length} bytes`, quoted: message })
          const s = bot.stats()
          return await say(`Subidas: ${s.mediaUploaded} · reutilizadas sin volver a subir: ${s.mediaReused}`)
        }
        case '!rafaga':
          return await Promise.all(Array.from({ length: 5 }, (_, i) => bot.sendText(chat, `ráfaga ${i + 1} de 5`)))
        case '!unir':
          try {
            return await say(`Entré a ${await bot.groupJoin(rest)}`)
          } catch (error) {
            if (error.code === 'APPROVAL_REQUIRED') return await say('Ese grupo pide aprobación; ya mandé la solicitud (APPROVAL_REQUIRED).')
            throw error
          }
        case '!enlaceinfo': {
          const info = await bot.groupInviteInfo(rest)
          return await say(`${info.subject} · ${info.participants.length} miembros · aprobación ${info.join_approval ? 'sí' : 'no'}`)
        }
        case '!enlace':
          return await say(`https://chat.whatsapp.com/${await bot.groupInviteCode(chat)}`)
        case '!nombre':
          await bot.groupSubject(chat, rest)
          return await say('Nombre cambiado.')
        case '!desc':
          await bot.groupDescription(chat, rest)
          return await say('Descripción cambiada.')
        case '!cerrar':
        case '!abrir':
          await bot.groupSetting(chat, 'announce', command === '!cerrar')
          return await say(command === '!cerrar' ? 'Solo admins pueden escribir.' : 'Todos pueden escribir.')
        case '!admin':
        case '!quitaradmin':
        case '!sacar': {
          if (!message.mentions.length) return await say('Menciona a alguien.')
          const action = { '!admin': 'groupPromote', '!quitaradmin': 'groupDemote', '!sacar': 'groupKick' }[command]
          const results = await bot[action](chat, message.mentions)
          return await say(results.map((r) => `${phone(r.jid)}: ${r.ok ? 'listo' : `no (${r.status})`}`).join('\n'))
        }
        case '!salir':
          await say('Me salgo.')
          return await bot.groupLeave(chat)
        default:
          return
      }
    } catch (error) {
      report('commandError', { command, code: error.code, error: error.message })
      await say(`error ${error.code || ''}: ${error.message}`).catch(() => {})
    }
  })

  return snapshot
}

async function main() {
  if (process.argv.includes('--help') || process.argv.includes('-h')) return console.log(usage)
  const resumen = option('--resumen')
  if (resumen) return summarize(resumen)

  const store = JSON.parse(option('--store', JSON.stringify({ type: 'sqlite', path: 'prueba-en-vivo/auth.db' })))
  if (store.path) fs.mkdirSync(path.dirname(path.resolve(store.path)), { recursive: true })
  const reportFile = option('--report', 'prueba-en-vivo/live-check.jsonl')
  fs.mkdirSync(path.dirname(path.resolve(reportFile)), { recursive: true })
  const run = Date.now()
  const report = (event, fields = {}) => fs.appendFileSync(reportFile, JSON.stringify({ at: Date.now(), run, event, ...fields }) + '\n')

  const bot = await SoblendWA.create({ store, pushName: 'SoblendWA prueba' })
  report('start', { store: store.type })
  const snapshot = attach(bot, report)
  process.on('uncaughtException', (error) => {
    report('crash', { error: String(error?.stack || error) })
    console.error(error)
    process.exit(1)
  })
  process.on('SIGINT', () => {
    snapshot()
    bot.disconnect().finally(() => process.exit(0))
  })
  setInterval(snapshot, 10 * 60 * 1000).unref()

  await bot.connect()
  console.log(`Reporte en ${reportFile}. Ctrl+C para salir; luego: node live-check.js --resumen ${reportFile}`)
}

if (require.main === module) {
  main().catch((error) => {
    console.error(error)
    process.exit(1)
  })
}

module.exports = { attach, summarize }
