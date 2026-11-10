use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::error::Error;

pub const USER_SERVER: &str = "s.whatsapp.net";
pub const LEGACY_USER_SERVER: &str = "c.us";
pub const GROUP_SERVER: &str = "g.us";
pub const LID_SERVER: &str = "lid";
pub const BROADCAST_SERVER: &str = "broadcast";
pub const NEWSLETTER_SERVER: &str = "newsletter";
pub const HOSTED_SERVER: &str = "hosted";
pub const HOSTED_LID_SERVER: &str = "hosted.lid";
pub const CALL_SERVER: &str = "call";

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Jid {
    pub user: String,
    pub device: u16,
    pub server: String,
}

impl Jid {
    pub fn new(user: impl Into<String>, server: impl Into<String>) -> Self {
        Jid { user: user.into(), device: 0, server: server.into() }
    }

    pub fn phone(number: &str) -> Self {
        let digits: String = number.chars().filter(char::is_ascii_digit).collect();
        Jid::new(digits, USER_SERVER)
    }

    pub fn group(id: impl Into<String>) -> Self {
        Jid::new(id, GROUP_SERVER)
    }

    pub fn lid(id: impl Into<String>) -> Self {
        Jid::new(id, LID_SERVER)
    }

    pub fn server() -> Self {
        Jid::new("", USER_SERVER)
    }

    pub fn group_server() -> Self {
        Jid::new("", GROUP_SERVER)
    }

    pub fn status_broadcast() -> Self {
        Jid::new("status", BROADCAST_SERVER)
    }

    pub(crate) fn from_agent(user: String, agent: u8, device: u16) -> Self {
        let server = match agent {
            1 => LID_SERVER,
            128 => HOSTED_SERVER,
            129 => HOSTED_LID_SERVER,
            _ => USER_SERVER,
        };
        Jid { user, device, server: server.to_owned() }
    }

    pub fn with_device(&self, device: u16) -> Self {
        Jid { user: self.user.clone(), device, server: self.server.clone() }
    }

    pub fn to_non_device(&self) -> Self {
        self.with_device(0)
    }

    pub fn agent(&self) -> u8 {
        match self.server.as_str() {
            LID_SERVER => 1,
            HOSTED_SERVER => 128,
            HOSTED_LID_SERVER => 129,
            _ => 0,
        }
    }

    pub fn is_user(&self) -> bool {
        matches!(
            self.server.as_str(),
            USER_SERVER | LID_SERVER | HOSTED_SERVER | HOSTED_LID_SERVER | LEGACY_USER_SERVER
        ) && !self.user.is_empty()
    }

    pub fn is_pn(&self) -> bool {
        matches!(self.server.as_str(), USER_SERVER | HOSTED_SERVER | LEGACY_USER_SERVER)
    }

    pub fn is_lid(&self) -> bool {
        matches!(self.server.as_str(), LID_SERVER | HOSTED_LID_SERVER)
    }

    pub fn is_group(&self) -> bool {
        self.server == GROUP_SERVER
    }

    pub fn is_broadcast(&self) -> bool {
        self.server == BROADCAST_SERVER
    }

    pub fn is_status_broadcast(&self) -> bool {
        self.is_broadcast() && self.user == "status"
    }

    pub fn is_newsletter(&self) -> bool {
        self.server == NEWSLETTER_SERVER
    }

    pub fn is_server(&self) -> bool {
        self.user.is_empty()
    }

    pub fn same_user(&self, other: &Jid) -> bool {
        self.user == other.user && self.normalized_server() == other.normalized_server()
    }

    fn normalized_server(&self) -> &str {
        if self.server == LEGACY_USER_SERVER {
            USER_SERVER
        } else {
            &self.server
        }
    }

    pub fn signal_address(&self) -> String {
        match self.agent() {
            0 => format!("{}.{}", self.user, self.device),
            agent => format!("{}_{}.{}", self.user, agent, self.device),
        }
    }

    pub fn parse(input: &str) -> Result<Self, Error> {
        input.parse()
    }
}

impl fmt::Display for Jid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if !self.user.is_empty() {
            f.write_str(&self.user)?;
            if self.device > 0 {
                let mut digits = [b':'; 6];
                let mut at = digits.len();
                let mut rest = self.device;
                while rest > 0 {
                    at -= 1;
                    digits[at] = b'0' + (rest % 10) as u8;
                    rest /= 10;
                }
                f.write_str(std::str::from_utf8(&digits[at - 1..]).unwrap_or_default())?;
            }
            f.write_str("@")?;
        }
        f.write_str(&self.server)
    }
}

impl FromStr for Jid {
    type Err = Error;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let Some((left, server)) = input.split_once('@') else {
            return match input {
                USER_SERVER | GROUP_SERVER | BROADCAST_SERVER | LID_SERVER | NEWSLETTER_SERVER | CALL_SERVER => {
                    Ok(Jid::new("", input))
                }
                _ => Err(Error::InvalidJid(input.to_owned())),
            };
        };
        if server.is_empty() {
            return Err(Error::InvalidJid(input.to_owned()));
        }
        let (user_part, device) = match left.split_once(':') {
            Some((user, device)) => {
                let device = device.parse().map_err(|_| Error::InvalidJid(input.to_owned()))?;
                (user, device)
            }
            None => (left, 0),
        };
        let user = user_part.split_once('_').map_or(user_part, |(user, _)| user);
        let server = if server == LEGACY_USER_SERVER { USER_SERVER } else { server };
        Ok(Jid { user: user.to_owned(), device, server: server.to_owned() })
    }
}

impl TryFrom<&str> for Jid {
    type Error = Error;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        value.parse()
    }
}

impl TryFrom<String> for Jid {
    type Error = Error;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}

impl Serialize for Jid {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Jid {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        raw.parse().map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_formats() {
        let jid: Jid = "5215512345678:12@s.whatsapp.net".parse().unwrap();
        assert_eq!(jid.user, "5215512345678");
        assert_eq!(jid.device, 12);
        assert_eq!(jid.to_string(), "5215512345678:12@s.whatsapp.net");
        assert_eq!(Jid::phone("1").with_device(65535).to_string(), "1:65535@s.whatsapp.net");
        assert_eq!(format!("{:>40}", Jid::phone("1").with_device(9)), "1:9@s.whatsapp.net");
        assert_eq!(jid.signal_address(), "5215512345678.12");

        let lid: Jid = "123456789:3@lid".parse().unwrap();
        assert_eq!(lid.signal_address(), "123456789_1.3");
        assert!(lid.is_lid());

        let group = Jid::group("120363000000000000");
        assert_eq!(group.to_string(), "120363000000000000@g.us");
        assert_eq!(Jid::server().to_string(), "s.whatsapp.net");
        assert_eq!(Jid::phone("+52 1 55 1234-5678").user, "5215512345678");
    }
}
