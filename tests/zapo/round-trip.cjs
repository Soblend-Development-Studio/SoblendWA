'use strict'

// zapo writes a real session with its own store, SoblendWA imports it and keeps talking to the same contacts
// without new sessions, then the session goes back to zapo and zapo carries on from where SoblendWA left it.
//
//   npm install --prefix tests/zapo
//   node tests/zapo/round-trip.cjs sqlite
//   node tests/zapo/round-trip.cjs mongo mongodb://127.0.0.1:27017
//
// Needs the Node module built in bindings/node (soblendwa.node).

const assert = require('node:assert/strict')
const fs = require('node:fs')
const os = require('node:os')
const path = require('node:path')
const { WaClient, createStore } = require('zapo-js')
const { FakeWaServer, parsePairingQrString } = require('@zapo-js/fake-server')
const { SoblendWA, importZapo, rollbackZapo } = require('../../bindings/node/index.js')

const [, , kind = 'sqlite', mongoUrl] = process.argv
const BOT = '5511999999999'
const BOT_DEVICE = `${BOT}:1@s.whatsapp.net`
const GROUP = '120363000000000077@g.us'
const SESSION = 'bot'
const DOMAINS = ['auth', 'signal', 'preKey', 'session', 'identity', 'senderKey', 'appState', 'privacyToken', 'messages', 'threads', 'contacts']
const scratch = fs.mkdtempSync(path.join(os.tmpdir(), 'soblendwa-zapo-'))
const textOf = (message) => message?.conversation ?? message?.extendedTextMessage?.text ?? null

function step(name) {
  console.log(`ok  ${name}`)
}

function zapoStorage() {
  if (kind === 'mongo') {
    assert.ok(mongoUrl, 'pass the MongoDB url: node round-trip.cjs mongo mongodb://…')
    const { createMongoStore } = require('@zapo-js/store-mongo')
    const database = `soblendwa_zapo_${process.pid}`
    return {
      open: () => createMongoStore({ db: { uri: mongoUrl, database } }),
      source: { type: 'mongo', url: mongoUrl, database, session: SESSION },
      async clean() {
        const { MongoClient } = require('mongodb')
        const client = await MongoClient.connect(mongoUrl)
        await client.db(database).dropDatabase()
        await client.db(`${database}_soblendwa`).dropDatabase()
        await client.close()
      },
    }
  }
  if (kind !== 'sqlite') throw new Error(`unknown store ${kind}: use sqlite or mongo`)
  const { createSqliteStore } = require('@zapo-js/store-sqlite')
  const file = path.join(scratch, 'zapo.sqlite')
  return { open: () => createSqliteStore({ path: file }), source: { type: 'sqlite', path: file, session: SESSION }, clean: async () => {} }
}

function waitForEvent(on, off, type, match, ms = 20000) {
  return new Promise((resolve, reject) => {
    const timer = setTimeout(() => {
      off(type, listener)
      reject(new Error(`timed out waiting for ${type}`))
    }, ms)
    const listener = (event) => {
      if (!match(event)) return
      clearTimeout(timer)
      off(type, listener)
      resolve(event)
    }
    on(type, listener)
  })
}

async function startZapo(server, storage) {
  const store = createStore({
    backends: { [kind]: storage.open() },
    providers: Object.fromEntries(DOMAINS.map((domain) => [domain, kind])),
  })
  const client = new WaClient({
    store,
    sessionId: SESSION,
    chatSocketUrls: [server.url],
    testHooks: { noiseRootCa: { publicKey: server.noiseRootCa.publicKey, serial: 0 } },
  })
  const on = client.on.bind(client)
  const off = client.off.bind(client)
  const zapo = {
    client,
    store,
    received: (text, chat) =>
      waitForEvent(on, off, 'message', (e) => !e.key?.fromMe && textOf(e.message) === text && (!chat || e.key?.remoteJid === chat)),
    async close() {
      await client.disconnect().catch(() => {})
      await store.destroy?.()
    },
  }
  zapo.opened = waitForEvent(on, off, 'connection', (e) => e.status === 'open', 30000)
  zapo.qr = waitForEvent(on, off, 'auth_qr', () => true, 30000)
  return zapo
}

function follow(peers, pipeline) {
  for (const peer of peers) peer.deps.pushStanza = (stanza) => pipeline.sendStanza(stanza)
}

async function talk(label, { ana, dora, received, send }) {
  const fromAna = `${label}: hola, soy ana`
  const gotAna = received(fromAna)
  await ana.sendConversation(fromAna)
  await gotAna

  const toAna = `${label}: hola ana`
  const answer = ana.expectMessage({ timeoutMs: 20000 })
  await send(ana.jid, toAna)
  const reply = await answer
  assert.equal(textOf(reply.message), toAna)

  const fromDora = `${label}: hola grupo`
  const gotDora = received(fromDora, GROUP)
  await dora.sendGroupConversation(GROUP, fromDora)
  await gotDora

  const toGroup = `${label}: hola a todos`
  const inGroup = dora.expectGroupMessage(GROUP, { timeoutMs: 20000, senderJid: BOT_DEVICE })
  await send(GROUP, toGroup)
  const groupReply = await inGroup
  assert.equal(textOf(groupReply.message), toGroup)
  return { pairwise: reply.encType, group: groupReply.encType }
}

