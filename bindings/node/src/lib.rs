use std::fmt::Display;
use std::future::Future;

use napi::bindgen_prelude::{Buffer, Promise};
use napi::threadsafe_function::ThreadsafeFunction;
use napi::{Error, Result, Status};
use napi_derive::napi;
use serde_json::Value;
use soblendwa_bridge::{self as bridge, Session};

fn fail(err: impl Display) -> Error {
    Error::from_reason(err.to_string())
}

async fn on<T, F, Fut>(session: &Session, work: F) -> Result<T>
where
    T: Send + 'static,
    F: FnOnce(Session) -> Fut,
    Fut: Future<Output = soblendwa_bridge::Outcome<T>> + Send + 'static,
{
    bridge::run(work(session.clone())).await.map_err(fail)
}

#[napi]
pub fn credits() -> String {
    bridge::credits()
}

#[napi]
pub fn version() -> String {
    bridge::version().to_owned()
}

#[napi]
pub async fn import_baileys(source: String, target: Option<Value>) -> Result<Value> {
    let target = match target {
        Some(value) => serde_json::from_value(value).map_err(fail)?,
        None => bridge::StoreOptions::default(),
    };
    bridge::run(async move { bridge::migrate_baileys(&source, target).await }).await.map_err(fail)
}

fn parse<T: serde::de::DeserializeOwned + Default>(value: Option<Value>) -> Result<T> {
    match value {
        Some(value) => serde_json::from_value(value).map_err(fail),
        None => Ok(T::default()),
    }
}

#[derive(serde::Deserialize)]
struct ContactCard {
    name: String,
    phone: String,
}

#[derive(serde::Deserialize)]
struct QuickButton {
    id: String,
    text: String,
}

type StoreCallback = ThreadsafeFunction<String, Promise<Option<String>>, String, Status, false, true>;

struct JsStore(StoreCallback);

#[bridge::async_trait]
impl bridge::StoreHandler for JsStore {
    async fn call(&self, request: String) -> bridge::Outcome<Option<String>> {
        let refused = |err: Error| soblendwa_bridge::store_error(err.reason.clone());
        let pending = self.0.call_async(request).await.map_err(refused)?;
        pending.await.map_err(refused)
    }
}

#[napi(js_name = "NativeClient")]
pub struct NativeClient {
    session: Session,
}

#[napi]
impl NativeClient {
    #[napi(factory)]
    pub async fn open(options: Option<Value>) -> Result<NativeClient> {
        let options = match options {
            Some(value) => serde_json::from_value(value).map_err(fail)?,
            None => bridge::Options::default(),
        };
        let session = bridge::run(Session::open(options)).await.map_err(fail)?;
        Ok(NativeClient { session })
    }

    #[napi(factory)]
    pub async fn open_with_store(options: Option<Value>, store: StoreCallback) -> Result<NativeClient> {
        let mut options: bridge::Options = match options {
            Some(value) => serde_json::from_value(value).map_err(fail)?,
            None => bridge::Options::default(),
        };
        options.custom_store = Some(bridge::CustomStore(std::sync::Arc::new(JsStore(store))));
        let session = bridge::run(Session::open(options)).await.map_err(fail)?;
        Ok(NativeClient { session })
    }

    #[napi]
    pub fn stats(&self) -> Value {
        self.session.client().stats().to_json()
    }

    #[napi]
    pub async fn pending_sends(&self) -> Result<Value> {
        on(&self.session, |s| async move { s.call("pendingSends", &Value::Null).await }).await
    }

    #[napi]
    pub async fn send_text_choosing_preview(
        &self,
        chat: String,
        text: String,
        quoted: Option<String>,
        preview: Value,
    ) -> Result<String> {
        on(
            &self.session,
            |s| async move { s.send_text_choosing_preview(&chat, &text, quoted.as_deref(), &preview).await },
        )
        .await
    }

    #[napi(getter)]
    pub fn me(&self) -> Option<String> {
        self.session.me()
    }

    #[napi(getter)]
    pub fn connected(&self) -> bool {
        self.session.is_connected()
    }

    #[napi]
    pub fn elapsed(&self, message_id: String) -> Option<f64> {
        self.session.elapsed_ms(&message_id)
    }

    #[napi]
    pub async fn connect(&self) -> Result<()> {
        on(&self.session, |s| async move { s.connect().await }).await
    }

    #[napi]
    pub async fn disconnect(&self) -> Result<()> {
        on(&self.session, |s| async move {
            s.disconnect().await;
            Ok(())
        })
        .await
    }

    #[napi]
    pub async fn logout(&self) -> Result<()> {
        on(&self.session, |s| async move { s.logout().await }).await
    }

