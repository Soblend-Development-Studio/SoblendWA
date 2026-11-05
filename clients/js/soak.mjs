import { appendFileSync, readFileSync } from 'node:fs'
import { execFileSync } from 'node:child_process'
import { SoblendClient } from './index.js'

const PIXEL = Buffer.from(
  'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg==',
  'base64'
)

function options(argv) {
  const out = { url: 'ws://127.0.0.1:8787', sessions: ['default'], hours: 72, interval: 60, mediaEvery: 10, pids: {}, out: 'soak.jsonl' }
  for (let i = 0; i < argv.length; i += 1) {
    const [key, value] = [argv[i], argv[i + 1]]
    if (key === '--url') out.url = value
    else if (key === '--token') out.token = value
    else if (key === '--sessions') out.sessions = value.split(',').filter(Boolean)
    else if (key === '--hours') out.hours = Number(value)
    else if (key === '--interval') out.interval = Number(value)
    else if (key === '--media-every') out.mediaEvery = Number(value)
    else if (key === '--out') out.out = value
    else if (key === '--webhook') out.webhook = value
    else if (key === '--pid') {
      const [name, pid] = value.includes('=') ? value.split('=') : ['*', value]
      out.pids[name] = Number(pid)
    } else continue
    i += 1
  }
  return out
}

function rssKb(pid) {
  if (!pid) return null
  try {
    const status = readFileSync(`/proc/${pid}/status`, 'utf8')
    const match = status.match(/VmRSS:\s+(\d+)\s+kB/)
    if (match) return Number(match[1])
  } catch {}
  try {
    if (process.platform === 'win32') {
      const row = execFileSync('tasklist', ['/FI', `PID eq ${pid}`, '/FO', 'CSV', '/NH'], { encoding: 'utf8' })
      const kb = row.split('","').at(-1)?.replace(/[^\d]/g, '')
      return kb ? Number(kb) : null
    }
    return Number(execFileSync('ps', ['-o', 'rss=', '-p', String(pid)], { encoding: 'utf8' }).trim()) || null
  } catch {
    return null
  }
}

async function alert(webhook, text) {
  if (!webhook) return
  try {
    await fetch(webhook, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ text, content: text }) })
  } catch (err) {
    console.error('alert failed:', err.message)
  }
}

const config = options(process.argv.slice(2))
const accounts = []
for (const name of config.sessions) {
  const client = await SoblendClient.connect(config.url, { session: name, token: config.token })
  const account = { name, client, sent: 0, failed: 0, acked: 0, rejected: 0, received: 0, undecryptable: 0, recovered: 0, disconnects: 0, blocked: null, waiting: new Map(), lost: 0 }
  client.on('serverAck', (ack) => (ack.error == null ? (account.acked += 1) : (account.rejected += 1)))
  client.on('undecryptableMessage', () => (account.undecryptable += 1))
  client.on('messageRecovered', () => (account.recovered += 1))
  client.on('disconnected', () => (account.disconnects += 1))
  client.on('accountBlocked', (event) => {
    account.blocked = event
    alert(config.webhook, `SoblendWA soak: ${name} was blocked (${event.code}): ${event.reason}`)
  })
  client.on('message', (message) => {
    if (!message.text?.startsWith('soak:') && !message.media) return
    account.received += 1
    for (const peer of accounts) peer.waiting.delete(message.info.id)
  })
  accounts.push(account)
}

const deadline = Date.now() + config.hours * 3600 * 1000
let round = 0
let lastUndecryptable = 0

async function tick() {
  round += 1
  const at = Date.now()
  for (const [index, account] of accounts.entries()) {
    const peer = accounts[(index + 1) % accounts.length]
    const target = (peer.client.me ?? account.client.me)?.replace(/:\d+@/, '@')
    if (!target || account.blocked) continue
    try {
      const id = await account.client.sendText(target, `soak:${account.name}:${round}`)
      account.sent += 1
      if (peer !== account) account.waiting.set(id, at)
      if (config.mediaEvery > 0 && round % config.mediaEvery === 0) {
        const media = await account.client.sendImage(target, PIXEL, { caption: `soak:${account.name}:${round}:image` })
        account.sent += 1
        if (peer !== account) account.waiting.set(media, at)
      }
    } catch (err) {
      account.failed += 1
      console.error(`[${account.name}] send failed: ${err.message}`)
    }
    for (const [id, sentAt] of account.waiting) {
      if (at - sentAt > 120000) {
        account.waiting.delete(id)
        account.lost += 1
      }
    }
  }
  const line = { at: new Date(at).toISOString(), round, accounts: {} }
  for (const account of accounts) {
    const stats = await account.client.stats().catch(() => null)
    const pid = config.pids[account.name] ?? config.pids['*']
    line.accounts[account.name] = {
      rssKb: rssKb(pid),
      sent: account.sent,
      failed: account.failed,
      acked: account.acked,
      rejected: account.rejected,
      received: account.received,
      lost: account.lost,
      undecryptable: account.undecryptable,
      recovered: account.recovered,
      disconnects: account.disconnects,
      blocked: account.blocked,
      stats
    }
  }
  appendFileSync(config.out, JSON.stringify(line) + '\n')
  const undecryptable = accounts.reduce((sum, a) => sum + a.undecryptable, 0)
  if (undecryptable > lastUndecryptable) {
    await alert(config.webhook, `SoblendWA soak: ${undecryptable - lastUndecryptable} new undecryptable messages (round ${round})`)
    lastUndecryptable = undecryptable
  }
  const summary = accounts.map((a) => `${a.name} sent=${a.sent} recv=${a.received} lost=${a.lost} undec=${a.undecryptable} rec=${a.recovered} rss=${line.accounts[a.name].rssKb ?? '?'}kB`).join(' | ')
  console.log(`round ${round} · ${summary}`)
}

function finish() {
  const lost = accounts.reduce((sum, a) => sum + a.lost, 0)
  const undecryptable = accounts.reduce((sum, a) => sum + a.undecryptable, 0)
  const recovered = accounts.reduce((sum, a) => sum + a.recovered, 0)
  console.log(`soak finished after ${round} rounds: lost=${lost} undecryptable=${undecryptable} recovered=${recovered}`)
  for (const account of accounts) account.client.close()
  process.exit(lost === 0 && undecryptable === recovered ? 0 : 1)
}

process.on('SIGINT', finish)
console.log(`SoblendWA soak · ${accounts.length} accounts · every ${config.interval}s for ${config.hours}h · writing ${config.out}`)
while (Date.now() < deadline) {
  await tick()
  await new Promise((resolve) => setTimeout(resolve, config.interval * 1000))
}
finish()
