export type ErrorCode =
  | 'APPROVAL_REQUIRED'
  | 'PRIVATE_PHOTO'
  | 'MESSAGE_NOT_FOUND'
  | 'NO_DEVICES'
  | 'INVALID_ARGUMENT'
  | 'RATE_LIMITED'
  | 'ACCOUNT_BLOCKED'
  | 'NOT_CONNECTED'
  | 'NOT_LOGGED_IN'
  | 'TIMEOUT'
  | 'INVALID_JID'
  | 'MEDIA_ERROR'
  | 'STORE_ERROR'
  | 'BAD_REQUEST'
  | 'NOT_AUTHORIZED'
  | 'FORBIDDEN'
  | 'NOT_FOUND'
  | 'NOT_ALLOWED'
  | 'NOT_ACCEPTABLE'
  | 'REQUEST_TIMEOUT'
  | 'CONFLICT'
  | 'GONE'
  | 'RATE_OVERLIMIT'
  | 'INTERNAL_SERVER_ERROR'
  | 'NOT_IMPLEMENTED'
  | 'SERVICE_UNAVAILABLE'
  | 'SERVER_ERROR'
  | (string & {})

export declare class SoblendError extends Error {
  readonly code: ErrorCode
  readonly status: number | null
}

export interface Spread {
  p50: number
  p95: number
  max: number
}

export interface TimingSummary {
  samples: number
  queuedMicros: Spread
  uploadMicros: Spread | null
  prepareMicros: Spread
  encryptMicros: Spread
  serverMicros: Spread | null
  totalMicros: Spread
}

export interface GroupParticipant {
  jid: string
  phone_number: string | null
  lid: string | null
  is_admin: boolean
  is_super_admin: boolean
}

export interface GroupInfo {
  jid: string
  subject: string
  subject_owner: string | null
  subject_time: number
  owner: string | null
  creation: number
  description: string | null
  description_id: string | null
  announce: boolean
  locked: boolean
  ephemeral: number
  addressing_mode: string
  join_approval: boolean
  members_can_add: boolean
  linked_parent: string | null
  is_community: boolean
  is_community_announce: boolean
  participants: GroupParticipant[]
}

export interface ParticipantChange {
  jid: string
  status: number
  ok: boolean
}

export type GroupSetting = 'announce' | 'locked' | 'joinApproval' | 'memberAdd' | 'ephemeral'
export type PrivacySetting = 'lastSeen' | 'online' | 'profilePicture' | 'about' | 'readReceipts' | 'groupsAdd' | 'calls' | 'messages'
export type PrivacyValue = 'everyone' | 'contacts' | 'contactsExcept' | 'nobody' | 'matchLastSeen' | 'known'

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
  lost: number
  sendTimings: TimingSummary
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
  rateDelayed: number
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
  thumbnail?: string | Uint8Array | ArrayBuffer
  image?: string | Uint8Array | ArrayBuffer
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
  | { type: 'loggedOut'; reason: string; cause: 'removedFromPhone' | 'sessionInvalidated' | 'userLogout'; code: number | null }
  | { type: 'messageLost'; id: string; chat: string; sender: string; error: string; waitedSecs: number }
  | { type: 'joinedGroup'; group: string; author: string | null; via: 'created' | 'added' | 'invite' }
  | { type: 'leftGroup'; group: string; author: string | null; removed: boolean }
  | { type: 'lidMapping'; phone: string; lid: string }
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
  mentions?: (string | number)[]
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

  sendText(chat: string, text: string, options?: { quoted?: MessageRef; preview?: boolean | LinkPreviewOverride; mentions?: (string | number)[] }): Promise<string>
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

  groupInfo(chat: string): Promise<GroupInfo>
  joinedGroups(): Promise<GroupInfo[]>
  groupMetadata(chat: string): Promise<GroupInfo>
  groupCreate(subject: string, participants?: (string | number)[]): Promise<GroupInfo>
  groupParticipants(chat: string, participants: string | number | (string | number)[], action: 'add' | 'remove' | 'promote' | 'demote'): Promise<ParticipantChange[]>
  groupAdd(chat: string, participants: string | number | (string | number)[]): Promise<ParticipantChange[]>
  groupKick(chat: string, participants: string | number | (string | number)[]): Promise<ParticipantChange[]>
  groupPromote(chat: string, participants: string | number | (string | number)[]): Promise<ParticipantChange[]>
  groupDemote(chat: string, participants: string | number | (string | number)[]): Promise<ParticipantChange[]>
  groupSubject(chat: string, subject: string): Promise<void>
  groupDescription(chat: string, description?: string | null): Promise<void>
  groupSetting(chat: string, setting: GroupSetting, value: boolean | number | 'all' | 'admins'): Promise<void>
  groupPicture(chat: string, data: Uint8Array | ArrayBuffer | string): Promise<void>
  groupLeave(chat: string): Promise<void>
  groupInviteCode(chat: string): Promise<string>
  groupRevokeInvite(chat: string): Promise<string>
  groupJoin(invite: string): Promise<string>
  groupInviteInfo(invite: string): Promise<GroupInfo>
  groupRequests(chat: string): Promise<{ jid: string; method: string | null; requestedAt: number }[]>
  groupRespondRequests(chat: string, participants: string | number | (string | number)[], approve?: boolean): Promise<ParticipantChange[]>
  setProfilePicture(data: Uint8Array | ArrayBuffer | string, target?: string | null): Promise<void>
  removeProfilePicture(target?: string | null): Promise<void>
  setAbout(text: string): Promise<void>
  fetchAbout(...jids: (string | string[])[]): Promise<{ jid: string; text: string | null; setAt: number }[]>
  setPushName(name: string): Promise<void>
  businessProfile(jid: string): Promise<{ jid: string | null; description: string; address: string | null; email: string | null; website: string | null; category: string | null } | null>
  blocklist(): Promise<string[]>
  privacySettings(): Promise<Record<string, string>>
  setPrivacy(setting: PrivacySetting, value: PrivacyValue): Promise<void>
  lidForPhone(phone: string | number): Promise<string | null>
  phoneForLid(lid: string): Promise<string | null>
  restart(): Promise<unknown>
  waitFor<T = SoblendEvent>(type: string, filter?: ((event: T) => boolean) | null, timeout?: number): Promise<T>
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
