use std::time::{Duration, Instant};

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use prost::Message as _;
use serde::Deserialize;
use tokio::sync::oneshot;

use super::Client;
use crate::binary::Node;
use crate::crypto;
use crate::error::{Error, Result};
use crate::events::MessageEvent;
use crate::jid::Jid;
use crate::message::now_secs;
use crate::proto::{
    AudioMessage, DocumentMessage, HistorySyncNotification, ImageMessage, MediaRetryNotification, Message,
    ServerErrorReceipt, StickerMessage, VideoMessage,
};

pub(super) const MEDIA_HOST: &str = "mmg.whatsapp.net";
pub(super) const MAC_LEN: usize = 10;
const MEDIA_RETRY_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MediaKind {
    Image,
    Video,
    Audio,
    Document,
    Sticker,
    History,
    AppState,
    LinkThumbnail,
}

impl MediaKind {
    fn info(self) -> &'static [u8] {
        match self {
            MediaKind::Image | MediaKind::Sticker => b"WhatsApp Image Keys",
            MediaKind::Video => b"WhatsApp Video Keys",
            MediaKind::Audio => b"WhatsApp Audio Keys",
            MediaKind::Document => b"WhatsApp Document Keys",
            MediaKind::History => b"WhatsApp History Keys",
            MediaKind::AppState => b"WhatsApp App State Keys",
            MediaKind::LinkThumbnail => b"WhatsApp Link Thumbnail Keys",
        }
    }

    pub(super) fn upload_path(self) -> &'static str {
        match self {
            MediaKind::Image | MediaKind::Sticker => "/mms/image",
            MediaKind::Video => "/mms/video",
            MediaKind::Audio => "/mms/audio",
            MediaKind::Document => "/mms/document",
            MediaKind::History => "/mms/md-app-state",
            MediaKind::AppState => "",
            MediaKind::LinkThumbnail => "/mms/thumbnail-link",
        }
    }
}

pub(super) struct MediaKeys {
    pub iv: [u8; 16],
    pub cipher: [u8; 32],
    pub mac: [u8; 32],
}

pub(super) fn expand_keys(media_key: &[u8], kind: MediaKind) -> MediaKeys {
    let expanded: [u8; 112] = crypto::hkdf_array(media_key, None, kind.info());
    let mut keys = MediaKeys { iv: [0; 16], cipher: [0; 32], mac: [0; 32] };
    keys.iv.copy_from_slice(&expanded[..16]);
    keys.cipher.copy_from_slice(&expanded[16..48]);
    keys.mac.copy_from_slice(&expanded[48..80]);
    keys
}

pub(crate) struct MediaConn {
    auth: String,
    hosts: Vec<String>,
    expires: Instant,
}

#[derive(Clone, Debug)]
pub struct UploadedMedia {
    pub url: String,
    pub direct_path: String,
    pub media_key: Vec<u8>,
    pub file_sha256: Vec<u8>,
    pub file_enc_sha256: Vec<u8>,
    pub file_length: u64,
    pub reused: bool,
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub(crate) struct UploadResponse {
    pub url: Option<String>,
    pub direct_path: Option<String>,
    pub meta_hmac: Option<String>,
    pub fbid: Option<serde_json::Value>,
    pub ts: Option<serde_json::Value>,
}

fn scalar(value: &Option<serde_json::Value>) -> Option<String> {
    match value.as_ref()? {
        serde_json::Value::String(text) => Some(text.clone()),
        serde_json::Value::Number(number) => Some(number.to_string()),
        _ => None,
    }
}

impl UploadResponse {
    pub(crate) fn fbid(&self) -> Option<String> {
        scalar(&self.fbid)
    }

