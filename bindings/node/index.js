'use strict'

const { EventEmitter } = require('node:events')
const { existsSync } = require('node:fs')
const { join } = require('node:path')

function platformFile() {
  const platform = `${process.platform}-${process.arch}`
  if (process.platform === 'win32') return `soblendwa.${platform}-msvc.node`
  if (process.platform === 'linux') {
    const libc = process.report.getReport().header.glibcVersionRuntime ? 'gnu' : 'musl'
    return `soblendwa.${platform}-${libc}.node`
  }
  return `soblendwa.${platform}.node`
}

function loadNative() {
  const expected = platformFile()
  const candidates = [join(__dirname, expected), join(__dirname, 'soblendwa.node')]
  const found = candidates.find((file) => existsSync(file))
  if (!found) {
    throw new Error(
      `SoblendWA has no native build here (${expected}). Use soblendwa-server with soblendwa-client, or build it with "npm run build" from the repository.`,
    )
  }
  return require(found)
}

const native = loadNative()

function isCustomStore(store) {
  return Boolean(store) && typeof store.get === 'function'
}

function storeHandler(store) {
  const apply = async (writes) => {
    const decoded = writes.map((write) => (write.value ? { ...write, value: Buffer.from(write.value, 'base64') } : write))
    if (typeof store.apply === 'function') return store.apply(decoded)
    for (const write of decoded) {
      if (write.op === 'set') await store.set(write.kind, write.key, write.value)
      else if (write.op === 'remove') await store.remove(write.kind, write.key)
      else await store.clear()
    }
  }
  return async (raw) => {
    const request = JSON.parse(raw)
    switch (request.op) {
      case 'loadCreds':
        return (await store.loadCreds()) ?? null
      case 'saveCreds':
        await store.saveCreds(request.data)
        return null
      case 'get': {
        const value = await store.get(request.kind, request.key)
        return value == null ? null : Buffer.from(value).toString('base64')
      }
      case 'keys':
        return typeof store.keys === 'function' ? JSON.stringify(await store.keys(request.kind)) : null
      case 'apply':
        await apply(request.writes)
        return null
      default:
        throw new Error(`unknown store operation ${request.op}`)
    }
  }
}

class SoblendWA extends EventEmitter {
  static async create(options = {}) {
    if (isCustomStore(options.store)) {
      const { store, ...rest } = options
      return new SoblendWA(await native.NativeClient.openWithStore(rest, storeHandler(store)))
    }
    return new SoblendWA(await native.NativeClient.open(options))
  }

  stats() {
    return this.native.stats()
  }

  pendingSends() {
    return this.native.pendingSends()
  }

  constructor(client) {
    super()
    this.native = client
    this.pumping = false
  }

  get me() {
    return this.native.me
  }

  get connected() {
    return this.native.connected
  }

  async connect() {
    await this.native.connect()
    this.pump()
    return this
  }

  async pump() {
    if (this.pumping) return
    this.pumping = true
    try {
      for (;;) {
        const event = await this.native.nextEvent()
        if (!event) break
        this.emit(event.type, event)
        this.emit('event', event)
      }
    } catch (error) {
      this.emit('error', error)
    } finally {
      this.pumping = false
    }
  }

  async *events() {
    for (;;) {
      const event = await this.native.nextEvent()
      if (!event) return
      yield event
    }
  }

  disconnect() {
    return this.native.disconnect()
  }

  logout() {
    return this.native.logout()
  }

  ping() {
    return this.native.ping()
  }

  elapsed(message) {
    return this.native.elapsed(idOf(message))
  }

  requestPairCode(phone) {
    return this.native.requestPairCode(String(phone))
  }

  sendText(chat, text, { quoted, preview } = {}) {
    if (preview === undefined) return this.native.sendText(chat, text, quoted ?? null)
    return this.native.sendTextChoosingPreview(chat, text, quoted ?? null, preview)
  }

  sendProto(chat, bytes) {
    return this.native.sendProto(chat, Buffer.from(bytes))
  }

  reply(message, text) {
    return this.native.reply(idOf(message), text)
  }

  react(message, emoji) {
    return this.native.react(idOf(message), emoji)
  }

  sendImage(chat, data, options = {}) {
    return this.sendMedia(chat, 'image', data, options)
  }

  sendVideo(chat, data, options = {}) {
    return this.sendMedia(chat, 'video', data, options)
  }

  sendAudio(chat, data, options = {}) {
    return this.sendMedia(chat, 'audio', data, options)
  }

