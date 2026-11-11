'use strict'

// The same long test as examples/soak.rs, but through the Node.js module, so a run on a real number also covers
// Node on Linux. It reads and writes the same session folder and the same soak.jsonl, so you can switch between
// the two without linking again, and `soak --resumen` from either one sums both.
//
//   node soak.js [session folder] [--pair 5215512345678]
//   node soak.js [session folder] --resumen

const fs = require('node:fs')
const path = require('node:path')
const { SoblendWA, credits, version } = require('./index.js')

const COMMAND = '!yowner'
const SNAPSHOT_EVERY_MS = 10 * 60 * 1000
const HELP = `Pruebas con tu propio número (escríbelas aquí):
!yowner · resumen de la prueba larga
!yowner ping · cuánto tarda el bot y la red
!yowner fases · tiempos de los envíos por fase, con tu red
!yowner cita · te responde citándote y mencionándote
!yowner lid · tu número y tu LID, y la consulta en los dos sentidos
!yowner foto · reenvía la última foto que mandaste en este chat (la segunda vez no se vuelve a subir)
!yowner media · manda siempre la misma imagen: tras !yowner reiniciar no se debe volver a subir
!yowner reiniciar · reinicia el proceso (vuelve solo en unos 20 s)
!yowner mac · daña la sesión del bot con tu teléfono; tu próximo mensaje debe volver por el reintento
!yowner cola · te manda 3 mensajes y corta la conexión antes de que WhatsApp confirme; deben llegarte una vez cada uno
Además cuenta solo: crea un grupo, sal de uno o entra por enlace y aparece en el resumen.`
const TILE = Buffer.from(
  'iVBORw0KGgoAAAANSUhEUgAAAGAAAABgCAIAAABt+uBvAAAAtUlEQVR42u3ZMQ0AIAxFwUpgYEAF/iUgCwll6QBcwtyE215+rDbT1/tI36t3AhAgQIAAAQIECBAgQIAKgH7+/MkdQIAAAQIECBAgQIAAAaoAEqtqHhAgQIAAAQIECBAgQHYxNQ8IECBAgAABAgQIECC7mFgFBAgQIECAAAECBAgQILuYmgcECBAgQIAAAQIECJBdTM0DAgQIECBAgAABAgQIkF1MzQMCBAgQIECAAAECBOgSoA0EeO1LOBqdLwAAAABJRU5ErkJggg==',
  'base64',
)

const args = process.argv.slice(2)
const pairAt = args.indexOf('--pair')
let pair = pairAt >= 0 ? args[pairAt + 1] : null
const folder = args.find((arg, at) => !arg.startsWith('--') && (at === 0 || args[at - 1] !== '--pair')) || 'sesion-soak'
const reportPath = path.join(folder, 'soak.jsonl')
const run = Date.now()

function write(event, fields = {}) {
  try {
    fs.appendFileSync(reportPath, `${JSON.stringify({ at: Date.now(), run, event, ...fields })}\n`)
  } catch (error) {
    console.error(`no pude escribir el reporte: ${error.message}`)
  }
}

function history() {
  try {
    return fs
      .readFileSync(reportPath, 'utf8')
      .split('\n')
      .filter(Boolean)
      .flatMap((line) => {
        try {
          return [JSON.parse(line)]
        } catch {
          return []
        }
      })
  } catch {
    return []
  }
}

const rssMb = () => Math.round(process.memoryUsage().rss / 1048576)

function errorKind(error) {
  const text = String(error).toLowerCase()
  if (text.includes('sender key')) return 'sender_key'
  if (text.includes('mac')) return 'bad_mac'
  if (text.includes('pre-key') || text.includes('prekey')) return 'prekey'
  if (text.includes('session')) return 'no_session'
  return 'other'
}

const isPrivate = (chat) => chat.endsWith('@s.whatsapp.net') || chat.endsWith('@lid')

function ownerCommand(message) {
  const { info } = message
  if (!info.fromMe || !isPrivate(info.chat) || Date.now() / 1000 - info.timestamp > 120 || !message.text) return null
  const words = message.text.trim().split(/\s+/)
  if (words[0].toLowerCase() !== COMMAND || words.length > 2) return null
  return (words[1] || '').toLowerCase()
}

