use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serializer};

pub mod array {
    use super::*;

    pub fn serialize<S: Serializer, const N: usize>(value: &[u8; N], serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&STANDARD.encode(value))
    }

    pub fn deserialize<'de, D: Deserializer<'de>, const N: usize>(deserializer: D) -> Result<[u8; N], D::Error> {
        let raw = String::deserialize(deserializer)?;
        let bytes = STANDARD.decode(raw).map_err(D::Error::custom)?;
        bytes.try_into().map_err(|_| D::Error::custom(format!("expected {N} bytes")))
    }
}

pub mod option {
    use super::*;

    pub fn serialize<S: Serializer>(value: &Option<Vec<u8>>, serializer: S) -> Result<S::Ok, S::Error> {
        match value {
            Some(bytes) => serializer.serialize_some(&STANDARD.encode(bytes)),
            None => serializer.serialize_none(),
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<Vec<u8>>, D::Error> {
        Option::<String>::deserialize(deserializer)?
            .map(|raw| STANDARD.decode(raw).map_err(D::Error::custom))
            .transpose()
    }
}
