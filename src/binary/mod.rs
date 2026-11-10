mod decoder;
mod encoder;
mod node;
#[cfg(test)]
mod robustness;
mod tokens;

use std::collections::HashMap;
use std::sync::LazyLock;

pub use decoder::{decode, decode_frame};
pub use encoder::encode;
pub use node::{Content, Node, Str, Value};

pub(crate) mod tag {
    pub const LIST_EMPTY: u8 = 0;
    pub const DICTIONARY_0: u8 = 236;
    pub const DICTIONARY_3: u8 = 239;
    pub const INTEROP_JID: u8 = 245;
    pub const FB_JID: u8 = 246;
    pub const AD_JID: u8 = 247;
    pub const LIST_8: u8 = 248;
    pub const LIST_16: u8 = 249;
    pub const JID_PAIR: u8 = 250;
    pub const HEX_8: u8 = 251;
    pub const BINARY_8: u8 = 252;
    pub const BINARY_20: u8 = 253;
    pub const BINARY_32: u8 = 254;
    pub const NIBBLE_8: u8 = 255;
    pub const PACKED_MAX: usize = 127;
}

#[derive(Clone, Copy)]
pub(crate) enum Token {
    Single(u8),
    Double(u8, u8),
}

static TOKEN_INDEX: LazyLock<HashMap<&'static str, Token>> = LazyLock::new(|| {
    let mut index = HashMap::with_capacity(tokens::SINGLE_BYTE.len() + 4 * 256);
    for (dict, table) in tokens::DOUBLE_BYTE.iter().enumerate() {
        for (i, token) in table.iter().enumerate() {
            index.insert(*token, Token::Double(dict as u8, i as u8));
        }
    }
    for (i, token) in tokens::SINGLE_BYTE.iter().enumerate().skip(1) {
        index.insert(*token, Token::Single(i as u8));
    }
    index
});

pub(crate) fn lookup_token(value: &str) -> Option<Token> {
    TOKEN_INDEX.get(value).copied()
}

pub(crate) fn single_token(index: u8) -> Option<&'static str> {
    tokens::SINGLE_BYTE.get(index as usize).copied()
}

pub(crate) fn double_token(dict: u8, index: u8) -> Option<&'static str> {
    tokens::DOUBLE_BYTE.get(dict as usize).and_then(|t| t.get(index as usize)).copied()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jid::Jid;

    #[test]
    fn roundtrip_all() {
        let node = Node::new("message")
            .attr("id", "3EB0A1B2C3D4E5F6A7B8C9")
            .attr("to", Jid::phone("5215512345678").with_device(7))
            .attr("type", "text")
            .attr("custom_attribute_value", "hello world, this is raw text")
            .attr("t", 1_700_000_000u64)
            .children([
                Node::new("enc").attr("v", "2").attr("type", "pkmsg").bytes(vec![1u8; 300]),
                Node::new("participants").children([Node::new("to").attr("jid", Jid::group("120363025246125486"))]),
                Node::new("device-identity").bytes(vec![9u8; 5]),
            ]);
        let encoded = encode(&node);
        let decoded = decode_frame(&encoded).unwrap();
        assert_eq!(decoded.tag, "message");
        assert_eq!(decoded.attr_str("id"), Some("3EB0A1B2C3D4E5F6A7B8C9"));
        assert_eq!(decoded.attr_jid("to"), Some(Jid::phone("5215512345678").with_device(7)));
        assert_eq!(decoded.attr_str("custom_attribute_value"), Some("hello world, this is raw text"));
        assert_eq!(decoded.attr_u64("t"), Some(1_700_000_000));
        let enc = decoded.child("enc").unwrap();
        assert_eq!(enc.content_bytes().unwrap(), &[1u8; 300][..]);
        let to = decoded.path(&["participants", "to"]).unwrap();
        assert_eq!(to.attr_jid("jid").unwrap().to_string(), "120363025246125486@g.us");
    }

    #[test]
    fn jid_like_text_untouched() {
        for text in ["ventas_mx@empresa.com", "soporte@soblend.dev", "a@b@c", "1:300@s.whatsapp.net", "x@c.us"] {
            let node = Node::new("email").text(text.to_owned()).attr("note", text.to_owned());
            let decoded = decode_frame(&encode(&node)).unwrap();
            assert_eq!(decoded.content_string().as_deref(), Some(text));
            assert_eq!(decoded.attr_string("note").as_deref(), Some(text));
        }
        let jid = decode_frame(&encode(&Node::new("x").attr("jid", "5215512345678:3@s.whatsapp.net"))).unwrap();
        assert_eq!(jid.attr_jid("jid"), Some(Jid::phone("5215512345678").with_device(3)));
    }

    #[test]
    fn tokens_bijective() {
        for (word, token) in TOKEN_INDEX.iter() {
            let back = match *token {
                Token::Single(i) => single_token(i),
                Token::Double(d, i) => double_token(d, i),
            };
            assert_eq!(back, Some(*word));
        }
    }
}
