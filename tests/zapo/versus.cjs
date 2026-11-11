'use strict'

// SoblendWA against zapo with a bot closer to a real one than an echo: state in SQLite, a network in between with
// latency and jitter, documents uploaded on demand, a group, and a restart from the database.
//
//   npm install --prefix tests/zapo
//   node tests/zapo/versus.cjs [--rtt 60] [--jitter 10] [--runs 3] [--out versus.json]
//
// Both bots talk to the same @zapo-js/fake-server through a TCP proxy that delays every chunk by rtt/2 ± jitter,
// in order. SoblendWA reaches the fake media server through a small local HTTP bridge (it only trusts real
// certificates), one extra hop that zapo doesn't have.

const fs = require('node:fs')
const http = require('node:http')
const https = require('node:https')
const net = require('node:net')
const os = require('node:os')
const path = require('node:path')
const { fork } = require('node:child_process')
const { FakeWaServer, parsePairingQrString } = require('@zapo-js/fake-server')

const option = (name, fallback) => {
  const at = process.argv.indexOf(`--${name}`)
  return at > 0 ? process.argv[at + 1] : fallback
}
const RTT = Number(option('rtt', 60))
const JITTER = Number(option('jitter', 10))
const RUNS = Number(option('runs', 3))
const OUT = option('out', null)
const BOT = '5511999999999'
const BOT_DEVICE = `${BOT}:1@s.whatsapp.net`
const GROUP = '120363000000000088@g.us'
const now = () => Number(process.hrtime.bigint()) / 1e6
const textOf = (message) => message?.conversation ?? message?.extendedTextMessage?.text ?? null
const percentile = (values, p) => {
  const sorted = [...values].sort((a, b) => a - b)
  return +sorted[Math.min(sorted.length - 1, Math.floor((sorted.length * p) / 100))].toFixed(1)
}
const median = (values) => percentile(values, 50)

// Every chunk leaves after rtt/2 ± jitter, never before the previous one. One queue and one timer per direction:
// separate timers can fire out of order when they expire on the same millisecond, which breaks the Noise stream.
function delayed(from, to) {
  const queue = []
  let timer = null
  let closing = false
  const drain = () => {
    timer = null
    while (queue.length && queue[0].at <= Date.now()) {
      const { chunk } = queue.shift()
      if (to.writable) to.write(chunk)
    }
    if (queue.length) timer = setTimeout(drain, queue[0].at - Date.now())
    else if (closing) to.destroy()
  }
  from.on('data', (chunk) => {
    const delay = RTT / 2 + (Math.random() * 2 - 1) * JITTER
    const at = Math.max(queue.length ? queue[queue.length - 1].at : 0, Date.now() + delay)
    queue.push({ at, chunk })
    if (!timer) timer = setTimeout(drain, at - Date.now())
  })
  from.on('close', () => {
    closing = true
    if (!timer) drain()
  })
  from.on('error', () => {})
}

function latencyProxy(target) {
  const server = net.createServer((client) => {
    const upstream = net.connect(target.port, target.hostname)
    client.setNoDelay(true)
    upstream.setNoDelay(true)
    delayed(client, upstream)
    delayed(upstream, client)
  })
  return new Promise((resolve) => server.listen(0, '127.0.0.1', () => resolve(server)))
}

function mediaBridge(origin) {
  const agent = new https.Agent({ rejectUnauthorized: false, keepAlive: true })
  const target = new URL(origin)
  const server = http.createServer((req, res) => {
    const upstream = https.request(
      { host: target.hostname, port: target.port, path: req.url, method: req.method, headers: { ...req.headers, host: target.host }, agent },
      (answer) => {
        res.writeHead(answer.statusCode, answer.headers)
        answer.pipe(res)
      },
    )
    upstream.on('error', (error) => res.destroy(error))
    req.pipe(upstream)
  })
  return new Promise((resolve) => server.listen(0, '127.0.0.1', () => resolve(server)))
}

