const DEFAULT_URL = 'ws://127.0.0.1:8787'

class Emitter {
  constructor() {
    this.listeners = new Map()
  }

  on(type, listener) {
    if (!this.listeners.has(type)) this.listeners.set(type, new Set())
    this.listeners.get(type).add(listener)
    return this
  }

  off(type, listener) {
    this.listeners.get(type)?.delete(listener)
    return this
  }

  once(type, listener) {
    const wrapped = (value) => {
      this.off(type, wrapped)
      listener(value)
    }
    return this.on(type, wrapped)
  }

  emit(type, value) {
    for (const listener of [...(this.listeners.get(type) ?? [])]) {
      try {
        const outcome = listener(value)
        if (outcome && typeof outcome.catch === 'function') outcome.catch((error) => this.emit('error', error))
      } catch (error) {
        if (type !== 'error') this.emit('error', error)
      }
    }
  }
}

async function socketClass() {
  if (typeof globalThis.WebSocket === 'function') return globalThis.WebSocket
  try {
    const { WebSocket } = await import('ws')
    return WebSocket
  } catch {
    throw new Error('This runtime has no WebSocket: use Node 22+, Bun, Deno or a browser, or install the "ws" package')
  }
}

function toBase64(data) {
  if (typeof data === 'string') return data
  const bytes = data instanceof Uint8Array ? data : new Uint8Array(data)
  if (typeof Buffer !== 'undefined') return Buffer.from(bytes).toString('base64')
  let binary = ''
  for (let i = 0; i < bytes.length; i += 0x8000) binary += String.fromCharCode(...bytes.subarray(i, i + 0x8000))
  return btoa(binary)
}

function fromBase64(text) {
  if (typeof Buffer !== 'undefined') return Buffer.from(text, 'base64')
  return Uint8Array.from(atob(text), (c) => c.charCodeAt(0))
}

function idOf(message) {
  if (typeof message === 'string') return message
  return message?.info?.id ?? message?.id
}

function media({ quoted, ...options } = {}) {
  return { ...options, quotedId: quoted ? idOf(quoted) : undefined }
}

export class SoblendClient extends Emitter {
  static async connect(url = DEFAULT_URL, options = {}) {
    const client = new SoblendClient(url, options)
    await client.open()
    return client
  }

  constructor(url = DEFAULT_URL, { session = 'default', token, reconnect = true, timeout = 120000 } = {}) {
    super()
    this.base = String(url).replace(/^http/, 'ws').replace(/\/+$/, '')
    this.session = session
    this.token = token
    this.reconnect = reconnect
    this.timeout = timeout
    this.pending = new Map()
    this.counter = 0
    this.state = { me: null, connected: false, qr: null, methods: [] }
    this.closing = false
    this.attempt = 0
  }

  get me() {
    return this.state.me
  }

  get connected() {
    return this.state.connected
  }

  get qr() {
    return this.state.qr
  }

  async open() {
    const Socket = await socketClass()
    const query = this.token ? `?token=${encodeURIComponent(this.token)}` : ''
    const socket = new Socket(`${this.base}/ws/${encodeURIComponent(this.session)}${query}`)
    this.socket = socket
    return new Promise((resolve, reject) => {
      let greeted = false
      socket.onmessage = (message) => {
        const frame = JSON.parse(typeof message.data === 'string' ? message.data : String(message.data))
        if (frame.type === 'hello') {
          greeted = true
          this.attempt = 0
          Object.assign(this.state, { me: frame.me, connected: frame.connected, qr: frame.qr, methods: frame.methods })
          this.emit('open', frame)
          resolve(this)
          return
        }
        this.handle(frame)
      }
      socket.onerror = (error) => {
        if (!greeted) reject(new Error(`could not reach SoblendWA at ${this.base}: ${error?.message ?? 'connection failed'}`))
      }
      socket.onclose = (event) => {
        this.failPending(new Error('the connection to the SoblendWA server closed'))
        this.emit('close', { code: event?.code, reason: event?.reason })
        if (!greeted) reject(new Error(`SoblendWA refused the connection (${event?.code ?? 'closed'}): check the url and token`))
        if (greeted && this.reconnect && !this.closing) this.retry()
      }
    })
  }

