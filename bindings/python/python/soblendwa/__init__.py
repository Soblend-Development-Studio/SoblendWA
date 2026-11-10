from __future__ import annotations

import asyncio
import base64
import inspect
import json
import logging
import re
from collections import defaultdict, deque
from typing import Any, AsyncIterator, Awaitable, Callable, Iterable, Optional, Union

from ._native import NativeClient, __author__, __version__, credits
from ._native import import_baileys as _import_baileys
from ._native import start_fake_server as _start_fake_server

Event = dict
Handler = Callable[[Event], Union[Awaitable[None], None]]

__all__ = [
    "SoblendWA",
    "SoblendError",
    "FakeWhatsApp",
    "NativeClient",
    "import_baileys",
    "credits",
    "__version__",
    "__author__",
]

_log = logging.getLogger("soblendwa")
_CODED = re.compile(r"^\[([A-Z_]+)\] (.*)$", re.S)


class SoblendError(RuntimeError):
    def __init__(self, code: str, message: str):
        super().__init__(message)
        self.code = code
        self.message = message


def _translate(error: BaseException) -> BaseException:
    match = _CODED.match(str(error))
    if not match:
        return error
    return SoblendError(match.group(1), match.group(2))


async def _settle(awaitable: Awaitable) -> Any:
    try:
        return await awaitable
    except RuntimeError as error:
        raise _translate(error) from None


class _Guarded:
    def __init__(self, target: Any):
        self._target = target

    def __getattr__(self, name: str) -> Any:
        value = getattr(self._target, name)
        if not callable(value):
            return value

        def call(*args: Any, **kwargs: Any) -> Any:
            try:
                result = value(*args, **kwargs)
            except RuntimeError as error:
                raise _translate(error) from None
            return _settle(result) if inspect.isawaitable(result) else result

        return call


def _encode(data: Union[bytes, bytearray, memoryview, str]) -> str:
    if isinstance(data, str):
        return data
    return base64.b64encode(bytes(data)).decode("ascii")


def _texts(values: Iterable[Any]) -> list:
    flat = []
    for value in values:
        if isinstance(value, (list, tuple, set)):
            flat.extend(str(item) for item in value)
        elif value is not None:
            flat.append(str(value))
    return flat


def _preview(preview: Union[bool, dict, None]) -> Any:
    if not isinstance(preview, dict):
        return preview
    return {key: _encode(value) if key in ("thumbnail", "image") and value is not None else value for key, value in preview.items()}


async def import_baileys(source: str, **target: Any) -> dict:
    target = target or {"type": "file", "path": "session"}
    return json.loads(await _settle(_import_baileys(str(source), json.dumps(target))))


def _camel(name: str) -> str:
    head, *tail = name.split("_")
    return head + "".join(part.title() for part in tail)


def _camelize(value: Any) -> Any:
    if isinstance(value, dict):
        return {_camel(key): _camelize(item) for key, item in value.items() if item is not None}
    if isinstance(value, list):
        return [_camelize(item) for item in value]
    return value


def _options(values: dict) -> str:
    return json.dumps(_camelize(values))


def _id(message: Union[Event, str]) -> str:
    if isinstance(message, str):
        return message
    info = message.get("info") or {}
    return info.get("id") or message.get("id")


