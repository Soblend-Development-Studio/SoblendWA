'use strict'

// Run from a folder where soblendwa-node was installed from npm: connects to WhatsApp until the first QR and stops.
// Nothing gets linked; it proves the published binary loads on this machine and gets through the real handshake.

const installed = require.resolve('soblendwa-node', { paths: [process.cwd()] })
const { SoblendWA, version } = require(installed)

async function main() {
  const started = Date.now()
  const bot = await SoblendWA.create({ store: { type: 'memory' } })
  const qr = new Promise((resolve) => bot.on('qr', resolve))
  const timeout = new Promise((_, reject) => setTimeout(() => reject(new Error('no QR within 60 s')), 60000))
  await bot.connect()
  await Promise.race([qr, timeout])
  console.log(`soblendwa-node ${version()} on ${process.platform}-${process.arch}, Node ${process.version}: QR after ${Date.now() - started} ms`)
  await bot.disconnect()
}

main().then(
  () => process.exit(0),
  (error) => {
    console.error(error)
    process.exit(1)
  },
)
