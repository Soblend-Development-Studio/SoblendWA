use std::io;
use std::path::{Path, PathBuf};

use aes::cipher::generic_array::GenericArray;
use aes::cipher::{BlockDecryptMut, BlockEncryptMut, KeyIvInit};
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use bytes::Bytes;
use futures_util::{stream, Stream};
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};
use tokio::fs::File;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use super::media::{expand_keys, media_message, MediaKeys, MAC_LEN};
use super::{Client, Downloadable, MediaKind, MediaOptions, UploadedMedia};
use crate::crypto;
use crate::error::{Error, Result};
use crate::events::MessageEvent;
use crate::jid::Jid;
use crate::proto::Message;

const CHUNK: usize = 256 * 1024;
const BLOCK: usize = 16;

type Encryptor = cbc::Encryptor<aes::Aes256>;
type Decryptor = cbc::Decryptor<aes::Aes256>;
type HmacSha256 = Hmac<Sha256>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct MediaDigest {
    pub file_sha256: [u8; 32],
    pub file_enc_sha256: [u8; 32],
    pub file_length: u64,
}

pub(crate) fn sealed_length(plain_length: u64) -> u64 {
    (plain_length / BLOCK as u64 + 1) * BLOCK as u64 + MAC_LEN as u64
}

fn keyed_mac(keys: &MediaKeys) -> HmacSha256 {
    let mut mac = <HmacSha256 as Mac>::new_from_slice(&keys.mac).expect("hmac accepts any key length");
    mac.update(&keys.iv);
    mac
}

pub(crate) struct Sealer {
    cipher: Encryptor,
    mac: HmacSha256,
    plain: Sha256,
    sealed: Sha256,
    carry: Vec<u8>,
    length: u64,
}

impl Sealer {
    pub(crate) fn new(keys: &MediaKeys) -> Self {
        Sealer {
            cipher: Encryptor::new_from_slices(&keys.cipher, &keys.iv).expect("media keys have fixed sizes"),
            mac: keyed_mac(keys),
            plain: Sha256::new(),
            sealed: Sha256::new(),
            carry: Vec::with_capacity(BLOCK),
            length: 0,
        }
    }

    fn seal(&mut self, data: &mut [u8]) {
        for block in data.as_chunks_mut::<BLOCK>().0 {
            self.cipher.encrypt_block_mut(GenericArray::from_mut_slice(block));
        }
        self.mac.update(&*data);
        self.sealed.update(&*data);
    }

    pub(crate) fn update(&mut self, mut input: &[u8], out: &mut Vec<u8>) {
        self.plain.update(input);
        self.length += input.len() as u64;
        let start = out.len();
        if !self.carry.is_empty() {
            let take = (BLOCK - self.carry.len()).min(input.len());
            self.carry.extend_from_slice(&input[..take]);
            input = &input[take..];
            if self.carry.len() < BLOCK {
                return;
            }
            out.extend_from_slice(&self.carry);
            self.carry.clear();
        }
        let whole = input.len() - input.len() % BLOCK;
        out.extend_from_slice(&input[..whole]);
        self.carry.extend_from_slice(&input[whole..]);
        self.seal(&mut out[start..]);
    }

    pub(crate) fn finish(mut self, out: &mut Vec<u8>) -> MediaDigest {
        let pad = (BLOCK - self.carry.len()) as u8;
        let start = out.len();
        out.extend_from_slice(&self.carry);
        out.resize(start + BLOCK, pad);
        self.seal(&mut out[start..]);
        let mac = self.mac.finalize().into_bytes();
        out.extend_from_slice(&mac[..MAC_LEN]);
        self.sealed.update(&mac[..MAC_LEN]);
        MediaDigest {
            file_sha256: self.plain.finalize().into(),
            file_enc_sha256: self.sealed.finalize().into(),
            file_length: self.length,
        }
    }
}

pub(crate) struct Opener {
    cipher: Decryptor,
    mac: HmacSha256,
    plain: Sha256,
    sealed: Sha256,
    held: Vec<u8>,
    length: u64,
}

impl Opener {
    const RESERVE: usize = MAC_LEN + BLOCK;

