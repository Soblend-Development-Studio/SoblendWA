use aes_gcm::aead::{Aead, Payload};
use aes_gcm::{Aes256Gcm, Nonce};
use prost::Message as _;

use crate::crypto::{self, KeyPair};
use crate::error::{Error, Result};
use crate::proto::{CertChain, CertificateDetails, ServerHello};
#[cfg(any(test, feature = "testing"))]
use crate::proto::{ClientFinish, NoiseCertificate};

const NOISE_MODE: &[u8; 32] = b"Noise_XX_25519_AESGCM_SHA256\0\0\0\0";
pub(crate) const WA_HEADER: [u8; 4] = [b'W', b'A', 6, 3];
pub(crate) const WA_ROOT_KEY: [u8; 32] = [
    0x14, 0x23, 0x75, 0x57, 0x4d, 0x0a, 0x58, 0x71, 0x66, 0xaa, 0xe7, 0x1e, 0xbe, 0x51, 0x64, 0x37, 0xc4, 0xa2, 0x8b,
    0x73, 0xe3, 0x69, 0x5c, 0x6c, 0xe1, 0xf7, 0xf9, 0x54, 0x5d, 0xa8, 0xee, 0x6b,
];
const WA_ROOT_SERIAL: u32 = 0;
const CERTIFICATE_SKEW_SECS: u64 = 24 * 60 * 60;
pub(crate) const MAX_FRAME: usize = (1 << 24) - 1;

pub(crate) fn intro_header(routing_info: Option<&[u8]>) -> Vec<u8> {
    match routing_info {
        Some(routing) => {
            let mut header = Vec::with_capacity(7 + routing.len() + WA_HEADER.len());
            header.extend_from_slice(b"ED");
            header.extend_from_slice(&[0, 1]);
            header.push((routing.len() >> 16) as u8);
            header.extend_from_slice(&(routing.len() as u16).to_be_bytes());
            header.extend_from_slice(routing);
            header.extend_from_slice(&WA_HEADER);
            header
        }
        None => WA_HEADER.to_vec(),
    }
}

pub(crate) struct Handshake {
    hash: [u8; 32],
    salt: [u8; 32],
    cipher: Aes256Gcm,
    counter: u32,
}

impl Handshake {
    pub(crate) fn new(ephemeral_public: &[u8; 32]) -> Self {
        let mut handshake =
            Handshake { hash: *NOISE_MODE, salt: *NOISE_MODE, cipher: crypto::gcm(NOISE_MODE), counter: 0 };
        handshake.authenticate(&WA_HEADER);
        handshake.authenticate(ephemeral_public);
        handshake
    }

    fn authenticate(&mut self, data: &[u8]) {
        self.hash = crypto::sha256_concat(&[&self.hash, data]);
    }

    fn mix_key(&mut self, input: &[u8]) {
        let derived: [u8; 64] = crypto::hkdf_array(input, Some(&self.salt), b"");
        self.salt.copy_from_slice(&derived[..32]);
        self.cipher = crypto::gcm(&derived[32..]);
        self.counter = 0;
    }

    fn next_iv(&mut self) -> [u8; 12] {
        let iv = crypto::gcm_iv(self.counter);
        self.counter += 1;
        iv
    }

    pub(crate) fn encrypt(&mut self, plaintext: &[u8]) -> Result<Vec<u8>> {
        let iv = self.next_iv();
        let ciphertext = self
            .cipher
            .encrypt(Nonce::from_slice(&iv), Payload { msg: plaintext, aad: &self.hash })
            .map_err(|_| Error::Crypto("noise encryption failed"))?;
        self.authenticate(&ciphertext);
        Ok(ciphertext)
    }

    fn decrypt(&mut self, ciphertext: &[u8]) -> Result<Vec<u8>> {
        let iv = self.next_iv();
        let plaintext = self
            .cipher
            .decrypt(Nonce::from_slice(&iv), Payload { msg: ciphertext, aad: &self.hash })
            .map_err(|_| Error::Handshake("server hello failed authentication".into()))?;
        self.authenticate(ciphertext);
        Ok(plaintext)
    }

    pub(crate) fn process_server_hello(
        &mut self,
        hello: &ServerHello,
        ephemeral: &KeyPair,
        noise_key: &KeyPair,
        root: &[u8; 32],
    ) -> Result<Vec<u8>> {
        let field = |v: &Option<Vec<u8>>, name: &str| {
            v.clone().ok_or_else(|| Error::Handshake(format!("server hello is missing {name}")))
        };
        let server_ephemeral: [u8; 32] = field(&hello.ephemeral, "ephemeral")?
            .try_into()
            .map_err(|_| Error::Handshake("server ephemeral key has the wrong size".into()))?;

        self.authenticate(&server_ephemeral);
        self.mix_key(&ephemeral.shared_secret(&server_ephemeral));

        let server_static: [u8; 32] = self
            .decrypt(&field(&hello.static_key, "static key")?)?
            .try_into()
            .map_err(|_| Error::Handshake("server static key has the wrong size".into()))?;
        self.mix_key(&ephemeral.shared_secret(&server_static));

        let certificate = self.decrypt(&field(&hello.payload, "certificate")?)?;
        verify_certificate(&certificate, &server_static, root)?;

        let encrypted_static = self.encrypt(&noise_key.public)?;
        self.mix_key(&noise_key.shared_secret(&server_ephemeral));
        Ok(encrypted_static)
    }