// Same arithmetic as totals() in examples/soak.rs: counters come from the last snapshot of every earlier run plus
// the live one; failures are matched per run to know which never got an answer.
function totals(lines, currentRun, live) {
  const last = new Map()
  const open = new Map()
  const out = { since: null, runs: 0, disconnects: 0, joined: 0, left: 0, lidPairs: 0, maxRss: 0, kinds: {} }
  const kind = (line) => (out.kinds[line.kind || 'desconocido'] ||= [0, 0, 0])
  for (const line of lines) {
    out.since ??= line.at
    const delta = { undecryptable: 1, recovered: -1, lost: -1 }[line.event]
    if (delta) open.set(line.run, (open.get(line.run) || 0) + delta)
    switch (line.event) {
      case 'start': out.runs += 1; break
      case 'disconnected': out.disconnects += 1; break
      case 'joinedGroup': out.joined += 1; break
      case 'leftGroup': out.left += 1; break
      case 'lidMapping': out.lidPairs += 1; break
      case 'undecryptable': kind(line)[0] += 1; break
      case 'recovered': kind(line)[1] += 1; break
      case 'lost': kind(line)[2] += 1; break
      case 'stats':
        out.maxRss = Math.max(out.maxRss, line.rssMb || 0)
        if (line.run !== currentRun) last.set(line.run, line.stats || {})
        break
    }
  }
  const field = (name) => [...last.values(), live].reduce((sum, stats) => sum + (stats[name] || 0), 0)
  for (const name of ['received', 'sent', 'undecryptable', 'recovered', 'lost', 'reconnects', 'panics']) out[name] = field(name)
  out.unresolved = Math.max(0, [...open.values()].reduce((a, b) => a + b, 0))
  return out
}

function describe(all) {
  const days = all.since ? (Date.now() - all.since) / 86400000 : 0
  const percent = all.received ? (all.undecryptable * 100) / all.received : 0
  const perDay = days >= 1 ? `${(all.undecryptable / days).toFixed(1)} por día` : 'el promedio por día sale después del primer día'
  const kinds = Object.entries(all.kinds)
    .filter(([, [failed]]) => failed > 0)
    .map(([kind, [failed, recovered, lost]]) => `${kind} ${failed} (recuperados ${recovered}, perdidos ${lost})`)
    .join(', ')
  return [
    `Prueba: ${days.toFixed(1)} días, ${Math.max(all.runs, 1)} arranques`,
    `Recibidos ${all.received} · enviados ${all.sent}`,
    `Sin descifrar ${all.undecryptable} (${percent.toFixed(2)}%) · ${perDay}`,
    ...(kinds ? [`Tipos: ${kinds}`] : []),
    `Recuperados ${all.recovered} · perdidos ${all.lost} · sin resolver ${all.unresolved}`,
    `Reconexiones ${all.reconnects} · desconexiones ${all.disconnects} · pánicos ${all.panics}`,
    `Grupos: entró a ${all.joined} · salió de ${all.left} · pares número/LID nuevos ${all.lidPairs}`,
    `Memoria máxima ${all.maxRss ? `${all.maxRss} MB` : '?'}`,
  ].join('\n')
}

const ms = (micros) => (micros / 1000).toFixed(0)

function summary(bot) {
  const stats = bot.stats()
  const all = totals(history(), run, stats)
  all.unresolved = Math.max(0, all.unresolved - stats.awaitingRecovery)
  all.maxRss = Math.max(all.maxRss, rssMb())
  const up = Math.floor(stats.uptimeSecs)
  const header = `SoblendWA ${version()} en Node.js ${process.version} · prendido hace ${Math.floor(up / 86400)}d ${Math.floor((up % 86400) / 3600)}h ${Math.floor((up % 3600) / 60)}m · memoria ${rssMb()} MB · esperando reintento ${stats.awaitingRecovery}`
  const timings = stats.sendTimings
  const sends = timings.samples
    ? `Envíos p50/p95: total ${ms(timings.totalMicros.p50)}/${ms(timings.totalMicros.p95)} ms · WhatsApp ${timings.serverMicros ? `${ms(timings.serverMicros.p50)}/${ms(timings.serverMicros.p95)} ms` : '-'}`
    : 'Envíos: todavía no hay ninguno medido'
  return [header, describe(all), sends].join('\n')
}

