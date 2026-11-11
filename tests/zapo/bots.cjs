'use strict'

// The same small bot written with each library, for versus.cjs. Runs in its own process so CPU and memory are
// its own:   node bots.cjs <zapo|soblendwa> <ws url> <noise root hex> <store folder> <media origin>
// !ping answers "pong" in the same chat; !archivo answers with a fresh 64 KB document, so neither library can
// skip the upload with a cache. Both keep their state in SQLite.

const crypto = require('node:crypto')
const fs = require('node:fs')
const path = require('node:path')

const [, , library, url, rootHex, folder, mediaOrigin] = process.argv
fs.mkdirSync(folder, { recursive: true })

function report() {
  const cpu = process.cpuUsage()
  return { cpuMs: Math.round((cpu.user + cpu.system) / 1000), rssMb: +(process.memoryUsage().rss / 1048576).toFixed(1) }
}
let timings = () => null
process.on('message', (message) => {
  if (message === 'stats') process.send({ stats: { ...report(), timings: timings() } })
})

const document = () => crypto.randomBytes(64 * 1024)

async function zapo() {
  const { WaClient, createStore } = require('zapo-js')
  const { createSqliteStore } = require('@zapo-js/store-sqlite')
  const { Agent } = require('node:https')
  const domains = ['auth', 'signal', 'preKey', 'session', 'identity', 'senderKey', 'appState', 'privacyToken', 'messages', 'threads', 'contacts']
  const store = createStore({
    backends: { sqlite: createSqliteStore({ path: path.join(folder, 'zapo.sqlite') }) },
    providers: Object.fromEntries(domains.map((domain) => [domain, 'sqlite'])),
  })
  const media = new Agent({ rejectUnauthorized: false })
  const client = new WaClient({
    store,
    sessionId: 'versus',
    chatSocketUrls: [url],
    proxy: { mediaUpload: media, mediaDownload: media },
    testHooks: { noiseRootCa: { publicKey: new Uint8Array(Buffer.from(rootHex, 'hex')), serial: 0 } },
  })
  client.on('auth_qr', ({ qr }) => console.log(`QR ${qr}`))
  client.on('connection', (event) => {
    if (event.status === 'open') console.log('READY')
    if (event.status === 'close') console.log(`DISCONNECTED ${event.reason ?? ''}`)
  })
  client.on('message', async (event) => {
    if (event.key?.fromMe) return
    const text = event.message?.conversation ?? event.message?.extendedTextMessage?.text
    const chat = event.key.remoteJid
    try {
      if (text === '!ping') await client.message.send(chat, 'pong')
      else if (text === '!archivo') {
        await client.message.send(chat, { type: 'document', media: document(), mimetype: 'application/octet-stream', fileName: 'datos.bin' })
      }
    } catch (error) {
      console.log(`ERROR ${error.message}`)
    }
  })
  await client.connect()
}

async function soblendwa() {
  const { SoblendWA } = require('../../bindings/node/index.js')
  const bot = await SoblendWA.create({
    store: { type: 'sqlite', path: path.join(folder, 'soblendwa.db') },
    server: { wsUrl: url, noiseRootHex: rootHex, mediaEndpoint: mediaOrigin },
    sendLimit: false,
  })
  timings = () => bot.stats().sendTimings
  bot.on('qr', ({ code }) => console.log(`QR ${code}`))
  bot.on('connected', () => console.log('READY'))
  bot.on('disconnected', (event) => console.log(`DISCONNECTED ${JSON.stringify(event.reason)}`))
  bot.on('message', async (message) => {
    if (message.info.fromMe) return
    try {
      if (message.text === '!ping') await bot.sendText(message.info.chat, 'pong', { preview: false })
      else if (message.text === '!archivo') {
        await bot.sendDocument(message.info.chat, document(), { mimetype: 'application/octet-stream', fileName: 'datos.bin' })
      }
    } catch (error) {
      console.log(`ERROR ${error.message}`)
    }
  })
  await bot.connect()
}

;(library === 'zapo' ? zapo : soblendwa)().catch((error) => {
  console.error(error)
  process.exit(1)
})