    pub(crate) fn ts(&self) -> Option<String> {
        scalar(&self.ts)
    }
}

#[derive(Clone, Debug, Default)]
pub struct MediaOptions {
    pub mimetype: Option<String>,
    pub caption: Option<String>,
    pub file_name: Option<String>,
    pub voice_note: bool,
    pub gif: bool,
    pub view_once: bool,
    pub seconds: Option<u32>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub thumbnail: Option<Vec<u8>>,
}

pub trait Downloadable: Sync {
    fn media_kind(&self) -> MediaKind;
    fn media_key(&self) -> Option<&[u8]>;
    fn direct_path(&self) -> Option<&str>;
    fn url(&self) -> Option<&str>;
    fn file_enc_sha256(&self) -> Option<&[u8]>;
    fn file_sha256(&self) -> Option<&[u8]>;
}

macro_rules! downloadable {
    ($ty:ty, $kind:expr) => {
        impl Downloadable for $ty {
            fn media_kind(&self) -> MediaKind {
                $kind
            }
            fn media_key(&self) -> Option<&[u8]> {
                self.media_key.as_deref()
            }
            fn direct_path(&self) -> Option<&str> {
                self.direct_path.as_deref()
            }
            fn url(&self) -> Option<&str> {
                self.url.as_deref()
            }
            fn file_enc_sha256(&self) -> Option<&[u8]> {
                self.file_enc_sha256.as_deref()
            }
            fn file_sha256(&self) -> Option<&[u8]> {
                self.file_sha256.as_deref()
            }
        }
    };
}

downloadable!(ImageMessage, MediaKind::Image);
downloadable!(VideoMessage, MediaKind::Video);
downloadable!(AudioMessage, MediaKind::Audio);
downloadable!(DocumentMessage, MediaKind::Document);
downloadable!(StickerMessage, MediaKind::Sticker);

impl Downloadable for HistorySyncNotification {
    fn media_kind(&self) -> MediaKind {
        MediaKind::History
    }
    fn media_key(&self) -> Option<&[u8]> {
        self.media_key.as_deref()
    }
    fn direct_path(&self) -> Option<&str> {
        self.direct_path.as_deref()
    }
    fn url(&self) -> Option<&str> {
        None
    }
    fn file_enc_sha256(&self) -> Option<&[u8]> {
        self.file_enc_sha256.as_deref()
    }
    fn file_sha256(&self) -> Option<&[u8]> {
        self.file_sha256.as_deref()
    }
}

impl Message {
    pub fn media(&self) -> Option<&dyn Downloadable> {
        let m = self.unwrapped();
        m.image_message
            .as_deref()
            .map(|x| x as &dyn Downloadable)
            .or_else(|| m.video_message.as_deref().map(|x| x as &dyn Downloadable))
            .or_else(|| m.ptv_message.as_deref().map(|x| x as &dyn Downloadable))
            .or_else(|| m.audio_message.as_deref().map(|x| x as &dyn Downloadable))
            .or_else(|| m.document_message.as_deref().map(|x| x as &dyn Downloadable))
            .or_else(|| m.sticker_message.as_deref().map(|x| x as &dyn Downloadable))
    }
}

impl Client {
    pub(super) async fn media_conn(&self, force: bool) -> Result<(String, Vec<String>)> {
        let mut cached = self.inner.media_conn.lock().await;
        if let Some(conn) = cached.as_ref().filter(|c| !force && c.expires > Instant::now()) {
            return Ok((conn.auth.clone(), conn.hosts.clone()));
        }
        let request = Node::new("iq")
            .attr("to", Jid::server())
            .attr("type", "set")
            .attr("xmlns", "w:m")
            .child_node(Node::new("media_conn"));
        let response = self.query(request).await?;
        let conn = response.child("media_conn").ok_or_else(|| Error::Media("missing media_conn".into()))?;
        let auth = conn.attr_string("auth").ok_or_else(|| Error::Media("media_conn without auth".into()))?;
        let ttl = conn.attr_u64("ttl").unwrap_or(300);
        let hosts: Vec<String> = conn.children_by("host").filter_map(|h| h.attr_string("hostname")).collect();
        *cached = Some(MediaConn {
            auth: auth.clone(),
            hosts: hosts.clone(),
            expires: Instant::now() + Duration::from_secs(ttl.saturating_sub(10)),
        });
        Ok((auth, hosts))
    }

