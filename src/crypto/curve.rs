use curve25519_dalek::edwards::EdwardsPoint;
use curve25519_dalek::montgomery::MontgomeryPoint;
use curve25519_dalek::scalar::Scalar;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha512};

use crate::error::{Error, Result};
use crate::serde_b64;

pub const KEY_TYPE: u8 = 5;

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyPair {
    #[serde(with = "serde_b64::array")]
    pub public: [u8; 32],
    #[serde(with = "serde_b64::array")]
    pub private: [u8; 32],
}

impl std::fmt::Debug for KeyPair {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KeyPair").field("public", &hex::encode(self.public)).finish_non_exhaustive()
    }
}

impl KeyPair {
    pub fn generate() -> Self {
        Self::from_private(super::random_array())
    }

    pub fn from_private(mut private: [u8; 32]) -> Self {
        private[0] &= 248;
        private[31] &= 127;
        private[31] |= 64;
        let public = MontgomeryPoint::mul_base_clamped(private).to_bytes();
        KeyPair { public, private }
    }

    pub fn shared_secret(&self, their_public: &[u8; 32]) -> [u8; 32] {
        MontgomeryPoint(*their_public).mul_clamped(self.private).to_bytes()
    }

    pub fn signal_public(&self) -> [u8; 33] {
        signal_key(&self.public)
    }

    pub fn sign(&self, message: &[u8]) -> [u8; 64] {
        sign(&self.private, message)
    }
}

pub fn edwards_public(private: &[u8; 32]) -> [u8; 32] {
    EdwardsPoint::mul_base(&Scalar::from_bytes_mod_order(*private)).compress().to_bytes()
}

pub fn sign(private: &[u8; 32], message: &[u8]) -> [u8; 64] {
    sign_with(private, &edwards_public(private), message)
}

pub fn sign_with(private: &[u8; 32], edwards: &[u8; 32], message: &[u8]) -> [u8; 64] {
    let nonce: [u8; 64] = super::random_array();
    let a = Scalar::from_bytes_mod_order(*private);
    let sign_bit = edwards[31] & 0x80;

    let mut prefix = [0xffu8; 32];
    prefix[0] = 0xfe;
    let r = Scalar::from_hash(
        Sha512::new().chain_update(prefix).chain_update(private).chain_update(message).chain_update(nonce),
    );
    let cap_r = EdwardsPoint::mul_base(&r).compress();
    let h = Scalar::from_hash(Sha512::new().chain_update(cap_r.as_bytes()).chain_update(edwards).chain_update(message));
    let s = h * a + r;

    let mut signature = [0u8; 64];
    signature[..32].copy_from_slice(cap_r.as_bytes());
    signature[32..].copy_from_slice(s.as_bytes());
    signature[63] &= 0x7f;
    signature[63] |= sign_bit;
    signature
}

pub fn verify(public: &[u8; 32], message: &[u8], signature: &[u8; 64]) -> bool {
    let Some(point) = MontgomeryPoint(*public).to_edwards((signature[63] & 0x80) >> 7) else {
        return false;
    };
    let mut s = [0u8; 32];
    s.copy_from_slice(&signature[32..]);
    s[31] &= 0x7f;
    if s[31] & 0xe0 != 0 {
        return false;
    }
    let cap_a = point.compress();
    let h = Scalar::from_hash(
        Sha512::new().chain_update(&signature[..32]).chain_update(cap_a.as_bytes()).chain_update(message),
    );
    let check = EdwardsPoint::vartime_double_scalar_mul_basepoint(&h, &-point, &Scalar::from_bytes_mod_order(s));
    super::constant_time_eq(check.compress().as_bytes(), &signature[..32])
}

pub fn signal_key(public: &[u8; 32]) -> [u8; 33] {
    let mut out = [0u8; 33];
    out[0] = KEY_TYPE;
    out[1..].copy_from_slice(public);
    out
}

pub fn strip_key_prefix(key: &[u8]) -> Result<[u8; 32]> {
    let raw = match key.len() {
        33 if key[0] == KEY_TYPE => &key[1..],
        32 => key,
        _ => return Err(Error::Crypto("invalid curve25519 public key")),
    };
    let mut out = [0u8; 32];
    out.copy_from_slice(raw);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agreement() {
        let alice = KeyPair::generate();
        let bob = KeyPair::generate();
        assert_eq!(alice.shared_secret(&bob.public), bob.shared_secret(&alice.public));
    }

    #[test]
    fn xeddsa_sign_and_verify() {
        let pair = KeyPair::generate();
        for len in [0usize, 1, 33, 200] {
            let message = vec![0x42u8; len];
            let signature = pair.sign(&message);
            assert!(verify(&pair.public, &message, &signature));
            let mut tampered = signature;
            tampered[5] ^= 1;
            assert!(!verify(&pair.public, &message, &tampered));
        }
    }

    #[test]
    fn strips_signal_prefix() {
        let pair = KeyPair::generate();
        assert_eq!(strip_key_prefix(&pair.signal_public()).unwrap(), pair.public);
        assert_eq!(strip_key_prefix(&pair.public).unwrap(), pair.public);
        assert!(strip_key_prefix(&[1, 2, 3]).is_err());
    }
}
