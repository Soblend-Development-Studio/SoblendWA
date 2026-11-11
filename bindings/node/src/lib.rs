use std::future::Future;
use std::sync::Arc;

use napi::bindgen_prelude::{Buffer, Promise, PromiseRaw, ToNapiValue};
use napi::threadsafe_function::ThreadsafeFunction;
use napi::{Env, Error, Result, Status};
use napi_derive::napi;
use serde_json::Value;
use soblendwa_bridge::{self as bridge, Session};

#[global_allocator]
// the system allocator on Windows (and musl's on Alpine) was the slowest part of handling a message
static ALLOCATOR: mimalloc::MiMalloc = mimalloc::MiMalloc;

fn fail(err: impl bridge::Describe) -> Error {
    Error::from_reason(err.describe())
}

async fn on<T, F, Fut>(session: &Session, work: F) -> Result<T>
where
    T: Send + 'static,
    F: FnOnce(Session) -> Fut,
    Fut: Future<Output = soblendwa_bridge::Outcome<T>> + Send + 'static,
{
    bridge::run(work(session.clone())).await.map_err(fail)
}

// napi runs the body of an `async fn` later, on the runtime, so two sends fired back to back from JS
// could swap places. The turn has to be taken here, while we're still on the JS thread.
fn ordered<'env, T, F, Fut>(
    env: &'env Env,
    session: &Session,
    turn: Option<bridge::Turn>,
    work: F,
) -> Result<PromiseRaw<'env, T>>
where
    T: Send + ToNapiValue + 'static,
    F: FnOnce(Session) -> Fut,
    Fut: Future<Output = soblendwa_bridge::Outcome<T>> + Send + 'static,
{
    let work = work(session.clone());
    env.spawn_future(async move { bridge::run(bridge::in_turn(turn, work)).await.map_err(fail) })
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

#[napi]
pub async fn import_zapo(source: Value, target: Option<Value>, check: Option<bool>) -> Result<Value> {
    let source: bridge::ZapoSource = serde_json::from_value(source).map_err(fail)?;
    let target: Option<bridge::StoreOptions> = match target {
        Some(Value::Null) | None => None,
        Some(value) => Some(serde_json::from_value(value).map_err(fail)?),
    };
    let check = check.unwrap_or(false);
    bridge::run(async move { bridge::import_zapo(source, target, check).await }).await.map_err(fail)
}

#[napi]
pub async fn rollback_zapo(source: Value, target: Value) -> Result<Value> {
    let source: bridge::StoreOptions = serde_json::from_value(source).map_err(fail)?;
    let target: bridge::ZapoSource = serde_json::from_value(target).map_err(fail)?;
    bridge::run(async move { bridge::rollback_zapo(source, target).await }).await.map_err(fail)
}

#[napi(js_name = "NativeClient")]
pub struct NativeClient {
    session: Session,
}

#[napi]
impl NativeClient {
    #[napi(factory)]
    pub async fn open(options: Option<Value>) -> Result<NativeClient> {
        let options = bridge::options_from_value(options.unwrap_or(Value::Null)).map_err(fail)?;
        let session = bridge::run(Session::open(options)).await.map_err(fail)?;
        Ok(NativeClient { session })
    }

    #[napi(factory)]
    pub async fn open_with_store(options: Option<Value>, store: StoreCallback) -> Result<NativeClient> {
        let mut options = bridge::options_from_value(options.unwrap_or(Value::Null)).map_err(fail)?;
        options.custom_store = Some(bridge::CustomStore(std::sync::Arc::new(JsStore(store))));
        let session = bridge::run(Session::open(options)).await.map_err(fail)?;
        Ok(NativeClient { session })
    }

    #[napi]
    pub fn call<'env>(&self, env: &'env Env, method: String, params: Option<Value>) -> Result<PromiseRaw<'env, Value>> {
        let params = params.unwrap_or(Value::Null);
        ordered(env, &self.session, self.session.turn(&method, &params), |s| s.begin(&method, params))
    }

    #[napi]
    pub fn stats(&self) -> Value {
        self.session.client().stats().to_json()
    }

    #[napi]
    pub fn stats_json(&self) -> String {
        self.session.client().stats().to_json().to_string()
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
    pub async fn next_events(&self, max: u32) -> Result<Option<String>> {
        on(&self.session, |s| async move { Ok(s.next_events_json(max as usize).await) }).await
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
    pub fn send_text<'env>(
        &self,
        env: &'env Env,
        chat: String,
        text: String,
        quoted_id: Option<String>,
    ) -> Result<PromiseRaw<'env, String>> {
        ordered(env, &self.session, Some(self.session.turn_for_chat(&chat)), |s| async move {
            s.send_text(&chat, &text, quoted_id.as_deref()).await
        })
    }

    #[napi]
    pub fn send_text_with_preview<'env>(
        &self,
        env: &'env Env,
        chat: String,
        text: String,
    ) -> Result<PromiseRaw<'env, String>> {
        ordered(env, &self.session, Some(self.session.turn_for_chat(&chat)), |s| async move {
            s.send_text_with_preview(&chat, &text).await
        })
    }

    #[napi]
    pub fn send_proto<'env>(&self, env: &'env Env, chat: String, proto: Buffer) -> Result<PromiseRaw<'env, String>> {
        let proto = proto.to_vec();
        ordered(env, &self.session, Some(self.session.turn_for_chat(&chat)), |s| async move {
            s.send_proto(&chat, &proto).await
        })
    }

    #[napi]
    pub fn reply<'env>(&self, env: &'env Env, message_id: String, text: String) -> Result<PromiseRaw<'env, String>> {
        ordered(env, &self.session, self.session.turn_for_message(&message_id), |s| async move {
            s.reply(&message_id, &text).await
        })
    }

    #[napi]
    pub fn react<'env>(&self, env: &'env Env, message_id: String, emoji: String) -> Result<PromiseRaw<'env, String>> {
        ordered(env, &self.session, self.session.turn_for_message(&message_id), |s| async move {
            s.react(&message_id, &emoji).await
        })
    }

    #[napi]
    pub fn send_media<'env>(
        &self,
        env: &'env Env,
        chat: String,
        kind: String,
        data: Buffer,
        options: Option<Value>,
    ) -> Result<PromiseRaw<'env, String>> {
        let data = data.to_vec();
        let input: bridge::MediaInput = parse(options)?;
        ordered(env, &self.session, Some(self.session.turn_for_chat(&chat)), |s| {
            s.send_media(&chat, &kind, data, input)
        })
    }

    #[napi]
    pub fn send_media_file<'env>(
        &self,
        env: &'env Env,
        chat: String,
        kind: String,
        path: String,
        options: Option<Value>,
    ) -> Result<PromiseRaw<'env, String>> {
        let input: bridge::MediaInput = parse(options)?;
        ordered(env, &self.session, Some(self.session.turn_for_chat(&chat)), |s| {
            s.send_media_file(&chat, &kind, &path, input)
        })
    }

    #[napi]
    pub async fn download_to_file(&self, message_id: String, path: String) -> Result<f64> {
        on(&self.session, |s| async move { s.download_to_file(&message_id, &path).await }).await.map(|n| n as f64)
    }

    #[napi]
    pub fn send_interactive<'env>(
        &self,
        env: &'env Env,
        chat: String,
        spec: Value,
    ) -> Result<PromiseRaw<'env, String>> {
        let spec: bridge::InteractiveSpec = serde_json::from_value(spec).map_err(fail)?;
        ordered(env, &self.session, Some(self.session.turn_for_chat(&chat)), |s| async move {
            s.send_interactive(&chat, spec).await
        })
    }

    #[napi]
    pub fn send_buttons<'env>(
        &self,
        env: &'env Env,
        chat: String,
        text: String,
        buttons: Value,
        footer: Option<String>,
    ) -> Result<PromiseRaw<'env, String>> {
        let buttons: Vec<QuickButton> = serde_json::from_value(buttons).map_err(fail)?;
        let pairs = buttons.into_iter().map(|b| (b.id, b.text)).collect();
        ordered(env, &self.session, Some(self.session.turn_for_chat(&chat)), |s| async move {
            s.send_buttons(&chat, &text, footer.as_deref(), pairs).await
        })
    }

    #[napi]
    #[allow(clippy::too_many_arguments)]
    pub fn send_list<'env>(
        &self,
        env: &'env Env,
        chat: String,
        title: String,
        text: String,
        button_text: String,
        sections: Value,
        footer: Option<String>,
    ) -> Result<PromiseRaw<'env, String>> {
        let sections: Vec<bridge::SectionSpec> = serde_json::from_value(sections).map_err(fail)?;
        ordered(env, &self.session, Some(self.session.turn_for_chat(&chat)), |s| async move {
            s.send_list(&chat, &title, &text, &button_text, footer.as_deref(), sections).await
        })
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
    pub fn send_contacts<'env>(&self, env: &'env Env, chat: String, cards: Value) -> Result<PromiseRaw<'env, String>> {
        let cards: Vec<ContactCard> = serde_json::from_value(cards).map_err(fail)?;
        let pairs = cards.into_iter().map(|c| (c.name, c.phone)).collect();
        ordered(env, &self.session, Some(self.session.turn_for_chat(&chat)), |s| async move {
            s.send_contacts(&chat, pairs).await
        })
    }

    #[napi]
    pub fn send_mention_all<'env>(
        &self,
        env: &'env Env,
        chat: String,
        text: String,
    ) -> Result<PromiseRaw<'env, String>> {
        ordered(env, &self.session, Some(self.session.turn_for_chat(&chat)), |s| async move {
            s.send_mention_all(&chat, &text).await
        })
    }

    #[napi]
    pub fn send_group_invite<'env>(
        &self,
        env: &'env Env,
        to: String,
        group: String,
        caption: Option<String>,
    ) -> Result<PromiseRaw<'env, String>> {
        ordered(env, &self.session, Some(self.session.turn_for_chat(&to)), |s| async move {
            s.send_group_invite(&to, &group, caption.as_deref()).await
        })
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
    pub fn send_poll<'env>(
        &self,
        env: &'env Env,
        chat: String,
        name: String,
        options: Vec<String>,
        selectable: Option<u32>,
    ) -> Result<PromiseRaw<'env, String>> {
        ordered(env, &self.session, Some(self.session.turn_for_chat(&chat)), |s| async move {
            s.send_poll(&chat, &name, options, selectable.unwrap_or(1)).await
        })
    }

    #[napi]
    pub fn vote_poll<'env>(
        &self,
        env: &'env Env,
        chat: String,
        poll_id: String,
        options: Vec<String>,
    ) -> Result<PromiseRaw<'env, String>> {
        ordered(env, &self.session, Some(self.session.turn_for_chat(&chat)), |s| async move {
            s.vote_poll(&chat, &poll_id, options).await
        })
    }

    #[napi]
    pub fn edit<'env>(
        &self,
        env: &'env Env,
        chat: String,
        message_id: String,
        text: String,
    ) -> Result<PromiseRaw<'env, String>> {
        ordered(env, &self.session, Some(self.session.turn_for_chat(&chat)), |s| async move {
            s.edit(&chat, &message_id, &text).await
        })
    }

    #[napi]
    pub fn revoke<'env>(&self, env: &'env Env, chat: String, message_id: String) -> Result<PromiseRaw<'env, String>> {
        ordered(env, &self.session, Some(self.session.turn_for_chat(&chat)), |s| async move {
            s.revoke(&chat, &message_id).await
        })
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