    pub(super) fn media_origin(&self) -> String {
        self.inner.config.media_endpoint.clone().unwrap_or_else(|| format!("https://{MEDIA_HOST}"))
    }

    pub(super) fn download_url(&self, media: &dyn Downloadable) -> Result<String> {
        let origin = self.media_origin();
        Ok(match (media.url(), media.direct_path()) {
            (Some(url), _) if url.starts_with(&origin) => url.to_owned(),
            (_, Some(path)) => format!("{origin}{path}"),
            (Some(url), None) => url.to_owned(),
            (None, None) => return Err(Error::Media("message has no download location".into())),
        })
    }

    pub async fn upload(&self, data: &[u8], kind: MediaKind) -> Result<UploadedMedia> {
        let file_sha256 = crypto::sha256(data);
        if let Some(hit) = self.cached_upload(kind, &file_sha256).await {
            return Ok(hit.uploaded());
        }
        let uploaded = self.upload_fresh(data, kind).await?;
        self.remember_upload(kind, &uploaded, &MediaOptions::default()).await;
        Ok(uploaded)
    }

    pub(super) async fn upload_fresh(&self, data: &[u8], kind: MediaKind) -> Result<UploadedMedia> {
        self.upload_fresh_at(data, kind, kind.upload_path()).await
    }

    pub(super) async fn upload_fresh_at(&self, data: &[u8], kind: MediaKind, path: &str) -> Result<UploadedMedia> {
        let media_key = crypto::random_array::<32>();
        let keys = expand_keys(&media_key, kind);
        let file_sha256 = crypto::sha256(data);
        let mut body = crypto::aes_cbc_encrypt(&keys.cipher, &keys.iv, data)?;
        let mac = crypto::hmac_sha256(&keys.mac, &[&keys.iv, &body]);
        body.extend_from_slice(&mac[..MAC_LEN]);
        let file_enc_sha256 = crypto::sha256(&body);
        let token = URL_SAFE_NO_PAD.encode(file_enc_sha256);
        let length = body.len() as u64;
        let body = bytes::Bytes::from(body);
        let answer = self
            .post_media(
                path,
                &token,
                length,
                || Ok(body.clone().into()),
                |a| a.url.is_some() && a.direct_path.is_some(),
            )
            .await?;
        Ok(UploadedMedia {
            url: answer.url.unwrap_or_default(),
            direct_path: answer.direct_path.unwrap_or_default(),
            media_key: media_key.to_vec(),
            file_sha256: file_sha256.to_vec(),
            file_enc_sha256: file_enc_sha256.to_vec(),
            file_length: data.len() as u64,
            reused: false,
        })
    }

    pub(crate) async fn upload_plain(&self, data: Vec<u8>, path: &str) -> Result<UploadResponse> {
        let token = URL_SAFE_NO_PAD.encode(crypto::sha256(&data));
        let length = data.len() as u64;
        let body = bytes::Bytes::from(data);
        self.post_media(
            path,
            &token,
            length,
            || Ok(body.clone().into()),
            |a| a.direct_path.is_some() || a.fbid.is_some(),
        )
        .await
    }