  retry() {
    const delay = Math.min(30000, 500 * 2 ** this.attempt++)
    setTimeout(() => {
      this.open().catch((error) => {
        this.emit('error', error)
        if (this.reconnect && !this.closing) this.retry()
      })
    }, delay)
  }

  handle(frame) {
    if (frame.id !== undefined && frame.id !== null && this.pending.has(frame.id)) {
      const { resolve, reject, timer } = this.pending.get(frame.id)
      this.pending.delete(frame.id)
      clearTimeout(timer)
      if (frame.error) reject(new Error(frame.error.message))
      else resolve(frame.result)
      return
    }
    if (frame.type === 'lagged') {
      this.emit('lagged', frame.missed)
      return
    }
    const event = frame.event
    if (!event) return
    if (event.type === 'qr') this.state.qr = event.code
    if (event.type === 'connected') Object.assign(this.state, { connected: true, qr: null })
    if (event.type === 'disconnected') this.state.connected = false
    if (event.type === 'pairSuccess') this.state.me = event.id
    if (event.type === 'loggedOut') Object.assign(this.state, { connected: false, me: null })
    this.emit(event.type, event)
    this.emit('event', event)
  }

  failPending(error) {
    for (const { reject, timer } of this.pending.values()) {
      clearTimeout(timer)
      reject(error)
    }
    this.pending.clear()
  }

