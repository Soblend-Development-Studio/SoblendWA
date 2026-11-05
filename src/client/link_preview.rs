use std::fmt;
use std::io::Cursor;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use image::imageops::FilterType;
use image::{DynamicImage, GenericImageView, ImageReader, Limits};
use reqwest::dns::{Addrs, Name, Resolve, Resolving};
use reqwest::redirect::Policy;
use reqwest::Url;

use super::enrich::encode_jpeg;
use super::{Client, Config, Downloadable, MediaKind};
use crate::error::{Error, Result};
use crate::jid::Jid;
use crate::message::now_secs;
use crate::proto::{ExtendedTextMessage, Message};

const INLINE_THUMBNAIL_WIDTH: u32 = 192;
const INLINE_THUMBNAIL_QUALITY: u8 = 50;
const HQ_THUMBNAIL_WIDTH: u32 = 1200;
const HQ_THUMBNAIL_QUALITY: u8 = 80;
const MAX_REDIRECTS: usize = 5;
const MAX_CACHED_PREVIEWS: usize = 2048;
const MAX_IMAGE_SIDE: u32 = 12_000;
const MAX_IMAGE_ALLOC: u64 = 256 * 1024 * 1024;
const PREVIEW_VIDEO: i32 = 1;
const NEWSLETTER_THUMBNAIL_PATH: &str = "/newsletter/newsletter-thumbnail-link";
const IMAGE_THUMBNAIL_PATH: &str = "/mms/image";
const USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";

#[derive(Clone, Debug, Default)]
pub struct LinkPreview {
    pub url: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub image_url: Option<String>,
    pub thumbnail: Option<Vec<u8>>,
    pub image: Option<Vec<u8>>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub video: bool,
}

#[async_trait]
pub trait LinkPreviewFetcher: Send + Sync {
    async fn fetch(&self, url: &str) -> Result<Option<LinkPreview>>;
}

#[derive(Clone)]
pub struct LinkPreviewConfig {
    pub enabled: bool,
    pub timeout: Duration,
    pub upload_hq_thumbnail: bool,
    pub allow_private_hosts: bool,
    pub max_html_bytes: usize,
    pub max_image_bytes: usize,
    pub user_agent: String,
    pub cache_ttl: Option<Duration>,
    pub fetcher: Option<Arc<dyn LinkPreviewFetcher>>,
}

impl Default for LinkPreviewConfig {
    fn default() -> Self {
        LinkPreviewConfig {
            enabled: true,
            timeout: Duration::from_secs(3),
            upload_hq_thumbnail: true,
            allow_private_hosts: false,
            max_html_bytes: 512 * 1024,
            max_image_bytes: 4 * 1024 * 1024,
            user_agent: USER_AGENT.to_owned(),
            cache_ttl: Some(Duration::from_secs(60 * 60)),
            fetcher: None,
        }
    }
}

impl fmt::Debug for LinkPreviewConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LinkPreviewConfig")
            .field("enabled", &self.enabled)
            .field("timeout", &self.timeout)
            .field("upload_hq_thumbnail", &self.upload_hq_thumbnail)
            .field("allow_private_hosts", &self.allow_private_hosts)
            .field("max_html_bytes", &self.max_html_bytes)
            .field("max_image_bytes", &self.max_image_bytes)
            .field("cache_ttl", &self.cache_ttl)
            .field("custom_fetcher", &self.fetcher.is_some())
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Debug, Default)]
pub enum PreviewChoice {
    #[default]
    Auto,
    On,
    Off,
    Custom(Box<LinkPreview>),
}