async function main() {
  const server = await FakeWaServer.start()
  const noiseRootHex = Buffer.from(server.noiseRootCa.publicKey).toString('hex')
  const storage = zapoStorage()

  const first = await startZapo(server, storage)
  const connecting = first.client.connect()
  const registration = await server.waitForAuthenticatedPipeline(30000)
  await server.runPairing(registration, { deviceJid: BOT_DEVICE }, async () => {
    const parsed = parsePairingQrString((await first.qr).qr)
    return { advSecretKey: parsed.advSecretKey, identityPublicKey: parsed.identityPublicKey }
  })
  let pipeline = await server.waitForNextAuthenticatedPipeline(30000)
  await first.opened
  await connecting
  const ana = await server.createFakePeer({ jid: '5511888888888@s.whatsapp.net' }, pipeline)
  const dora = await server.createFakePeer({ jid: '5511666666666@s.whatsapp.net' }, pipeline)
  server.createFakeGroup({ groupJid: GROUP, subject: 'ida y vuelta', participants: [ana, dora], creator: ana.jid })
  await talk('zapo', { ana, dora, received: first.received, send: (jid, text) => first.client.message.send(jid, text) })
  await new Promise((resolve) => setTimeout(resolve, 500))
  await first.close()
  step(`zapo ${kind}: vinculado, privado y grupo, guardado en su propio store`)

  const check = await importZapo(storage.source, null, { check: true })
  assert.equal(check.checkOnly, true)
  assert.deepEqual(check.failed, [])
  assert.ok(check.sessions >= 2, `sessions: ${check.sessions}`)
  assert.ok(check.senderKeys >= 2, `sender keys: ${check.senderKeys}`)
  assert.equal(check.me, BOT_DEVICE)
  // With Mongo the session lands in SoblendWA's own MongoStore, so that store goes through the whole trip too.
  const target =
    kind === 'mongo'
      ? { type: 'mongo', url: mongoUrl, database: `${storage.source.database}_soblendwa`, account: SESSION }
      : { type: 'sqlite', path: path.join(scratch, 'soblendwa.db') }
  const imported = await importZapo(storage.source, target)
  assert.deepEqual(imported.failed, [])
  assert.equal(imported.sessions, check.sessions)
  step(`importado: ${imported.sessions} sesiones, ${imported.senderKeys} sender keys, ${imported.preKeys} pre-keys`)

  const bot = await SoblendWA.create({ store: target, server: { wsUrl: server.url, noiseRootHex } })
  assert.equal(bot.me, BOT_DEVICE)
  const ready = bot.waitFor('connected', null, 30000)
  const next = server.waitForNextAuthenticatedPipeline(30000)
  await bot.connect()
  pipeline = await next
  assert.equal(pipeline.clientPayload.kind, 'login', 'the imported session logs in, no pairing')
  await ready
  follow([ana, dora], pipeline)
  const withSoblend = await talk('soblendwa', {
    ana,
    dora,
    received: (text, chat) => bot.waitFor('message', (m) => m.text === text && (!chat || m.info.chat === chat), 20000),
    send: (jid, text) => bot.sendText(jid, text, { preview: false }),
  })
  assert.equal(withSoblend.pairwise, 'msg', 'SoblendWA kept the ratchet zapo started instead of a new session')
  assert.equal(withSoblend.group, 'skmsg')
  await bot.close()
  step('SoblendWA siguió las mismas sesiones: privado sin X3DH nuevo y grupo con las sender keys de zapo')

  const back = await rollbackZapo(target, storage.source)
  assert.deepEqual(back.failed, [])
  const again = await startZapo(server, storage)
  const reconnect = server.waitForNextAuthenticatedPipeline(30000)
  await again.client.connect()
  pipeline = await reconnect
  assert.equal(pipeline.clientPayload.kind, 'login')
  await again.opened
  follow([ana, dora], pipeline)
  const withZapo = await talk('zapo otra vez', {
    ana,
    dora,
    received: again.received,
    send: (jid, text) => again.client.message.send(jid, text),
  })
  assert.equal(withZapo.pairwise, 'msg', 'zapo continued the ratchet SoblendWA advanced')
  await again.close()
  step(`vuelta a zapo: ${back.sessions} sesiones devueltas y zapo sigue la conversación`)

  await storage.clean()
  await server.stop().catch(() => {})
  try {
    fs.rmSync(scratch, { recursive: true, force: true })
  } catch {
    // Windows keeps the SQLite file locked until the process exits; the temp folder can stay.
  }
  console.log(`zapo ${kind}: ida y vuelta completa`)
  process.exit(0)
}

main().catch((error) => {
  console.error(error)
  process.exit(1)
})
