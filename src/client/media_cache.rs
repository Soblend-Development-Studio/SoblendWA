use std::time::Duration;

use prost::Message as _;

use super::{Client, MediaKind, MediaOptions, UploadedMedia};
use crate::message::now_secs;
use crate::store::KeyKind;

const TRUSTED_FOR: Duration = Duration::from_secs(60 * 60);
const CHECK_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct CachedUpload {
    #[prost(string, tag = "1")]
    pub url: String,
    #[prost(string, tag = "2")]
    pub direct_path: String,
    #[prost(bytes = "vec", tag = "3")]
    pub media_key: Vec<u8>,
    #[prost(bytes = "vec", tag = "4")]
    pub file_sha256: Vec<u8>,
    #[prost(bytes = "vec", tag = "5")]
    pub file_enc_sha256: Vec<u8>,
    #[prost(uint64, tag = "6")]
    pub file_length: u64,
    #[prost(int64, tag = "7")]
    pub uploaded_at: i64,
    #[prost(uint32, optional, tag = "8")]
    pub width: Option<u32>,
    #[prost(uint32, optional, tag = "9")]
    pub height: Option<u32>,
    #[prost(bytes = "vec", optional, tag = "10")]
    pub thumbnail: Option<Vec<u8>>,
}

impl CachedUpload {
    pub(crate) fn uploaded(&self) -> UploadedMedia {
        UploadedMedia {
            url: self.url.clone(),
            direct_path: self.direct_path.clone(),
            media_key: self.media_key.clone(),
            file_sha256: self.file_sha256.clone(),
            file_enc_sha256: self.file_enc_sha256.clone(),
            file_length: self.file_length,
            reused: true,
        }
    }

    pub(crate) fn fill(&self, options: &mut MediaOptions) {
        options.width = options.width.or(self.width);
        options.height = options.height.or(self.height);
        options.thumbnail = options.thumbnail.take().or_else(|| self.thumbnail.clone());
    }
}

fn kind_name(kind: MediaKind) -> &'static str {
    match kind {
        MediaKind::Image => "image",
        MediaKind::Video => "video",
        MediaKind::Audio => "audio",
        MediaKind::Document => "document",
        MediaKind::Sticker => "sticker",
        MediaKind::History => "history",
        MediaKind::AppState => "app-state",
        MediaKind::LinkThumbnail => "link-thumbnail",
    }
}

fn cache_key(kind: MediaKind, file_sha256: &[u8]) -> String {
    format!("{}:{}", kind_name(kind), hex::encode(file_sha256))
}

impl Client {
    pub(crate) async fn cached_upload(&self, kind: MediaKind, file_sha256: &[u8]) -> Option<CachedUpload> {
        let ttl = self.inner.config.media_cache_ttl?;
        let key = cache_key(kind, file_sha256);
        let raw = self.inner.store.get(KeyKind::MediaCache, &key).await.ok()??;
        let entry = CachedUpload::decode(raw.as_slice()).ok()?;
        let age = Duration::from_secs(now_secs().saturating_sub(entry.uploaded_at).max(0) as u64);
        let usable = age <= ttl && (age <= TRUSTED_FOR || self.still_hosted(&entry).await);
        if !usable {
            let _ = self.inner.store.remove(KeyKind::MediaCache, &key).await;
            tracing::debug!(%key, ?age, "cached upload expired, uploading again");
            return None;
        }
        super::guard::Counters::bump(&self.inner.counters.media_reused);
        Some(entry)
    }

    async fn still_hosted(&self, entry: &CachedUpload) -> bool {
        let url = format!("{}{}", self.media_origin(), entry.direct_path);
        match self.inner.http.head(url).timeout(CHECK_TIMEOUT).send().await {
            Ok(response) => response.status().is_success(),
            Err(_) => false,
        }
    }

    pub(crate) async fn remember_upload(&self, kind: MediaKind, uploaded: &UploadedMedia, options: &MediaOptions) {
        if !uploaded.reused {
            super::guard::Counters::bump(&self.inner.counters.media_uploaded);
        }
        if self.inner.config.media_cache_ttl.is_none() || uploaded.reused {
            return;
        }
        let entry = CachedUpload {
            url: uploaded.url.clone(),
            direct_path: uploaded.direct_path.clone(),
            media_key: uploaded.media_key.clone(),
            file_sha256: uploaded.file_sha256.clone(),
            file_enc_sha256: uploaded.file_enc_sha256.clone(),
            file_length: uploaded.file_length,
            uploaded_at: now_secs(),
            width: options.width,
            height: options.height,
            thumbnail: options.thumbnail.clone(),
        };
        let key = cache_key(kind, &uploaded.file_sha256);
        if let Err(err) = self.inner.store.set(KeyKind::MediaCache, &key, entry.encode_to_vec()).await {
            tracing::debug!(%err, "could not remember the upload");
        }
    }

    pub async fn forget_upload(&self, kind: MediaKind, file_sha256: &[u8]) -> crate::Result<()> {
        self.inner.store.remove(KeyKind::MediaCache, &cache_key(kind, file_sha256)).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto;

    fn entry(file_sha256: [u8; 32], uploaded_at: i64) -> CachedUpload {
        CachedUpload {
            url: "https://mmg.whatsapp.net/o1/v/t62.7118-24/cached".into(),
            direct_path: "/o1/v/t62.7118-24/cached".into(),
            media_key: vec![9; 32],
            file_sha256: file_sha256.to_vec(),
            file_enc_sha256: vec![8; 32],
            file_length: 1234,
            uploaded_at,
            width: Some(640),
            height: Some(480),
            thumbnail: Some(vec![1, 2, 3]),
        }
    }

    #[tokio::test]
    async fn repeated_media_is_sent_without_uploading_again() {
        let (client, _events) = Client::builder().build().await.unwrap();
        let picture = b"rollwaifu: the same picture every time".to_vec();
        let sha = crypto::sha256(&picture);
        let key = cache_key(MediaKind::Image, &sha);
        client.store().set(KeyKind::MediaCache, &key, entry(sha, now_secs()).encode_to_vec()).await.unwrap();

        let options = MediaOptions { caption: Some("waifu del día".into()), ..Default::default() };
        let message = client.prepare_media(&picture, MediaKind::Image, options).await.unwrap();
        let image = message.image_message.unwrap();
        assert_eq!(image.direct_path.as_deref(), Some("/o1/v/t62.7118-24/cached"));
        assert_eq!(
            (image.width, image.height, image.caption.as_deref()),
            (Some(640), Some(480), Some("waifu del día"))
        );
        assert_eq!(image.jpeg_thumbnail.as_deref(), Some(&[1u8, 2, 3][..]));
        assert!(client.upload(&picture, MediaKind::Image).await.unwrap().reused);

        let expired = now_secs() - 30 * 24 * 60 * 60;
        client.store().set(KeyKind::MediaCache, &key, entry(sha, expired).encode_to_vec()).await.unwrap();
        assert!(client.prepare_media(&picture, MediaKind::Image, MediaOptions::default()).await.is_err());
        assert!(client.store().get(KeyKind::MediaCache, &key).await.unwrap().is_none());

        let (uncached, _events) = Client::builder().media_cache(None).build().await.unwrap();
        uncached.store().set(KeyKind::MediaCache, &key, entry(sha, now_secs()).encode_to_vec()).await.unwrap();
        assert!(uncached.prepare_media(&picture, MediaKind::Image, MediaOptions::default()).await.is_err());
    }
}
