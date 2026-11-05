from __future__ import annotations

import asyncio
import json
from collections import defaultdict
from typing import Any, AsyncIterator, Awaitable, Callable, Iterable, Optional, Union

from ._native import NativeClient, __author__, __version__, credits
from ._native import import_baileys as _import_baileys

Event = dict
Handler = Callable[[Event], Union[Awaitable[None], None]]

__all__ = ["SoblendWA", "NativeClient", "import_baileys", "credits", "__version__", "__author__"]


async def import_baileys(source: str, **target: Any) -> dict:
    target = target or {"type": "file", "path": "session"}
    return json.loads(await _import_baileys(str(source), json.dumps(target)))


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
        self.native = native
        self._handlers: dict[str, list[Handler]] = defaultdict(list)
        self._tasks: set[asyncio.Task] = set()

    @classmethod
    async def create(cls, **options: Any) -> "SoblendWA":
        return cls(await NativeClient.open(_options(options)))

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

    async def events(self) -> AsyncIterator[Event]:
        while True:
            raw = await self.native.next_event()
            if raw is None:
                return
            yield json.loads(raw)

    async def run(self) -> None:
        await self.connect()
        async for event in self.events():
            for handler in (*self._handlers[event["type"]], *self._handlers["event"]):
                outcome = handler(event)
                if asyncio.iscoroutine(outcome):
                    task = asyncio.create_task(outcome)
                    self._tasks.add(task)
                    task.add_done_callback(self._tasks.discard)

    async def disconnect(self) -> None:
        await self.native.disconnect()

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
    ) -> str:
        quoted_id = _id(quoted) if quoted else None
        if preview is None:
            return await self.native.send_text(chat, text, quoted_id)
        return await self.native.send_text_choosing_preview(chat, text, quoted_id, json.dumps(preview))

    async def send_proto(self, chat: str, proto: bytes) -> str:
        return await self.native.send_proto(chat, bytes(proto))

    async def reply(self, message: Union[Event, str], text: str) -> str:
        return await self.native.reply(_id(message), text)

    async def react(self, message: Union[Event, str], emoji: str) -> str:
        return await self.native.react(_id(message), emoji)

    async def send_media(self, chat: str, kind: str, data: bytes, quoted: Union[Event, str, None] = None, **options: Any) -> str:
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