function phases(bot) {
  const timings = bot.stats().sendTimings
  if (!timings.samples) return 'Todavía no hay envíos medidos. Prueba con !yowner ping y vuelve a pedirlo.'
  const row = (name, spread) =>
    spread ? `${name}: ${(spread.p50 / 1000).toFixed(1)} / ${(spread.p95 / 1000).toFixed(1)} / ${(spread.max / 1000).toFixed(1)} ms` : `${name}: -`
  return [
    `Últimos ${timings.samples} envíos, p50 / p95 / máximo`,
    row('cola', timings.queuedMicros),
    row('subida', timings.uploadMicros),
    row('preparar', timings.prepareMicros),
    row('cifrar', timings.encryptMicros),
    row('WhatsApp', timings.serverMicros),
    row('total', timings.totalMicros),
  ].join('\n')
}

async function answer(bot, message, command, lastImage) {
  const chat = message.info.chat
  let text
  switch (command) {
    case '':
      text = summary(bot)
      break
    case 'ayuda':
      text = HELP
      break
    case 'ping': {
      const latency = bot.stats().latencyMicros
      text = `Pong · ${bot.elapsed(message).toFixed(2)} ms dentro del bot · red ${latency ? Math.round(latency / 1000) : '?'} ms`
      break
    }
    case 'fases':
      text = phases(bot)
      break
    case 'cita': {
      const me = (bot.me || chat).split(':')[0].split('@')[0]
      await bot.sendText(chat, `@${me} esto es una cita con mención`, { quoted: message, mentions: [me] })
      return
    }
    case 'lid': {
      const phone = bot.me ? `${bot.me.split(':')[0].split('@')[0]}@s.whatsapp.net` : null
      const lid = phone ? await bot.lidForPhone(phone) : null
      const back = lid ? await bot.phoneForLid(lid) : null
      text = `Número: ${phone || '-'}\nLID: ${lid || '-'}\nlidForPhone: ${lid || '-'}\nphoneForLid: ${back || '-'}`
      break
    }
    case 'foto': {
      if (!lastImage) {
        await bot.sendText(chat, 'Manda una foto en este chat y luego !yowner foto.')
        return
      }
      const data = await bot.download(lastImage)
      await bot.sendImage(chat, data, { caption: `La misma foto, ${data.length} bytes` })
      const stats = bot.stats()
      text = `Subidas: ${stats.mediaUploaded} · reutilizadas sin volver a subir: ${stats.mediaReused}`
      break
    }
    case 'media': {
      const before = bot.stats()
      await bot.sendImage(chat, TILE, { caption: 'Siempre la misma imagen' })
      const after = bot.stats()
      const reused = after.mediaReused > before.mediaReused
      write('mediaCheck', { reused })
      text = reused
        ? 'No se volvió a subir: salió de la multimedia guardada.'
        : 'Se subió. Haz !yowner reiniciar y luego !yowner media: la segunda vez no se debe subir.'
      break
    }
    case 'reiniciar':
      await bot.sendText(chat, 'Reinicio el proceso; vuelvo en unos 20 s.', { preview: false })
      write('restartRequested')
      setTimeout(() => process.exit(75), 500)
      return
    case 'mac': {
      const phone = `${bot.me.split(':')[0].split('@')[0]}@s.whatsapp.net`
      await bot.sendText(chat, 'Dañando la sesión con tu teléfono. Ahora manda un mensaje cualquiera desde el teléfono, a quien sea, y luego !yowner: debe salir bad_mac recuperado.', { preview: false })
      const damaged = await bot.call('damageSessions', { jid: phone })
      write('macTest', { damaged })
      return
    }
    case 'cola': {
      const resent = bot.stats().outboxResent
      for (let i = 1; i <= 3; i += 1) await bot.sendText(chat, `cola ${i}/3`, { preview: false })
      await bot.call('simulateOutage')
      write('outageTest')
      await bot.waitFor('connected', null, 60000)
      await new Promise((resolve) => setTimeout(resolve, 3000))
      text = `Volví. Reenviados por la cola: ${bot.stats().outboxResent - resent}. Deberías ver "cola 1/3", "cola 2/3" y "cola 3/3" una sola vez cada uno y en orden.`
      break
    }
    default:
      text = `No conozco "${command}". Escribe !yowner ayuda.`
  }
  console.log(text)
  await bot.sendText(chat, text, { preview: false })
}

