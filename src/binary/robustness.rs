use std::io::Write;

use flate2::write::ZlibEncoder;
use flate2::Compression;

use super::{decode, decode_frame, encode, tag, tokens, Node};
use crate::jid::Jid;

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    fn below(&mut self, bound: usize) -> usize {
        (self.next() % bound.max(1) as u64) as usize
    }

    fn chance(&mut self, percent: usize) -> bool {
        self.below(100) < percent
    }

    fn bytes(&mut self, len: usize) -> Vec<u8> {
        (0..len).map(|_| self.next() as u8).collect()
    }

    fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.below(items.len())]
    }
}

fn digits(rng: &mut Rng, len: usize) -> String {
    (0..len).map(|_| char::from(b'0' + rng.below(10) as u8)).collect()
}

fn text(rng: &mut Rng) -> String {
    const ALPHABET: &[char] = &['a', 'z', 'Q', '7', ' ', '-', '.', '_', ':', 'ñ', 'é', '🦀', '@', 'F', '0'];
    match rng.below(5) {
        0 => rng.pick(&tokens::SINGLE_BYTE[1..]).to_string(),
        1 => tokens::DOUBLE_BYTE[rng.below(4)][rng.below(256)].to_owned(),
        2 => {
            let len = rng.below(40);
            digits(rng, len)
        }
        3 => (0..rng.below(30)).map(|_| *rng.pick(&['0', '9', 'A', 'F', 'C'])).collect(),
        _ => (0..rng.below(60)).map(|_| *rng.pick(ALPHABET)).collect(),
    }
}

fn jid(rng: &mut Rng) -> Jid {
    match rng.below(5) {
        0 => Jid::phone(&digits(rng, 12)).with_device(rng.below(100) as u16),
        1 => Jid::group(digits(rng, 18)),
        2 => Jid::lid(digits(rng, 15)).with_device(rng.below(3) as u16),
        3 => Jid::server(),
        _ => Jid::new(digits(rng, 18), "newsletter"),
    }
}

fn node(rng: &mut Rng, depth: usize) -> Node {
    let mut tag = text(rng);
    if tag.is_empty() {
        tag.push('x');
    }
    let mut out = Node::new(tag);
    for _ in 0..rng.below(5) {
        let key = match text(rng) {
            key if key.is_empty() => "k".to_owned(),
            key => key,
        };
        out = match rng.below(3) {
            0 => out.attr(key, jid(rng)),
            1 => out.attr(key, rng.next() % 10_000_000_000),
            _ => out.attr(key, text(rng)),
        };
    }
    match rng.below(4) {
        0 if depth < 5 => {
            let count = rng.below(4);
            out.children((0..count).map(|_| node(rng, depth + 1)).collect::<Vec<_>>())
        }
        1 => {
            let len = if rng.chance(3) { 256 + rng.below(70_000) } else { rng.below(300) };
            out.bytes(rng.bytes(len))
        }
        2 => out.text(text(rng)),
        _ => out,
    }
}

fn compressed(raw: &[u8]) -> Vec<u8> {
    let mut encoder = ZlibEncoder::new(vec![2u8], Compression::fast());
    encoder.write_all(raw).unwrap();
    encoder.finish().unwrap()
}

#[test]
fn random_nodes_survive_a_round_trip() {
    let mut rng = Rng(0x5eb1_e4d0_c0de_f00d);
    for round in 0..3_000 {
        let original = node(&mut rng, 0);
        let encoded = encode(&original);
        let decoded = decode_frame(&encoded).unwrap_or_else(|err| panic!("round {round}: {err}\n{original}"));
        assert_eq!(encode(&decoded), encoded, "round {round} is not a fixed point\n{original}");
        let squeezed = compressed(&encoded[1..]);
        assert_eq!(encode(&decode_frame(&squeezed).unwrap()), encoded);
    }
}

#[test]
fn random_garbage_never_panics() {
    let mut rng = Rng(0xdead_beef_cafe_babe);
    for _ in 0..200_000 {
        let len = rng.below(96);
        let mut data = rng.bytes(len);
        if let Some(first) = data.first_mut() {
            *first &= 0x01;
        }
        let _ = decode_frame(&data);
        let _ = decode(&data);
    }
}

#[test]
fn mutated_frames_never_panic() {
    let mut rng = Rng(0x0123_4567_89ab_cdef);
    let seeds: Vec<Vec<u8>> = (0..64).map(|_| encode(&node(&mut rng, 0))).collect();
    for _ in 0..60_000 {
        let mut data = rng.pick(&seeds).clone();
        for _ in 0..1 + rng.below(4) {
            if data.len() < 2 {
                break;
            }
            let at = 1 + rng.below(data.len() - 1);
            match rng.below(4) {
                0 => data[at] = rng.next() as u8,
                1 => data.truncate(at),
                2 => data.insert(at, *rng.pick(&[tag::LIST_16, tag::BINARY_32, tag::JID_PAIR, tag::NIBBLE_8, 0xff])),
                _ => {
                    data.remove(at);
                }
            }
        }
        let _ = decode_frame(&data);
    }
}

#[test]
fn hostile_nesting_is_rejected_without_blowing_the_stack() {
    let mut nested_nodes = vec![0u8];
    for _ in 0..200_000 {
        nested_nodes.extend_from_slice(&[tag::LIST_8, 2, 3, tag::LIST_8, 1]);
    }
    assert!(decode_frame(&nested_nodes).is_err());

    let mut nested_jids = vec![0u8, tag::LIST_8, 3, 3, 4];
    nested_jids.extend(std::iter::repeat_n(tag::JID_PAIR, 1_000_000));
    assert!(decode_frame(&nested_jids).is_err());

    let mut wide = vec![0u8, tag::LIST_8, 2, 3, tag::LIST_16, 0xff, 0xff];
    for _ in 0..3 {
        wide.extend_from_slice(&[tag::LIST_16, 0xff, 0xff]);
    }
    assert!(decode_frame(&wide).is_err());
}

#[test]
fn compression_bombs_are_capped() {
    let bomb = compressed(&vec![0u8; 40 * 1024 * 1024]);
    assert!(decode_frame(&bomb).is_err());
}
