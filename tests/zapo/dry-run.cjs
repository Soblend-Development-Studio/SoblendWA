'use strict'

// Dry run of a zapo → SoblendWA migration over a real MongoDB, every session in it, without connecting anything.
// Point it at a copy of your database, never at the one your bot is using:
//
//   mongodump --db zapo --archive | mongorestore --archive --nsInclude 'zapo.wa_*' --nsFrom 'zapo.*' --nsTo 'zapo_copia.*'
//   node tests/zapo/dry-run.cjs mongodb://127.0.0.1:27017 zapo_copia
//
// For each collection prefix (zapo's collectionPrefix, one per sub-bot) and each session in it: the read-only check,
// a full import into a throwaway SQLite file, and whether the imported store opens as a linked account. That last
// one does not connect: it only says the account and its credentials made it across. Prints counts and timings
// only, no phone numbers.

const fs = require('node:fs')
const os = require('node:os')
const path = require('node:path')
const { MongoClient } = require('mongodb')
const { SoblendWA, importZapo } = require('../../bindings/node/index.js')

const [, , url, database] = process.argv
if (!url || !database) {
  console.error('uso: node dry-run.cjs <mongodb url> <base>')
  process.exit(2)
}
const now = () => Number(process.hrtime.bigint()) / 1e6

async function main() {
  const mongo = await MongoClient.connect(url)
  const db = mongo.db(database)
  const names = (await db.listCollections({}, { nameOnly: true }).toArray()).map((c) => c.name)
  const prefixes = names.filter((n) => n.endsWith('auth_credentials')).map((n) => n.slice(0, -'auth_credentials'.length))
  const work = []
  for (const prefix of prefixes.sort()) {
    const ids = await db.collection(`${prefix}auth_credentials`).distinct('_id')
    for (const session of ids) work.push({ prefix, session: String(session) })
  }
  await mongo.close()
  console.log(`${names.length} colecciones, ${prefixes.length} prefijos, ${work.length} sesiones`)

  const scratch = fs.mkdtempSync(path.join(os.tmpdir(), 'soblendwa-dry-run-'))
  const rows = []
  const started = now()
  for (const [index, { prefix, session }] of work.entries()) {
    const source = { type: 'mongo', url, database, session, collectionPrefix: prefix }
    const row = { n: index + 1 }
    try {
      let at = now()
      const check = await importZapo(source, null, { check: true })
      row.checkMs = Math.round(now() - at)
      const target = { type: 'sqlite', path: path.join(scratch, `${index}.db`) }
      at = now()
      const done = await importZapo(source, target)
      row.importMs = Math.round(now() - at)
      Object.assign(row, {
        sessions: done.sessions,
        preKeys: done.preKeys,
        senderKeys: done.senderKeys,
        appStateKeys: done.appStateKeys,
        lid: done.lidMappings,
        failed: done.failed.length,
        sameAsCheck: check.sessions === done.sessions && check.senderKeys === done.senderKeys,
      })
      const bot = await SoblendWA.create({ store: target })
      row.linked = Boolean(bot.me)
      await bot.close()
      if (done.failed.length) row.firstFailure = done.failed[0].error
    } catch (error) {
      row.error = error.code || error.message
    }
    rows.push(row)
    console.log(JSON.stringify(row))
  }
  const total = Math.round(now() - started)
  fs.rmSync(scratch, { recursive: true, force: true })

  const ok = rows.filter((r) => !r.error)
  const sum = (key) => ok.reduce((a, r) => a + (r[key] || 0), 0)
  const sorted = (key) => ok.map((r) => r[key]).sort((a, b) => a - b)
  const p = (values, q) => values[Math.min(values.length - 1, Math.floor(values.length * q))] ?? 0
  console.log('\nResumen')
  console.log(`  sesiones de zapo: ${rows.length} · importadas sin error: ${ok.length} · con error: ${rows.length - ok.length}`)
  console.log(`  pasaron la comprobación de vinculación (sin conectar): ${ok.filter((r) => r.linked).length}`)
  console.log(`  sesiones Signal ${sum('sessions')} · sender keys ${sum('senderKeys')} · pre-keys ${sum('preKeys')} · pares LID ${sum('lid')}`)
  console.log(`  elementos que no se pudieron convertir: ${sum('failed')}`)
  console.log(`  simulacro por cuenta p50/máx: ${p(sorted('checkMs'), 0.5)}/${p(sorted('checkMs'), 1)} ms`)
  console.log(`  importación por cuenta p50/máx: ${p(sorted('importMs'), 0.5)}/${p(sorted('importMs'), 1)} ms`)
  console.log(`  todo: ${(total / 1000).toFixed(1)} s`)
  process.exit(rows.length - ok.length ? 1 : 0)
}

main().catch((error) => {
  console.error(error)
  process.exit(1)
})