async function main() {
  if (args.includes('--help') || args.includes('-h')) {
    console.log('Uso: node soak.js [carpeta de la sesión] [--pair 5215512345678]\n       node soak.js [carpeta de la sesión] --resumen')
    return
  }
  if (args.includes('--resumen')) {
    console.log(describe(totals(history(), 0, {})))
    return
  }
  fs.mkdirSync(folder, { recursive: true })
  const bot = await SoblendWA.create({
    store: { type: 'file', path: folder },
    onlineOnConnect: false,
    deviceName: 'SoblendWA',
  })
  write('start', { version: version(), runtime: `node ${process.version}` })
  console.log(`${credits()} · Node.js ${process.version} · sesión en ${folder} · reporte en ${reportPath}`)

  const pending = new Map()
  const lastImage = new Map()
  let askedCode = false
  const snapshot = () => write('stats', { stats: bot.stats(), rssMb: rssMb() })
  const timer = setInterval(snapshot, SNAPSHOT_EVERY_MS)
  const stop = async (code) => {
    clearInterval(timer)
    snapshot()
    await bot.close().catch(() => {})
    process.exit(code)
  }
  process.on('SIGTERM', () => stop(0))
  process.on('SIGINT', () => stop(0))

  bot.on('qr', async ({ code }) => {
    if (pair && !askedCode) {
      askedCode = true
      try {
        console.log(`\nCódigo para vincular: ${await bot.requestPairCode(pair)}`)
        console.log('En el teléfono: Dispositivos vinculados > Vincular dispositivo > Vincular con número de teléfono')
      } catch (error) {
        console.error(`no pude pedir el código (${error.message}); usa el QR`)
        pair = null
        console.log(`QR: ${code}`)
      }
    } else if (!pair) {
      console.log(`QR: ${code}`)
    }
  })
  bot.on('connected', () => {
    console.log(`Conectado como ${bot.me}. Escribe ${COMMAND} en tu chat contigo mismo.`)
    write('connected')
  })
  bot.on('disconnected', (event) => write('disconnected', { reason: event.reason, reconnecting: event.reconnecting }))
  bot.on('loggedOut', async (event) => {
    console.log(`Se cerró la sesión: ${event.cause} (${event.code}). Hay que volver a vincular.`)
    write('loggedOut', { cause: event.cause, code: event.code })
    await stop(0)
  })
  bot.on('accountBlocked', async (event) => {
    console.log(`WhatsApp bloqueó la cuenta (${event.code}): ${event.reason}. La sesión se conserva.`)
    write('accountBlocked', { code: event.code, reason: event.reason })
    await stop(0)
  })
  bot.on('undecryptableMessage', (event) => {
    const kind = errorKind(event.error)
    pending.set(event.info.id, kind)
    write('undecryptable', { kind })
  })
  bot.on('messageRecovered', (event) => {
    write('recovered', { kind: pending.get(event.id) || 'desconocido' })
    pending.delete(event.id)
  })
  bot.on('messageLost', (event) => {
    write('lost', { kind: pending.get(event.id) || 'desconocido', waitedSecs: event.waitedSecs })
    pending.delete(event.id)
  })
  bot.on('joinedGroup', (event) => write('joinedGroup', { via: event.via }))
  bot.on('leftGroup', (event) => write('leftGroup', { removed: event.removed }))
  bot.on('lidMapping', () => write('lidMapping'))
  bot.on('message', (message) => {
    if (message.info.fromMe && isPrivate(message.info.chat) && message.mediaType === 'image') {
      lastImage.set(message.info.chat, message)
    }
    const command = ownerCommand(message)
    if (command === null) return
    answer(bot, message, command, lastImage.get(message.info.chat)).catch(async (error) => {
      console.error(`no pude responder ${COMMAND} ${command}: ${error.message}`)
      write('replyFailed', { error: error.code || 'OTHER' })
      await bot.sendText(message.info.chat, `error ${error.code || 'OTHER'}: ${error.message}`).catch(() => {})
    })
  })
  await bot.connect()
}

module.exports = { totals, describe, ownerCommand, errorKind, answer }

if (require.main === module) {
  main().catch((error) => {
    console.error(error)
    process.exit(1)
  })
}