    pub(crate) fn new(keys: &MediaKeys) -> Self {
        Opener {
            cipher: Decryptor::new_from_slices(&keys.cipher, &keys.iv).expect("media keys have fixed sizes"),
            mac: keyed_mac(keys),
            plain: Sha256::new(),
            sealed: Sha256::new(),
            held: Vec::with_capacity(CHUNK + Self::RESERVE),
            length: 0,
        }
    }

    fn open(&mut self, data: &mut [u8]) {
        self.mac.update(&*data);
        self.sealed.update(&*data);
        for block in data.as_chunks_mut::<BLOCK>().0 {
            self.cipher.decrypt_block_mut(GenericArray::from_mut_slice(block));
        }
    }

    pub(crate) fn update(&mut self, input: &[u8], out: &mut Vec<u8>) {
        self.held.extend_from_slice(input);
        let available = self.held.len().saturating_sub(Self::RESERVE);
        let ready = available - available % BLOCK;
        if ready == 0 {
            return;
        }
        let start = out.len();
        out.extend_from_slice(&self.held[..ready]);
        self.held.drain(..ready);
        self.open(&mut out[start..]);
        self.plain.update(&out[start..]);
        self.length += ready as u64;
    }

    pub(crate) fn finish(
        mut self,
        out: &mut Vec<u8>,
        file_sha256: Option<&[u8]>,
        file_enc_sha256: Option<&[u8]>,
    ) -> Result<u64> {
        let held = std::mem::take(&mut self.held);
        if held.len() < Self::RESERVE || !(held.len() - MAC_LEN).is_multiple_of(BLOCK) {
            return Err(Error::Media("downloaded file is truncated".into()));
        }
        let (body, mac) = held.split_at(held.len() - MAC_LEN);
        let mut body = body.to_vec();
        self.open(&mut body);
        self.sealed.update(mac);
        let expected_mac = self.mac.finalize().into_bytes();
        if !crypto::constant_time_eq(&expected_mac[..MAC_LEN], mac) {
            return Err(Error::Media("media mac mismatch".into()));
        }
        if let Some(expected) = file_enc_sha256 {
            if self.sealed.finalize().as_slice() != expected {
                return Err(Error::Media("encrypted file hash mismatch".into()));
            }
        }
        let pad = usize::from(*body.last().unwrap_or(&0));
        if pad == 0 || pad > BLOCK || !body[body.len() - pad..].iter().all(|b| usize::from(*b) == pad) {
            return Err(Error::Media("media padding is invalid".into()));
        }
        body.truncate(body.len() - pad);
        self.plain.update(&body);
        self.length += body.len() as u64;
        out.extend_from_slice(&body);
        if let Some(expected) = file_sha256 {
            if self.plain.finalize().as_slice() != expected {
                return Err(Error::Media("decrypted file hash mismatch".into()));
            }
        }
        Ok(self.length)
    }
}

async fn digest_file(path: &Path, keys: &MediaKeys) -> Result<MediaDigest> {
    let mut file = File::open(path).await?;
    let mut sealer = Sealer::new(keys);
    let mut buf = vec![0u8; CHUNK];
    let mut scratch = Vec::with_capacity(CHUNK + BLOCK);
    loop {
        let read = file.read(&mut buf).await?;
        if read == 0 {
            break;
        }
        scratch.clear();
        sealer.update(&buf[..read], &mut scratch);
    }
    scratch.clear();
    Ok(sealer.finish(&mut scratch))
}

pub(crate) fn sealed_stream(
    path: PathBuf,
    keys: MediaKeys,
    expected_length: u64,
) -> impl Stream<Item = io::Result<Bytes>> + Send + 'static {
    let state = (path, None::<File>, Some(Sealer::new(&keys)), 0u64);
    stream::try_unfold(state, move |(path, file, sealer, read)| async move {
        let Some(mut sealer) = sealer else { return Ok(None) };
        let mut file = match file {
            Some(file) => file,
            None => File::open(&path).await?,
        };
        let mut buf = vec![0u8; CHUNK];
        let count = file.read(&mut buf).await?;
        let mut out = Vec::with_capacity(count + BLOCK + MAC_LEN);
        if count == 0 {
            if read != expected_length {
                return Err(io::Error::other("the file changed while it was being uploaded"));
            }
            sealer.finish(&mut out);
            return Ok(Some((Bytes::from(out), (path, None, None, read))));
        }
        sealer.update(&buf[..count], &mut out);
        Ok(Some((Bytes::from(out), (path, Some(file), Some(sealer), read + count as u64))))
    })
}