#[napi(js_name = "FakeServer")]
pub struct FakeServer {
    fake: Arc<bridge::FakeWhatsApp>,
}

#[napi]
pub async fn start_fake_server() -> Result<FakeServer> {
    let fake = bridge::run(async { bridge::FakeWhatsApp::start().await }).await.map_err(fail)?;
    Ok(FakeServer { fake: Arc::new(fake) })
}

#[napi]
impl FakeServer {
    #[napi(getter)]
    pub fn url(&self) -> String {
        self.fake.url()
    }

    #[napi]
    pub async fn open(&self, phone: String, options: Option<Value>) -> Result<NativeClient> {
        let mut options = options.unwrap_or_else(|| Value::Object(Default::default()));
        if let Value::Object(fields) = &mut options {
            fields.entry("store").or_insert_with(|| serde_json::json!({ "type": "memory" }));
        }
        let mut options = bridge::options_from_value(options).map_err(fail)?;
        options.fake = Some(bridge::fake_link(&self.fake, &phone));
        let session = bridge::run(Session::open(options)).await.map_err(fail)?;
        Ok(NativeClient { session })
    }

    #[napi]
    pub fn create_group(&self, subject: String, creator: String, members: Vec<String>) -> String {
        let members: Vec<&str> = members.iter().map(String::as_str).collect();
        self.fake.create_group(&subject, &creator, &members).to_string()
    }