    pub(super) async fn post_media(
        &self,
        path: &str,
        token: &str,
        length: u64,
        body: impl Fn() -> Result<reqwest::Body>,
        complete: impl Fn(&UploadResponse) -> bool,
    ) -> Result<UploadResponse> {
        let mut last_error = Error::Media("no upload hosts available".into());
        // The second round only asks for new hosts when WhatsApp refused the auth. A connection the CDN had
        // already closed fails the same way and just needs a fresh one; asking again cost one more round trip.
        let mut refresh = false;
        for _ in 0..2 {
            let (auth, hosts) = self.media_conn(refresh).await?;
            for host in &hosts {
                let url = match &self.inner.config.media_endpoint {
                    Some(endpoint) => format!("{endpoint}{path}/{token}"),
                    None => format!("https://{host}{path}/{token}"),
                };
                let response = self
                    .inner
                    .http
                    .post(url)
                    .query(&[("auth", auth.as_str()), ("token", token)])
                    .header("Origin", "https://web.whatsapp.com")
                    .header("Content-Type", "application/octet-stream")
                    .header("Content-Length", length)
                    .body(body()?)
                    .send()
                    .await;
                let parsed = match response {
                    Ok(response) if response.status().is_success() => response.json::<UploadResponse>().await,
                    Ok(response) => {
                        refresh |= matches!(response.status().as_u16(), 401 | 403);
                        last_error = Error::Media(format!("upload to {host} failed with {}", response.status()));
                        continue;
                    }
                    Err(err) => {
                        last_error = err.into();
                        continue;
                    }
                };
                match parsed {
                    Ok(answer) if complete(&answer) => return Ok(answer),
                    Ok(_) => last_error = Error::Media(format!("{host} returned an incomplete upload response")),
                    Err(err) => last_error = err.into(),
                }
            }
        }
        Err(last_error)
    }

    pub async fn download(&self, media: &dyn Downloadable) -> Result<Vec<u8>> {
        let media_key = media.media_key().ok_or_else(|| Error::Media("message has no media key".into()))?;
        let url = self.download_url(media)?;
        let encrypted = self
            .inner
            .http
            .get(&url)
            .header("Origin", "https://web.whatsapp.com")
            .send()
            .await?
            .error_for_status()?
            .bytes()
            .await?;
        if let Some(expected) = media.file_enc_sha256() {
            if crypto::sha256(&encrypted) != expected {
                return Err(Error::Media("encrypted file hash mismatch".into()));
            }
        }
        decrypt_media(&encrypted, media_key, media.media_kind(), media.file_sha256())
    }

    pub async fn reupload_media(&self, event: &MessageEvent) -> Result<Message> {
        let media = event.message.media().ok_or_else(|| Error::Media("message does not contain media".into()))?;
        let media_key = media.media_key().ok_or_else(|| Error::Media("message has no media key".into()))?;
        let retry_key: [u8; 32] = crypto::hkdf_array(media_key, None, b"WhatsApp Media Retry Notification");
        let id = event.info.id.clone();
        let iv = crypto::random_array::<12>();
        let receipt = ServerErrorReceipt { stanza_id: Some(id.clone()) }.encode_to_vec();
        let sealed = crypto::aes_gcm_encrypt(&retry_key, &iv, id.as_bytes(), &receipt)?;
        let me = self.require_me()?.to_non_device();
        let participant = (event.info.is_group && !event.info.from_me).then(|| event.info.sender.to_non_device());
        let request =
            Node::new("receipt").attr("id", id.clone()).attr("to", me).attr("type", "server-error").children([
                Node::new("encrypt")
                    .children([Node::new("enc_p").bytes(sealed), Node::new("enc_iv").bytes(iv.to_vec())]),
                Node::new("rmr")
                    .attr("jid", event.info.chat.clone())
                    .attr("from_me", if event.info.from_me { "true" } else { "false" })
                    .attr_opt("participant", participant),
            ]);

        let (tx, rx) = oneshot::channel();
        self.inner.media_retries.insert(id.clone(), tx);
        if let Err(err) = self.send_node(&request) {
            self.inner.media_retries.remove(&id);
            return Err(err);
        }
        let reply = match tokio::time::timeout(MEDIA_RETRY_TIMEOUT, rx).await {
            Ok(Ok(reply)) => reply,
            _ => {
                self.inner.media_retries.remove(&id);
                return Err(Error::Media("the phone did not re-upload the media in time".into()));
            }
        };
        if let Some((code, _)) = reply.error_code() {
            return Err(Error::Media(format!("the phone could not re-upload the media ({code})")));
        }
        let encrypted = reply.child("encrypt").ok_or_else(|| Error::Media("re-upload answer is empty".into()))?;
        let (Some(payload), Some(reply_iv)) = (encrypted.child_bytes("enc_p"), encrypted.child_bytes("enc_iv")) else {
            return Err(Error::Media("re-upload answer is incomplete".into()));
        };
        let plain = crypto::aes_gcm_decrypt(&retry_key, reply_iv, id.as_bytes(), payload)?;
        let notification = MediaRetryNotification::decode(plain.as_slice())?;
        if notification.result != Some(MediaRetryNotification::SUCCESS) {
            return Err(Error::Media(format!("the phone refused to re-upload ({:?})", notification.result)));
        }
        let path = notification.direct_path.ok_or_else(|| Error::Media("re-upload answer has no path".into()))?;
        let mut message = event.message.clone();
        retarget(&mut message, &path);
        Ok(message)
    }

