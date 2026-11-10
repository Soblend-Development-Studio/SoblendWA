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

class SoblendError extends Error {
  constructor(code, message, status = null) {
    super(message)
    this.name = 'SoblendError'
    this.code = code
    this.status = status
  }
}

const CODED = /^\[([A-Z_]+)\] ([\s\S]*)$/

function translate(error) {
  const match = typeof error?.message === 'string' ? CODED.exec(error.message) : null
  if (!match) return error
  const status = /error (\d{3})/.exec(match[2])
  return new SoblendError(match[1], match[2], status ? Number(status[1]) : null)
}

function guard(target) {
  return new Proxy(target, {
    get(object, property) {
      const value = Reflect.get(object, property, object)
      if (typeof value !== 'function') return value
      return (...args) => {
        try {
          const result = value.apply(object, args)
          return result && typeof result.then === 'function'
            ? result.catch((error) => {
                throw translate(error)
              })
            : result
        } catch (error) {
          throw translate(error)
        }
      }
    },
  })
}

function bytes(data) {
  if (typeof data === 'string') return data
  return Buffer.from(data).toString('base64')
}

function list(values) {
  return [values].flat(Infinity).filter((value) => value != null).map(String)
}

function previewOption(preview) {
  if (preview === undefined || preview === null || typeof preview === 'boolean') return preview ?? null
  const normalized = { ...preview }
  for (const key of ['thumbnail', 'image']) {
    if (normalized[key] != null && typeof normalized[key] !== 'string') normalized[key] = bytes(normalized[key])
  }
  return normalized
}

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
    return JSON.parse(this.native.statsJson())
  }

  pendingSends() {
    return this.native.pendingSends()
  }

  constructor(client) {
    super({ captureRejections: true })
    this.native = guard(client)
    this.pumping = false
    this.closed = false
  }

  [Symbol.for('nodejs.rejection')](error) {
    this.report(error)
  }

  report(error) {
    if (this.listenerCount('error') > 0) {
      try {
        super.emit('error', error)
        return
      } catch (thrown) {
        error = thrown
      }
    }
    console.error('[SoblendWA]', error)
  }

  deliver(type, event) {
    // not emit(): it stops at the first listener that throws, and one broken command
    // handler shouldn't silence the rest of the bot
    for (const listener of this.rawListeners(type)) {
      try {
        const result = listener.call(this, event)
        if (result && typeof result.then === 'function') result.then(null, (error) => this.report(error))
      } catch (error) {
        this.report(error)
      }
    }
  }

  waitFor(type, filter = null, timeout = 30000) {
    return new Promise((resolve, reject) => {
      const listener = (event) => {
        if (filter && !filter(event)) return
        clearTimeout(timer)
        this.off(type, listener)
        resolve(event)
      }
      const timer = setTimeout(() => {
        this.off(type, listener)
        reject(new SoblendError('TIMEOUT', `no "${type}" event within ${timeout} ms`))
      }, timeout)
      this.on(type, listener)
    })
  }

  call(method, params = {}) {
    return this.native.call(method, params)
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
      while (!this.closed) {
        const raw = await this.native.nextEvents(256)
        if (raw === null) break
        for (const event of JSON.parse(raw)) {
          this.deliver(event.type, event)
          this.deliver('event', event)
        }
      }
    } catch (error) {
      this.report(error)
    } finally {
      this.pumping = false
    }
  }

  async *events() {
    for (;;) {
      const raw = await this.native.nextEvents(256)
      if (raw === null) return
      yield* JSON.parse(raw)
    }
  }

  disconnect() {
    return this.native.disconnect()
  }

  async restart() {
    await this.native.call('restart', {})
    this.pump()
    return this
  }

  async close() {
    this.closed = true
    await this.native.disconnect()
    this.removeAllListeners()
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

  sendText(chat, text, { quoted, preview, mentions } = {}) {
    return this.native.call('sendText', {
      chat: String(chat),
      text,
      quotedId: quoted ? idOf(quoted) : null,
      preview: previewOption(preview),
      mentions: mentions ? list(mentions) : null,
    })
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

  sendMedia(chat, kind, data, { quoted, mentions, ...options } = {}) {
    const extra = { ...options, quotedId: quoted ? idOf(quoted) : null, mentions: mentions ? list(mentions) : [] }
    return this.native.sendMedia(chat, kind, Buffer.from(data), extra)
  }

  sendFile(chat, kind, path, { quoted, mentions, ...options } = {}) {
    const extra = { ...options, quotedId: quoted ? idOf(quoted) : null, mentions: mentions ? list(mentions) : [] }
    return this.native.sendMediaFile(chat, kind, String(path), extra)
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

  groupMetadata(chat) {
    return this.call('groupMetadata', { chat: String(chat) })
  }

  groupCreate(subject, participants = []) {
    return this.call('groupCreate', { subject, participants: list(participants) })
  }

  groupParticipants(chat, participants, action) {
    return this.call('groupParticipants', { chat: String(chat), participants: list(participants), action })
  }

  groupAdd(chat, participants) {
    return this.groupParticipants(chat, participants, 'add')
  }

  groupKick(chat, participants) {
    return this.groupParticipants(chat, participants, 'remove')
  }

  groupPromote(chat, participants) {
    return this.groupParticipants(chat, participants, 'promote')
  }

  groupDemote(chat, participants) {
    return this.groupParticipants(chat, participants, 'demote')
  }

  groupSubject(chat, subject) {
    return this.call('groupSubject', { chat: String(chat), subject })
  }

  groupDescription(chat, description = null) {
    return this.call('groupDescription', { chat: String(chat), description })
  }

  groupSetting(chat, setting, value) {
    return this.call('groupSetting', { chat: String(chat), setting, value })
  }

  groupPicture(chat, data) {
    return this.call('groupPicture', { chat: String(chat), data: bytes(data) })
  }

  groupLeave(chat) {
    return this.call('groupLeave', { chat: String(chat) })
  }

  groupInviteCode(chat) {
    return this.call('groupInviteCode', { chat: String(chat) })
  }

  groupRevokeInvite(chat) {
    return this.call('groupRevokeInvite', { chat: String(chat) })
  }

  groupJoin(invite) {
    return this.call('groupJoin', { invite: String(invite) })
  }

  groupInviteInfo(invite) {
    return this.call('groupInviteInfo', { invite: String(invite) })
  }

  groupRequests(chat) {
    return this.call('groupRequests', { chat: String(chat) })
  }

  groupRespondRequests(chat, participants, approve = true) {
    return this.call('groupRespondRequests', { chat: String(chat), participants: list(participants), approve })
  }

  setProfilePicture(data, target = null) {
    return this.call('setProfilePicture', { data: bytes(data), target: target == null ? null : String(target) })
  }

  removeProfilePicture(target = null) {
    return this.call('removeProfilePicture', { target: target == null ? null : String(target) })
  }

  setAbout(text) {
    return this.call('setAbout', { text })
  }

  fetchAbout(...jids) {
    return this.call('fetchAbout', { jids: list(jids) })
  }

  setPushName(name) {
    return this.call('setPushName', { name })
  }

  businessProfile(jid) {
    return this.call('businessProfile', { jid: String(jid) })
  }

  blocklist() {
    return this.call('blocklist', {})
  }

  privacySettings() {
    return this.call('privacySettings', {})
  }

  setPrivacy(setting, value) {
    return this.call('setPrivacy', { setting, value })
  }

  lidForPhone(phone) {
    return this.call('lidForPhone', { phone: String(phone) })
  }

  phoneForLid(lid) {
    return this.call('phoneForLid', { lid: String(lid) })
  }
}

class FakeWhatsApp {
  static async start() {
    return new FakeWhatsApp(await native.startFakeServer())
  }

  static jid(phone) {
    return `${String(phone).replace(/\D/g, '')}@s.whatsapp.net`
  }

  constructor(server) {
    this.server = guard(server)
  }

  get url() {
    return this.server.url
  }

  async connect(phone, options = {}) {
    const bot = new SoblendWA(await this.server.open(String(phone), options))
    const ready = bot.waitFor('connected', null, 15000)
    await bot.connect()
    await ready
    return bot
  }

  createGroup(subject, creator, members = []) {
    return this.server.createGroup(subject, String(creator), list(members))
  }

  addToGroup(group, by, phones) {
    return this.server.addToGroup(String(group), String(by), list(phones))
  }

  removeFromGroup(group, by, phones) {
    return this.server.removeFromGroup(String(group), String(by), list(phones))
  }

  inviteCode(group) {
    return this.server.inviteCode(String(group))
  }

  setJoinApproval(group, enabled = true) {
    return this.server.setJoinApproval(String(group), enabled)
  }

  dropConnections(phone) {
    return this.server.dropConnections(String(phone))
  }

  refuseConnections(phone, refuse = true) {
    return this.server.refuseConnections(String(phone), refuse)
  }

  block(phone, code = 403) {
    return this.server.block(String(phone), code)
  }

  unblock(phone) {
    return this.server.unblock(String(phone))
  }

  holdAcks(phone, hold = true) {
    return this.server.holdAcks(String(phone), hold)
  }

  corruptNext(phone, messages = 1) {
    return this.server.corruptNext(String(phone), messages)
  }

  expireMedia() {
    return this.server.expireMedia()
  }

  mediaUploads() {
    return this.server.mediaUploads()
  }

  routed() {
    return this.server.routed()
  }

  isOnline(phone) {
    return this.server.isOnline(String(phone))
  }
}

function idOf(message) {
  if (typeof message === 'string') return message
  return message?.info?.id ?? message?.id
}

function importBaileys(source, target = { type: 'file', path: 'session' }) {
  return native.importBaileys(String(source), target).catch((error) => {
    throw translate(error)
  })
}

function importZapo(source, target = null, { check = false } = {}) {
  return native.importZapo(source, target, check).catch((error) => {
    throw translate(error)
  })
}

function rollbackZapo(source, target) {
  return native.rollbackZapo(source, target).catch((error) => {
    throw translate(error)
  })
}

module.exports = {
  SoblendWA,
  importZapo,
  rollbackZapo,
  FakeWhatsApp,
  SoblendError,
  NativeClient: native.NativeClient,
  importBaileys,
  credits: native.credits,
  version: native.version,
}