fn file_stream(path: PathBuf) -> impl Stream<Item = io::Result<Bytes>> + Send + 'static {
    stream::try_unfold((path, None::<File>), |(path, file)| async move {
        let mut file = match file {
            Some(file) => file,
            None => File::open(&path).await?,
        };
        let mut buf = vec![0u8; CHUNK];
        let count = file.read(&mut buf).await?;
        if count == 0 {
            return Ok(None);
        }
        buf.truncate(count);
        Ok(Some((Bytes::from(buf), (path, Some(file)))))
    })
}

fn spool_path() -> PathBuf {
    std::env::temp_dir().join(format!("soblendwa-{}.enc", hex::encode(crypto::random_array::<8>())))
}

fn partial_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().map(|n| n.to_os_string()).unwrap_or_default();
    name.push(".part");
    path.with_file_name(name)
}

fn is_expired(err: &Error) -> bool {
    matches!(err, Error::Http(http) if matches!(http.status().map(|s| s.as_u16()), Some(404 | 410)))
}

pub fn guess_mimetype(path: &Path) -> Option<&'static str> {
    let extension = path.extension()?.to_str()?.to_ascii_lowercase();
    Some(match extension.as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "webp" => "image/webp",
        "gif" => "image/gif",
        "mp4" | "m4v" => "video/mp4",
        "3gp" => "video/3gpp",
        "mov" => "video/quicktime",
        "mkv" => "video/x-matroska",
        "mp3" => "audio/mpeg",
        "m4a" => "audio/mp4",
        "aac" => "audio/aac",
        "ogg" | "opus" => "audio/ogg; codecs=opus",
        "wav" => "audio/wav",
        "pdf" => "application/pdf",
        "zip" => "application/zip",
        "rar" => "application/vnd.rar",
        "7z" => "application/x-7z-compressed",
        "apk" => "application/vnd.android.package-archive",
        "txt" => "text/plain",
        "csv" => "text/csv",
        "json" => "application/json",
        "doc" => "application/msword",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "xls" => "application/vnd.ms-excel",
        "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "ppt" => "application/vnd.ms-powerpoint",
        "pptx" => "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        _ => return None,
    })
}

impl Client {
    pub async fn upload_file(&self, path: impl AsRef<Path>, kind: MediaKind) -> Result<UploadedMedia> {
        let path = path.as_ref().to_path_buf();
        let media_key = crypto::random_array::<32>();
        let digest = digest_file(&path, &expand_keys(&media_key, kind)).await?;
        if let Some(hit) = self.cached_upload(kind, &digest.file_sha256).await {
            return Ok(hit.uploaded());
        }
        let token = URL_SAFE_NO_PAD.encode(digest.file_enc_sha256);
        let body = || {
            let stream = sealed_stream(path.clone(), expand_keys(&media_key, kind), digest.file_length);
            Ok(reqwest::Body::wrap_stream(stream))
        };
        let answer = self
            .post_media(kind.upload_path(), &token, sealed_length(digest.file_length), body, |a| {
                a.url.is_some() && a.direct_path.is_some()
            })
            .await?;
        let uploaded = uploaded(answer, media_key, digest);
        self.remember_upload(kind, &uploaded, &MediaOptions::default()).await;
        Ok(uploaded)
    }

