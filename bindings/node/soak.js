'use strict'

// The same long test as examples/soak.rs, but through the Node.js module, so a run on a real number also covers
// Node on Linux. It reads and writes the same session folder and the same soak.jsonl, so you can switch between
// the two without linking again, and `soak --resumen` from either one sums both.
//
//   node soak.js [session folder] [--pair 5215512345678] [--testigo otra/soak.jsonl] [--numero-testigo 5215511111111]
//                [--compartida carpeta]
//   node soak.js [session folder] --resumen | --por-dia | --pruebas

const crypto = require('node:crypto')
const fs = require('node:fs')
const path = require('node:path')
const zlib = require('node:zlib')
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
!yowner mac · daña la sesión del bot con tu teléfono; tu próximo mensaje debe volver por el reintento. Repítelo: cada !yowner mac mide la ronda anterior
!yowner cola · te manda 3 mensajes y corta la conexión antes de que WhatsApp confirme; deben llegarte una vez cada uno
!yowner caida 5 · 5 rondas de: mandar 5 mensajes y matar el proceso con kill -9 a mitad del envío. Necesita --testigo
!yowner compartida 5 · 5 rondas de: subir una imagen nueva, kill -9, y mandarla otra vez desde aquí y desde el otro dispositivo. Necesita --testigo y --compartida
!yowner mac 10 · con --numero-testigo: 10 rondas sin tocar el teléfono, el testigo contesta con la sesión dañada
!yowner pruebas · la tabla de mac, caida y compartida
!yowner parar · no empieza más rondas
Además cuenta solo: crea un grupo, sal de uno o entra por enlace y aparece en el resumen.`
const TILE = Buffer.from(
  'iVBORw0KGgoAAAANSUhEUgAAAGAAAABgCAIAAABt+uBvAAAAtUlEQVR42u3ZMQ0AIAxFwUpgYEAF/iUgCwll6QBcwtyE215+rDbT1/tI36t3AhAgQIAAAQIECBAgQIAKgH7+/MkdQIAAAQIECBAgQIAAAaoAEqtqHhAgQIAAAQIECBAgQHYxNQ8IECBAgAABAgQIECC7mFgFBAgQIECAAAECBAgQILuYmgcECBAgQIAAAQIECJBdTM0DAgQIECBAgAABAgQIkF1MzQMCBAgQIECAAAECBOgSoA0EeO1LOBqdLwAAAABJRU5ErkJggg==',
  'base64',
)

const args = process.argv.slice(2)
const WITH_VALUE = ['--pair', '--testigo', '--numero-testigo', '--compartida']
const option = (name) => {
  const at = args.indexOf(`--${name}`)
  return at >= 0 ? args[at + 1] : null
}
let pair = option('pair')
// The witness is a second device of the same number (examples/soak.rs with --comando): it sees what this one sends
// to the owner's chat and writes it down, so a kill -9 test can count what really arrived.
const witnessPath = option('testigo')
// With a witness on another number, the test messages go to that account and arrive like any other message would.
const witnessNumber = option('numero-testigo')
const sharedDir = option('compartida')
const folder = args.find((arg, at) => !arg.startsWith('--') && (at === 0 || !WITH_VALUE.includes(args[at - 1]))) || 'sesion-soak'
const reportPath = path.join(folder, 'soak.jsonl')
const run = Date.now()
const KILL_MESSAGES = 5
const MAX_ROUNDS = 10

function write(event, fields = {}) {
  try {
    fs.appendFileSync(reportPath, `${JSON.stringify({ at: Date.now(), run, event, ...fields })}\n`)
  } catch (error) {
    console.error(`no pude escribir el reporte: ${error.message}`)
  }
}

function history(file = reportPath) {
  try {
    return fs
      .readFileSync(file, 'utf8')
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
const userOf = (jid) => (jid || '').split(':')[0].split('@')[0]

// Where a message that failed came from, without saying who: a group, statuses, the account's own devices, or
// someone in private.
const own = { lid: null }

function chatKind(bot, chat, fromMe = false) {
  if (chat.endsWith('@g.us')) return 'grupo'
  if (chat === 'status@broadcast') return 'estado'
  if (fromMe || [bot.me, own.lid].some((jid) => jid && userOf(jid) === userOf(chat))) return 'propio'
  return 'privado'
}

function ownerCommand(message) {
  const { info } = message
  if (!info.fromMe || !isPrivate(info.chat) || Date.now() / 1000 - info.timestamp > 120 || !message.text) return null
  const words = message.text.trim().split(/\s+/)
  if (words[0].toLowerCase() !== COMMAND || words.length > 3) return null
  return words.slice(1).join(' ').toLowerCase()
}

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms))
const tagOf = () => crypto.randomBytes(3).toString('hex')
const selfChat = (bot) => `${bot.me.split(':')[0].split('@')[0]}@s.whatsapp.net`
const probeChat = (bot) => (witnessNumber ? `${witnessNumber}@s.whatsapp.net` : selfChat(bot))
const median = (values) => (values.length ? [...values].sort((a, b) => a - b)[Math.floor(values.length / 2)] : null)
const killNow = () => process.kill(process.pid, 'SIGKILL')

// A 64x64 PNG of a random colour, so each round uploads something WhatsApp has never seen.
function freshImage() {
  const size = 64
  const [r, g, b] = crypto.randomBytes(3)
  const row = Buffer.alloc(1 + size * 3)
  for (let x = 0; x < size; x += 1) row.set([r, g, b], 1 + x * 3)
  const chunk = (type, data) => {
    const body = Buffer.concat([Buffer.from(type), data])
    const head = Buffer.alloc(4)
    head.writeUInt32BE(data.length)
    const crc = Buffer.alloc(4)
    crc.writeUInt32BE(zlib.crc32(body))
    return Buffer.concat([head, body, crc])
  }
  const header = Buffer.alloc(13)
  header.writeUInt32BE(size, 0)
  header.writeUInt32BE(size, 4)
  header.set([8, 2], 8)
  const pixels = zlib.deflateSync(Buffer.concat(Array.from({ length: size }, () => row)))
  const signature = Buffer.from([137, 80, 78, 71, 13, 10, 26, 10])
  return Buffer.concat([signature, chunk('IHDR', header), chunk('IDAT', pixels), chunk('IEND', Buffer.alloc(0))])
}

// The witness snapshots its stats every 10 minutes, so a file quiet for longer means it was not running.
function witnessLines() {
  if (!witnessPath || !fs.existsSync(witnessPath)) return null
  const lines = history(witnessPath)
  const last = lines[lines.length - 1]
  return last && Date.now() - last.at < 15 * 60 * 1000 ? lines : null
}

// The last test of a kind that never got its result: the process died in the middle of it, which is the point.
function unfinished(lines, test, result) {
  const done = new Set(lines.filter((line) => line.event === result).map((line) => line.tag))
  const started = lines.filter((line) => line.event === test)
  const last = started[started.length - 1]
  return last && !done.has(last.tag) ? last : null
}

function judgeKill(test, lines, witness) {
  const accepted = new Set(lines.filter((line) => line.event === 'killAccepted' && line.tag === test.tag).map((line) => line.i))
  const seen = (witness || []).filter((line) => line.event === 'observed' && line.tag === test.tag)
  const ids = new Map()
  for (const line of seen) ids.set(line.i, (ids.get(line.i) || new Set()).add(line.id))
  const order = seen.map((line) => line.i)
  return {
    tag: test.tag,
    round: test.round,
    rounds: test.rounds,
    killAfterMs: test.killAfterMs,
    witness: witness !== null,
    accepted: accepted.size,
    arrived: ids.size,
    missing: [...accepted].filter((i) => !ids.has(i)).length,
    duplicates: [...ids.values()].reduce((sum, set) => sum + set.size - 1, 0),
    inOrder: order.every((i, at) => at === 0 || i > order[at - 1]),
  }
}

const describeKill = (r) =>
  `Caída ${r.round}/${r.rounds}, kill -9 a los ${r.killAfterMs} ms: aceptados ${r.accepted}/${KILL_MESSAGES} · llegaron ${r.arrived} · ` +
  `aceptados que no llegaron ${r.missing} · duplicados ${r.duplicates} · en orden ${r.inOrder ? 'sí' : 'no'} · reenviados por la cola ${r.resent}` +
  (r.witness ? '' : ' · SIN TESTIGO')

const describeShared = (r) =>
  `Compartida ${r.round}/${r.rounds}: subida ${r.uploaded ? 'sí' : 'no'} · tras el kill -9 ${r.afterCrash ? 'reutilizada' : 'se volvió a subir'} · ` +
  `desde el otro dispositivo ${r.otherDevice === null ? 'sin respuesta' : r.otherDevice ? 'reutilizada' : 'se volvió a subir'}`

function testTable(lines, detail = false) {
  const mac = lines.filter((line) => line.event === 'macResult')
  const kills = lines.filter((line) => line.event === 'killResult')
  const shared = lines.filter((line) => line.event === 'sharedResult')
  const recovered = mac.filter((line) => !line.lost).map((line) => line.ms)
  const sum = (rows, field) => rows.reduce((total, row) => total + (row[field] || 0), 0)
  const out = [
    `MAC dañada: ${mac.length} rondas · recuperadas ${recovered.length} · perdidas ${mac.length - recovered.length}` +
      (recovered.length ? ` · ${median(recovered)} ms la mediana, ${Math.max(...recovered)} ms la peor` : ''),
    `kill -9 a mitad del envío: ${kills.length} rondas · aceptados ${sum(kills, 'accepted')} · llegaron ${sum(kills, 'arrived')} · ` +
      `aceptados que no llegaron ${sum(kills, 'missing')} · duplicados ${sum(kills, 'duplicates')} · en orden ${kills.filter((k) => k.inOrder).length}/${kills.length}`,
    `Multimedia compartida: ${shared.length} rondas · reutilizada tras el kill -9 ${shared.filter((s) => s.afterCrash).length}/${shared.length} · ` +
      `desde el otro dispositivo ${shared.filter((s) => s.otherDevice).length}/${shared.length}`,
  ]
  if (detail) {
    out.push('', ...mac.map((m) => `MAC ${m.round}: ${m.lost ? 'perdida' : `${m.ms} ms`}`), ...kills.map(describeKill), ...shared.map(describeShared))
  }
  return out.join('\n')
}

// Why the connection dropped, from what either soak wrote: Node keeps the reason as an object, Rust as text.
function cutCause(reason) {
  const text = (typeof reason === 'string' ? reason : JSON.stringify(reason || {})).toLowerCase()
  if (text.includes('simulated outage')) return 'prueba'
  if (text.includes('manual')) return 'reinicio'
  if (text.includes('qrtimeout') || text.includes('restartrequired') || text.includes('restart required')) return 'vinculación'
  if (text.includes('503')) return '503'
  if (text.includes('stream error') || text.includes('streamerror')) return 'stream:error'
  if (text.includes('reset') || text.includes('timed out') || text.includes('timeout') || text.includes('connectionlost') || text.includes('connection lost')) return 'red'
  return 'otra'
}

const dayOf = (at) => {
  const d = new Date(at)
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`
}