    #[napi]
    pub fn add_to_group(&self, group: String, by: String, phones: Vec<String>) -> Result<()> {
        let group = bridge::jid(&group).map_err(fail)?;
        let phones: Vec<&str> = phones.iter().map(String::as_str).collect();
        self.fake.add_to_group(&group, &by, &phones);
        Ok(())
    }

    #[napi]
    pub fn remove_from_group(&self, group: String, by: String, phones: Vec<String>) -> Result<()> {
        let group = bridge::jid(&group).map_err(fail)?;
        let phones: Vec<&str> = phones.iter().map(String::as_str).collect();
        self.fake.remove_from_group(&group, &by, &phones);
        Ok(())
    }

    #[napi]
    pub fn invite_code(&self, group: String) -> Result<Option<String>> {
        Ok(self.fake.invite_code(&bridge::jid(&group).map_err(fail)?))
    }

    #[napi]
    pub fn set_join_approval(&self, group: String, enabled: bool) -> Result<()> {
        self.fake.set_join_approval(&bridge::jid(&group).map_err(fail)?, enabled);
        Ok(())
    }

    #[napi]
    pub fn drop_connections(&self, phone: String) {
        self.fake.drop_connections(&phone);
    }

    #[napi]
    pub fn refuse_connections(&self, phone: String, refuse: bool) {
        self.fake.refuse_connections(&phone, refuse);
    }

    #[napi]
    pub fn block(&self, phone: String, code: u32) {
        self.fake.block(&phone, code as u16);
    }

    #[napi]
    pub fn unblock(&self, phone: String) {
        self.fake.unblock(&phone);
    }

    #[napi]
    pub fn hold_acks(&self, phone: String, hold: bool) {
        self.fake.hold_acks(&phone, hold);
    }

    #[napi]
    pub fn corrupt_next(&self, phone: String, messages: u32) {
        self.fake.corrupt_next(&phone, messages);
    }

    #[napi]
    pub fn expire_media(&self) {
        self.fake.expire_media();
    }

    #[napi]
    pub fn slow_uploads(&self, millis: u32) {
        self.fake.slow_uploads(std::time::Duration::from_millis(u64::from(millis)));
    }

    #[napi]
    pub fn media_uploads(&self) -> f64 {
        self.fake.media_uploads() as f64
    }

    #[napi]
    pub fn routed(&self) -> f64 {
        self.fake.routed() as f64
    }

    #[napi]
    pub fn is_online(&self, phone: String) -> bool {
        self.fake.is_online(&phone)
    }
}
