'use strict'

const { existsSync } = require('node:fs')
const { join } = require('node:path')
const { files } = require('./package.json')

const missing = files.filter((file) => file.endsWith('.node') && !existsSync(join(__dirname, file)))

if (missing.length > 0) {
  console.error(`Refusing to publish without the native builds: ${missing.join(', ')}`)
  console.error('Publish the soblendwa-node-<version>.tgz attached to the GitHub release instead.')
  process.exit(1)
}