function startBot(library, wsUrl, rootHex, folder, mediaOrigin) {
  const child = fork(path.join(__dirname, 'bots.cjs'), [library, wsUrl, rootHex, folder, mediaOrigin], { stdio: ['ignore', 'pipe', 'inherit', 'ipc'] })
  const waiting = { qr: [], ready: [] }
  const problems = []
  let tail = ''
  child.stdout.on('data', (chunk) => {
    tail += chunk
    const lines = tail.split('\n')
    tail = lines.pop()
    for (const line of lines.map((l) => l.trim()).filter(Boolean)) {
      if (line.startsWith('QR ')) waiting.qr.splice(0).forEach((resolve) => resolve(line.slice(3)))
      else if (line === 'READY') waiting.ready.splice(0).forEach((resolve) => resolve())
      else if (line.startsWith('ERROR') || line.startsWith('DISCONNECTED')) {
        problems.push(line)
        if (process.env.VERSUS_DEBUG) console.error(`[${library}] ${line}`)
      }
    }
  })
  return {
    child,
    problems,
    qr: () => new Promise((resolve) => waiting.qr.push(resolve)),
    ready: () => new Promise((resolve) => waiting.ready.push(resolve)),
    stats: () =>
      new Promise((resolve) => {
        child.once('message', (message) => resolve(message.stats))
        child.send('stats')
      }),
    stop: () =>
      new Promise((resolve) => {
        child.once('exit', resolve)
        child.kill()
      }),
  }
}

const latest = { pipeline: null }

async function round(library, server, proxy, bridge, rootHex) {
  const folder = fs.mkdtempSync(path.join(os.tmpdir(), `versus-${library}-`))
  const wsUrl = server.url.replace(/:\d+/, `:${proxy.address().port}`)
  const mediaOrigin = `http://127.0.0.1:${bridge.address().port}`
  const result = { library }

  let bot = startBot(library, wsUrl, rootHex, folder, mediaOrigin)
  const qr = bot.qr()
  const ready = bot.ready()
  const registration = await server.waitForAuthenticatedPipeline(60000)
  await server.runPairing(registration, { deviceJid: BOT_DEVICE }, async () => {
    const parsed = parsePairingQrString(await qr)
    return { advSecretKey: parsed.advSecretKey, identityPublicKey: parsed.identityPublicKey }
  })
  await server.waitForNextAuthenticatedPipeline(60000)
  await ready
  await bot.stop()

  const restartedAt = now()
  bot = startBot(library, wsUrl, rootHex, folder, mediaOrigin)
  await bot.ready()
  result.restartFromDatabaseMs = Math.round(now() - restartedAt)
  const idle = await bot.stats()
  result.rssIdleMb = idle.rssMb

  // A bot may reconnect on its own; peers always write to whatever connection is current.
  const peers = []
  for (const jid of ['5511888888888', '5511777777777', '5511666666666']) {
    const peer = await server.createFakePeer({ jid: `${jid}@s.whatsapp.net` }, latest.pipeline)
    peer.deps.pushStanza = (stanza) => latest.pipeline.sendStanza(stanza)
    peers.push(peer)
  }
  const [ana, carlos, dora] = peers
  server.createFakeGroup({ groupJid: GROUP, subject: 'versus', participants: [ana, dora], creator: ana.jid })

  const first = ana.expectMessage({ timeoutMs: 30000 })
  await ana.sendConversation('!ping')
  await first

  const before = await bot.stats()
  const pings = []
  for (let i = 0; i < 100; i += 1) {
    const at = now()
    const reply = ana.expectMessage({ timeoutMs: 30000 })
    await ana.sendConversation('!ping')
    if (textOf((await reply).message) !== 'pong') bot.problems.push('wrong ping reply')
    pings.push(now() - at)
  }
  result.privatePingMs = { p50: percentile(pings, 50), p95: percentile(pings, 95), p99: percentile(pings, 99) }

  const groupFirst = dora.expectGroupMessage(GROUP, { timeoutMs: 30000, senderJid: BOT_DEVICE })
  await dora.sendGroupConversation(GROUP, '!ping')
  await groupFirst
  const groupPings = []
  for (let i = 0; i < 30; i += 1) {
    const at = now()
    const reply = dora.expectGroupMessage(GROUP, { timeoutMs: 30000, senderJid: BOT_DEVICE })
    await dora.sendGroupConversation(GROUP, '!ping')
    await reply
    groupPings.push(now() - at)
  }
  result.groupPingMs = { p50: percentile(groupPings, 50), p95: percentile(groupPings, 95) }

  const documents = []
  for (let i = 0; i < 20; i += 1) {
    const at = now()
    const reply = ana.expectMessage({ timeoutMs: 60000 })
    await ana.sendConversation('!archivo')
    const message = (await reply).message
    if (!message?.documentMessage) bot.problems.push('the document did not arrive as a document')
    documents.push(now() - at)
  }
  result.documentMs = { p50: percentile(documents, 50), p95: percentile(documents, 95) }

  const burst = 300
  let replies = 0
  let done
  const finished = new Promise((resolve) => (done = resolve))
  const unsubscribe = carlos.deps.subscribeInboundMessages((stanza) => {
    if (stanza.attrs.to === carlos.jid || String(stanza.attrs.to).startsWith(carlos.jid.split('@')[0])) {
      replies += 1
      if (replies === burst) done()
    }
  })
  const burstAt = now()
  for (let i = 0; i < burst; i += 1) await carlos.sendConversation('!ping')
  await Promise.race([finished, new Promise((resolve) => setTimeout(resolve, 120000))])
  unsubscribe()
  const burstMs = now() - burstAt
  result.burst = { replies, perSecond: Math.round((replies * 1000) / burstMs) }

  const after = await bot.stats()
  const handled = 100 + 30 + 20 + burst
  result.cpuMsPerMessage = +((after.cpuMs - before.cpuMs) / handled).toFixed(2)
  result.rssAfterMb = after.rssMb
  result.problems = bot.problems.slice(0, 5)
  await bot.stop()
  fs.rmSync(folder, { recursive: true, force: true })
  return result
}

