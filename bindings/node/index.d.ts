import { EventEmitter } from 'node:events'

export interface StoreWrite {
  op: 'set' | 'remove' | 'clear'
  kind?: string
  key?: string
  value?: Buffer
}

export interface CustomStore {
  loadCreds(): Promise<string | null> | string | null
  saveCreds(json: string): Promise<void> | void
  get(kind: string, key: string): Promise<Buffer | Uint8Array | null> | Buffer | Uint8Array | null
  keys?(kind: string): Promise<string[]> | string[]
  apply?(writes: StoreWrite[]): Promise<void> | void
  set?(kind: string, key: string, value: Buffer): Promise<void> | void
  remove?(kind: string, key: string): Promise<void> | void
  clear?(): Promise<void> | void
}

export interface LinkPreviewOptions {
  enabled?: boolean
  timeoutMs?: number
  uploadHqThumbnail?: boolean
  allowPrivateHosts?: boolean
  maxHtmlBytes?: number
  maxImageBytes?: number
  userAgent?: string
  cacheTtlSecs?: number
}

export interface LinkPreviewOverride {
  url?: string
  title?: string
  description?: string
  thumbnail?: string
  image?: string
  width?: number
  height?: number
  video?: boolean
}

export type StoreOptions =
  | { type: 'memory' }
  | { type: 'file'; path: string }
  | { type: 'sqlite'; path: string; account?: string }
  | { type: 'postgres'; url: string; account?: string }
  | { type: 'redis'; url: string; account?: string }
  | { type: 'mongo'; url: string; database?: string; account?: string }
  | CustomStore

export interface ClientOptions {
  store?: StoreOptions
  linkPreviews?: LinkPreviewOptions
  server?: { wsUrl?: string; noiseRootHex?: string; mediaEndpoint?: string }
  pushName?: string
  deviceName?: string
  platform?: 'chrome' | 'firefox' | 'safari' | 'edge' | 'opera' | 'desktop'
  countryCode?: string
  onlineOnConnect?: boolean
  autoReconnect?: boolean
  proxy?: string
  chatStore?: { path?: string; limit?: number }
}

export type InteractiveButton =
  | { type: 'reply'; id: string; text: string }
  | { type: 'url'; text: string; url: string }
  | { type: 'copy'; text: string; code: string }
  | { type: 'call'; text: string; phone: string }
  | { type: 'select'; title: string; sections: ListSection[] }

export interface ListSection {
  title: string
  rows: { id: string; title: string; description?: string }[]
}

export interface InteractiveSpec {
  body: string
  footer?: string
  title?: string
  subtitle?: string
  buttons: InteractiveButton[]
  quoted?: MessageEvent | string
}

export interface Chat {
  jid: string
  name: string | null
  unread: number
  lastMessageAt: number
  archived: boolean
  pinned: boolean
  mutedUntil: number | null
  locked: boolean
  labels: string[]
}

export interface Contact {
  jid: string
  name: string | null
  firstName: string | null
  pushName: string | null
  lid: string | null
  phone: string | null
}

export interface StoredMessage {
  id: string
  chat: string
  sender: string | null
  fromMe: boolean
  timestamp: number
  pushName: string | null
  text: string | null
  kind: string
  status: 'error' | 'pending' | 'sent' | 'delivered' | 'read' | 'played'
  starred: boolean
  edited: boolean
  revoked: boolean
  reactions: Record<string, string>
  pollVotes: Record<string, string[]>
}

export interface BaileysImport {
  me: string | null
  sessions: number
  preKeys: number
  senderKeys: number
  appStateKeys: number
  appStateVersions: number
  lidMappings: number
  skipped: string[]
  failed: { file: string; error: string }[]
}

export interface MediaOptions {
  caption?: string
  mimetype?: string
  fileName?: string
  voiceNote?: boolean
  gif?: boolean
  viewOnce?: boolean
  seconds?: number
  quoted?: MessageEvent | string
}

export interface MessageInfo {
  id: string
  chat: string
  sender: string
  senderAlt: string | null
  fromMe: boolean
  isGroup: boolean
  pushName: string | null
  timestamp: number
  category: string | null
  offline: boolean
  edit: string | null
  serverId: string | null
  elapsedMicros: number
}

export interface MessageEvent {
  type: 'message'
  info: MessageInfo
  kind: string
  mediaType: string | null
  text: string | null
  quotedId: string | null
  mentions: string[]
  reaction: { targetId: string | null; emoji: string | null } | null
  reply: { id: string; text: string | null; source: 'buttons' | 'list' | 'template' | 'nativeFlow'; flow: string | null; params: unknown } | null
  proto: string
}

export type SoblendEvent =
  | MessageEvent
  | { type: 'qr'; code: string }
  | { type: 'pairSuccess'; id: string; lid: string | null; platform: string | null; businessName: string | null }
  | { type: 'connected' }
  | { type: 'disconnected'; reason: { type: string; detail?: string }; reconnecting: boolean }
  | { type: 'loggedOut'; reason: string }
  | { type: 'serverAck'; id: string; chat: string; error: number | null; latencyMicros: number | null }
  | { type: 'pollVote'; chat: string; pollId: string; voter: string; selected: string[]; timestamp: number }
  | { type: string; [field: string]: unknown }