// One row per day and runtime, in the machine's time zone. Received messages come from the stats snapshots
// (every 10 minutes), so a few minutes around midnight can land on the next day.
function perDay(lines) {
  const runtime = new Map()
  const rows = new Map()
  const lastReceived = new Map()
  for (const line of lines) {
    if (line.event === 'start') runtime.set(line.run, String(line.runtime || '').startsWith('node') ? 'Node.js' : 'Rust')
    const key = `${dayOf(line.at)}|${runtime.get(line.run) || 'Rust'}`
    if (!rows.has(key)) rows.set(key, { received: 0, undecryptable: 0, recovered: 0, lost: 0, starts: 0, cuts: {}, maxRss: 0 })
    const row = rows.get(key)
    switch (line.event) {
      case 'start': row.starts += 1; break
      case 'undecryptable': row.undecryptable += 1; break
      case 'recovered': row.recovered += 1; break
      case 'lost': row.lost += 1; break
      case 'disconnected': {
        const cause = cutCause(line.reason)
        row.cuts[cause] = (row.cuts[cause] || 0) + 1
        break
      }
      case 'stats': {
        const received = line.stats?.received || 0
        row.received += Math.max(0, received - (lastReceived.get(line.run) || 0))
        lastReceived.set(line.run, received)
        row.maxRss = Math.max(row.maxRss, line.rssMb || 0)
        break
      }
    }
  }
  const table = ['| Día | Proceso | Recibidos | Sin descifrar | Recuperados | Perdidos | Cortes | Memoria máx | Arranques |', '|---|---|---|---|---|---|---|---|---|']
  for (const [key, row] of [...rows].sort(([a], [b]) => a.localeCompare(b))) {
    const [day, process] = key.split('|')
    const cuts = Object.entries(row.cuts).map(([cause, n]) => `${cause} ${n}`).join(', ') || '0'
    table.push(`| ${day} | ${process} | ${row.received} | ${row.undecryptable} | ${row.recovered} | ${row.lost} | ${cuts} | ${row.maxRss || '?'} MB | ${row.starts} |`)
  }
  return table.join('\n')
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

// State of the live tests that only lives while the process does: the MAC probe waiting for its message to fail
// and come back, and whether more rounds should start.
const live = { probe: null, stopped: false }

function settleProbe(id, lost = false) {
  const probe = live.probe
  if (!probe || probe.failedId !== id) return null
  live.probe = null
  const result = { round: probe.round, lost, ms: lost ? null : Date.now() - probe.failedAt }
  write('macResult', result)
  probe.done?.(result)
  return result
}

function noteFailure(event, kind) {
  const probe = live.probe
  if (!probe || probe.failedId || kind !== 'bad_mac') return
  const { sender, senderAlt, senderPhone } = event.info
  if (probe.from && ![sender, senderAlt, senderPhone].some((jid) => jid && jid.startsWith(`${probe.from}@`))) return
  Object.assign(probe, { failedId: event.info.id, failedAt: Date.now() })
}

// The same MAC test without the owner's phone: damage our side of the session with the witness's account, ask the
// witness for an answer, and time how long that answer takes to come back through the retry.
async function macSeries(bot, rounds) {
  const peer = probeChat(bot)
  const before = history().filter((line) => line.event === 'macResult').length
  for (let i = 1; i <= rounds && !live.stopped; i += 1) {
    const round = before + i
    const damaged = await bot.call('damageSessions', { jid: peer })
    const settled = new Promise((resolve) => {
      live.probe = { round, from: witnessNumber, failedId: null, failedAt: null, done: resolve }
    })
    write('macTest', { damaged, round, witness: true })
    await bot.sendText(peer, `mac? ${i}/${rounds} #${tagOf()}`, { preview: false })
    const result = await Promise.race([settled, sleep(90000).then(() => null)])
    if (!result) {
      live.probe = null
      write('macResult', { round, lost: true, timedOut: true })
    }
    await sleep(3000)
  }
  await bot.sendText(selfChat(bot), testTable(history()), { preview: false }).catch(() => {})
}

function startKillRound(bot, round, rounds) {
  const tag = tagOf()
  const killAfterMs = Math.floor(Math.random() * 400)
  write('killTest', { tag, round, rounds, killAfterMs })
  const chat = probeChat(bot)
  for (let i = 1; i <= KILL_MESSAGES; i += 1) {
    bot.sendText(chat, `caída ${i}/${KILL_MESSAGES} #${tag}`, { preview: false }).then(
      () => write('killAccepted', { tag, i }),
      () => {},
    )
  }
  setTimeout(killNow, killAfterMs)
}

async function startSharedRound(bot, round, rounds) {
  const tag = tagOf()
  const image = freshImage()
  fs.writeFileSync(path.join(sharedDir, `${tag}.png`), image)
  write('sharedTest', { tag, round, rounds })
  const before = bot.stats().mediaUploaded
  await bot.sendImage(probeChat(bot), image, { caption: `compartida ${round}/${rounds} #${tag}` })
  write('sharedSent', { tag, uploaded: bot.stats().mediaUploaded > before })
  setTimeout(killNow, 100)
}

// After a kill -9 the process comes back here: judge the round it died in and start the next one.
async function resumeTests(bot) {
  const lines = history()
  const kill = unfinished(lines, 'killTest', 'killResult')
  if (kill) {
    const result = { ...judgeKill(kill, lines, witnessLines()), resent: bot.stats().outboxResent }
    write('killResult', result)
    console.log(describeKill(result))
    await bot.sendText(selfChat(bot), describeKill(result), { preview: false }).catch(() => {})
    if (kill.round < kill.rounds && !live.stopped) setTimeout(() => startKillRound(bot, kill.round + 1, kill.rounds), 5000)
    return
  }
  const shared = unfinished(lines, 'sharedTest', 'sharedResult')
  if (!shared) return
  const file = path.join(sharedDir, `${shared.tag}.png`)
  const before = bot.stats().mediaReused
  await bot.sendImage(probeChat(bot), fs.readFileSync(file), { caption: `compartida ${shared.round}/${shared.rounds} #${shared.tag} otra vez` })
  const afterCrash = bot.stats().mediaReused > before
  let check = null
  for (let i = 0; i < 30 && !check; i += 1) {
    await sleep(1000)
    check = (witnessLines() || []).find((line) => line.event === 'sharedCheck' && line.tag === shared.tag) || null
  }
  fs.rmSync(file, { force: true })
  const sent = lines.find((line) => line.event === 'sharedSent' && line.tag === shared.tag)
  const result = { tag: shared.tag, round: shared.round, rounds: shared.rounds, uploaded: Boolean(sent?.uploaded), afterCrash, otherDevice: check ? (check.reused ?? null) : null }
  write('sharedResult', result)
  console.log(describeShared(result))
  await bot.sendText(selfChat(bot), describeShared(result), { preview: false }).catch(() => {})
  if (shared.round < shared.rounds && !live.stopped) setTimeout(() => startSharedRound(bot, shared.round + 1, shared.rounds).catch(() => {}), 5000)
}

async function answer(bot, message, command, lastImage, restart = () => process.exit(75)) {
  const chat = message.info.chat
  const [name, arg] = command.split(' ')
  const rounds = Math.min(MAX_ROUNDS, Math.max(1, Number(arg) || 5))
  let text
  switch (name) {
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
      setTimeout(restart, 500)
      return
    case 'mac': {
      if (arg && witnessNumber) {
        live.stopped = false
        await bot.sendText(chat, `Empiezo ${rounds} rondas de MAC dañada contra el testigo; !yowner parar las detiene.`, { preview: false })
        macSeries(bot, rounds).catch((error) => console.error(`la serie de mac se cortó: ${error.message}`))
        return
      }
      // This very message may be the probe of the last round: it failed, the retry brought it back, and here it is.
      const previous = settleProbe(message.info.id)
      const round = history().filter((line) => line.event === 'macResult').length + 1
      const done = previous ? `Ronda ${previous.round}: volvió en ${previous.ms} ms. ` : ''
      await bot.sendText(chat, `${done}Ronda ${round}: dañé la sesión con tu teléfono. Manda otro !yowner mac para medirla, o !yowner pruebas para ver la tabla.`, { preview: false })
      const damaged = await bot.call('damageSessions', { jid: selfChat(bot) })
      live.probe = { round, failedId: null, failedAt: null }
      write('macTest', { damaged, round })
      return
    }
    case 'caida':
    case 'caída':
      if (!witnessLines()) {
        text = 'No veo al testigo: arranca con --testigo <soak.jsonl del otro dispositivo> y que ese esté corriendo.'
        break
      }
      live.stopped = false
      await bot.sendText(chat, `Empiezo ${rounds} rondas de kill -9. Vuelvo solo después de cada una; !yowner parar las detiene.`, { preview: false })
      startKillRound(bot, 1, rounds)
      return
    case 'compartida':
      if (!witnessLines() || !sharedDir) {
        text = 'Falta --compartida, o no veo al testigo corriendo.'
        break
      }
      live.stopped = false
      await bot.sendText(chat, `Empiezo ${rounds} rondas de multimedia compartida con kill -9; !yowner parar las detiene.`, { preview: false })
      await startSharedRound(bot, 1, rounds)
      return
    case 'pruebas':
      text = testTable(history())
      break
    case 'parar':
      live.stopped = true
      write('testsStopped')
      text = 'No empiezo más rondas.'
      break
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
  if (args.includes('--por-dia')) {
    console.log(perDay(history()))
    return
  }
  if (args.includes('--pruebas')) {
    console.log(testTable(history(), true))
    return
  }
  fs.mkdirSync(folder, { recursive: true })
  if (sharedDir) fs.mkdirSync(sharedDir, { recursive: true })
  const bot = await SoblendWA.create({
    store: { type: 'file', path: folder },
    ...(sharedDir ? { mediaStore: { type: 'sqlite', path: path.join(sharedDir, 'media.db') } } : {}),
    onlineOnConnect: false,
    deviceName: 'SoblendWA',
  })
  write('start', { version: version(), runtime: `node ${process.version}` })
  console.log(`${credits()} · Node.js ${process.version} · sesión en ${folder} · reporte en ${reportPath}`)

  const pending = new Map()
  const handled = new Set(history().filter((line) => line.event === 'command').map((line) => line.id))
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
  let resumed = false
  bot.on('connected', () => {
    console.log(`Conectado como ${bot.me}. Escribe ${COMMAND} en tu chat contigo mismo.`)
    write('connected')
    bot.lidForPhone(selfChat(bot)).then((lid) => { own.lid = lid }, () => {})
    if (resumed) return
    resumed = true
    // long enough for the outbox to empty and the witness to see what arrived
    setTimeout(() => resumeTests(bot).catch((error) => console.error(`no pude cerrar la prueba: ${error.message}`)), 20000)
  })
  bot.on('disconnected', (event) => {
    // a pairing code only lives as long as the connection that asked for it
    askedCode = false
    write('disconnected', { reason: event.reason, reconnecting: event.reconnecting })
  })
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
    const chat = chatKind(bot, event.info.chat, event.info.fromMe)
    pending.set(event.info.id, { kind, chat })
    write('undecryptable', { kind, chat })
    noteFailure(event, kind)
  })
  const settled = (event) => pending.get(event.id) || { kind: 'desconocido', chat: chatKind(bot, event.chat || '') }
  bot.on('messageRecovered', (event) => {
    write('recovered', settled(event))
    pending.delete(event.id)
    settleProbe(event.id)
  })
  bot.on('messageLost', (event) => {
    write('lost', { ...settled(event), waitedSecs: event.waitedSecs })
    pending.delete(event.id)
    settleProbe(event.id, true)
  })
  bot.on('joinedGroup', (event) => write('joinedGroup', { via: event.via }))
  bot.on('leftGroup', (event) => write('leftGroup', { removed: event.removed }))
  bot.on('lidMapping', () => write('lidMapping'))
  // Orders from the server itself: write a command (what you would put after !yowner) to <carpeta>/orden and send
  // SIGUSR2. It runs as if the owner had typed it, with the answers in the owner's own chat.
  process.on('SIGUSR2', () => {
    const file = path.join(folder, 'orden')
    let order
    try {
      order = fs.readFileSync(file, 'utf8').trim().toLowerCase()
      fs.rmSync(file, { force: true })
    } catch {
      return
    }
    if (!order || !bot.me) return
    write('order', { order })
    const fake = { info: { chat: selfChat(bot), id: `orden-${Date.now()}` }, text: `${COMMAND} ${order}` }
    answer(bot, fake, order, null, () => stop(75)).catch((error) => console.error(`no pude cumplir la orden ${order}: ${error.message}`))
  })
  bot.on('message', (message) => {
    if (message.info.fromMe && isPrivate(message.info.chat) && message.mediaType === 'image') {
      lastImage.set(message.info.chat, message)
    }
    const command = ownerCommand(message)
    if (command === null) return
    // WhatsApp hands back a message whose ack died with a kill -9; running caida twice would never end.
    if (handled.has(message.info.id)) return
    handled.add(message.info.id)
    write('command', { id: message.info.id })
    answer(bot, message, command, lastImage.get(message.info.chat), () => stop(75)).catch(async (error) => {
      console.error(`no pude responder ${COMMAND} ${command}: ${error.message}`)
      write('replyFailed', { error: error.code || 'OTHER' })
      await bot.sendText(message.info.chat, `error ${error.code || 'OTHER'}: ${error.message}`).catch(() => {})
    })
  })
  await bot.connect()
}

module.exports = { totals, describe, ownerCommand, errorKind, answer, judgeKill, perDay, cutCause, testTable, freshImage }

if (require.main === module) {
  main().catch((error) => {
    console.error(error)
    process.exit(1)
  })
}
