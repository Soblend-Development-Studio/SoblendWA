export interface ConnectOptions {
  session?: string
  token?: string
  reconnect?: boolean
  timeout?: number
}

export interface MessageInfo {
  id: string
  chat: string
  sender: string
  senderAlt: string | null
  senderPhone: string | null
  senderLid: string | null
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

export interface MediaInfo {
  type: 'image' | 'video' | 'gif' | 'ptv' | 'audio' | 'ptt' | 'document' | 'sticker'
  mimetype: string | null
  size: number | null
  caption?: string | null
  seconds?: number | null
  width?: number | null
  height?: number | null
  fileName?: string | null
  title?: string | null
  pages?: number | null
  animated?: boolean
  viewOnce: boolean
}

export interface QuotedInfo {
  id: string
  sender: string | null
  chat: string | null
  text: string | null
  mediaType: string | null
  media: MediaInfo | null
}

export interface SendTimings {
  queuedMicros: number
  uploadMicros: number | null
  prepareMicros: number
  encryptMicros: number
  serverMicros: number | null
  totalMicros: number
}

export interface Stats {
  connected: boolean
  uptimeSecs: number
  received: number
  sent: number
  undecryptable: number
  recovered: number
  duplicates: number
  awaitingRecovery: number
  reconnects: number
  rateLimited: number
  mediaUploaded: number
  mediaReused: number
  outboxResent: number
  panics: number
  cachedDevices: number
  cachedGroups: number
  pendingQueries: number
  openLanes: number
  latencyMicros: number | null
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

export interface PendingSend {
  id: string
  chat: string
  queuedAt: number
  attempts: number
}

export interface MessageEvent {
  type: 'message'
  info: MessageInfo
  kind: string
  mediaType: string | null
  text: string | null
  media: MediaInfo | null
  quoted: QuotedInfo | null
  viewOnce: boolean
  ageSecs: number
  linkPreview: { url: string | null; title: string | null; description: string | null; video: boolean; hasThumbnail: boolean; hasHighQualityThumbnail: boolean; thumbnailWidth: number | null; thumbnailHeight: number | null } | null
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
  | { type: 'messageRecovered'; id: string; chat: string; sender: string }
  | { type: 'accountBlocked'; code: number; reason: string; expires: number | null }
  | { type: 'receipt'; chat: string; sender: string; ids: string[]; receipt: string; timestamp: number; fromMe: boolean }
  | { type: 'serverAck'; id: string; chat: string; error: number | null; latencyMicros: number | null; timings: SendTimings | null }
  | { type: 'pollVote'; chat: string; pollId: string; voter: string; selected: string[]; timestamp: number }
  | { type: 'eventResponse'; chat: string; eventId: string; responder: string; response: string; extraGuests: number; timestamp: number }
  | { type: 'chatUpdate'; update: { type: string; [field: string]: unknown }; timestamp: number; initialSync: boolean }
  | { type: string; [field: string]: unknown }

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

export interface ListSection {
  title: string
  rows: { id: string; title: string; description?: string }[]
}

export type InteractiveButton =
  | { type: 'reply'; id: string; text: string }
  | { type: 'url'; text: string; url: string }
  | { type: 'copy'; text: string; code: string }
  | { type: 'call'; text: string; phone: string }
  | { type: 'select'; title: string; sections: ListSection[] }

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

type MessageRef = MessageEvent | string
type Bytes = Uint8Array | ArrayBuffer | string

export declare class SoblendClient {
  static connect(url?: string, options?: ConnectOptions): Promise<SoblendClient>
  constructor(url?: string, options?: ConnectOptions)
  readonly me: string | null
  readonly connected: boolean
  readonly qr: string | null
  open(): Promise<this>
  close(): void
  call<T = unknown>(method: string, params?: Record<string, unknown>): Promise<T>
  events(): AsyncGenerator<SoblendEvent>
  on(event: 'message', listener: (event: MessageEvent) => unknown): this
  on(event: 'open' | 'close' | 'lagged' | 'error', listener: (value: any) => unknown): this
  on(event: string, listener: (event: SoblendEvent) => unknown): this
  once(event: string, listener: (event: any) => unknown): this
  off(event: string, listener: (event: any) => unknown): this

  status(): Promise<{ me: string | null; connected: boolean }>
  ping(): Promise<number>
  stats(): Promise<Stats>
  pendingSends(): Promise<PendingSend[]>
  elapsed(message: MessageRef): Promise<number | null>
  requestPairCode(phone: string | number): Promise<string>
  logout(): Promise<void>