    #[napi]
    pub async fn next_event(&self) -> Result<Option<Value>> {
        on(&self.session, |s| async move { Ok(s.next_event().await) }).await
    }

    #[napi]
    pub async fn ping(&self) -> Result<f64> {
        on(&self.session, |s| async move { s.ping().await }).await
    }

    #[napi]
    pub async fn request_pair_code(&self, phone: String) -> Result<String> {
        on(&self.session, |s| async move { s.pair_code(&phone).await }).await
    }

    #[napi]
    pub async fn send_text(&self, chat: String, text: String, quoted_id: Option<String>) -> Result<String> {
        on(&self.session, |s| async move { s.send_text(&chat, &text, quoted_id.as_deref()).await }).await
    }

    #[napi]
    pub async fn send_text_with_preview(&self, chat: String, text: String) -> Result<String> {
        on(&self.session, |s| async move { s.send_text_with_preview(&chat, &text).await }).await
    }

    #[napi]
    pub async fn send_proto(&self, chat: String, proto: Buffer) -> Result<String> {
        let proto = proto.to_vec();
        on(&self.session, |s| async move { s.send_proto(&chat, &proto).await }).await
    }

    #[napi]
    pub async fn reply(&self, message_id: String, text: String) -> Result<String> {
        on(&self.session, |s| async move { s.reply(&message_id, &text).await }).await
    }

    #[napi]
    pub async fn react(&self, message_id: String, emoji: String) -> Result<String> {
        on(&self.session, |s| async move { s.react(&message_id, &emoji).await }).await
    }

    #[napi]
    pub async fn send_media(&self, chat: String, kind: String, data: Buffer, options: Option<Value>) -> Result<String> {
        let data = data.to_vec();
        let input: bridge::MediaInput = parse(options)?;
        on(&self.session, |s| async move { s.send_media(&chat, &kind, data, input).await }).await
    }

    #[napi]
    pub async fn send_media_file(
        &self,
        chat: String,
        kind: String,
        path: String,
        options: Option<Value>,
    ) -> Result<String> {
        let input: bridge::MediaInput = parse(options)?;
        on(&self.session, |s| async move { s.send_media_file(&chat, &kind, &path, input).await }).await
    }

    #[napi]
    pub async fn download_to_file(&self, message_id: String, path: String) -> Result<f64> {
        on(&self.session, |s| async move { s.download_to_file(&message_id, &path).await }).await.map(|n| n as f64)
    }

    #[napi]
    pub async fn send_interactive(&self, chat: String, spec: Value) -> Result<String> {
        let spec: bridge::InteractiveSpec = serde_json::from_value(spec).map_err(fail)?;
        on(&self.session, |s| async move { s.send_interactive(&chat, spec).await }).await
    }

    #[napi]
    pub async fn send_buttons(
        &self,
        chat: String,
        text: String,
        buttons: Value,
        footer: Option<String>,
    ) -> Result<String> {
        let buttons: Vec<QuickButton> = serde_json::from_value(buttons).map_err(fail)?;
        let pairs = buttons.into_iter().map(|b| (b.id, b.text)).collect();
        on(&self.session, |s| async move { s.send_buttons(&chat, &text, footer.as_deref(), pairs).await }).await
    }

    #[napi]
    pub async fn send_list(
        &self,
        chat: String,
        title: String,
        text: String,
        button_text: String,
        sections: Value,
        footer: Option<String>,
    ) -> Result<String> {
        let sections: Vec<bridge::SectionSpec> = serde_json::from_value(sections).map_err(fail)?;
        on(&self.session, |s| async move {
            s.send_list(&chat, &title, &text, &button_text, footer.as_deref(), sections).await
        })
        .await
    }

    #[napi]
    pub async fn modify_chat(&self, chat: String, action: String, seconds: Option<u32>) -> Result<()> {
        on(&self.session, |s| async move { s.modify_chat(&chat, &action, seconds.map(u64::from)).await }).await
    }

    #[napi]
    pub async fn label(&self, chat: String, label_id: String, labeled: bool, message_id: Option<String>) -> Result<()> {
        on(&self.session, |s| async move { s.label(&chat, &label_id, message_id.as_deref(), labeled).await }).await
    }

    #[napi]
    pub async fn edit_label(&self, id: String, name: Option<String>, color: Option<i32>) -> Result<()> {
        on(&self.session, |s| async move { s.edit_label(&id, name.as_deref(), color.unwrap_or(0)).await }).await
    }

    #[napi]
    pub async fn save_quick_reply(&self, shortcut: String, text: String) -> Result<String> {
        on(&self.session, |s| async move { s.save_quick_reply(&shortcut, &text).await }).await
    }

