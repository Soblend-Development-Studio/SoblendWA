use std::borrow::Cow;
use std::future::Future;

use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use soblendwa_bridge::{self as bridge, Outcome, Session};

#[global_allocator]
static ALLOCATOR: mimalloc::MiMalloc = mimalloc::MiMalloc;

fn fail(err: impl bridge::Describe) -> PyErr {
    PyRuntimeError::new_err(err.describe())
}

async fn on<T, F, Fut>(session: &Session, work: F) -> PyResult<T>
where
    T: Send + 'static,
    F: FnOnce(Session) -> Fut,
    Fut: Future<Output = Outcome<T>> + Send + 'static,
{
    bridge::run(work(session.clone())).await.map_err(fail)
}

async fn ordered<T, F, Fut>(session: &Session, turn: Option<bridge::Turn>, work: F) -> PyResult<T>
where
    T: Send + 'static,
    F: FnOnce(Session) -> Fut,
    Fut: Future<Output = Outcome<T>> + Send + 'static,
{
    bridge::run(bridge::in_turn(turn, work(session.clone()))).await.map_err(fail)
}

#[pyclass(frozen, name = "NativeClient", module = "soblendwa._native")]
struct NativeClient {
    session: Session,
}

#[pymethods]
impl NativeClient {
    #[staticmethod]
    #[pyo3(signature = (options = None))]
    async fn open(options: Option<String>) -> PyResult<NativeClient> {
        let options = bridge::parse_options(options.as_deref()).map_err(fail)?;
        let session = bridge::run(Session::open(options)).await.map_err(fail)?;
        Ok(NativeClient { session })
    }

    #[getter]
    fn me(&self) -> Option<String> {
        self.session.me()
    }

    #[getter]
    fn connected(&self) -> bool {
        self.session.is_connected()
    }

    fn elapsed(&self, message_id: &str) -> Option<f64> {
        self.session.elapsed_ms(message_id)
    }

    #[pyo3(signature = (method, params = None))]
    async fn call(&self, method: String, params: Option<String>) -> PyResult<String> {
        let params: serde_json::Value = match params {
            Some(raw) => serde_json::from_str(&raw).map_err(fail)?,
            None => serde_json::Value::Null,
        };
        ordered(&self.session, self.session.turn(&method, &params), |s| async move {
            s.call(&method, &params).await.map(|v| v.to_string())
        })
        .await
    }

    fn stats(&self) -> String {
        self.session.client().stats().to_json().to_string()
    }

    async fn pending_sends(&self) -> PyResult<String> {
        on(
            &self.session,
            |s| async move { s.call("pendingSends", &serde_json::Value::Null).await.map(|v| v.to_string()) },
        )
        .await
    }

    async fn connect(&self) -> PyResult<()> {
        on(&self.session, |s| async move { s.connect().await }).await
    }

    async fn disconnect(&self) -> PyResult<()> {
        on(&self.session, |s| async move {
            s.disconnect().await;
            Ok(())
        })
        .await
    }

    async fn logout(&self) -> PyResult<()> {
        on(&self.session, |s| async move { s.logout().await }).await
    }

    async fn next_event(&self) -> PyResult<Option<String>> {
        on(&self.session, |s| async move { Ok(s.next_event().await.map(|event| event.to_string())) }).await
    }

    async fn next_events(&self, max: usize) -> PyResult<Option<String>> {
        on(&self.session, |s| async move { Ok(s.next_events_json(max).await) }).await
    }

    async fn ping(&self) -> PyResult<f64> {
        on(&self.session, |s| async move { s.ping().await }).await
    }

    async fn request_pair_code(&self, phone: String) -> PyResult<String> {
        on(&self.session, |s| async move { s.pair_code(&phone).await }).await
    }

    #[pyo3(signature = (chat, text, quoted_id = None))]
    async fn send_text(&self, chat: String, text: String, quoted_id: Option<String>) -> PyResult<String> {
        ordered(&self.session, Some(self.session.turn_for_chat(&chat)), |s| async move {
            s.send_text(&chat, &text, quoted_id.as_deref()).await
        })
        .await
    }

    async fn send_text_choosing_preview(
        &self,
        chat: String,
        text: String,
        quoted_id: Option<String>,
        preview_json: String,
    ) -> PyResult<String> {
        on(&self.session, |s| async move {
            let preview = serde_json::from_str(&preview_json)?;
            s.send_text_choosing_preview(&chat, &text, quoted_id.as_deref(), &preview).await
        })
        .await
    }

    async fn send_text_with_preview(&self, chat: String, text: String) -> PyResult<String> {
        on(&self.session, |s| async move { s.send_text_with_preview(&chat, &text).await }).await
    }

    async fn send_proto(&self, chat: String, proto: Vec<u8>) -> PyResult<String> {
        ordered(
            &self.session,
            Some(self.session.turn_for_chat(&chat)),
            |s| async move { s.send_proto(&chat, &proto).await },
        )
        .await
    }