    pub async fn download_event(&self, event: &MessageEvent) -> Result<Vec<u8>> {
        match self.download_message(&event.message).await {
            Err(Error::Http(err)) if matches!(err.status().map(|s| s.as_u16()), Some(404 | 410)) => {
                let refreshed = self.reupload_media(event).await?;
                self.download_message(&refreshed).await
            }
            other => other,
        }
    }

    pub async fn download_message(&self, message: &Message) -> Result<Vec<u8>> {
        let media = message.media().ok_or_else(|| Error::Media("message does not contain media".into()))?;
        self.download(media).await
    }

    pub async fn prepare_media(&self, data: &[u8], kind: MediaKind, mut options: MediaOptions) -> Result<Message> {
        let file_sha256 = crypto::sha256(data);
        if let Some(hit) = self.cached_upload(kind, &file_sha256).await {
            hit.fill(&mut options);
            return media_message(kind, hit.uploaded(), options);
        }
        if matches!(kind, MediaKind::Image | MediaKind::Sticker) {
            let bytes = data.to_vec();
            let wants_thumbnail = kind == MediaKind::Image && options.thumbnail.is_none();
            let (dimensions, preview) = tokio::task::spawn_blocking(move || {
                let dimensions = super::enrich::image_dimensions(&bytes);
                let preview = wants_thumbnail
                    .then(|| super::enrich::thumbnail(&bytes, super::enrich::THUMBNAIL_WIDTH).ok())
                    .flatten();
                (dimensions, preview)
            })
            .await
            .map_err(|err| Error::Media(err.to_string()))?;
            if let Some((width, height)) = dimensions {
                options.width = options.width.or(Some(width));
                options.height = options.height.or(Some(height));
            }
            options.thumbnail = options.thumbnail.or(preview);
        }
        let uploaded = self.upload_fresh(data, kind).await?;
        self.remember_upload(kind, &uploaded, &options).await;
        media_message(kind, uploaded, options)
    }
}

pub(super) fn media_message(kind: MediaKind, uploaded: UploadedMedia, options: MediaOptions) -> Result<Message> {
    let now = Some(now_secs());
    let mimetype = |fallback: &str| Some(options.mimetype.clone().unwrap_or_else(|| fallback.to_owned()));
    let mut message = Message::default();
    match kind {
        MediaKind::Image => {
            message.image_message = Some(Box::new(ImageMessage {
                url: Some(uploaded.url),
                mimetype: mimetype("image/jpeg"),
                caption: options.caption,
                file_sha256: Some(uploaded.file_sha256),
                file_length: Some(uploaded.file_length),
                height: options.height,
                width: options.width,
                media_key: Some(uploaded.media_key),
                file_enc_sha256: Some(uploaded.file_enc_sha256),
                direct_path: Some(uploaded.direct_path),
                media_key_timestamp: now,
                jpeg_thumbnail: options.thumbnail,
                context_info: None,
                view_once: options.view_once.then_some(true),
            }));
        }
        MediaKind::Video => {
            message.video_message = Some(Box::new(VideoMessage {
                url: Some(uploaded.url),
                mimetype: mimetype("video/mp4"),
                file_sha256: Some(uploaded.file_sha256),
                file_length: Some(uploaded.file_length),
                seconds: options.seconds,
                media_key: Some(uploaded.media_key),
                caption: options.caption,
                gif_playback: options.gif.then_some(true),
                height: options.height,
                width: options.width,
                file_enc_sha256: Some(uploaded.file_enc_sha256),
                direct_path: Some(uploaded.direct_path),
                media_key_timestamp: now,
                jpeg_thumbnail: options.thumbnail,
                context_info: None,
                view_once: options.view_once.then_some(true),
            }));
        }
        MediaKind::Audio => {
            let fallback = if options.voice_note { "audio/ogg; codecs=opus" } else { "audio/mpeg" };
            message.audio_message = Some(Box::new(AudioMessage {
                url: Some(uploaded.url),
                mimetype: mimetype(fallback),
                file_sha256: Some(uploaded.file_sha256),
                file_length: Some(uploaded.file_length),
                seconds: options.seconds,
                ptt: Some(options.voice_note),
                media_key: Some(uploaded.media_key),
                file_enc_sha256: Some(uploaded.file_enc_sha256),
                direct_path: Some(uploaded.direct_path),
                media_key_timestamp: now,
                context_info: None,
                waveform: None,
                view_once: options.view_once.then_some(true),
            }));
        }
        MediaKind::Document => {
            message.document_message = Some(Box::new(DocumentMessage {
                url: Some(uploaded.url),
                mimetype: mimetype("application/octet-stream"),
                title: options.file_name.clone(),
                file_sha256: Some(uploaded.file_sha256),
                file_length: Some(uploaded.file_length),
                page_count: None,
                media_key: Some(uploaded.media_key),
                file_name: options.file_name,
                file_enc_sha256: Some(uploaded.file_enc_sha256),
                direct_path: Some(uploaded.direct_path),
                media_key_timestamp: now,
                jpeg_thumbnail: options.thumbnail,
                context_info: None,
                caption: options.caption,
            }));
        }
        MediaKind::Sticker => {
            message.sticker_message = Some(Box::new(StickerMessage {
                url: Some(uploaded.url),
                file_sha256: Some(uploaded.file_sha256),
                file_enc_sha256: Some(uploaded.file_enc_sha256),
                media_key: Some(uploaded.media_key),
                mimetype: mimetype("image/webp"),
                height: options.height,
                width: options.width,
                direct_path: Some(uploaded.direct_path),
                file_length: Some(uploaded.file_length),
                media_key_timestamp: now,
                is_animated: None,
                context_info: None,
            }));
        }
        MediaKind::History | MediaKind::AppState | MediaKind::LinkThumbnail => {
            return Err(Error::Media("this media kind cannot be sent as a message".into()));
        }
    }
    Ok(message)
}

impl Client {
    // The chat's order is fixed on the first poll; `prepare` (usually an upload) keeps running while the
    // messages ahead of it go out.
    pub async fn send_prepared(
        &self,
        chat: &Jid,
        prepare: impl std::future::Future<Output = Result<Message>>,
    ) -> Result<String> {
        let chat = chat.to_non_device();
        self.admit_send(&chat)?;
        let id = crate::message::generate_message_id(Some(&self.require_me()?));
        let upload = async {
            let clock = Instant::now();
            (prepare.await, clock.elapsed())
        };
        let (id, target) = (&id, &chat);
        self.send_in_lane_after(&chat, upload, move |(message, took)| async move {
            let message = message?;
            self.inner.upload_times.lock().insert(id.clone(), took);
            let options = super::SendOptions { id: Some(id.clone()), ..Default::default() };
            let sent = Box::pin(self.deliver(target, message, options, false)).await;
            self.inner.upload_times.lock().remove(id);
            sent
        })
        .await
    }

