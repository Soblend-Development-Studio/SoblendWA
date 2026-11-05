use std::borrow::Cow;
use std::fmt;

use crate::jid::Jid;

pub type Str = Cow<'static, str>;

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Str(Str),
    Jid(Jid),
}

impl Value {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Str(s) => Some(s),
            Value::Jid(_) => None,
        }
    }

    pub fn to_jid(&self) -> Option<Jid> {
        match self {
            Value::Jid(jid) => Some(jid.clone()),
            Value::Str(s) => s.parse().ok(),
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Str(s) => f.write_str(s),
            Value::Jid(jid) => jid.fmt(f),
        }
    }
}

impl From<&'static str> for Value {
    fn from(value: &'static str) -> Self {
        Value::Str(Cow::Borrowed(value))
    }
}

impl From<String> for Value {
    fn from(value: String) -> Self {
        Value::Str(Cow::Owned(value))
    }
}

impl From<&String> for Value {
    fn from(value: &String) -> Self {
        Value::Str(Cow::Owned(value.clone()))
    }
}

impl From<Str> for Value {
    fn from(value: Str) -> Self {
        Value::Str(value)
    }
}

impl From<&Value> for Value {
    fn from(value: &Value) -> Self {
        value.clone()
    }
}

impl From<Jid> for Value {
    fn from(value: Jid) -> Self {
        Value::Jid(value)
    }
}

impl From<&Jid> for Value {
    fn from(value: &Jid) -> Self {
        Value::Jid(value.clone())
    }
}

macro_rules! numeric_value {
    ($($ty:ty),*) => {
        $(impl From<$ty> for Value {
            fn from(value: $ty) -> Self {
                Value::Str(Cow::Owned(value.to_string()))
            }
        })*
    };
}

numeric_value!(u8, u16, u32, u64, usize, i32, i64);

#[derive(Clone, Debug, Default, PartialEq)]
pub enum Content {
    #[default]
    None,
    Bytes(Vec<u8>),
    Text(Str),
    Nodes(Vec<Node>),
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Node {
    pub tag: Str,
    pub attrs: Vec<(Str, Value)>,
    pub content: Content,
}

impl Node {
    pub fn new(tag: impl Into<Str>) -> Self {
        Node { tag: tag.into(), attrs: Vec::new(), content: Content::None }
    }

    pub fn attr(mut self, key: impl Into<Str>, value: impl Into<Value>) -> Self {
        self.set_attr(key, value);
        self
    }

    pub fn attr_opt<V: Into<Value>>(self, key: impl Into<Str>, value: Option<V>) -> Self {
        match value {
            Some(value) => self.attr(key, value),
            None => self,
        }
    }

    pub fn set_attr(&mut self, key: impl Into<Str>, value: impl Into<Value>) {
        let key = key.into();
        let value = value.into();
        match self.attrs.iter_mut().find(|(k, _)| *k == key) {
            Some(slot) => slot.1 = value,
            None => self.attrs.push((key, value)),
        }
    }

    pub fn children(mut self, nodes: impl IntoIterator<Item = Node>) -> Self {
        self.content = Content::Nodes(nodes.into_iter().collect());
        self
    }

    pub fn child_node(self, node: Node) -> Self {
        self.children([node])
    }

    pub fn bytes(mut self, data: impl Into<Vec<u8>>) -> Self {
        self.content = Content::Bytes(data.into());
        self
    }

    pub fn text(mut self, text: impl Into<Str>) -> Self {
        self.content = Content::Text(text.into());
        self
    }

    pub fn push_child(&mut self, node: Node) {
        match &mut self.content {
            Content::Nodes(nodes) => nodes.push(node),
            _ => self.content = Content::Nodes(vec![node]),
        }
    }