export declare class NativeClient {
  static open(options?: ClientOptions): Promise<NativeClient>
  get me(): string | null
  get connected(): boolean
  nextEvent(): Promise<SoblendEvent | null>
}

export declare class SoblendWA extends EventEmitter {
  static create(options?: ClientOptions): Promise<SoblendWA>
  readonly native: NativeClient
  get me(): string | null
  get connected(): boolean
  connect(): Promise<this>
  events(): AsyncGenerator<SoblendEvent>
  disconnect(): Promise<void>
  logout(): Promise<void>
  ping(): Promise<number>
  stats(): Record<string, unknown>
  pendingSends(): Promise<Array<{ id: string; chat: string; queuedAt: number; attempts: number }>>
  elapsed(message: MessageEvent | string): number | null
  requestPairCode(phone: string | number): Promise<string>
  sendText(chat: string, text: string, options?: { quoted?: string; preview?: boolean | LinkPreviewOverride }): Promise<string>
  sendProto(chat: string, bytes: Uint8Array): Promise<string>
  reply(message: MessageEvent | string, text: string): Promise<string>
  react(message: MessageEvent | string, emoji: string): Promise<string>
  sendImage(chat: string, data: Uint8Array, options?: MediaOptions): Promise<string>
  sendVideo(chat: string, data: Uint8Array, options?: MediaOptions): Promise<string>
  sendAudio(chat: string, data: Uint8Array, options?: MediaOptions): Promise<string>
  sendDocument(chat: string, data: Uint8Array, options?: MediaOptions): Promise<string>
  sendSticker(chat: string, data: Uint8Array, options?: MediaOptions): Promise<string>
  sendMedia(chat: string, kind: 'image' | 'video' | 'audio' | 'document' | 'sticker', data: Uint8Array, options?: MediaOptions): Promise<string>
  sendFile(chat: string, kind: 'image' | 'video' | 'audio' | 'document' | 'sticker', path: string, options?: MediaOptions): Promise<string>
  download(message: MessageEvent | string): Promise<Buffer>
  downloadToFile(message: MessageEvent | string, path: string): Promise<number>
  sendInteractive(chat: string, spec: InteractiveSpec): Promise<string>
  sendButtons(chat: string, text: string, buttons: { id: string; text: string }[], footer?: string | null): Promise<string>
  sendList(chat: string, list: { title: string; text: string; buttonText: string; sections: ListSection[]; footer?: string | null }): Promise<string>
  modifyChat(chat: string, action: 'archive' | 'unarchive' | 'pin' | 'unpin' | 'mute' | 'unmute' | 'read' | 'unread' | 'delete' | 'clear', seconds?: number | null): Promise<void>
  labelChat(chat: string, labelId: string | number, labeled?: boolean): Promise<void>
  labelMessage(chat: string, message: MessageEvent | string, labelId: string | number, labeled?: boolean): Promise<void>
  editLabel(id: string | number, name: string, color?: number): Promise<void>
  deleteLabel(id: string | number): Promise<void>
  saveQuickReply(shortcut: string, text: string): Promise<string>
  sendContacts(chat: string, cards: { name: string; phone: string | number }[]): Promise<string>
  sendMentionAll(chat: string, text: string): Promise<string>
  sendGroupInvite(to: string, group: string, caption?: string | null): Promise<string>
  setMemberLabel(group: string, label: string): Promise<string>
  markPlayed(message: MessageEvent | string): Promise<void>
  botList(): Promise<{ jid: string; personaId: string | null }[]>
  reachoutTimelock(): Promise<{ active: boolean; endsAt: number | null; enforcement: string | null }>
  newChatMessageCap(): Promise<unknown>
  disappearingModes(...jids: (string | string[])[]): Promise<{ jid: string; seconds: number; setAt: number }[]>
  chats(): Chat[]
  contacts(): Contact[]
  messages(chat: string, options?: { limit?: number; before?: number | null }): StoredMessage[]
  search(text: string, limit?: number): StoredMessage[]
  sendPoll(chat: string, name: string, options: string[], selectable?: number): Promise<string>
  votePoll(chat: string, pollId: string, options: string[]): Promise<string>
  edit(chat: string, messageId: string, text: string): Promise<string>
  revoke(chat: string, messageId: string): Promise<string>
  typing(chat: string, state?: 'composing' | 'recording' | 'paused'): Promise<void>
  setOnline(available?: boolean): Promise<void>
  groupInfo(chat: string): Promise<Record<string, unknown>>
  joinedGroups(): Promise<Record<string, unknown>[]>
  onWhatsApp(...phones: (string | number | (string | number)[])[]): Promise<{ query: string; jid: string; exists: boolean; lid: string | null }[]>
  profilePicture(jid: string, fullSize?: boolean): Promise<string | null>
  block(jid: string): Promise<void>
  unblock(jid: string): Promise<void>
  on(event: 'message', listener: (event: MessageEvent) => void): this
  on(event: string, listener: (event: SoblendEvent) => void): this
}

export declare function importBaileys(source: string, target?: StoreOptions): Promise<BaileysImport>
export declare function credits(): string
export declare function version(): string
