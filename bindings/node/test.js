'use strict'

const assert = require('node:assert/strict')
const { mkdtempSync, rmSync } = require('node:fs')
const { tmpdir } = require('node:os')
const { join } = require('node:path')
const { SoblendWA, importBaileys, credits, version } = require('./index.js')

async function javascriptStore() {
  let creds = null
  const rows = new Map()
  const store = {
    loadCreds: () => creds,
    saveCreds: (json) => {
      creds = json
    },
    get: async (kind, key) => rows.get(`${kind}|${key}`) ?? null,
    keys: (kind) => [...rows.keys()].filter((id) => id.startsWith(`${kind}|`)).map((id) => id.slice(kind.length + 1)),
    set: (kind, key, value) => {
      rows.set(`${kind}|${key}`, value)
    },
    remove: (kind, key) => {
      rows.delete(`${kind}|${key}`)
    },
    clear: () => rows.clear(),
  }
  const first = await SoblendWA.create({ store })
  assert.ok(creds, 'the credentials must be saved through the JS store')
  const saved = creds
  assert.equal(first.me, null)
  assert.equal(first.stats().connected, false)
  await first.disconnect()
  await SoblendWA.create({ store })
  assert.equal(creds, saved, 'a second client must load the same credentials')
  console.log('a store written in JavaScript keeps the session')
}

async function migration() {
  const scratch = mkdtempSync(join(tmpdir(), 'soblendwa-node-'))
  const database = join(scratch, 'auth.db')
  const fixture = join(__dirname, '..', '..', 'tests', 'fixtures', 'baileys-auth')
  const report = await importBaileys(fixture, { type: 'sqlite', path: database })
  assert.equal(report.sessions, 1)
  assert.equal(report.senderKeys, 1)
  assert.deepEqual(report.failed, [])
  await assert.rejects(importBaileys(fixture, { type: 'sqlite', path: database }), /already holds a linked session/)

  const imported = await SoblendWA.create({ store: { type: 'sqlite', path: database }, chatStore: {} })
  assert.equal(imported.me, '5215511111111:7@s.whatsapp.net')
  assert.deepEqual(imported.chats(), [])
  assert.deepEqual(imported.messages('5215522222222'), [])
  console.log('baileys session imported through the native module')
  process.once('exit', () => {
    try {
      rmSync(scratch, { recursive: true, force: true })
    } catch {}
  })
}

async function main() {
  assert.match(credits(), /Soblend Development Studio/)
  assert.equal(typeof version(), 'string')
  await javascriptStore()
  await migration()
  await assert.rejects(SoblendWA.create({ store: { type: 'memory' }, proxy: 'ftp://nope' }), /unsupported scheme/)

  const bot = await SoblendWA.create({ store: { type: 'memory' }, pushName: 'SoblendWA test' })
  assert.equal(bot.me, null)
  assert.equal(bot.elapsed('unknown'), null)
  await assert.rejects(bot.reply('unknown', 'hola'), /not among the recent messages/)
  await assert.rejects(SoblendWA.create({ store: { type: 'nope' } }))

  const started = Date.now()
  const qr = await new Promise((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error('no QR within 30 s')), 30_000)
    bot.once('qr', (event) => {
      clearTimeout(timer)
      resolve(event.code)
    })
    bot.connect().catch(reject)
  })
  assert.match(qr, /^https:\/\/wa\.me\/settings\/linked_devices#/)
  console.log(`QR received from WhatsApp in ${Date.now() - started} ms through the native module`)
  await bot.disconnect()
  console.log('node bindings: ok')
  process.exit(0)
}

main().catch((error) => {
  console.error(error)
  process.exit(1)
})
