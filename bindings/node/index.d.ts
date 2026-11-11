import { EventEmitter } from 'node:events'

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
  thumbnail?: string | Uint8Array | ArrayBuffer
  image?: string | Uint8Array | ArrayBuffer
  width?: number
  height?: number
  video?: boolean
}

export type BuiltinStore =
  | { type: 'memory' }
  | { type: 'file'; path: string }
  | { type: 'sqlite'; path: string; account?: string }
  | { type: 'postgres'; url: string; account?: string }
  | { type: 'redis'; url: string; account?: string }
  | { type: 'mongo'; url: string; database?: string; account?: string }

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
  mediaStore?: BuiltinStore
  mediaCache?: boolean | { ttlSecs: number }
  retryDeadlineSecs?: number
  sendLimit?: boolean | SendLimitOptions
  outbox?: boolean
  workers?: number
  version?: string | [number, number, number]
  queryTimeoutSecs?: number
  connectTimeoutSecs?: number
  keepaliveSecs?: number
}

export interface SendLimitOptions {
  perMinute?: number
  burst?: number
  perChatPerMinute?: number
  perChatBurst?: number
  perGroupPerMinute?: number
  perGroupBurst?: number
  /** How long a send over the limit waits in its chat's line before failing with RATE_LIMITED. 0 fails at once. */
  maxWaitSecs?: number
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
  mentions?: (string | number)[]
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
  | { type: 'serverAck'; id: string; chat: string; error: number | null; latencyMicros: number | null }
  | { type: 'pollVote'; chat: string; pollId: string; voter: string; selected: string[]; timestamp: number }
  | { type: string; [field: string]: unknown }

export declare class NativeClient {
  static open(options?: ClientOptions): Promise<NativeClient>
  call(method: string, params?: Record<string, unknown>): Promise<unknown>
  get me(): string | null
  get connected(): boolean
  nextEvent(): Promise<SoblendEvent | null>
  nextEvents(max: number): Promise<string | null>
  statsJson(): string
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
  stats(): Record<string, unknown> & { lost: number; sendTimings: TimingSummary }
  call<T = unknown>(method: string, params?: Record<string, unknown>): Promise<T>
  close(): Promise<void>
  pendingSends(): Promise<Array<{ id: string; chat: string; queuedAt: number; attempts: number }>>
  elapsed(message: MessageEvent | string): number | null
  requestPairCode(phone: string | number): Promise<string>
  sendText(chat: string, text: string, options?: { quoted?: MessageEvent | string; preview?: boolean | LinkPreviewOverride; mentions?: (string | number)[] }): Promise<string>
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
  on(event: 'message', listener: (event: MessageEvent) => void): this
  on(event: string, listener: (event: SoblendEvent) => void): this
}

export declare class FakeWhatsApp {
  static start(): Promise<FakeWhatsApp>
  static jid(phone: string | number): string
  readonly url: string
  connect(phone: string | number, options?: ClientOptions): Promise<SoblendWA>
  createGroup(subject: string, creator: string | number, members?: (string | number)[]): string
  addToGroup(group: string, by: string | number, phones: string | number | (string | number)[]): void
  removeFromGroup(group: string, by: string | number, phones: string | number | (string | number)[]): void
  inviteCode(group: string): string | null
  setJoinApproval(group: string, enabled?: boolean): void
  dropConnections(phone: string | number): void
  refuseConnections(phone: string | number, refuse?: boolean): void
  block(phone: string | number, code?: number): void
  unblock(phone: string | number): void
  holdAcks(phone: string | number, hold?: boolean): void
  corruptNext(phone: string | number, messages?: number): void
  expireMedia(): void
  slowUploads(ms: number): void
  mediaUploads(): number
  routed(): number
  isOnline(phone: string | number): boolean
}

export type ZapoSource =
  | { type: 'sqlite'; path: string; session?: string }
  | { type: 'mongo'; url: string; database: string; session?: string; collectionPrefix?: string }

export interface ZapoImport {
  session: string
  checkOnly: boolean
  me: string | null
  sessions: number
  preKeys: number
  senderKeys: number
  appStateKeys: number
  appStateVersions: number
  lidMappings: number
  failed: { item: string; error: string }[]
}

export declare function importZapo(source: ZapoSource, target?: BuiltinStore | null, options?: { check?: boolean }): Promise<ZapoImport>
export declare function rollbackZapo(source: BuiltinStore, target: ZapoSource): Promise<Omit<ZapoImport, 'checkOnly' | 'me' | 'lidMappings'>>

export declare function importBaileys(source: string, target?: StoreOptions): Promise<BaileysImport>
export declare function credits(): string
export declare function version(): string