    pub fn get(&self, key: &str) -> Option<&Value> {
        self.attrs.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    pub fn has(&self, key: &str) -> bool {
        self.get(key).is_some()
    }

    pub fn attr_str(&self, key: &str) -> Option<&str> {
        self.get(key).and_then(Value::as_str)
    }

    pub fn attr_string(&self, key: &str) -> Option<String> {
        self.get(key).map(ToString::to_string)
    }

    pub fn attr_jid(&self, key: &str) -> Option<Jid> {
        self.get(key).and_then(Value::to_jid)
    }

    pub fn attr_u64(&self, key: &str) -> Option<u64> {
        self.attr_str(key).and_then(|v| v.parse().ok())
    }

    pub fn attr_i64(&self, key: &str) -> Option<i64> {
        self.attr_str(key).and_then(|v| v.parse().ok())
    }

    pub fn attr_is(&self, key: &str, expected: &str) -> bool {
        self.attr_str(key) == Some(expected)
    }

    pub fn nodes(&self) -> &[Node] {
        match &self.content {
            Content::Nodes(nodes) => nodes,
            _ => &[],
        }
    }

    pub fn into_nodes(self) -> Vec<Node> {
        match self.content {
            Content::Nodes(nodes) => nodes,
            _ => Vec::new(),
        }
    }

    pub fn child(&self, tag: &str) -> Option<&Node> {
        self.nodes().iter().find(|n| n.tag == tag)
    }

    pub fn children_by<'a>(&'a self, tag: &'a str) -> impl Iterator<Item = &'a Node> + 'a {
        self.nodes().iter().filter(move |n| n.tag == tag)
    }

    pub fn first_child(&self) -> Option<&Node> {
        self.nodes().first()
    }

    pub fn path(&self, tags: &[&str]) -> Option<&Node> {
        tags.iter().try_fold(self, |node, tag| node.child(tag))
    }

    pub fn content_bytes(&self) -> Option<&[u8]> {
        match &self.content {
            Content::Bytes(data) => Some(data),
            Content::Text(text) => Some(text.as_bytes()),
            _ => None,
        }
    }

    pub fn content_string(&self) -> Option<String> {
        self.content_bytes().map(|b| String::from_utf8_lossy(b).into_owned())
    }

    pub fn child_bytes(&self, tag: &str) -> Option<&[u8]> {
        self.child(tag).and_then(Node::content_bytes)
    }

    pub fn child_uint(&self, tag: &str) -> Option<u32> {
        self.child_bytes(tag).map(|b| b.iter().fold(0u32, |acc, byte| (acc << 8) | u32::from(*byte)))
    }

    pub fn error_code(&self) -> Option<(u16, String)> {
        let error = self.child("error")?;
        let code = error.attr_str("code").and_then(|c| c.parse().ok()).unwrap_or(0);
        let text = error.attr_str("text").unwrap_or_default().to_owned();
        Some((code, text))
    }
}

impl fmt::Display for Node {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_xml(self, f, 0)
    }
}

fn write_xml(node: &Node, f: &mut fmt::Formatter<'_>, depth: usize) -> fmt::Result {
    let pad = "  ".repeat(depth);
    write!(f, "{pad}<{}", node.tag)?;
    for (key, value) in &node.attrs {
        write!(f, " {key}=\"{value}\"")?;
    }
    match &node.content {
        Content::None => writeln!(f, "/>"),
        Content::Text(text) => writeln!(f, ">{text}</{}>", node.tag),
        Content::Bytes(data) => match std::str::from_utf8(data) {
            Ok(text) if text.chars().all(|c| !c.is_control()) => writeln!(f, ">{text}</{}>", node.tag),
            _ => writeln!(
                f,
                "><!-- {} bytes: {} --></{}>",
                data.len(),
                hex::encode(&data[..data.len().min(32)]),
                node.tag
            ),
        },
        Content::Nodes(children) => {
            writeln!(f, ">")?;
            for child in children {
                write_xml(child, f, depth + 1)?;
            }
            writeln!(f, "{pad}</{}>", node.tag)
        }
    }
}