  call(method, params = {}) {
    if (!this.socket || this.socket.readyState !== 1) {
      return Promise.reject(new Error('not connected to the SoblendWA server'))
    }
    const id = ++this.counter
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        this.pending.delete(id)
        reject(new Error(`${method} timed out`))
      }, this.timeout)
      this.pending.set(id, { resolve, reject, timer })
      this.socket.send(JSON.stringify({ id, method, params }))
    })
  }

  async *events() {
    const queue = []
    let wake = null
    const push = (event) => {
      queue.push(event)
      wake?.()
    }
    this.on('event', push)
    try {
      for (;;) {
        while (queue.length) yield queue.shift()
        await new Promise((resolve) => (wake = resolve))
        wake = null
      }
    } finally {
      this.off('event', push)
    }
  }

  close() {
    this.closing = true
    this.failPending(new Error('client closed'))
    this.socket?.close()
  }

  status() {
    return this.call('status')
  }

  ping() {
    return this.call('ping')
  }

  stats() {
    return this.call('stats')
  }

  pendingSends() {
    return this.call('pendingSends')
  }

  elapsed(message) {
    return this.call('elapsed', { messageId: idOf(message) })
  }

  requestPairCode(phone) {
    return this.call('requestPairCode', { phone: String(phone) })
  }

  logout() {
    return this.call('logout')
  }

  sendText(chat, text, { quoted, preview } = {}) {
    return this.call('sendText', { chat, text, quotedId: quoted ? idOf(quoted) : undefined, preview })
  }

  reply(message, text) {
    return this.call('reply', { messageId: idOf(message), text })
  }

  react(message, emoji) {
    return this.call('react', { messageId: idOf(message), emoji })
  }

  read(message) {
    return this.call('read', { messageId: idOf(message) })
  }

  markPlayed(message) {
    return this.call('markPlayed', { messageId: idOf(message) })
  }

  sendMedia(chat, kind, data, options = {}) {
    return this.call('sendMedia', { chat, kind, data: toBase64(data), ...media(options) })
  }

  sendImage(chat, data, options) {
    return this.sendMedia(chat, 'image', data, options)
  }

  sendVideo(chat, data, options) {
    return this.sendMedia(chat, 'video', data, options)
  }

  sendAudio(chat, data, options) {
    return this.sendMedia(chat, 'audio', data, options)
  }

  sendDocument(chat, data, options) {
    return this.sendMedia(chat, 'document', data, options)
  }

  sendSticker(chat, data, options) {
    return this.sendMedia(chat, 'sticker', data, options)
  }

  sendFile(chat, kind, path, options = {}) {
    return this.call('sendFile', { chat, kind, path: String(path), ...media(options) })
  }

  async download(message) {
    const { base64 } = await this.call('download', { messageId: idOf(message) })
    return fromBase64(base64)
  }

  downloadToFile(message, path) {
    return this.call('downloadToFile', { messageId: idOf(message), path: String(path) })
  }

  sendPoll(chat, name, options, selectable = 1) {
    return this.call('sendPoll', { chat, name, options, selectable })
  }

  votePoll(chat, pollId, options) {
    return this.call('votePoll', { chat, pollId, options })
  }

  edit(chat, messageId, text) {
    return this.call('edit', { chat, messageId, text })
  }

  revoke(chat, messageId) {
    return this.call('revoke', { chat, messageId })
  }

  typing(chat, state = 'composing') {
    return this.call('typing', { chat, state })
  }

  setOnline(available = true) {
    return this.call('setOnline', { available })
  }

  sendInteractive(chat, { quoted, ...spec }) {
    return this.call('sendInteractive', { chat, ...spec, quotedId: quoted ? idOf(quoted) : undefined })
  }

  sendButtons(chat, text, buttons, footer) {
    return this.call('sendButtons', { chat, text, buttons, footer })
  }

  sendList(chat, { title, text, buttonText, sections, footer }) {
    return this.call('sendList', { chat, title, text, buttonText, sections, footer })
  }

  sendContacts(chat, cards) {
    return this.call('sendContacts', { chat, cards })
  }

  sendMentionAll(chat, text) {
    return this.call('sendMentionAll', { chat, text })
  }

  sendGroupInvite(to, group, caption) {
    return this.call('sendGroupInvite', { to, group, caption })
  }

  setMemberLabel(group, label) {
    return this.call('setMemberLabel', { group, label })
  }

  sendProto(chat, bytes) {
    return this.call('sendProto', { chat, proto: toBase64(bytes) })
  }

  groupInfo(chat) {
    return this.call('groupInfo', { chat })
  }

  joinedGroups() {
    return this.call('joinedGroups')
  }

  onWhatsApp(...phones) {
    return this.call('onWhatsApp', { phones: phones.flat().map(String) })
  }

  profilePicture(jid, fullSize = false) {
    return this.call('profilePicture', { jid, fullSize })
  }

  block(jid) {
    return this.call('block', { jid })
  }

  unblock(jid) {
    return this.call('unblock', { jid })
  }

  modifyChat(chat, action, seconds) {
    return this.call('modifyChat', { chat, action, seconds })
  }

  labelChat(chat, labelId, labeled = true) {
    return this.call('labelChat', { chat, labelId: String(labelId), labeled })
  }

  labelMessage(chat, message, labelId, labeled = true) {
    return this.call('labelMessage', { chat, messageId: idOf(message), labelId: String(labelId), labeled })
  }

  editLabel(id, name, color = 0) {
    return this.call('editLabel', { id: String(id), name, color })
  }

  deleteLabel(id) {
    return this.call('deleteLabel', { id: String(id) })
  }

  saveQuickReply(shortcut, text) {
    return this.call('saveQuickReply', { shortcut, text })
  }

  chats() {
    return this.call('chats')
  }

  contacts() {
    return this.call('contacts')
  }

  messages(chat, { limit = 50, before } = {}) {
    return this.call('messages', { chat, limit, before })
  }

  search(text, limit = 50) {
    return this.call('search', { text, limit })
  }

  botList() {
    return this.call('botList')
  }

  reachoutTimelock() {
    return this.call('reachoutTimelock')
  }

  newChatMessageCap() {
    return this.call('newChatMessageCap')
  }

  disappearingModes(...jids) {
    return this.call('disappearingModes', { jids: jids.flat().map(String) })
  }
}

export function connect(url, options) {
  return SoblendClient.connect(url, options)
}

export default SoblendClient