    pub(crate) fn finish(self) -> (NoiseCipher, NoiseCipher) {
        let derived: [u8; 64] = crypto::hkdf_array(b"", Some(&self.salt), b"");
        (NoiseCipher::new(&derived[..32]), NoiseCipher::new(&derived[32..]))
    }
}

fn verify_certificate(raw: &[u8], server_static: &[u8; 32], root: &[u8; 32]) -> Result<()> {
    let invalid = |why: &str| Error::Handshake(format!("noise certificate rejected: {why}"));
    let chain = CertChain::decode(raw)?;
    let (Some(leaf), Some(intermediate)) = (chain.leaf, chain.intermediate) else {
        return Err(invalid("incomplete chain"));
    };
    let signed = |cert: &crate::proto::NoiseCertificate| -> Result<(Vec<u8>, [u8; 64])> {
        let details = cert.details.clone().ok_or_else(|| invalid("missing details"))?;
        let signature = cert
            .signature
            .clone()
            .and_then(|s| <[u8; 64]>::try_from(s).ok())
            .ok_or_else(|| invalid("missing signature"))?;
        Ok((details, signature))
    };

    let (intermediate_details, intermediate_signature) = signed(&intermediate)?;
    if !crypto::verify(root, &intermediate_details, &intermediate_signature) {
        return Err(invalid("intermediate is not signed by the whatsapp root"));
    }
    let intermediate = CertificateDetails::decode(intermediate_details.as_slice())?;
    if intermediate.issuer_serial.unwrap_or_default() != WA_ROOT_SERIAL {
        return Err(invalid("unexpected issuer serial"));
    }
    let now = crate::message::now_secs().max(0) as u64;
    let in_window = |details: &CertificateDetails| {
        details.not_before.is_none_or(|from| now + CERTIFICATE_SKEW_SECS >= from)
            && details.not_after.is_none_or(|to| now <= to + CERTIFICATE_SKEW_SECS)
    };
    if !in_window(&intermediate) {
        return Err(invalid("the intermediate is outside its validity window"));
    }
    let intermediate_serial = intermediate.serial;
    let intermediate_key: [u8; 32] =
        intermediate.key.and_then(|k| k.try_into().ok()).ok_or_else(|| invalid("intermediate key is malformed"))?;

    let (leaf_details, leaf_signature) = signed(&leaf)?;
    if !crypto::verify(&intermediate_key, &leaf_details, &leaf_signature) {
        return Err(invalid("leaf is not signed by the intermediate"));
    }
    let leaf = CertificateDetails::decode(leaf_details.as_slice())?;
    if !in_window(&leaf) {
        return Err(invalid("the leaf is outside its validity window"));
    }
    if intermediate_serial.is_some() && leaf.issuer_serial != intermediate_serial {
        return Err(invalid("the leaf was not issued by this intermediate"));
    }
    if leaf.key.as_deref() != Some(server_static.as_slice()) {
        return Err(invalid("leaf key does not match the server static key"));
    }
    Ok(())
}

#[cfg(any(test, feature = "testing"))]
pub(crate) struct ServerHandshake {
    inner: Handshake,
    ephemeral: KeyPair,
}

#[cfg(any(test, feature = "testing"))]
impl ServerHandshake {
    pub(crate) fn respond(
        client_ephemeral: &[u8; 32],
        server_static: &KeyPair,
        certificate: &[u8],
    ) -> Result<(Self, ServerHello)> {
        let mut inner = Handshake::new(client_ephemeral);
        let ephemeral = KeyPair::generate();
        inner.authenticate(&ephemeral.public);
        inner.mix_key(&ephemeral.shared_secret(client_ephemeral));
        let static_key = inner.encrypt(&server_static.public)?;
        inner.mix_key(&server_static.shared_secret(client_ephemeral));
        let payload = inner.encrypt(certificate)?;
        let hello = ServerHello {
            ephemeral: Some(ephemeral.public.to_vec()),
            static_key: Some(static_key),
            payload: Some(payload),
        };
        Ok((ServerHandshake { inner, ephemeral }, hello))
    }

    pub(crate) fn complete(mut self, finish: &ClientFinish) -> Result<([u8; 32], Vec<u8>, NoiseCipher, NoiseCipher)> {
        let missing = |what: &str| Error::Handshake(format!("client finish is missing {what}"));
        let client_static: [u8; 32] = self
            .inner
            .decrypt(finish.static_key.as_deref().ok_or_else(|| missing("the static key"))?)?
            .try_into()
            .map_err(|_| Error::Handshake("client static key has the wrong size".into()))?;
        self.inner.mix_key(&self.ephemeral.shared_secret(&client_static));
        let payload = self.inner.decrypt(finish.payload.as_deref().ok_or_else(|| missing("the payload"))?)?;
        let (client_writer, client_reader) = self.inner.finish();
        Ok((client_static, payload, client_reader, client_writer))
    }
}

