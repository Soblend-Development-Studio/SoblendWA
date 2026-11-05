import assert from 'node:assert/strict'
import { SoblendClient, connect } from './index.js'

const url = process.env.SOBLENDWA_URL ?? 'ws://127.0.0.1:8787'
const token = process.env.SOBLENDWA_TOKEN

await assert.rejects(connect(url, { session: 'js-test', token: 'wrong-token', reconnect: false }))

const started = performance.now()
const bot = await connect(url, { session: 'js-test', token })
assert.ok(bot instanceof SoblendClient)
assert.equal(bot.me, null)

const status = await bot.status()
assert.equal(status.connected, false)
await assert.rejects(bot.sendText('5215511111111', 'hola'), /not logged in/)
await assert.rejects(bot.call('nope'), /unknown method/)
assert.deepEqual(await bot.chats(), [])

const code = bot.qr ?? (await new Promise((resolve) => bot.once('qr', (event) => resolve(event.code))))
assert.match(code, /^https:\/\/wa\.me\/settings\/linked_devices#/)
console.log(`QR received through the SoblendWA server in ${Math.round(performance.now() - started)} ms`)
bot.close()
console.log('js client: ok')