  sendText(chat: string, text: string, options?: { quoted?: MessageRef; preview?: boolean | LinkPreviewOverride }): Promise<string>
  reply(message: MessageRef, text: string): Promise<string>
  react(message: MessageRef, emoji: string): Promise<string>
  read(message: MessageRef): Promise<void>
  markPlayed(message: MessageRef): Promise<void>
  sendMedia(chat: string, kind: 'image' | 'video' | 'audio' | 'document' | 'sticker', data: Bytes, options?: MediaOptions): Promise<string>
  sendImage(chat: string, data: Bytes, options?: MediaOptions): Promise<string>
  sendVideo(chat: string, data: Bytes, options?: MediaOptions): Promise<string>
  sendAudio(chat: string, data: Bytes, options?: MediaOptions): Promise<string>
  sendDocument(chat: string, data: Bytes, options?: MediaOptions): Promise<string>
  sendSticker(chat: string, data: Bytes, options?: MediaOptions): Promise<string>
  sendFile(chat: string, kind: 'image' | 'video' | 'audio' | 'document' | 'sticker', path: string, options?: MediaOptions): Promise<string>
  download(message: MessageRef): Promise<Uint8Array>
  downloadToFile(message: MessageRef, path: string): Promise<number>
  sendPoll(chat: string, name: string, options: string[], selectable?: number): Promise<string>
  votePoll(chat: string, pollId: string, options: string[]): Promise<string>
  edit(chat: string, messageId: string, text: string): Promise<string>
  revoke(chat: string, messageId: string): Promise<string>
  typing(chat: string, state?: 'composing' | 'recording' | 'paused'): Promise<void>
  setOnline(available?: boolean): Promise<void>
  sendInteractive(chat: string, spec: { body: string; buttons: InteractiveButton[]; title?: string; subtitle?: string; footer?: string; quoted?: MessageRef }): Promise<string>
  sendButtons(chat: string, text: string, buttons: { id: string; text: string }[], footer?: string): Promise<string>
  sendList(chat: string, list: { title: string; text: string; buttonText: string; sections: ListSection[]; footer?: string }): Promise<string>
  sendContacts(chat: string, cards: { name: string; phone: string | number }[]): Promise<string>
  sendMentionAll(chat: string, text: string): Promise<string>
  sendGroupInvite(to: string, group: string, caption?: string): Promise<string>
  setMemberLabel(group: string, label: string): Promise<string>
  sendProto(chat: string, bytes: Bytes): Promise<string>

  groupInfo(chat: string): Promise<Record<string, unknown>>
  joinedGroups(): Promise<Record<string, unknown>[]>
  onWhatsApp(...phones: (string | number | (string | number)[])[]): Promise<{ query: string; jid: string; exists: boolean; lid: string | null }[]>
  profilePicture(jid: string, fullSize?: boolean): Promise<string | null>
  block(jid: string): Promise<void>
  unblock(jid: string): Promise<void>

  modifyChat(chat: string, action: 'archive' | 'unarchive' | 'pin' | 'unpin' | 'mute' | 'unmute' | 'read' | 'unread' | 'delete' | 'clear', seconds?: number): Promise<void>
  labelChat(chat: string, labelId: string | number, labeled?: boolean): Promise<void>
  labelMessage(chat: string, message: MessageRef, labelId: string | number, labeled?: boolean): Promise<void>
  editLabel(id: string | number, name: string, color?: number): Promise<void>
  deleteLabel(id: string | number): Promise<void>
  saveQuickReply(shortcut: string, text: string): Promise<string>

  chats(): Promise<{ jid: string; name: string | null; unread: number; lastMessageAt: number; archived: boolean; pinned: boolean; mutedUntil: number | null; locked: boolean; labels: string[] }[]>
  contacts(): Promise<{ jid: string; name: string | null; firstName: string | null; pushName: string | null; lid: string | null; phone: string | null }[]>
  messages(chat: string, options?: { limit?: number; before?: number }): Promise<StoredMessage[]>
  search(text: string, limit?: number): Promise<StoredMessage[]>

  botList(): Promise<{ jid: string; personaId: string | null }[]>
  reachoutTimelock(): Promise<{ active: boolean; endsAt: number | null; enforcement: string | null }>
  newChatMessageCap(): Promise<unknown>
  disappearingModes(...jids: (string | string[])[]): Promise<{ jid: string; seconds: number; setAt: number }[]>
}

export declare function connect(url?: string, options?: ConnectOptions): Promise<SoblendClient>
export default SoblendClient
