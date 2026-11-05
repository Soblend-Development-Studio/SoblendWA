use std::borrow::Cow;
use std::io::Read;

use flate2::read::ZlibDecoder;

use super::{double_token, single_token, tag, Content, Node, Str, Value};
use crate::error::{Error, Result};
use crate::jid::Jid;

const MAX_DEPTH: usize = 64;
const MAX_INFLATED: u64 = 32 * 1024 * 1024;

pub fn decode_frame(data: &[u8]) -> Result<Node> {
    let (flags, body) = data.split_first().ok_or_else(|| Error::binary("empty frame"))?;
    if flags & 2 != 0 {
        let mut inflated = Vec::with_capacity(body.len() * 4);
        ZlibDecoder::new(body).take(MAX_INFLATED + 1).read_to_end(&mut inflated)?;
        if inflated.len() as u64 > MAX_INFLATED {
            return Err(Error::binary("compressed frame expands beyond the limit"));
        }
        decode(&inflated)
    } else {
        decode(body)
    }
}

pub fn decode(data: &[u8]) -> Result<Node> {
    Reader { data, pos: 0 }.node(0)
}

struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn byte(&mut self) -> Result<u8> {
        let byte = *self.data.get(self.pos).ok_or_else(|| Error::binary("unexpected end of data"))?;
        self.pos += 1;
        Ok(byte)
    }

    fn take(&mut self, len: usize) -> Result<&'a [u8]> {
        let end = self.pos.checked_add(len).filter(|end| *end <= self.data.len());
        let end = end.ok_or_else(|| Error::binary("length exceeds available data"))?;
        let slice = &self.data[self.pos..end];
        self.pos = end;
        Ok(slice)
    }

    fn int(&mut self, len: usize) -> Result<usize> {
        Ok(self.take(len)?.iter().fold(0usize, |acc, b| (acc << 8) | usize::from(*b)))
    }

    fn int20(&mut self) -> Result<usize> {
        let b = self.take(3)?;
        Ok((usize::from(b[0] & 0x0f) << 16) | (usize::from(b[1]) << 8) | usize::from(b[2]))
    }

    fn remaining(&self) -> usize {
        self.data.len() - self.pos
    }

    fn list_size(&mut self, kind: u8) -> Result<usize> {
        match kind {
            tag::LIST_EMPTY => Ok(0),
            tag::LIST_8 => Ok(usize::from(self.byte()?)),
            tag::LIST_16 => self.int(2),
            other => Err(Error::binary(format!("invalid list tag {other}"))),
        }
    }

    fn node(&mut self, depth: usize) -> Result<Node> {
        if depth > MAX_DEPTH {
            return Err(Error::binary("nodes are nested too deeply"));
        }
        let kind = self.byte()?;
        let size = self.list_size(kind)?;
        if size == 0 {
            return Err(Error::binary("empty node"));
        }
        let first = self.byte()?;
        let tag = self.string(first)?;
        if tag.is_empty() {
            return Err(Error::binary("node without tag"));
        }

        let attr_count = (size - 1) >> 1;
        let mut attrs = Vec::with_capacity(attr_count.min(self.remaining() / 2));
        for _ in 0..attr_count {
            let key_tag = self.byte()?;
            let key = self.string(key_tag)?;
            let value_tag = self.byte()?;
            attrs.push((key, self.value(value_tag)?));
        }

        let content = if size % 2 == 0 { self.content(depth)? } else { Content::None };
        Ok(Node { tag, attrs, content })
    }

    fn content(&mut self, depth: usize) -> Result<Content> {
        let kind = self.byte()?;
        Ok(match kind {
            tag::LIST_EMPTY | tag::LIST_8 | tag::LIST_16 => {
                let len = self.list_size(kind)?;
                let mut nodes = Vec::with_capacity(len.min(self.remaining() / 2));
                for _ in 0..len {
                    nodes.push(self.node(depth + 1)?);
                }
                Content::Nodes(nodes)
            }
            tag::BINARY_8 | tag::BINARY_20 | tag::BINARY_32 => {
                let len = self.binary_len(kind)?;
                Content::Bytes(self.take(len)?.to_vec())
            }
            other => match self.value(other)? {
                Value::Str(text) => Content::Text(text),
                Value::Jid(jid) => Content::Text(Cow::Owned(jid.to_string())),
            },
        })
    }

    fn binary_len(&mut self, kind: u8) -> Result<usize> {
        match kind {
            tag::BINARY_8 => Ok(usize::from(self.byte()?)),
            tag::BINARY_20 => self.int20(),
            _ => self.int(4),
        }
    }

    fn value(&mut self, kind: u8) -> Result<Value> {
        match kind {
            tag::JID_PAIR => {
                let user = self.jid_part()?;
                let server = self.jid_part()?;
                if server.is_empty() {
                    return Err(Error::binary("jid pair without server"));
                }
                Ok(Value::Jid(Jid::new(user.into_owned(), server.into_owned())))
            }
            tag::AD_JID => {
                let agent = self.byte()?;
                let device = u16::from(self.byte()?);
                let user = self.jid_part()?;
                Ok(Value::Jid(Jid::from_agent(user.into_owned(), agent, device)))
            }
            tag::FB_JID => {
                let user = self.jid_part()?.into_owned();
                let device = self.int(2)? as u16;
                let server = self.jid_part()?.into_owned();
                Ok(Value::Jid(Jid { user, device, server }))
            }
            tag::INTEROP_JID => {
                let user = self.jid_part()?;
                let device = self.int(2)? as u16;
                let integrator = self.int(2)?;
                let checkpoint = self.pos;
                let server = match self.jid_part() {
                    Ok(server) => server.into_owned(),
                    Err(_) => {
                        self.pos = checkpoint;
                        "interop".to_owned()
                    }
                };
                Ok(Value::Jid(Jid { user: format!("{integrator}-{user}"), device, server }))
            }
            other => self.string(other).map(Value::Str),
        }
    }

    fn jid_part(&mut self) -> Result<Str> {
        match self.byte()? {
            tag::JID_PAIR | tag::AD_JID | tag::FB_JID | tag::INTEROP_JID => {
                Err(Error::binary("jid nested inside a jid"))
            }
            kind => self.string(kind),
        }
    }

    fn string(&mut self, kind: u8) -> Result<Str> {
        match kind {
            tag::LIST_EMPTY => Ok(Cow::Borrowed("")),
            tag::DICTIONARY_0..=tag::DICTIONARY_3 => {
                let index = self.byte()?;
                double_token(kind - tag::DICTIONARY_0, index)
                    .map(Cow::Borrowed)
                    .ok_or_else(|| Error::binary(format!("unknown double token {kind}:{index}")))
            }
            tag::BINARY_8 | tag::BINARY_20 | tag::BINARY_32 => {
                let len = self.binary_len(kind)?;
                Ok(Cow::Owned(String::from_utf8_lossy(self.take(len)?).into_owned()))
            }
            tag::NIBBLE_8 | tag::HEX_8 => self.packed(kind),
            tag::JID_PAIR | tag::AD_JID | tag::FB_JID | tag::INTEROP_JID => {
                Ok(Cow::Owned(self.value(kind)?.to_string()))
            }
            index => {
                single_token(index).map(Cow::Borrowed).ok_or_else(|| Error::binary(format!("unknown token {index}")))
            }
        }
    }

    fn packed(&mut self, kind: u8) -> Result<Str> {
        let header = self.byte()?;
        let len = usize::from(header & 0x7f);
        let mut out = String::with_capacity(len * 2);
        for &byte in self.take(len)? {
            out.push(unpack(kind, byte >> 4)?);
            out.push(unpack(kind, byte & 0x0f)?);
        }
        if header & 0x80 != 0 {
            out.pop();
        }
        Ok(Cow::Owned(out))
    }
}

fn unpack(kind: u8, value: u8) -> Result<char> {
    let c = match (kind, value) {
        (_, 0..=9) => b'0' + value,
        (tag::NIBBLE_8, 10) => b'-',
        (tag::NIBBLE_8, 11) => b'.',
        (tag::NIBBLE_8, 15) => 0,
        (tag::HEX_8, 10..=15) => b'A' + value - 10,
        _ => return Err(Error::binary(format!("invalid packed value {value}"))),
    };
    Ok(char::from(c))
}
