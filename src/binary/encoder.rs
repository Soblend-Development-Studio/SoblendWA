use super::{lookup_token, tag, Content, Node, Token, Value};
use crate::jid::{Jid, HOSTED_LID_SERVER, HOSTED_SERVER, LID_SERVER, USER_SERVER};

pub fn encode(node: &Node) -> Vec<u8> {
    let mut writer = Writer { buf: Vec::with_capacity(estimate(node) + 1) };
    writer.buf.push(0);
    writer.node(node);
    writer.buf
}

fn estimate(node: &Node) -> usize {
    let attrs: usize = node.attrs.iter().map(|(k, v)| k.len() + 2 + value_len(v)).sum();
    let content = match &node.content {
        Content::None => 0,
        Content::Bytes(data) => data.len() + 5,
        Content::Text(text) => text.len() + 5,
        Content::Nodes(nodes) => nodes.iter().map(estimate).sum::<usize>() + 3,
    };
    node.tag.len() + 4 + attrs + content
}

fn value_len(value: &Value) -> usize {
    match value {
        Value::Str(s) => s.len(),
        Value::Jid(jid) => jid.user.len() + 4,
    }
}

struct Writer {
    buf: Vec<u8>,
}

impl Writer {
    fn node(&mut self, node: &Node) {
        let has_content = !matches!(node.content, Content::None);
        self.list_start(2 * node.attrs.len() + 1 + usize::from(has_content));
        self.string(&node.tag);
        for (key, value) in &node.attrs {
            self.string(key);
            match value {
                Value::Str(s) => self.string(s),
                Value::Jid(jid) => self.jid(jid),
            }
        }
        match &node.content {
            Content::None => {}
            Content::Bytes(data) => self.raw(data),
            Content::Text(text) => self.string(text),
            Content::Nodes(children) => {
                self.list_start(children.len());
                for child in children {
                    self.node(child);
                }
            }
        }
    }

    fn list_start(&mut self, size: usize) {
        match size {
            0 => self.buf.push(tag::LIST_EMPTY),
            1..=255 => self.buf.extend_from_slice(&[tag::LIST_8, size as u8]),
            _ => self.buf.extend_from_slice(&[tag::LIST_16, (size >> 8) as u8, size as u8]),
        }
    }

    fn string(&mut self, value: &str) {
        if value.is_empty() {
            self.buf.push(tag::LIST_EMPTY);
            return;
        }
        match lookup_token(value) {
            Some(Token::Single(index)) => self.buf.push(index),
            Some(Token::Double(dict, index)) => self.buf.extend_from_slice(&[tag::DICTIONARY_0 + dict, index]),
            None if is_packable(value, nibble) => self.packed(value, tag::NIBBLE_8),
            None if is_packable(value, hex) => self.packed(value, tag::HEX_8),
            None => match exact_jid(value) {
                Some(jid) => self.jid(&jid),
                None => self.raw(value.as_bytes()),
            },
        }
    }

    fn raw(&mut self, data: &[u8]) {
        let len = data.len();
        if len < 256 {
            self.buf.extend_from_slice(&[tag::BINARY_8, len as u8]);
        } else if len < 1 << 20 {
            self.buf.extend_from_slice(&[tag::BINARY_20, ((len >> 16) & 0x0f) as u8, (len >> 8) as u8, len as u8]);
        } else {
            self.buf.push(tag::BINARY_32);
            self.buf.extend_from_slice(&(len as u32).to_be_bytes());
        }
        self.buf.extend_from_slice(data);
    }

    fn packed(&mut self, value: &str, kind: u8) {
        let bytes = value.as_bytes();
        let mut header = bytes.len().div_ceil(2) as u8;
        if bytes.len() % 2 == 1 {
            header |= 0x80;
        }
        self.buf.extend_from_slice(&[kind, header]);
        let pack = if kind == tag::NIBBLE_8 { nibble } else { hex };
        for pair in bytes.chunks(2) {
            let high = pack(pair[0]).unwrap_or(15);
            let low = pair.get(1).and_then(|c| pack(*c)).unwrap_or(15);
            self.buf.push((high << 4) | low);
        }
    }

    fn jid(&mut self, jid: &Jid) {
        let server = jid.server.as_str();
        let addressed_device = matches!(server, USER_SERVER | LID_SERVER) && jid.device > 0;
        if addressed_device || matches!(server, HOSTED_SERVER | HOSTED_LID_SERVER) {
            self.buf.extend_from_slice(&[tag::AD_JID, jid.agent(), jid.device as u8]);
            self.string(&jid.user);
        } else {
            self.buf.push(tag::JID_PAIR);
            if jid.user.is_empty() {
                self.buf.push(tag::LIST_EMPTY);
            } else {
                self.string(&jid.user);
            }
            self.string(server);
        }
    }
}

fn exact_jid(value: &str) -> Option<Jid> {
    if !value.contains('@') {
        return None;
    }
    let jid = value.parse::<Jid>().ok()?;
    let lossless = jid.device <= u16::from(u8::MAX) && !jid.server.contains('@') && jid.to_string() == value;
    lossless.then_some(jid)
}

fn nibble(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'-' => Some(10),
        b'.' => Some(11),
        _ => None,
    }
}

fn hex(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

fn is_packable(value: &str, pack: fn(u8) -> Option<u8>) -> bool {
    value.len() <= tag::PACKED_MAX && value.bytes().all(|c| pack(c).is_some())
}