    async fn reply(&self, message_id: String, text: String) -> PyResult<String> {
        ordered(&self.session, self.session.turn_for_message(&message_id), |s| async move {
            s.reply(&message_id, &text).await
        })
        .await
    }

    async fn react(&self, message_id: String, emoji: String) -> PyResult<String> {
        ordered(&self.session, self.session.turn_for_message(&message_id), |s| async move {
            s.react(&message_id, &emoji).await
        })
        .await
    }

    #[pyo3(signature = (chat, kind, data, options = None))]
    async fn send_media(&self, chat: String, kind: String, data: Vec<u8>, options: Option<String>) -> PyResult<String> {
        let input = bridge::parse_media(options.as_deref()).map_err(fail)?;
        ordered(&self.session, Some(self.session.turn_for_chat(&chat)), |s| async move {
            s.send_media(&chat, &kind, data, input).await
        })
        .await
    }

    #[pyo3(signature = (chat, kind, path, options = None))]
    async fn send_media_file(
        &self,
        chat: String,
        kind: String,
        path: String,
        options: Option<String>,
    ) -> PyResult<String> {
        let input = bridge::parse_media(options.as_deref()).map_err(fail)?;
        ordered(&self.session, Some(self.session.turn_for_chat(&chat)), |s| async move {
            s.send_media_file(&chat, &kind, &path, input).await
        })
        .await
    }

    async fn download_to_file(&self, message_id: String, path: String) -> PyResult<u64> {
        on(&self.session, |s| async move { s.download_to_file(&message_id, &path).await }).await
    }

    async fn send_interactive(&self, chat: String, spec: String) -> PyResult<String> {
        let spec: bridge::InteractiveSpec = serde_json::from_str(&spec).map_err(fail)?;
        ordered(&self.session, Some(self.session.turn_for_chat(&chat)), |s| async move {
            s.send_interactive(&chat, spec).await
        })
        .await
    }

    #[pyo3(signature = (chat, text, buttons, footer = None))]
    async fn send_buttons(
        &self,
        chat: String,
        text: String,
        buttons: Vec<(String, String)>,
        footer: Option<String>,
    ) -> PyResult<String> {
        ordered(&self.session, Some(self.session.turn_for_chat(&chat)), |s| async move {
            s.send_buttons(&chat, &text, footer.as_deref(), buttons).await
        })
        .await
    }

    #[pyo3(signature = (chat, title, text, button_text, sections, footer = None))]
    async fn send_list(
        &self,
        chat: String,
        title: String,
        text: String,
        button_text: String,
        sections: String,
        footer: Option<String>,
    ) -> PyResult<String> {
        let sections: Vec<bridge::SectionSpec> = serde_json::from_str(&sections).map_err(fail)?;
        ordered(&self.session, Some(self.session.turn_for_chat(&chat)), |s| async move {
            s.send_list(&chat, &title, &text, &button_text, footer.as_deref(), sections).await
        })
        .await
    }

    #[pyo3(signature = (chat, action, seconds = None))]
    async fn modify_chat(&self, chat: String, action: String, seconds: Option<u64>) -> PyResult<()> {
        on(&self.session, |s| async move { s.modify_chat(&chat, &action, seconds).await }).await
    }

    #[pyo3(signature = (chat, label_id, labeled = true, message_id = None))]
    async fn label(&self, chat: String, label_id: String, labeled: bool, message_id: Option<String>) -> PyResult<()> {
        on(&self.session, |s| async move { s.label(&chat, &label_id, message_id.as_deref(), labeled).await }).await
    }

    #[pyo3(signature = (id, name = None, color = 0))]
    async fn edit_label(&self, id: String, name: Option<String>, color: i32) -> PyResult<()> {
        on(&self.session, |s| async move { s.edit_label(&id, name.as_deref(), color).await }).await
    }

    async fn save_quick_reply(&self, shortcut: String, text: String) -> PyResult<String> {
        on(&self.session, |s| async move { s.save_quick_reply(&shortcut, &text).await }).await
    }

    async fn send_contacts(&self, chat: String, cards: Vec<(String, String)>) -> PyResult<String> {
        ordered(&self.session, Some(self.session.turn_for_chat(&chat)), |s| async move {
            s.send_contacts(&chat, cards).await
        })
        .await
    }

    async fn send_mention_all(&self, chat: String, text: String) -> PyResult<String> {
        ordered(&self.session, Some(self.session.turn_for_chat(&chat)), |s| async move {
            s.send_mention_all(&chat, &text).await
        })
        .await
    }

    #[pyo3(signature = (to, group, caption = None))]
    async fn send_group_invite(&self, to: String, group: String, caption: Option<String>) -> PyResult<String> {
        ordered(&self.session, Some(self.session.turn_for_chat(&to)), |s| async move {
            s.send_group_invite(&to, &group, caption.as_deref()).await
        })
        .await
    }