class SoblendWA:
    def __init__(self, native: NativeClient):
        self.native = _Guarded(native)
        self._handlers: dict[str, list[Handler]] = defaultdict(list)
        self._tasks: set[asyncio.Task] = set()
        self._pending: deque = deque()
        self._reading: Optional[asyncio.Future] = None

    @classmethod
    async def create(cls, **options: Any) -> "SoblendWA":
        return cls(await _settle(NativeClient.open(_options(options))))

    async def call(self, method: str, **params: Any) -> Any:
        return json.loads(await self.native.call(method, json.dumps(params)))

    def _finished(self, task: asyncio.Task) -> None:
        self._tasks.discard(task)
        if not task.cancelled() and task.exception() is not None:
            _log.error("a SoblendWA handler failed", exc_info=task.exception())

    @property
    def me(self) -> Optional[str]:
        return self.native.me

    @property
    def connected(self) -> bool:
        return self.native.connected

    def on(self, kind: str) -> Callable[[Handler], Handler]:
        def register(handler: Handler) -> Handler:
            self._handlers[kind].append(handler)
            return handler

        return register

    async def connect(self) -> "SoblendWA":
        await self.native.connect()
        return self

    # The native read runs on its own thread and can't be called back, so a timeout here must not drop
    # it: the next reader picks up the same read instead of losing the batch it brings.
    async def _refill(self) -> bool:
        if self._reading is None:
            self._reading = asyncio.ensure_future(self.native.next_events(256))
        try:
            raw = await asyncio.shield(self._reading)
        finally:
            if self._reading is not None and self._reading.done():
                self._reading = None
        if raw is None:
            return False
        self._pending.extend(json.loads(raw))
        return True

    async def events(self) -> AsyncIterator[Event]:
        while True:
            if not self._pending and not await self._refill():
                return
            yield self._pending.popleft()

    async def wait_for(
        self, kind: str, predicate: Optional[Callable[[Event], bool]] = None, timeout: float = 15.0
    ) -> Event:
        async def first() -> Event:
            async for event in self.events():
                if event["type"] == kind and (predicate is None or predicate(event)):
                    return event
            raise SoblendError("NOT_CONNECTED", f"the event stream ended before a {kind} event")

        try:
            return await asyncio.wait_for(first(), timeout)
        except asyncio.TimeoutError:
            raise SoblendError("TIMEOUT", f"no {kind} event within {timeout} s") from None

    async def run(self) -> None:
        await self.connect()
        async for event in self.events():
            for handler in (*self._handlers[event["type"]], *self._handlers["event"]):
                try:
                    outcome = handler(event)
                except Exception:
                    _log.exception("a SoblendWA handler failed")
                    continue
                if asyncio.iscoroutine(outcome):
                    task = asyncio.create_task(outcome)
                    self._tasks.add(task)
                    task.add_done_callback(self._finished)

    async def disconnect(self) -> None:
        await self.native.disconnect()

    async def restart(self) -> None:
        await self.call("restart")

    async def logout(self) -> None:
        await self.native.logout()

    async def ping(self) -> float:
        return await self.native.ping()

    def elapsed(self, message: Union[Event, str]) -> Optional[float]:
        return self.native.elapsed(_id(message))

    def stats(self) -> dict:
        return json.loads(self.native.stats())

    async def pending_sends(self) -> list:
        return json.loads(await self.native.pending_sends())

    async def request_pair_code(self, phone: Union[str, int]) -> str:
        return await self.native.request_pair_code(str(phone))

    async def send_text(
        self,
        chat: str,
        text: str,
        quoted: Union[Event, str, None] = None,
        preview: Union[bool, dict, None] = None,
        mentions: Optional[Iterable[Any]] = None,
    ) -> str:
        return await self.call(
            "sendText",
            chat=str(chat),
            text=text,
            quotedId=_id(quoted) if quoted else None,
            preview=_preview(preview),
            mentions=_texts(mentions) if mentions else None,
        )

    async def send_proto(self, chat: str, proto: bytes) -> str:
        return await self.native.send_proto(chat, bytes(proto))

    async def reply(self, message: Union[Event, str], text: str) -> str:
        return await self.native.reply(_id(message), text)

    async def react(self, message: Union[Event, str], emoji: str) -> str:
        return await self.native.react(_id(message), emoji)

    async def send_media(self, chat: str, kind: str, data: bytes, quoted: Union[Event, str, None] = None, **options: Any) -> str:
        if options.get("mentions"):
            options["mentions"] = _texts(options["mentions"])
        if quoted:
            options["quoted_id"] = _id(quoted)
        return await self.native.send_media(chat, kind, bytes(data), _options(options))

    async def send_image(self, chat: str, data: bytes, **options: Any) -> str:
        return await self.send_media(chat, "image", data, **options)

    async def send_video(self, chat: str, data: bytes, **options: Any) -> str:
        return await self.send_media(chat, "video", data, **options)

    async def send_audio(self, chat: str, data: bytes, **options: Any) -> str:
        return await self.send_media(chat, "audio", data, **options)

    async def send_document(self, chat: str, data: bytes, **options: Any) -> str:
        return await self.send_media(chat, "document", data, **options)

    async def send_sticker(self, chat: str, data: bytes, **options: Any) -> str:
        return await self.send_media(chat, "sticker", data, **options)

    async def send_file(self, chat: str, kind: str, path: str, quoted: Union[Event, str, None] = None, **options: Any) -> str:
        if quoted:
            options["quoted_id"] = _id(quoted)
        return await self.native.send_media_file(chat, kind, str(path), _options(options))

    async def download(self, message: Union[Event, str]) -> bytes:
        return await self.native.download(_id(message))

    async def download_to_file(self, message: Union[Event, str], path: str) -> int:
        return await self.native.download_to_file(_id(message), str(path))

    async def send_interactive(
        self,
        chat: str,
        body: str,
        buttons: Iterable[dict],
        quoted: Union[Event, str, None] = None,
        **header: Any,
    ) -> str:
        spec = {"body": body, "buttons": list(buttons), **header}
        if quoted:
            spec["quoted_id"] = _id(quoted)
        return await self.native.send_interactive(chat, _options(spec))

    async def send_buttons(self, chat: str, text: str, buttons: Iterable[tuple], footer: Optional[str] = None) -> str:
        return await self.native.send_buttons(chat, text, [(str(i), str(t)) for i, t in buttons], footer)

    async def send_list(
        self,
        chat: str,
        title: str,
        text: str,
        button_text: str,
        sections: Iterable[dict],
        footer: Optional[str] = None,
    ) -> str:
        return await self.native.send_list(chat, title, text, button_text, json.dumps(list(sections)), footer)

    async def modify_chat(self, chat: str, action: str, seconds: Optional[int] = None) -> None:
        await self.native.modify_chat(chat, action, seconds)

    async def label_chat(self, chat: str, label_id: Union[str, int], labeled: bool = True) -> None:
        await self.native.label(chat, str(label_id), labeled, None)

    async def label_message(self, chat: str, message: Union[Event, str], label_id: Union[str, int], labeled: bool = True) -> None:
        await self.native.label(chat, str(label_id), labeled, _id(message))

    async def edit_label(self, id: Union[str, int], name: str, color: int = 0) -> None:
        await self.native.edit_label(str(id), name, color)

    async def delete_label(self, id: Union[str, int]) -> None:
        await self.native.edit_label(str(id), None, 0)

    async def save_quick_reply(self, shortcut: str, text: str) -> str:
        return await self.native.save_quick_reply(shortcut, text)

    async def send_contacts(self, chat: str, cards: Iterable[tuple]) -> str:
        return await self.native.send_contacts(chat, [(str(name), str(phone)) for name, phone in cards])

    async def send_mention_all(self, chat: str, text: str) -> str:
        return await self.native.send_mention_all(chat, text)

    async def send_group_invite(self, to: str, group: str, caption: Optional[str] = None) -> str:
        return await self.native.send_group_invite(to, group, caption)

    async def set_member_label(self, group: str, label: str) -> str:
        return await self.native.set_member_label(group, label)

    async def mark_played(self, message: Union[Event, str]) -> None:
        await self.native.mark_played(_id(message))

    async def bot_list(self) -> list:
        return json.loads(await self.native.account("bots"))

    async def reachout_timelock(self) -> dict:
        return json.loads(await self.native.account("timelock"))

    async def new_chat_message_cap(self) -> Any:
        return json.loads(await self.native.account("messageCap"))

    async def disappearing_modes(self, *jids: str) -> list:
        return json.loads(await self.native.account("disappearing", [str(j) for j in jids]))

    def chats(self) -> list:
        return json.loads(self.native.chats())

    def contacts(self) -> list:
        return json.loads(self.native.contacts())

    def messages(self, chat: str, limit: int = 50, before: Optional[int] = None) -> list:
        return json.loads(self.native.messages(chat, limit, before))

    def search(self, text: str, limit: int = 50) -> list:
        return json.loads(self.native.search(text, limit))

    async def send_poll(self, chat: str, name: str, options: Iterable[str], selectable: int = 1) -> str:
        return await self.native.send_poll(chat, name, list(options), selectable)

    async def vote_poll(self, chat: str, poll_id: str, options: Iterable[str]) -> str:
        return await self.native.vote_poll(chat, poll_id, list(options))

    async def edit(self, chat: str, message_id: str, text: str) -> str:
        return await self.native.edit(chat, message_id, text)

    async def revoke(self, chat: str, message_id: str) -> str:
        return await self.native.revoke(chat, message_id)

    async def typing(self, chat: str, state: str = "composing") -> None:
        await self.native.chat_presence(chat, state)

    async def set_online(self, available: bool = True) -> None:
        await self.native.set_presence(available)

    async def group_info(self, chat: str) -> dict:
        return json.loads(await self.native.group_info(chat))

    async def joined_groups(self) -> list:
        return json.loads(await self.native.joined_groups())

    async def on_whatsapp(self, *phones: Union[str, int]) -> list:
        return json.loads(await self.native.on_whatsapp([str(phone) for phone in phones]))

    async def profile_picture(self, jid: str, full_size: bool = False) -> Optional[str]:
        return await self.native.profile_picture(jid, full_size)

    async def block(self, jid: str) -> None:
        await self.native.block(jid, True)

    async def unblock(self, jid: str) -> None:
        await self.native.block(jid, False)

    async def group_metadata(self, chat: str) -> dict:
        return await self.call("groupMetadata", chat=str(chat))

    async def group_create(self, subject: str, participants: Iterable[Any] = ()) -> dict:
        return await self.call("groupCreate", subject=subject, participants=_texts(participants))

    async def group_participants(self, chat: str, participants: Iterable[Any], action: str) -> list:
        return await self.call("groupParticipants", chat=str(chat), participants=_texts([participants]), action=action)

    async def group_add(self, chat: str, participants: Any) -> list:
        return await self.group_participants(chat, participants, "add")

    async def group_kick(self, chat: str, participants: Any) -> list:
        return await self.group_participants(chat, participants, "remove")

    async def group_promote(self, chat: str, participants: Any) -> list:
        return await self.group_participants(chat, participants, "promote")

    async def group_demote(self, chat: str, participants: Any) -> list:
        return await self.group_participants(chat, participants, "demote")

    async def group_subject(self, chat: str, subject: str) -> None:
        await self.call("groupSubject", chat=str(chat), subject=subject)

    async def group_description(self, chat: str, description: Optional[str] = None) -> None:
        await self.call("groupDescription", chat=str(chat), description=description)

    async def group_setting(self, chat: str, setting: str, value: Any) -> None:
        await self.call("groupSetting", chat=str(chat), setting=setting, value=value)

    async def group_picture(self, chat: str, data: bytes) -> None:
        await self.call("groupPicture", chat=str(chat), data=_encode(data))

    async def group_leave(self, chat: str) -> None:
        await self.call("groupLeave", chat=str(chat))

    async def group_invite_code(self, chat: str) -> str:
        return await self.call("groupInviteCode", chat=str(chat))

    async def group_revoke_invite(self, chat: str) -> str:
        return await self.call("groupRevokeInvite", chat=str(chat))

    async def group_join(self, invite: str) -> str:
        return await self.call("groupJoin", invite=str(invite))

    async def group_invite_info(self, invite: str) -> dict:
        return await self.call("groupInviteInfo", invite=str(invite))

    async def group_requests(self, chat: str) -> list:
        return await self.call("groupRequests", chat=str(chat))

    async def group_respond_requests(self, chat: str, participants: Iterable[Any], approve: bool = True) -> list:
        return await self.call("groupRespondRequests", chat=str(chat), participants=_texts([participants]), approve=approve)

    async def set_profile_picture(self, data: bytes, target: Optional[str] = None) -> None:
        await self.call("setProfilePicture", data=_encode(data), target=target)

    async def remove_profile_picture(self, target: Optional[str] = None) -> None:
        await self.call("removeProfilePicture", target=target)

    async def set_about(self, text: str) -> None:
        await self.call("setAbout", text=text)

    async def fetch_about(self, *jids: Any) -> list:
        return await self.call("fetchAbout", jids=_texts(jids))

    async def set_push_name(self, name: str) -> None:
        await self.call("setPushName", name=name)

    async def business_profile(self, jid: str) -> Optional[dict]:
        return await self.call("businessProfile", jid=str(jid))

    async def blocklist(self) -> list:
        return await self.call("blocklist")

    async def privacy_settings(self) -> dict:
        return await self.call("privacySettings")

    async def set_privacy(self, setting: str, value: str) -> None:
        await self.call("setPrivacy", setting=setting, value=value)

    async def lid_for_phone(self, phone: Union[str, int]) -> Optional[str]:
        return await self.call("lidForPhone", phone=str(phone))

    async def phone_for_lid(self, lid: str) -> Optional[str]:
        return await self.call("phoneForLid", lid=str(lid))