function summarize(results) {
  const pick = (library, read) => median(results.filter((r) => r.library === library).map(read))
  const rows = [
    ['Privado, ida y vuelta p50 (ms)', (r) => r.privatePingMs.p50],
    ['Privado, ida y vuelta p95 (ms)', (r) => r.privatePingMs.p95],
    ['Grupo, ida y vuelta p50 (ms)', (r) => r.groupPingMs.p50],
    ['Documento de 64 KB, pedido → recibido p50 (ms)', (r) => r.documentMs.p50],
    ['Documento de 64 KB p95 (ms)', (r) => r.documentMs.p95],
    ['Ráfaga de 300, respuestas por segundo', (r) => r.burst.perSecond],
    ['CPU del bot por mensaje (ms)', (r) => r.cpuMsPerMessage],
    ['Memoria al terminar (MB)', (r) => r.rssAfterMb],
    ['Arrancar desde la base (ms)', (r) => r.restartFromDatabaseMs],
  ]
  return rows.map(([label, read]) => ({ label, soblendwa: pick('soblendwa', read), zapo: pick('zapo', read) }))
}

async function fresh() {
  const server = await FakeWaServer.start()
  server.onAuthenticatedPipeline((pipeline) => {
    latest.pipeline = pipeline
  })
  const proxy = await latencyProxy(new URL(server.url))
  const bridge = await mediaBridge(server.mediaUrl('/').replace(/\/$/, ''))
  const rootHex = Buffer.from(server.noiseRootCa.publicKey).toString('hex')
  return {
    server,
    proxy,
    bridge,
    rootHex,
    async close() {
      await server.stop().catch(() => {})
      proxy.close()
      bridge.close()
    },
  }
}

async function main() {
  const results = []
  for (let run = 0; run < RUNS; run += 1) {
    for (const library of run % 2 ? ['zapo', 'soblendwa'] : ['soblendwa', 'zapo']) {
      // a fresh fake server per round: it keeps the pre-keys of the last bot it saw
      const world = await fresh()
      const result = await round(library, world.server, world.proxy, world.bridge, world.rootHex)
      await world.close()
      console.error(`${library} ${run + 1}/${RUNS}: ${JSON.stringify(result)}`)
      results.push(result)
    }
  }
  const table = summarize(results)
  const report = { rttMs: RTT, jitterMs: JITTER, runs: RUNS, node: process.version, platform: `${os.platform()} ${os.arch()}`, cpus: os.cpus()[0]?.model, table, results }
  if (OUT) fs.writeFileSync(OUT, JSON.stringify(report, null, 2))
  console.log(`
Red simulada: ${RTT} ms de ida y vuelta ± ${JITTER} ms · ${RUNS} corridas alternadas, mediana`)
  for (const row of table) console.log(`${row.label.padEnd(48)} SoblendWA ${String(row.soblendwa).padStart(8)} · zapo ${String(row.zapo).padStart(8)}`)
  process.exit(0)
}

main().catch((error) => {
  console.error(error)
  process.exit(1)
})