    pub async fn upload_reader<R>(&self, mut reader: R, kind: MediaKind) -> Result<UploadedMedia>
    where
        R: AsyncRead + Unpin + Send,
    {
        let media_key = crypto::random_array::<32>();
        let spool = spool_path();
        let result = async {
            let mut file = File::create(&spool).await?;
            let mut sealer = Sealer::new(&expand_keys(&media_key, kind));
            let mut buf = vec![0u8; CHUNK];
            let mut out = Vec::with_capacity(CHUNK + BLOCK + MAC_LEN);
            loop {
                let read = reader.read(&mut buf).await?;
                if read == 0 {
                    break;
                }
                out.clear();
                sealer.update(&buf[..read], &mut out);
                file.write_all(&out).await?;
            }
            out.clear();
            let digest = sealer.finish(&mut out);
            file.write_all(&out).await?;
            file.flush().await?;
            drop(file);
            if let Some(hit) = self.cached_upload(kind, &digest.file_sha256).await {
                return Ok(hit.uploaded());
            }
            let token = URL_SAFE_NO_PAD.encode(digest.file_enc_sha256);
            let body = || Ok(reqwest::Body::wrap_stream(file_stream(spool.clone())));
            let answer = self
                .post_media(kind.upload_path(), &token, sealed_length(digest.file_length), body, |a| {
                    a.url.is_some() && a.direct_path.is_some()
                })
                .await?;
            let uploaded = uploaded(answer, media_key, digest);
            self.remember_upload(kind, &uploaded, &MediaOptions::default()).await;
            Ok(uploaded)
        }
        .await;
        let _ = tokio::fs::remove_file(&spool).await;
        result
    }

    pub async fn download_to_writer<W>(&self, media: &dyn Downloadable, writer: &mut W) -> Result<u64>
    where
        W: AsyncWrite + Unpin + Send + ?Sized,
    {
        let media_key = media.media_key().ok_or_else(|| Error::Media("message has no media key".into()))?;
        let mut response = self
            .inner
            .http
            .get(self.download_url(media)?)
            .header("Origin", "https://web.whatsapp.com")
            .send()
            .await?
            .error_for_status()?;
        let mut opener = Opener::new(&expand_keys(media_key, media.media_kind()));
        let mut out = Vec::with_capacity(CHUNK);
        while let Some(chunk) = response.chunk().await? {
            out.clear();
            opener.update(&chunk, &mut out);
            writer.write_all(&out).await?;
        }
        out.clear();
        let length = opener.finish(&mut out, media.file_sha256(), media.file_enc_sha256())?;
        writer.write_all(&out).await?;
        writer.flush().await?;
        Ok(length)
    }

    pub async fn download_to_file(&self, media: &dyn Downloadable, path: impl AsRef<Path>) -> Result<u64> {
        let path = path.as_ref();
        let partial = partial_path(path);
        let result = async {
            if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
                tokio::fs::create_dir_all(parent).await?;
            }
            let mut file = File::create(&partial).await?;
            let length = self.download_to_writer(media, &mut file).await?;
            file.sync_all().await?;
            drop(file);
            tokio::fs::rename(&partial, path).await?;
            Ok(length)
        }
        .await;
        if result.is_err() {
            let _ = tokio::fs::remove_file(&partial).await;
        }
        result
    }

    pub async fn download_event_to_file(&self, event: &MessageEvent, path: impl AsRef<Path>) -> Result<u64> {
        let path = path.as_ref();
        let media = event.message.media().ok_or_else(|| Error::Media("message does not contain media".into()))?;
        match self.download_to_file(media, path).await {
            Err(err) if is_expired(&err) => {
                let refreshed = self.reupload_media(event).await?;
                let media =
                    refreshed.media().ok_or_else(|| Error::Media("re-uploaded message lost its media".into()))?;
                self.download_to_file(media, path).await
            }
            other => other,
        }
    }

    pub async fn prepare_media_file(
        &self,
        path: impl AsRef<Path>,
        kind: MediaKind,
        mut options: MediaOptions,
    ) -> Result<Message> {
        let path = path.as_ref();
        if matches!(kind, MediaKind::History | MediaKind::AppState | MediaKind::LinkThumbnail) {
            return Err(Error::Media("this media kind cannot be sent as a message".into()));
        }
        if matches!(kind, MediaKind::Image | MediaKind::Sticker) {
            let data = tokio::fs::read(path).await?;
            options.mimetype = options.mimetype.or_else(|| guess_mimetype(path).map(str::to_owned));
            return self.prepare_media(&data, kind, options).await;
        }
        if kind == MediaKind::Document && options.file_name.is_none() {
            options.file_name = path.file_name().map(|name| name.to_string_lossy().into_owned());
        }
        if options.mimetype.is_none() && !(kind == MediaKind::Audio && options.voice_note) {
            options.mimetype = guess_mimetype(path).map(str::to_owned);
        }
        let uploaded = self.upload_file(path, kind).await?;
        media_message(kind, uploaded, options)
    }

    pub async fn send_media_file(
        &self,
        chat: &Jid,
        path: impl AsRef<Path>,
        kind: MediaKind,
        options: MediaOptions,
    ) -> Result<String> {
        self.send_prepared(chat, self.prepare_media_file(path, kind, options)).await
    }
}