    pub async fn send_media(&self, chat: &Jid, data: &[u8], kind: MediaKind, options: MediaOptions) -> Result<String> {
        self.send_prepared(chat, Box::pin(self.prepare_media(data, kind, options))).await
    }

    pub async fn send_image(&self, chat: &Jid, data: &[u8], caption: Option<&str>) -> Result<String> {
        let options = MediaOptions { caption: caption.map(str::to_owned), ..Default::default() };
        self.send_media(chat, data, MediaKind::Image, options).await
    }

    pub async fn send_video(&self, chat: &Jid, data: &[u8], caption: Option<&str>) -> Result<String> {
        let options = MediaOptions { caption: caption.map(str::to_owned), ..Default::default() };
        self.send_media(chat, data, MediaKind::Video, options).await
    }

    pub async fn send_audio(&self, chat: &Jid, data: &[u8], voice_note: bool) -> Result<String> {
        let options = MediaOptions { voice_note, ..Default::default() };
        self.send_media(chat, data, MediaKind::Audio, options).await
    }

    pub async fn send_document(&self, chat: &Jid, data: &[u8], file_name: &str, mimetype: &str) -> Result<String> {
        let options = MediaOptions {
            file_name: Some(file_name.to_owned()),
            mimetype: Some(mimetype.to_owned()),
            ..Default::default()
        };
        self.send_media(chat, data, MediaKind::Document, options).await
    }