    #[napi]
    pub async fn send_contacts(&self, chat: String, cards: Value) -> Result<String> {
        let cards: Vec<ContactCard> = serde_json::from_value(cards).map_err(fail)?;
        let pairs = cards.into_iter().map(|c| (c.name, c.phone)).collect();
        on(&self.session, |s| async move { s.send_contacts(&chat, pairs).await }).await
    }

    #[napi]
    pub async fn send_mention_all(&self, chat: String, text: String) -> Result<String> {
        on(&self.session, |s| async move { s.send_mention_all(&chat, &text).await }).await
    }

    #[napi]
    pub async fn send_group_invite(&self, to: String, group: String, caption: Option<String>) -> Result<String> {
        on(&self.session, |s| async move { s.send_group_invite(&to, &group, caption.as_deref()).await }).await
    }

    #[napi]
    pub async fn set_member_label(&self, group: String, label: String) -> Result<String> {
        on(&self.session, |s| async move { s.set_member_label(&group, &label).await }).await
    }

    #[napi]
    pub async fn mark_played(&self, message_id: String) -> Result<()> {
        on(&self.session, |s| async move { s.mark_played(&message_id).await }).await
    }

    #[napi]
    pub async fn account(&self, what: String, jids: Option<Vec<String>>) -> Result<Value> {
        on(&self.session, |s| async move { s.account(&what, jids.unwrap_or_default()).await }).await
    }

    #[napi]
    pub fn chats(&self) -> Result<Value> {
        self.session.chats().map_err(fail)
    }

    #[napi]
    pub fn contacts(&self) -> Result<Value> {
        self.session.contacts().map_err(fail)
    }

    #[napi]
    pub fn messages(&self, chat: String, limit: Option<u32>, before: Option<i64>) -> Result<Value> {
        self.session.messages(&chat, limit.unwrap_or(50) as usize, before).map_err(fail)
    }

    #[napi]
    pub fn search(&self, text: String, limit: Option<u32>) -> Result<Value> {
        self.session.search(&text, limit.unwrap_or(50) as usize).map_err(fail)
    }

    #[napi]
    pub async fn download(&self, message_id: String) -> Result<Buffer> {
        on(&self.session, |s| async move { s.download(&message_id).await }).await.map(Buffer::from)
    }

    #[napi]
    pub async fn send_poll(
        &self,
        chat: String,
        name: String,
        options: Vec<String>,
        selectable: Option<u32>,
    ) -> Result<String> {
        on(&self.session, |s| async move { s.send_poll(&chat, &name, options, selectable.unwrap_or(1)).await }).await
    }

    #[napi]
    pub async fn vote_poll(&self, chat: String, poll_id: String, options: Vec<String>) -> Result<String> {
        on(&self.session, |s| async move { s.vote_poll(&chat, &poll_id, options).await }).await
    }

    #[napi]
    pub async fn edit(&self, chat: String, message_id: String, text: String) -> Result<String> {
        on(&self.session, |s| async move { s.edit(&chat, &message_id, &text).await }).await
    }

    #[napi]
    pub async fn revoke(&self, chat: String, message_id: String) -> Result<String> {
        on(&self.session, |s| async move { s.revoke(&chat, &message_id).await }).await
    }

    #[napi]
    pub async fn chat_presence(&self, chat: String, state: String) -> Result<()> {
        on(&self.session, |s| async move { s.chat_presence(&chat, &state).await }).await
    }

    #[napi]
    pub async fn set_presence(&self, available: bool) -> Result<()> {
        on(&self.session, |s| async move { s.set_presence(available).await }).await
    }

    #[napi]
    pub async fn group_info(&self, chat: String) -> Result<Value> {
        on(&self.session, |s| async move { s.group_info(&chat).await }).await
    }

    #[napi]
    pub async fn joined_groups(&self) -> Result<Value> {
        on(&self.session, |s| async move { s.joined_groups().await }).await
    }

    #[napi]
    pub async fn on_whatsapp(&self, phones: Vec<String>) -> Result<Value> {
        on(&self.session, |s| async move { s.on_whatsapp(phones).await }).await
    }

    #[napi]
    pub async fn profile_picture(&self, jid: String, full_size: Option<bool>) -> Result<Option<String>> {
        on(&self.session, |s| async move { s.profile_picture(&jid, full_size.unwrap_or(false)).await }).await
    }

    #[napi]
    pub async fn block(&self, jid: String, blocked: bool) -> Result<()> {
        on(&self.session, |s| async move { s.block(&jid, blocked).await }).await
    }
}