fn uploaded(answer: super::media::UploadResponse, media_key: [u8; 32], digest: MediaDigest) -> UploadedMedia {
    UploadedMedia {
        url: answer.url.unwrap_or_default(),
        direct_path: answer.direct_path.unwrap_or_default(),
        media_key: media_key.to_vec(),
        file_sha256: digest.file_sha256.to_vec(),
        file_enc_sha256: digest.file_enc_sha256.to_vec(),
        file_length: digest.file_length,
        reused: false,
    }
}

#[cfg(test)]
mod tests {
    use futures_util::TryStreamExt;
    use tokio::net::TcpListener;

    use super::*;
    use crate::proto::DocumentMessage;

    fn one_shot(data: &[u8], keys: &MediaKeys) -> Vec<u8> {
        let mut body = crypto::aes_cbc_encrypt(&keys.cipher, &keys.iv, data).unwrap();
        let mac = crypto::hmac_sha256(&keys.mac, &[&keys.iv, &body]);
        body.extend_from_slice(&mac[..MAC_LEN]);
        body
    }

    fn splits(len: usize, seed: usize) -> Vec<usize> {
        let mut cuts = Vec::new();
        let mut at = 0;
        let mut step = seed;
        while at < len {
            step = (step * 7 + 3) % 4099 + 1;
            at = (at + step).min(len);
            cuts.push(at);
        }
        cuts
    }

    #[test]
    fn streaming_seal_matches_the_one_shot_encryption() {
        let key = crypto::random_array::<32>();
        let keys = expand_keys(&key, MediaKind::Video);
        for len in [0usize, 1, 15, 16, 17, 31, 32, 1000, 65_536, 100_003] {
            let data = crypto::random_vec(len);
            let mut sealer = Sealer::new(&keys);
            let mut out = Vec::new();
            let mut from = 0;
            for to in splits(len, len) {
                sealer.update(&data[from..to], &mut out);
                from = to;
            }
            let digest = sealer.finish(&mut out);
            let expected = one_shot(&data, &keys);
            assert_eq!(out, expected, "length {len}");
            assert_eq!(out.len() as u64, sealed_length(len as u64));
            assert_eq!(digest.file_sha256, crypto::sha256(&data));
            assert_eq!(digest.file_enc_sha256, crypto::sha256(&expected));
            assert_eq!(digest.file_length, len as u64);
        }
    }

    #[test]
    fn streaming_open_verifies_everything() {
        let key = crypto::random_array::<32>();
        let keys = expand_keys(&key, MediaKind::Document);
        for len in [0usize, 5, 16, 33, 4096, 77_777] {
            let data = crypto::random_vec(len);
            let sealed = one_shot(&data, &keys);
            let plain_hash = crypto::sha256(&data);
            let sealed_hash = crypto::sha256(&sealed);
            let open = |bytes: &[u8], seed: usize| {
                let mut opener = Opener::new(&keys);
                let mut out = Vec::new();
                let mut from = 0;
                for to in splits(bytes.len(), seed) {
                    opener.update(&bytes[from..to], &mut out);
                    from = to;
                }
                opener.finish(&mut out, Some(&plain_hash), Some(&sealed_hash)).map(|n| (n, out))
            };
            let (length, out) = open(&sealed, len + 1).unwrap();
            assert_eq!((length, out), (len as u64, data.clone()));

            let mut tampered = sealed.clone();
            tampered[len / 2] ^= 1;
            assert!(open(&tampered, 3).is_err());
            assert!(open(&sealed[..sealed.len() - 1], 5).is_err());
        }
    }