    async fn set_member_label(&self, group: String, label: String) -> PyResult<String> {
        on(&self.session, |s| async move { s.set_member_label(&group, &label).await }).await
    }

    async fn mark_played(&self, message_id: String) -> PyResult<()> {
        on(&self.session, |s| async move { s.mark_played(&message_id).await }).await
    }

    #[pyo3(signature = (what, jids = Vec::new()))]
    async fn account(&self, what: String, jids: Vec<String>) -> PyResult<String> {
        on(&self.session, |s| async move { s.account(&what, jids).await.map(|v| v.to_string()) }).await
    }

    fn chats(&self) -> PyResult<String> {
        self.session.chats().map(|v| v.to_string()).map_err(fail)
    }

    fn contacts(&self) -> PyResult<String> {
        self.session.contacts().map(|v| v.to_string()).map_err(fail)
    }

    #[pyo3(signature = (chat, limit = 50, before = None))]
    fn messages(&self, chat: &str, limit: usize, before: Option<i64>) -> PyResult<String> {
        self.session.messages(chat, limit, before).map(|v| v.to_string()).map_err(fail)
    }

    #[pyo3(signature = (text, limit = 50))]
    fn search(&self, text: &str, limit: usize) -> PyResult<String> {
        self.session.search(text, limit).map(|v| v.to_string()).map_err(fail)
    }

    async fn download(&self, message_id: String) -> PyResult<Cow<'static, [u8]>> {
        on(&self.session, |s| async move { s.download(&message_id).await }).await.map(Cow::Owned)
    }

    #[pyo3(signature = (chat, name, options, selectable = 1))]
    async fn send_poll(&self, chat: String, name: String, options: Vec<String>, selectable: u32) -> PyResult<String> {
        ordered(&self.session, Some(self.session.turn_for_chat(&chat)), |s| async move {
            s.send_poll(&chat, &name, options, selectable).await
        })
        .await
    }

    async fn vote_poll(&self, chat: String, poll_id: String, options: Vec<String>) -> PyResult<String> {
        ordered(&self.session, Some(self.session.turn_for_chat(&chat)), |s| async move {
            s.vote_poll(&chat, &poll_id, options).await
        })
        .await
    }

    async fn edit(&self, chat: String, message_id: String, text: String) -> PyResult<String> {
        ordered(&self.session, Some(self.session.turn_for_chat(&chat)), |s| async move {
            s.edit(&chat, &message_id, &text).await
        })
        .await
    }

    async fn revoke(&self, chat: String, message_id: String) -> PyResult<String> {
        ordered(&self.session, Some(self.session.turn_for_chat(&chat)), |s| async move {
            s.revoke(&chat, &message_id).await
        })
        .await
    }

    async fn chat_presence(&self, chat: String, state: String) -> PyResult<()> {
        on(&self.session, |s| async move { s.chat_presence(&chat, &state).await }).await
    }

    async fn set_presence(&self, available: bool) -> PyResult<()> {
        on(&self.session, |s| async move { s.set_presence(available).await }).await
    }

    async fn group_info(&self, chat: String) -> PyResult<String> {
        on(&self.session, |s| async move { s.group_info(&chat).await.map(|v| v.to_string()) }).await
    }

    async fn joined_groups(&self) -> PyResult<String> {
        on(&self.session, |s| async move { s.joined_groups().await.map(|v| v.to_string()) }).await
    }

    async fn on_whatsapp(&self, phones: Vec<String>) -> PyResult<String> {
        on(&self.session, |s| async move { s.on_whatsapp(phones).await.map(|v| v.to_string()) }).await
    }

    #[pyo3(signature = (jid, full_size = false))]
    async fn profile_picture(&self, jid: String, full_size: bool) -> PyResult<Option<String>> {
        on(&self.session, |s| async move { s.profile_picture(&jid, full_size).await }).await
    }

    async fn block(&self, jid: String, blocked: bool) -> PyResult<()> {
        on(&self.session, |s| async move { s.block(&jid, blocked).await }).await
    }
}

#[pyfunction]
fn credits() -> String {
    bridge::credits()
}

#[pyfunction]
#[pyo3(signature = (source, target = None))]
async fn import_baileys(source: String, target: Option<String>) -> PyResult<String> {
    let target = bridge::parse_store(target.as_deref()).map_err(fail)?;
    bridge::run(async move { bridge::migrate_baileys(&source, target).await })
        .await
        .map(|report| report.to_string())
        .map_err(fail)
}

#[pymodule]
fn _native(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<NativeClient>()?;
    module.add_function(wrap_pyfunction!(credits, module)?)?;
    module.add_function(wrap_pyfunction!(import_baileys, module)?)?;
    module.add("__version__", bridge::version())?;
    module.add("__author__", "Soblend Development Studio")?;
    Ok(())
}
