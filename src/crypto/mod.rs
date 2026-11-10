mod curve;

use aes::cipher::{BlockDecryptMut, BlockEncryptMut, KeyIvInit, StreamCipher};
use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Nonce};
use hmac::{Hmac, Mac};
use rand::RngCore;
use sha2::{Digest, Sha256};

pub use curve::{edwards_public, sign, sign_with, signal_key, strip_key_prefix, verify, KeyPair, KEY_TYPE};

use crate::error::{Error, Result};

type HmacSha256 = Hmac<Sha256>;
type Aes256CbcEnc = cbc::Encryptor<aes::Aes256>;
type Aes256CbcDec = cbc::Decryptor<aes::Aes256>;
type Aes256Ctr = ctr::Ctr128BE<aes::Aes256>;

pub fn sha256(data: &[u8]) -> [u8; 32] {
    Sha256::digest(data).into()
}

pub fn sha256_concat(parts: &[&[u8]]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    for part in parts {
        hasher.update(part);
    }
    hasher.finalize().into()
}

pub fn hmac_sha256(key: &[u8], parts: &[&[u8]]) -> [u8; 32] {
    let mut mac = <HmacSha256 as Mac>::new_from_slice(key).expect("hmac accepts any key length");
    for part in parts {
        mac.update(part);
    }
    mac.finalize().into_bytes().into()
}

pub fn hmac_sha512(key: &[u8], parts: &[&[u8]]) -> [u8; 64] {
    let mut mac = <Hmac<sha2::Sha512> as Mac>::new_from_slice(key).expect("hmac accepts any key length");
    for part in parts {
        mac.update(part);
    }
    mac.finalize().into_bytes().into()
}

pub fn hkdf(ikm: &[u8], salt: Option<&[u8]>, info: &[u8], len: usize) -> Vec<u8> {
    let mut out = vec![0u8; len];
    hkdf::Hkdf::<Sha256>::new(salt, ikm).expand(info, &mut out).expect("hkdf output length is within limits");
    out
}

pub fn hkdf_array<const N: usize>(ikm: &[u8], salt: Option<&[u8]>, info: &[u8]) -> [u8; N] {
    let mut out = [0u8; N];
    hkdf::Hkdf::<Sha256>::new(salt, ikm).expand(info, &mut out).expect("hkdf output length is within limits");
    out
}

pub fn gcm(key: &[u8]) -> Aes256Gcm {
    Aes256Gcm::new_from_slice(key).expect("aes-256-gcm key is 32 bytes")
}

pub fn gcm_iv(counter: u32) -> [u8; 12] {
    let mut iv = [0u8; 12];
    iv[8..].copy_from_slice(&counter.to_be_bytes());
    iv
}

pub fn aes_gcm_encrypt(key: &[u8], iv: &[u8], aad: &[u8], plaintext: &[u8]) -> Result<Vec<u8>> {
    if key.len() != 32 || iv.len() != 12 {
        return Err(Error::Crypto("aes-gcm needs a 32 byte key and a 12 byte iv"));
    }
    gcm(key)
        .encrypt(Nonce::from_slice(iv), Payload { msg: plaintext, aad })
        .map_err(|_| Error::Crypto("aes-gcm encryption failed"))
}

pub fn aes_gcm_decrypt(key: &[u8], iv: &[u8], aad: &[u8], ciphertext: &[u8]) -> Result<Vec<u8>> {
    if key.len() != 32 || iv.len() != 12 {
        return Err(Error::Crypto("aes-gcm needs a 32 byte key and a 12 byte iv"));
    }
    gcm(key)
        .decrypt(Nonce::from_slice(iv), Payload { msg: ciphertext, aad })
        .map_err(|_| Error::Crypto("aes-gcm authentication failed"))
}

pub fn aes_cbc_encrypt(key: &[u8], iv: &[u8], plaintext: &[u8]) -> Result<Vec<u8>> {
    let cipher = Aes256CbcEnc::new_from_slices(key, iv).map_err(|_| Error::Crypto("invalid aes-cbc key or iv"))?;
    Ok(cipher.encrypt_padded_vec_mut::<cbc::cipher::block_padding::Pkcs7>(plaintext))
}

pub fn aes_cbc_decrypt(key: &[u8], iv: &[u8], ciphertext: &[u8]) -> Result<Vec<u8>> {
    let cipher = Aes256CbcDec::new_from_slices(key, iv).map_err(|_| Error::Crypto("invalid aes-cbc key or iv"))?;
    cipher
        .decrypt_padded_vec_mut::<cbc::cipher::block_padding::Pkcs7>(ciphertext)
        .map_err(|_| Error::Crypto("aes-cbc padding is invalid"))
}

pub fn aes_ctr(key: &[u8], iv: &[u8], data: &[u8]) -> Result<Vec<u8>> {
    let mut cipher = Aes256Ctr::new_from_slices(key, iv).map_err(|_| Error::Crypto("invalid aes-ctr key or iv"))?;
    let mut out = data.to_vec();
    cipher.apply_keystream(&mut out);
    Ok(out)
}

pub fn pbkdf2_sha256(password: &[u8], salt: &[u8], rounds: u32) -> [u8; 32] {
    let mut out = [0u8; 32];
    pbkdf2::pbkdf2_hmac::<Sha256>(password, salt, rounds, &mut out);
    out
}

pub fn md5(data: &[u8]) -> [u8; 16] {
    md5::Md5::digest(data).into()
}

pub fn random_array<const N: usize>() -> [u8; N] {
    let mut out = [0u8; N];
    rand::thread_rng().fill_bytes(&mut out);
    out
}

pub fn random_vec(len: usize) -> Vec<u8> {
    let mut out = vec![0u8; len];
    rand::thread_rng().fill_bytes(&mut out);
    out
}

pub fn random_u32() -> u32 {
    rand::thread_rng().next_u32()
}

pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cbc_roundtrip() {
        let key = random_array::<32>();
        let iv = random_array::<16>();
        let ct = aes_cbc_encrypt(&key, &iv, b"soblend development studio").unwrap();
        assert_eq!(aes_cbc_decrypt(&key, &iv, &ct).unwrap(), b"soblend development studio");
    }

    #[test]
    fn gcm_roundtrip() {
        let key = random_array::<32>();
        let iv = gcm_iv(7);
        let ct = aes_gcm_encrypt(&key, &iv, b"aad", b"payload").unwrap();
        assert_eq!(aes_gcm_decrypt(&key, &iv, b"aad", &ct).unwrap(), b"payload");
        assert!(aes_gcm_decrypt(&key, &iv, b"other", &ct).is_err());
    }

    #[test]
    fn hkdf_matches_rfc5869_case_1() {
        let ikm = [0x0bu8; 22];
        let salt = hex::decode("000102030405060708090a0b0c").unwrap();
        let info = hex::decode("f0f1f2f3f4f5f6f7f8f9").unwrap();
        let okm = hkdf(&ikm, Some(&salt), &info, 42);
        assert_eq!(
            hex::encode(okm),
            "3cb25f25faacd57a90434f64d0362f2a2d2d0a90cf1a5a4c5db02d56ecc4c5bf34007208d5b887185865"
        );
    }
}