    #[tokio::test]
    async fn uploads_stream_from_disk_without_buffering_the_file() {
        let dir = std::env::temp_dir().join(format!("soblendwa-stream-{}", crypto::random_u32()));
        tokio::fs::create_dir_all(&dir).await.unwrap();
        let path = dir.join("video.mp4");
        let data = crypto::random_vec(3 * CHUNK + 123);
        tokio::fs::write(&path, &data).await.unwrap();

        let key = crypto::random_array::<32>();
        let keys = expand_keys(&key, MediaKind::Video);
        let digest = digest_file(&path, &keys).await.unwrap();
        let streamed: Vec<Bytes> = sealed_stream(path.clone(), expand_keys(&key, MediaKind::Video), digest.file_length)
            .try_collect()
            .await
            .unwrap();
        let streamed = streamed.concat();
        assert_eq!(streamed, one_shot(&data, &keys));
        assert_eq!(crypto::sha256(&streamed), digest.file_enc_sha256);

        tokio::fs::write(&path, b"changed").await.unwrap();
        let changed: io::Result<Vec<Bytes>> =
            sealed_stream(path.clone(), expand_keys(&key, MediaKind::Video), digest.file_length).try_collect().await;
        assert!(changed.is_err());
        let _ = tokio::fs::remove_dir_all(dir).await;
    }

    async fn serve_once(body: Vec<u8>) -> u16 {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            while let Ok((mut socket, _)) = listener.accept().await {
                let body = body.clone();
                tokio::spawn(async move {
                    let mut request = Vec::new();
                    let mut byte = [0u8; 1];
                    while !request.ends_with(b"\r\n\r\n") {
                        if socket.read_exact(&mut byte).await.is_err() {
                            return;
                        }
                        request.push(byte[0]);
                    }
                    let head =
                        format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
                    let _ = socket.write_all(head.as_bytes()).await;
                    for piece in body.chunks(7_919) {
                        let _ = socket.write_all(piece).await;
                    }
                });
            }
        });
        port
    }

    #[tokio::test]
    async fn downloads_land_on_disk_only_when_verified() {
        let (client, _events) = Client::builder().build().await.unwrap();
        let dir = std::env::temp_dir().join(format!("soblendwa-download-{}", crypto::random_u32()));
        let key = crypto::random_array::<32>();
        let keys = expand_keys(&key, MediaKind::Document);
        let data = crypto::random_vec(2 * CHUNK + 999);
        let sealed = one_shot(&data, &keys);

        let document = |port: u16, sha: Vec<u8>| DocumentMessage {
            url: Some(format!("http://127.0.0.1:{port}/media")),
            media_key: Some(key.to_vec()),
            file_sha256: Some(sha),
            file_enc_sha256: Some(crypto::sha256(&sealed).to_vec()),
            ..Default::default()
        };

        let good = document(serve_once(sealed.clone()).await, crypto::sha256(&data).to_vec());
        let target = dir.join("nested").join("report.pdf");
        let written = client.download_to_file(&good, &target).await.unwrap();
        assert_eq!(written, data.len() as u64);
        assert_eq!(tokio::fs::read(&target).await.unwrap(), data);
        assert!(!partial_path(&target).exists());

        let mut corrupted = sealed.clone();
        corrupted[100] ^= 0x40;
        let bad = document(serve_once(corrupted).await, crypto::sha256(&data).to_vec());
        let rejected = dir.join("rejected.pdf");
        assert!(client.download_to_file(&bad, &rejected).await.is_err());
        assert!(!rejected.exists() && !partial_path(&rejected).exists());

        let mut memory = Vec::new();
        let length = client.download_to_writer(&good, &mut memory).await.unwrap();
        assert_eq!((length, memory), (data.len() as u64, data));
        let _ = tokio::fs::remove_dir_all(dir).await;
    }

    #[test]
    fn guesses_common_mimetypes() {
        assert_eq!(guess_mimetype(Path::new("Reporte.PDF")), Some("application/pdf"));
        assert_eq!(guess_mimetype(Path::new("nota.opus")), Some("audio/ogg; codecs=opus"));
        assert_eq!(guess_mimetype(Path::new("sin_extension")), None);
        assert_eq!(partial_path(Path::new("a/b.mp4")), Path::new("a/b.mp4.part"));
    }
}