  sendDocument(chat, data, options = {}) {
    return this.sendMedia(chat, 'document', data, options)
  }

  sendSticker(chat, data, options = {}) {
    return this.sendMedia(chat, 'sticker', data, options)
  }

  sendMedia(chat, kind, data, { quoted, ...options } = {}) {
    return this.native.sendMedia(chat, kind, Buffer.from(data), { ...options, quotedId: quoted ? idOf(quoted) : null })
  }

  sendFile(chat, kind, path, { quoted, ...options } = {}) {
    return this.native.sendMediaFile(chat, kind, String(path), { ...options, quotedId: quoted ? idOf(quoted) : null })
  }

  download(message) {
    return this.native.download(idOf(message))
  }

  downloadToFile(message, path) {
    return this.native.downloadToFile(idOf(message), String(path))
  }

  sendInteractive(chat, { quoted, ...spec }) {
    return this.native.sendInteractive(chat, { ...spec, quotedId: quoted ? idOf(quoted) : null })
  }

  sendButtons(chat, text, buttons, footer = null) {
    return this.native.sendButtons(chat, text, buttons, footer)
  }

  sendList(chat, { title, text, buttonText, sections, footer = null }) {
    return this.native.sendList(chat, title, text, buttonText, sections, footer)
  }

  modifyChat(chat, action, seconds = null) {
    return this.native.modifyChat(chat, action, seconds)
  }

  labelChat(chat, labelId, labeled = true) {
    return this.native.label(chat, String(labelId), labeled, null)
  }

  labelMessage(chat, message, labelId, labeled = true) {
    return this.native.label(chat, String(labelId), labeled, idOf(message))
  }

  editLabel(id, name, color = 0) {
    return this.native.editLabel(String(id), name, color)
  }

  deleteLabel(id) {
    return this.native.editLabel(String(id), null, null)
  }

  saveQuickReply(shortcut, text) {
    return this.native.saveQuickReply(shortcut, text)
  }

  sendContacts(chat, cards) {
    return this.native.sendContacts(chat, cards.map(({ name, phone }) => ({ name, phone: String(phone) })))
  }

  sendMentionAll(chat, text) {
    return this.native.sendMentionAll(chat, text)
  }

  sendGroupInvite(to, group, caption = null) {
    return this.native.sendGroupInvite(to, group, caption)
  }

  setMemberLabel(group, label) {
    return this.native.setMemberLabel(group, label)
  }

  markPlayed(message) {
    return this.native.markPlayed(idOf(message))
  }

  botList() {
    return this.native.account('bots', null)
  }

  reachoutTimelock() {
    return this.native.account('timelock', null)
  }

  newChatMessageCap() {
    return this.native.account('messageCap', null)
  }

  disappearingModes(...jids) {
    return this.native.account('disappearing', jids.flat().map(String))
  }

  chats() {
    return this.native.chats()
  }

  contacts() {
    return this.native.contacts()
  }

  messages(chat, { limit = 50, before = null } = {}) {
    return this.native.messages(chat, limit, before)
  }

  search(text, limit = 50) {
    return this.native.search(text, limit)
  }

  sendPoll(chat, name, options, selectable = 1) {
    return this.native.sendPoll(chat, name, options, selectable)
  }

  votePoll(chat, pollId, options) {
    return this.native.votePoll(chat, pollId, options)
  }

  edit(chat, messageId, text) {
    return this.native.edit(chat, messageId, text)
  }

  revoke(chat, messageId) {
    return this.native.revoke(chat, messageId)
  }

  typing(chat, state = 'composing') {
    return this.native.chatPresence(chat, state)
  }

  setOnline(available = true) {
    return this.native.setPresence(available)
  }

  groupInfo(chat) {
    return this.native.groupInfo(chat)
  }

  joinedGroups() {
    return this.native.joinedGroups()
  }

  onWhatsApp(...phones) {
    return this.native.onWhatsapp(phones.flat().map(String))
  }

  profilePicture(jid, fullSize = false) {
    return this.native.profilePicture(jid, fullSize)
  }

  block(jid) {
    return this.native.block(jid, true)
  }

  unblock(jid) {
    return this.native.block(jid, false)
  }
}

function idOf(message) {
  if (typeof message === 'string') return message
  return message?.info?.id ?? message?.id
}

function importBaileys(source, target = { type: 'file', path: 'session' }) {
  return native.importBaileys(String(source), target)
}

module.exports = {
  SoblendWA,
  NativeClient: native.NativeClient,
  importBaileys,
  credits: native.credits,
  version: native.version,
}