def _phones(values: Iterable[Any]) -> list:
    return [str(value) for value in values]


class FakeWhatsApp:
    """A WhatsApp server that lives in this process, for tests. Nothing leaves the machine."""

    def __init__(self, server: Any):
        self.server = _Guarded(server)

    @classmethod
    async def start(cls) -> "FakeWhatsApp":
        return cls(await _settle(_start_fake_server()))

    @staticmethod
    def jid(phone: Union[str, int]) -> str:
        return re.sub(r"\D", "", str(phone)) + "@s.whatsapp.net"

    @property
    def url(self) -> str:
        return self.server.url

    async def connect(self, phone: Union[str, int], **options: Any) -> SoblendWA:
        bot = SoblendWA(await self.server.open(str(phone), _options(options)))
        await bot.connect()
        await bot.wait_for("connected", timeout=15)
        return bot

    def create_group(self, subject: str, creator: Union[str, int], members: Iterable[Any] = ()) -> str:
        return self.server.create_group(subject, str(creator), _phones(members))

    def add_to_group(self, group: str, by: Union[str, int], phones: Iterable[Any]) -> None:
        self.server.add_to_group(str(group), str(by), _phones(phones))

    def remove_from_group(self, group: str, by: Union[str, int], phones: Iterable[Any]) -> None:
        self.server.remove_from_group(str(group), str(by), _phones(phones))

    def invite_code(self, group: str) -> Optional[str]:
        return self.server.invite_code(str(group))

    def set_join_approval(self, group: str, enabled: bool = True) -> None:
        self.server.set_join_approval(str(group), enabled)

    def drop_connections(self, phone: Union[str, int]) -> None:
        self.server.drop_connections(str(phone))

    def refuse_connections(self, phone: Union[str, int], refuse: bool = True) -> None:
        self.server.refuse_connections(str(phone), refuse)

    def block(self, phone: Union[str, int], code: int = 403) -> None:
        self.server.block(str(phone), code)

    def unblock(self, phone: Union[str, int]) -> None:
        self.server.unblock(str(phone))

    def hold_acks(self, phone: Union[str, int], hold: bool = True) -> None:
        self.server.hold_acks(str(phone), hold)

    def corrupt_next(self, phone: Union[str, int], messages: int = 1) -> None:
        self.server.corrupt_next(str(phone), messages)

    def expire_media(self) -> None:
        self.server.expire_media()

    def slow_uploads(self, seconds: float) -> None:
        self.server.slow_uploads(seconds)

    def media_uploads(self) -> int:
        return self.server.media_uploads()

    def routed(self) -> int:
        return self.server.routed()

    def is_online(self, phone: Union[str, int]) -> bool:
        return self.server.is_online(str(phone))