    pub async fn send_sticker(&self, chat: &Jid, webp: &[u8]) -> Result<String> {
        self.send_media(chat, webp, MediaKind::Sticker, MediaOptions::default()).await
    }
}

fn retarget(message: &mut Message, path: &str) {
    let url = Some(format!("https://{MEDIA_HOST}{path}"));
    let path = Some(path.to_owned());
    macro_rules! point {
        ($($field:ident),*) => {
            $(if let Some(media) = message.$field.as_mut() {
                media.direct_path = path.clone();
                media.url = url.clone();
            })*
        };
    }
    point!(image_message, video_message, ptv_message, audio_message, document_message, sticker_message);
}

pub(crate) fn decrypt_media(
    encrypted: &[u8],
    media_key: &[u8],
    kind: MediaKind,
    file_sha256: Option<&[u8]>,
) -> Result<Vec<u8>> {
    if encrypted.len() <= MAC_LEN {
        return Err(Error::Media("downloaded file is too small".into()));
    }
    let keys = expand_keys(media_key, kind);
    let (body, mac) = encrypted.split_at(encrypted.len() - MAC_LEN);
    let expected = crypto::hmac_sha256(&keys.mac, &[&keys.iv, body]);
    if !crypto::constant_time_eq(&expected[..MAC_LEN], mac) {
        return Err(Error::Media("media mac mismatch".into()));
    }
    let plaintext = crypto::aes_cbc_decrypt(&keys.cipher, &keys.iv, body)?;
    if let Some(expected) = file_sha256 {
        if crypto::sha256(&plaintext) != expected {
            return Err(Error::Media("decrypted file hash mismatch".into()));
        }
    }
    Ok(plaintext)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn media_encryption_roundtrip() {
        let data = b"soblend development studio media payload".repeat(40);
        let media_key = crypto::random_array::<32>();
        let keys = expand_keys(&media_key, MediaKind::Document);
        let mut body = crypto::aes_cbc_encrypt(&keys.cipher, &keys.iv, &data).unwrap();
        let mac = crypto::hmac_sha256(&keys.mac, &[&keys.iv, &body]);
        body.extend_from_slice(&mac[..MAC_LEN]);
        let sha = crypto::sha256(&data);
        assert_eq!(decrypt_media(&body, &media_key, MediaKind::Document, Some(&sha)).unwrap(), data);
        body[3] ^= 0xff;
        assert!(decrypt_media(&body, &media_key, MediaKind::Document, Some(&sha)).is_err());
    }
}