#[cfg(any(test, feature = "testing"))]
pub(crate) fn issue_certificate_chain(root: &KeyPair, server_static: &[u8; 32]) -> Vec<u8> {
    let intermediate_key = KeyPair::generate();
    let now = crate::message::now_secs().max(0) as u64;
    let (not_before, not_after) = (now.saturating_sub(24 * 60 * 60), now + 10 * 365 * 24 * 60 * 60);
    let certificate = |signer: &KeyPair, details: CertificateDetails| {
        let details = details.encode_to_vec();
        NoiseCertificate { signature: Some(signer.sign(&details).to_vec()), details: Some(details) }
    };
    let intermediate = certificate(
        root,
        CertificateDetails {
            serial: Some(1),
            issuer_serial: Some(WA_ROOT_SERIAL),
            key: Some(intermediate_key.public.to_vec()),
            not_before: Some(not_before),
            not_after: Some(not_after),
        },
    );
    let leaf = certificate(
        &intermediate_key,
        CertificateDetails {
            serial: Some(2),
            issuer_serial: Some(1),
            key: Some(server_static.to_vec()),
            not_before: Some(not_before),
            not_after: Some(not_after),
        },
    );
    CertChain { leaf: Some(leaf), intermediate: Some(intermediate) }.encode_to_vec()
}
pub(crate) struct NoiseCipher {
    cipher: Aes256Gcm,
    counter: u32,
}

impl NoiseCipher {
    fn new(key: &[u8]) -> Self {
        NoiseCipher { cipher: crypto::gcm(key), counter: 0 }
    }

    pub(crate) fn encrypt(&mut self, plaintext: &[u8]) -> Result<Vec<u8>> {
        let iv = crypto::gcm_iv(self.counter);
        self.counter = self.counter.wrapping_add(1);
        self.cipher.encrypt(Nonce::from_slice(&iv), plaintext).map_err(|_| Error::Crypto("transport encryption failed"))
    }

    pub(crate) fn decrypt(&mut self, ciphertext: &[u8]) -> Result<Vec<u8>> {
        let iv = crypto::gcm_iv(self.counter);
        self.counter = self.counter.wrapping_add(1);
        self.cipher
            .decrypt(Nonce::from_slice(&iv), ciphertext)
            .map_err(|_| Error::Crypto("transport frame failed authentication"))
    }
}

pub(crate) fn frame(intro: Option<&[u8]>, payload: &[u8]) -> Result<Vec<u8>> {
    if payload.len() > MAX_FRAME {
        return Err(Error::other("frame is larger than 16 MiB"));
    }
    let intro = intro.unwrap_or_default();
    let mut out = Vec::with_capacity(intro.len() + 3 + payload.len());
    out.extend_from_slice(intro);
    let len = payload.len();
    out.extend_from_slice(&[(len >> 16) as u8, (len >> 8) as u8, len as u8]);
    out.extend_from_slice(payload);
    Ok(out)
}

#[derive(Default)]
pub(crate) struct FrameDecoder {
    buf: Vec<u8>,
    start: usize,
}

impl FrameDecoder {
    pub(crate) fn push(&mut self, data: &[u8]) {
        if self.start > 0 && self.start == self.buf.len() {
            self.buf.clear();
            self.start = 0;
        } else if self.start > 64 * 1024 && self.start * 2 > self.buf.len() {
            self.buf.drain(..self.start);
            self.start = 0;
        }
        self.buf.extend_from_slice(data);
    }

    pub(crate) fn next_frame(&mut self) -> Option<Vec<u8>> {
        let pending = &self.buf[self.start..];
        if pending.len() < 3 {
            return None;
        }
        let len = (usize::from(pending[0]) << 16) | (usize::from(pending[1]) << 8) | usize::from(pending[2]);
        if pending.len() < 3 + len {
            return None;
        }
        let frame = pending[3..3 + len].to_vec();
        self.start += 3 + len;
        Some(frame)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_split_across_reads() {
        let a = frame(None, b"first").unwrap();
        let b = frame(None, &[7u8; 70_000]).unwrap();
        let mut stream = a.clone();
        stream.extend_from_slice(&b);
        let mut decoder = FrameDecoder::default();
        let mut frames = Vec::new();
        for chunk in stream.chunks(1000) {
            decoder.push(chunk);
            while let Some(frame) = decoder.next_frame() {
                frames.push(frame);
            }
        }
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0], b"first");
        assert_eq!(frames[1].len(), 70_000);
    }

    #[test]
    fn intro_with_routing_info() {
        let header = intro_header(Some(&[1, 2, 3]));
        assert_eq!(&header[..4], b"ED\x00\x01");
        assert_eq!(&header[4..7], &[0, 0, 3]);
        assert_eq!(&header[header.len() - 4..], &WA_HEADER);
    }
}