#[derive(Clone, Debug, Default)]
pub struct LinkThumbnail {
    pub direct_path: Option<String>,
    pub media_key: Option<Vec<u8>>,
    pub file_sha256: Option<Vec<u8>>,
    pub file_enc_sha256: Option<Vec<u8>>,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

impl Downloadable for LinkThumbnail {
    fn media_kind(&self) -> MediaKind {
        MediaKind::LinkThumbnail
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
    pub fn link_thumbnail(&self) -> Option<LinkThumbnail> {
        let text = self.unwrapped().extended_text_message.as_ref()?;
        text.thumbnail_direct_path.as_ref()?;
        Some(LinkThumbnail {
            direct_path: text.thumbnail_direct_path.clone(),
            media_key: text.media_key.clone(),
            file_sha256: text.thumbnail_sha256.clone(),
            file_enc_sha256: text.thumbnail_enc_sha256.clone(),
            width: text.thumbnail_width,
            height: text.thumbnail_height,
        })
    }
}

fn is_private_v4(ip: Ipv4Addr) -> bool {
    let [a, b, ..] = ip.octets();
    ip.is_loopback()
        || ip.is_private()
        || ip.is_link_local()
        || ip.is_unspecified()
        || ip.is_broadcast()
        || ip.is_multicast()
        || ip.is_documentation()
        || a == 0
        || (a == 100 && (64..128).contains(&b))
        || (a == 198 && (b == 18 || b == 19))
}

fn is_private_v6(ip: Ipv6Addr) -> bool {
    if let Some(mapped) = ip.to_ipv4_mapped() {
        return is_private_v4(mapped);
    }
    let first = ip.segments()[0];
    ip.is_loopback()
        || ip.is_unspecified()
        || ip.is_multicast()
        || (first & 0xfe00) == 0xfc00
        || (first & 0xffc0) == 0xfe80
        || (first & 0xffc0) == 0xfec0
}

pub(crate) fn is_private_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => is_private_v4(ip),
        IpAddr::V6(ip) => is_private_v6(ip),
    }
}

pub(crate) fn is_private_url(url: &Url) -> bool {
    match url.host() {
        Some(url::Host::Ipv4(ip)) => is_private_v4(ip),
        Some(url::Host::Ipv6(ip)) => is_private_v6(ip),
        Some(url::Host::Domain(domain)) => {
            let domain = domain.trim_end_matches('.').to_ascii_lowercase();
            domain == "localhost"
                || domain.ends_with(".localhost")
                || domain.ends_with(".local")
                || domain.ends_with(".internal")
                || !domain.contains('.')
        }
        None => true,
    }
}

struct PublicOnly;

impl Resolve for PublicOnly {
    fn resolve(&self, name: Name) -> Resolving {
        Box::pin(async move {
            let found = tokio::net::lookup_host((name.as_str(), 0)).await?;
            let public: Vec<SocketAddr> = found.filter(|address| !is_private_ip(address.ip())).collect();
            if public.is_empty() {
                return Err("the host only resolves to private addresses".into());
            }
            Ok(Box::new(public.into_iter()) as Addrs)
        })
    }
}

pub(super) fn http_client(config: &Config) -> Result<reqwest::Client> {
    let previews = &config.link_previews;
    let allow_private = previews.allow_private_hosts;
    let mut builder = reqwest::Client::builder()
        .user_agent(previews.user_agent.clone())
        .connect_timeout(previews.timeout)
        .redirect(Policy::custom(move |attempt| {
            if attempt.previous().len() >= MAX_REDIRECTS {
                attempt.error("too many redirects")
            } else if !allow_private && is_private_url(attempt.url()) {
                attempt.error("redirect to a private host blocked")
            } else {
                attempt.follow()
            }
        }));
    if let Some(proxy) = &config.proxy {
        builder = builder.proxy(proxy.for_http()?);
    } else if !allow_private {
        builder = builder.dns_resolver(Arc::new(PublicOnly));
    }
    Ok(builder.build()?)
}

