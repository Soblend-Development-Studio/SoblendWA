'use strict'

const { FakeWhatsApp } = require('./index.js')

const counts = (process.argv[2] || '20,50').split(',').map(Number)
const megabytes = (bytes) => Math.round((bytes / 1048576) * 10) / 10
const settle = (ms) => new Promise((resolve) => setTimeout(resolve, ms))

function rss() {
  if (global.gc) global.gc()
  return process.memoryUsage().rss
}

async function measure(fake, total, offset) {
  const before = rss()
  const bots = []
  for (let i = 0; i < total; i += 1) {
    bots.push(await fake.connect(String(5215600000000 + offset + i)))
  }
  for (let round = 0; round < 3; round += 1) {
    await Promise.all(
      bots.map(async (bot, i) => {
        const next = bots[(i + 1) % bots.length]
        const arrived = next.waitFor('message', (m) => m.text === `ronda ${round} de ${i}`, 20000)
        await bot.sendText(next.me.split(':')[0], `ronda ${round} de ${i}`)
        await arrived
      }),
    )
  }
  await settle(500)
  const loaded = rss()

  const victim = bots[0]
  fake.dropConnections(victim.me.split(':')[0].split('@')[0])
  const survivor = bots[1]
  const target = bots[2]
  const arrived = target.waitFor('message', (m) => m.text === 'sigo aqui', 20000)
  await survivor.sendText(target.me.split(':')[0], 'sigo aqui')
  await arrived
  await victim.waitFor('connected', null, 20000)

  await bots[3].close()
  const stillTalking = target.waitFor('message', (m) => m.text === 'sin la cuenta 3', 20000)
  await survivor.sendText(target.me.split(':')[0], 'sin la cuenta 3')
  await stillTalking

  const result = {
    accounts: total,
    rssBeforeMb: megabytes(before),
    rssWithAccountsMb: megabytes(loaded),
    perAccountMb: megabytes((loaded - before) / total),
    oneDroppedOthersKeptWorking: true,
    oneClosedOthersKeptWorking: true,
  }
  for (const bot of bots) await bot.close().catch(() => {})
  await settle(500)
  return result
}

async function main() {
  const fake = await FakeWhatsApp.start()
  const results = []
  let offset = 0
  for (const total of counts) {
    results.push(await measure(fake, total, offset))
    offset += total
  }
  console.log(JSON.stringify({ node: process.version, results }, null, 2))
  process.exit(0)
}

main().catch((error) => {
  console.error(error)
  process.exit(1)
})
