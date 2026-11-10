use crate::binary::Node;
use crate::crypto;
use crate::proto::Message;

struct Field {
    number: u64,
    message: bool,
    children: &'static [Field],
}

const fn leaf(number: u64) -> Field {
    Field { number, message: false, children: &[] }
}

const fn wrapper(number: u64) -> Field {
    Field { number, message: true, children: &[] }
}

const fn nested(number: u64, children: &'static [Field]) -> Field {
    Field { number, message: false, children }
}

const CONTEXT: &[Field] = &[leaf(21), leaf(22)];
const POLL: &[Field] = &[leaf(2), nested(3, &[leaf(1), leaf(2)]), nested(5, CONTEXT), nested(8, &[leaf(1), leaf(2)])];
const MESSAGE: &[Field] = &[
    leaf(1),
    nested(3, &[leaf(2), leaf(3), leaf(8), leaf(11), nested(17, CONTEXT), leaf(25)]),
    nested(4, &[leaf(1), leaf(16), nested(17, CONTEXT)]),
    nested(5, &[leaf(3), leaf(4), leaf(5), leaf(16), nested(17, CONTEXT)]),
    nested(6, &[leaf(1), nested(17, CONTEXT), leaf(30)]),
    nested(7, &[leaf(2), leaf(7), leaf(10), nested(17, CONTEXT), leaf(20)]),
    nested(8, &[leaf(2), leaf(7), leaf(9), nested(17, CONTEXT), leaf(21)]),
    nested(9, &[leaf(2), leaf(6), leaf(7), leaf(13), nested(17, CONTEXT), leaf(20)]),
    nested(12, &[leaf(1), leaf(2), wrapper(14), leaf(15)]),
    nested(18, &[leaf(6), leaf(16), nested(17, CONTEXT)]),
    nested(26, &[leaf(4), leaf(5), leaf(8), leaf(13), nested(17, CONTEXT)]),
    nested(28, &[leaf(1), leaf(2), leaf(4), leaf(5), leaf(6), nested(7, CONTEXT)]),
    nested(37, &[wrapper(1)]),
    nested(49, POLL),
    nested(53, &[wrapper(1)]),
    nested(55, &[wrapper(1)]),
    nested(58, &[wrapper(1)]),
    nested(59, &[wrapper(1)]),
    nested(60, POLL),
    nested(64, POLL),
    nested(66, &[leaf(2), leaf(6), leaf(7), leaf(13), nested(17, CONTEXT), leaf(20)]),
    nested(74, &[wrapper(1)]),
    nested(87, &[wrapper(1)]),
    nested(88, &[leaf(1), nested(2, &[leaf(1)]), nested(3, CONTEXT)]),
    nested(92, &[wrapper(1)]),
    nested(93, &[wrapper(1)]),
    nested(94, &[wrapper(1)]),
];

fn varint(data: &[u8], at: usize) -> Option<(u64, usize)> {
    let mut value = 0u64;
    for (index, byte) in data.get(at..)?.iter().take(10).enumerate() {
        value |= u64::from(byte & 0x7f) << (7 * index);
        if byte & 0x80 == 0 {
            return Some((value, index + 1));
        }
    }
    None
}

fn push_varint(out: &mut Vec<u8>, mut value: u64) {
    while value > 0x7f {
        out.push((value as u8 & 0x7f) | 0x80);
        value >>= 7;
    }
    out.push(value as u8);
}

fn extract(data: &[u8], fields: &[Field]) -> Option<Vec<u8>> {
    let mut kept: Vec<(u64, Vec<u8>)> = Vec::new();
    let mut at = 0;
    while at < data.len() {
        let start = at;
        let (tag, tag_len) = varint(data, at)?;
        at += tag_len;
        let field = fields.iter().find(|f| f.number == tag >> 3);
        let end = match tag & 7 {
            0 => at + varint(data, at)?.1,
            1 => at + 8,
            5 => at + 4,
            2 => {
                let (len, len_len) = varint(data, at)?;
                let value_start = at + len_len;
                let value_end = value_start.checked_add(usize::try_from(len).ok()?)?;
                if value_end > data.len() {
                    return None;
                }
                if let Some(field) = field.filter(|f| f.message || !f.children.is_empty()) {
                    let inner = extract(&data[value_start..value_end], field.children)?;
                    if !inner.is_empty() {
                        let mut bytes = Vec::with_capacity(inner.len() + 6);
                        push_varint(&mut bytes, tag);
                        push_varint(&mut bytes, inner.len() as u64);
                        bytes.extend_from_slice(&inner);
                        kept.push((field.number, bytes));
                    }
                    at = value_end;
                    continue;
                }
                value_end
            }
            _ => return None,
        };
        if end > data.len() {
            return None;
        }
        if let Some(field) = field {
            kept.push((field.number, data[start..end].to_vec()));
        }
        at = end;
    }
    kept.sort_by_key(|(number, _)| *number);
    Some(kept.into_iter().flat_map(|(_, bytes)| bytes).collect())
}

pub(crate) fn wants_reporting_token(message: &Message) -> bool {
    message.reaction_message.is_none()
        && message.poll_update_message.is_none()
        && message.enc_event_response_message.is_none()
}

pub(crate) fn reporting_node(encoded: &[u8], secret: &[u8], id: &str, chat: &str) -> Option<Node> {
    let content = extract(encoded, MESSAGE).filter(|c| !c.is_empty())?;
    let info = [id.as_bytes(), chat.as_bytes(), chat.as_bytes(), b"Report Token"].concat();
    let key: [u8; 32] = crypto::hkdf_array(secret, None, &info);
    let token = crypto::hmac_sha256(&key, &[&content]);
    Some(Node::new("reporting").child_node(Node::new("reporting_token").attr("v", "2").bytes(token[..16].to_vec())))
}

#[cfg(test)]
mod tests {
    use prost::Message as _;

    use super::*;
    use crate::proto::MessageContextInfo;

    #[test]
    fn reportable_fields() {
        let mut poll = Message::poll("¿Rust?", ["Sí", "Claro"], 1);
        poll.message_context_info = Some(Box::new(MessageContextInfo {
            device_list_metadata_version: None,
            message_secret: Some(vec![7; 32]),
            message_add_on_duration_in_secs: None,
            message_association: None,
        }));
        let encoded = poll.encode_to_vec();
        let content = extract(&encoded, MESSAGE).unwrap();
        assert!(!content.is_empty());
        assert!(content.len() < encoded.len());
        assert!(content.windows(32).all(|window| window != [7u8; 32]));
        assert!(content.windows("Claro".len()).any(|window| window == "Claro".as_bytes()));
        assert!(reporting_node(&encoded, &[7; 32], "3EB0ID", "1@g.us").is_some());
    }

    #[test]
    fn long_varints() {
        let edit = Message::edit(
            crate::proto::MessageKey { id: Some("X".into()), ..Default::default() },
            Message::text("nuevo"),
        );
        assert!(extract(&edit.encode_to_vec(), MESSAGE).is_some());
    }
}