pub(crate) fn find_url(text: &str) -> Option<&str> {
    let lower = text.to_ascii_lowercase();
    let start = match (lower.find("https://"), lower.find("http://")) {
        (Some(a), Some(b)) => a.min(b),
        (a, b) => a.or(b)?,
    };
    if start > 0 && text[..start].chars().last().is_some_and(char::is_alphanumeric) {
        let rest = &text[start + 1..];
        return find_url(rest);
    }
    let rest = &text[start..];
    let end = rest.find(|c: char| c.is_whitespace() || matches!(c, '<' | '>' | '"')).unwrap_or(rest.len());
    let mut url = &rest[..end];
    loop {
        let last = url.chars().last()?;
        let strip = match last {
            '.' | ',' | ';' | ':' | '!' | '?' | '\'' | '»' | '›' => true,
            ')' => url.matches(')').count() > url.matches('(').count(),
            ']' => url.matches(']').count() > url.matches('[').count(),
            '}' => url.matches('}').count() > url.matches('{').count(),
            _ => false,
        };
        if !strip {
            break;
        }
        url = &url[..url.len() - last.len_utf8()];
    }
    Url::parse(url).ok().filter(|parsed| parsed.host().is_some())?;
    Some(url)
}

fn decode_entities(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find('&') {
        out.push_str(&rest[..start]);
        rest = &rest[start..];
        let Some(end) = rest[..rest.len().min(12)].find(';') else {
            out.push('&');
            rest = &rest[1..];
            continue;
        };
        let entity = &rest[1..end];
        let decoded = match entity {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            "nbsp" => Some(' '),
            _ => entity
                .strip_prefix("#x")
                .or_else(|| entity.strip_prefix("#X"))
                .map(|hex| u32::from_str_radix(hex, 16))
                .or_else(|| entity.strip_prefix('#').map(str::parse))
                .and_then(|code| code.ok())
                .and_then(char::from_u32),
        };
        match decoded {
            Some(character) => {
                out.push(character);
                rest = &rest[end + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

fn clean(text: &str) -> Option<String> {
    let collapsed = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let decoded = decode_entities(&collapsed);
    let trimmed = decoded.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

fn attribute<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let lower = tag.to_ascii_lowercase();
    let mut from = 0;
    while let Some(found) = lower[from..].find(name) {
        let at = from + found;
        from = at + name.len();
        let before = lower[..at].chars().last();
        if before.is_some_and(|c| c.is_ascii_alphanumeric() || c == '-' || c == ':') {
            continue;
        }
        let rest = tag[at + name.len()..].trim_start();
        let Some(rest) = rest.strip_prefix('=') else { continue };
        let rest = rest.trim_start();
        let quote = rest.chars().next()?;
        return if quote == '"' || quote == '\'' {
            rest[1..].find(quote).map(|end| &rest[1..1 + end])
        } else {
            Some(rest.split(|c: char| c.is_whitespace() || c == '>').next().unwrap_or_default())
        };
    }
    None
}

fn first_of(found: &[(String, String)], keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| found.iter().find(|(name, _)| name == key).map(|(_, value)| value.clone()))
}

pub(crate) fn parse_preview(url: &str, html: &str) -> LinkPreview {
    let lower = html.to_ascii_lowercase();
    let head_end = lower.find("</head").unwrap_or(lower.len());
    let (head, lower_head) = (&html[..head_end], &lower[..head_end]);
    let mut tags = Vec::new();
    let mut cursor = 0;
    while let Some(found) = lower_head[cursor..].find("<meta") {
        let start = cursor + found;
        let end = lower_head[start..].find('>').map_or(lower_head.len(), |e| start + e);
        let tag = &head[start..end];
        cursor = end;
        let key = attribute(tag, "property").or_else(|| attribute(tag, "name")).unwrap_or_default();
        if let Some(content) = attribute(tag, "content") {
            tags.push((key.to_ascii_lowercase(), content.to_owned()));
        }
    }
    let title_tag = lower_head.find("<title").and_then(|open| {
        let gt = open + lower_head[open..].find('>')? + 1;
        let close = gt + lower_head[gt..].find("</title")?;
        Some(head[gt..close].to_owned())
    });
    let title = first_of(&tags, &["og:title", "twitter:title"]).or(title_tag);
    let description = first_of(&tags, &["og:description", "twitter:description", "description"]);
    let image = first_of(&tags, &["og:image:secure_url", "og:image:url", "og:image", "twitter:image"]);
    LinkPreview {
        url: url.to_owned(),
        title: title.as_deref().and_then(clean),
        description: description.as_deref().and_then(clean),
        image_url: image.as_deref().and_then(clean).and_then(|link| absolutize(url, &link)),
        width: first_of(&tags, &["og:image:width"]).and_then(|w| w.trim().parse().ok()),
        height: first_of(&tags, &["og:image:height"]).and_then(|h| h.trim().parse().ok()),
        video: first_of(&tags, &["og:video:secure_url", "og:video:url", "og:video"]).is_some(),
        ..Default::default()
    }
}

fn absolutize(base: &str, link: &str) -> Option<String> {
    Url::parse(base).ok()?.join(link).ok().map(String::from)
}

fn hostname(url: &str) -> Option<String> {
    Url::parse(url).ok()?.host_str().map(str::to_owned)
}

fn decode_limited(data: &[u8]) -> Result<DynamicImage> {
    let mut reader = ImageReader::new(Cursor::new(data))
        .with_guessed_format()
        .map_err(|err| Error::Media(format!("unsupported image: {err}")))?;
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_IMAGE_SIDE);
    limits.max_image_height = Some(MAX_IMAGE_SIDE);
    limits.max_alloc = Some(MAX_IMAGE_ALLOC);
    reader.limits(limits);
    reader.decode().map_err(|err| Error::Media(format!("unsupported image: {err}")))
}

fn scaled(image: &DynamicImage, width: u32) -> DynamicImage {
    let (w, h) = image.dimensions();
    if w <= width {
        return image.clone();
    }
    let target_h = ((u64::from(h) * u64::from(width)) / u64::from(w.max(1))).max(1) as u32;
    image.resize_exact(width, target_h, FilterType::Triangle)
}

struct Thumbnails {
    inline: Vec<u8>,
    high_quality: Option<Vec<u8>>,
    width: u32,
    height: u32,
}

fn make_thumbnails(data: &[u8]) -> Result<Thumbnails> {
    let image = decode_limited(data)?;
    let inline = encode_jpeg(&scaled(&image, INLINE_THUMBNAIL_WIDTH), INLINE_THUMBNAIL_QUALITY)?;
    if image.width() <= INLINE_THUMBNAIL_WIDTH {
        return Ok(Thumbnails { inline, high_quality: None, width: image.width(), height: image.height() });
    }
    let large = scaled(&image, HQ_THUMBNAIL_WIDTH);
    let high_quality = Some(encode_jpeg(&large, HQ_THUMBNAIL_QUALITY)?);
    Ok(Thumbnails { inline, high_quality, width: large.width(), height: large.height() })
}

async fn read_html(mut response: reqwest::Response, limit: usize) -> Result<String> {
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        let seen_from = body.len().saturating_sub(8);
        body.extend_from_slice(&chunk);
        let window = String::from_utf8_lossy(&body[seen_from..]).to_ascii_lowercase();
        if window.contains("</head") || body.len() >= limit {
            body.truncate(limit);
            break;
        }
    }
    Ok(String::from_utf8_lossy(&body).into_owned())
}

async fn read_image(mut response: reqwest::Response, limit: usize) -> Result<Vec<u8>> {
    if response.content_length().is_some_and(|length| length > limit as u64) {
        return Err(Error::Media("the preview image is too large".into()));
    }
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        body.extend_from_slice(&chunk);
        if body.len() > limit {
            return Err(Error::Media("the preview image is too large".into()));
        }
    }
    Ok(body)
}

fn content_type(response: &reqwest::Response) -> String {
    response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_ascii_lowercase()
}

impl Client {
    fn cached_preview(&self, url: &str) -> Option<LinkPreview> {
        let ttl = self.inner.config.link_previews.cache_ttl?;
        let entry = self.inner.previews.get(url)?;
        (entry.0.elapsed() < ttl).then(|| entry.1.clone())
    }

    fn remember_preview(&self, url: &str, preview: &LinkPreview) {
        if self.inner.config.link_previews.cache_ttl.is_none() {
            return;
        }
        if self.inner.previews.len() >= MAX_CACHED_PREVIEWS {
            self.inner.previews.clear();
        }
        self.inner.previews.insert(url.to_owned(), (Instant::now(), preview.clone()));
    }

    async fn fetch_image(&self, url: &str) -> Result<Vec<u8>> {
        let config = &self.inner.config.link_previews;
        let parsed = Url::parse(url).map_err(|err| Error::Media(err.to_string()))?;
        if !config.allow_private_hosts && is_private_url(&parsed) {
            return Err(Error::Media("the preview image is on a private host".into()));
        }
        let response = self
            .inner
            .preview_http
            .get(parsed)
            .header("accept", "image/jpeg,image/*;q=0.8")
            .timeout(config.timeout)
            .send()
            .await?
            .error_for_status()?;
        if !content_type(&response).starts_with("image/") {
            return Err(Error::Media("the preview image is not an image".into()));
        }
        read_image(response, config.max_image_bytes).await
    }

    async fn fetch_preview(&self, url: &str) -> Result<LinkPreview> {
        let config = &self.inner.config.link_previews;
        let parsed = Url::parse(url).map_err(|err| Error::Media(err.to_string()))?;
        if !matches!(parsed.scheme(), "http" | "https") {
            return Err(Error::Media("only http and https links get a preview".into()));
        }
        if !config.allow_private_hosts && is_private_url(&parsed) {
            return Err(Error::Media("link preview blocked: private host".into()));
        }
        let response = self
            .inner
            .preview_http
            .get(parsed)
            .header("accept", "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8")
            .timeout(config.timeout)
            .send()
            .await?
            .error_for_status()?;
        let kind = content_type(&response);
        if !kind.contains("text/html") && !kind.contains("application/xhtml") {
            return Ok(LinkPreview { url: url.to_owned(), ..Default::default() });
        }
        let html = read_html(response, config.max_html_bytes).await?;
        let mut preview = parse_preview(url, &html);
        if let Some(image_url) = preview.image_url.clone() {
            match self.fetch_image(&image_url).await {
                Ok(data) => {
                    let made = tokio::task::spawn_blocking(move || make_thumbnails(&data))
                        .await
                        .map_err(|err| Error::Media(err.to_string()))?;
                    match made {
                        Ok(thumbnails) => {
                            preview.thumbnail = Some(thumbnails.inline);
                            preview.image = thumbnails.high_quality;
                            preview.width = Some(thumbnails.width);
                            preview.height = Some(thumbnails.height);
                        }
                        Err(err) => tracing::debug!(%err, "could not read the preview image"),
                    }
                }
                Err(err) => tracing::debug!(%err, "could not download the preview image"),
            }
        }
        Ok(preview)
    }

    pub async fn link_preview(&self, url: &str) -> Result<LinkPreview> {
        if let Some(cached) = self.cached_preview(url) {
            return Ok(cached);
        }
        let config = &self.inner.config.link_previews;
        let preview = match &config.fetcher {
            Some(fetcher) => {
                fetcher.fetch(url).await?.ok_or_else(|| Error::Media("the custom fetcher declined this link".into()))?
            }
            None => self.fetch_preview(url).await?,
        };
        self.remember_preview(url, &preview);
        Ok(preview)
    }

    async fn upload_link_thumbnail(&self, chat: &Jid, image: &[u8], text: &mut ExtendedTextMessage) -> Result<()> {
        if chat.is_newsletter() {
            let answer = self.upload_plain(image.to_vec(), NEWSLETTER_THUMBNAIL_PATH).await?;
            text.thumbnail_direct_path = answer.direct_path;
            text.thumbnail_sha256 = Some(crate::crypto::sha256(image).to_vec());
            return Ok(());
        }
        let uploaded = match self.upload(image, MediaKind::LinkThumbnail).await {
            Ok(uploaded) => uploaded,
            Err(err) => {
                tracing::debug!(%err, "the thumbnail path refused the upload, trying the image path");
                self.upload_fresh_at(image, MediaKind::LinkThumbnail, IMAGE_THUMBNAIL_PATH).await?
            }
        };
        text.thumbnail_direct_path = Some(uploaded.direct_path);
        text.thumbnail_sha256 = Some(uploaded.file_sha256);
        text.thumbnail_enc_sha256 = Some(uploaded.file_enc_sha256);
        text.media_key = Some(uploaded.media_key);
        text.media_key_timestamp = Some(now_secs());
        Ok(())
    }

    async fn fill_preview(&self, chat: &Jid, mut message: Message, matched: String, preview: LinkPreview) -> Message {
        let text = message.get_text().unwrap_or_default().to_owned();
        message.conversation = None;
        let extended = message.extended_text_message.get_or_insert_with(|| Box::new(ExtendedTextMessage::default()));
        extended.text = Some(text);
        extended.title = preview.title.or_else(|| hostname(&matched));
        extended.matched_text = Some(matched);
        extended.description = preview.description;
        extended.preview_type = Some(if preview.video { PREVIEW_VIDEO } else { 0 });
        extended.jpeg_thumbnail = preview.thumbnail;
        extended.thumbnail_width = preview.width;
        extended.thumbnail_height = preview.height;
        if self.inner.config.link_previews.upload_hq_thumbnail {
            if let Some(image) = &preview.image {
                if let Err(err) = self.upload_link_thumbnail(chat, image, extended).await {
                    tracing::debug!(%err, "sending the link preview with the small thumbnail only");
                }
            }
        }
        message
    }

    pub async fn with_link_preview(&self, message: Message) -> Message {
        self.preview_for(&Jid::default(), message).await
    }

    async fn preview_for(&self, chat: &Jid, message: Message) -> Message {
        let Some(url) = message.get_text().and_then(find_url).map(str::to_owned) else { return message };
        let budget = self.inner.config.link_previews.timeout * 2;
        match tokio::time::timeout(budget, self.link_preview(&url)).await {
            Ok(Ok(preview)) => self.fill_preview(chat, message, url, preview).await,
            Ok(Err(err)) if err.to_string().contains("private") => {
                tracing::debug!(%url, "link preview blocked: private host");
                message
            }
            Ok(Err(err)) => {
                tracing::debug!(%url, %err, "link preview failed, sending the minimal card");
                let minimal = LinkPreview { url: url.clone(), ..Default::default() };
                self.fill_preview(chat, message, url, minimal).await
            }
            Err(_) => {
                tracing::debug!(%url, "link preview timed out, sending the minimal card");
                let minimal = LinkPreview { url: url.clone(), ..Default::default() };
                self.fill_preview(chat, message, url, minimal).await
            }
        }
    }

    pub(super) async fn apply_link_preview(&self, chat: &Jid, message: Message, choice: &PreviewChoice) -> Message {
        let plain_text = message.conversation.is_some()
            || message.extended_text_message.as_ref().is_some_and(|text| text.matched_text.is_none());
        if !plain_text || chat.is_status_broadcast() {
            return message;
        }
        match choice {
            PreviewChoice::Off => message,
            PreviewChoice::Auto if !self.inner.config.link_previews.enabled => message,
            PreviewChoice::Auto | PreviewChoice::On => self.preview_for(chat, message).await,
            PreviewChoice::Custom(preview) => {
                let matched = if preview.url.is_empty() {
                    message.get_text().and_then(find_url).map(str::to_owned)
                } else {
                    Some(preview.url.clone())
                };
                match matched {
                    Some(matched) => self.fill_preview(chat, message, matched, (**preview).clone()).await,
                    None => message,
                }
            }
        }
    }

    pub async fn send_text_with_preview(&self, chat: &Jid, text: impl Into<String>) -> Result<String> {
        let options = super::SendOptions { link_preview: PreviewChoice::On, ..Default::default() };
        self.send_message_with(chat, Message::text(text), options).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_links_the_way_people_write_them() {
        assert_eq!(find_url("mira esto https://soblend.dev/docs, es genial"), Some("https://soblend.dev/docs"));
        assert_eq!(find_url("sin enlaces"), None);
        assert_eq!(find_url("(http://a.io/x)"), Some("http://a.io/x"));
        assert_eq!(find_url("HTTPS://A.io/b."), Some("HTTPS://A.io/b"));
        assert_eq!(find_url("ver https://es.wiki.org/a_(b) ahora"), Some("https://es.wiki.org/a_(b)"));
        assert_eq!(find_url("(ver https://es.wiki.org/a_(b))"), Some("https://es.wiki.org/a_(b)"));
        assert_eq!(find_url("«https://x.io/ñandú»"), Some("https://x.io/ñandú"));
        assert_eq!(find_url("xhttps://no.es"), None);
        assert_eq!(find_url("https://"), None);
    }

    #[test]
    fn reads_open_graph_tags_like_a_browser() {
        let html = r#"<html><head><title>Fallback</title>
            <meta property="og:title" content="SoblendWA &amp; Rust &#x1F980;">
            <meta name='description' content='  El  cliente
                más &#225;gil '>
            <meta content="/img/low.png" property="og:image" />
            <meta property="og:image:secure_url" content="/img/cover.png">
            <meta property="og:image:width" content="1200"><meta property="og:image:height" content="630">
            <meta property="og:video" content="https://x.io/v.mp4">
            </head><body><meta property="og:title" content="fuera del head"></body></html>"#;
        let preview = parse_preview("https://soblend.dev/blog/post", html);
        assert_eq!(preview.title.as_deref(), Some("SoblendWA & Rust 🦀"));
        assert_eq!(preview.description.as_deref(), Some("El cliente más ágil"));
        assert_eq!(preview.image_url.as_deref(), Some("https://soblend.dev/img/cover.png"));
        assert_eq!((preview.width, preview.height, preview.video), (Some(1200), Some(630), true));

        let bare = parse_preview("https://a.io/b/c", "<head><title>\n Solo   título </title></head>");
        assert_eq!((bare.title.as_deref(), bare.video), (Some("Solo título"), false));
    }

    #[test]
    fn private_hosts_are_refused() {
        for url in [
            "http://127.0.0.1/",
            "http://2130706433/",
            "http://10.1.2.3/",
            "http://172.16.0.1/",
            "http://192.168.1.1/",
            "http://169.254.169.254/latest/meta-data",
            "http://100.64.0.1/",
            "http://0.0.0.0/",
            "http://[::1]/",
            "http://[fe80::1]/",
            "http://[fd00::1]/",
            "http://[::ffff:10.0.0.1]/",
            "http://localhost/",
            "http://db.localhost/",
            "http://impresora.local/",
            "http://metadata.google.internal/",
            "http://intranet/",
        ] {
            assert!(is_private_url(&Url::parse(url).unwrap()), "{url}");
        }
        for url in ["https://soblend.dev/", "http://8.8.8.8/", "http://[2606:4700:4700::1111]/"] {
            assert!(!is_private_url(&Url::parse(url).unwrap()), "{url}");
        }
    }

    #[test]
    fn hostile_images_cannot_exhaust_memory() {
        let huge = image::RgbImage::new(13_000, 1);
        let mut png = Cursor::new(Vec::new());
        DynamicImage::ImageRgb8(huge).write_to(&mut png, image::ImageFormat::Png).unwrap();
        assert!(make_thumbnails(&png.into_inner()).is_err());

        let source = DynamicImage::new_rgb8(2400, 1260);
        let mut png = Cursor::new(Vec::new());
        source.write_to(&mut png, image::ImageFormat::Png).unwrap();
        let made = make_thumbnails(&png.into_inner()).unwrap();
        assert_eq!((made.width, made.height), (1200, 630));
        assert_eq!(super::super::image_dimensions(&made.inline), Some((192, 100)));
        assert_eq!(super::super::image_dimensions(made.high_quality.as_ref().unwrap()), Some((1200, 630)));
    }
}
