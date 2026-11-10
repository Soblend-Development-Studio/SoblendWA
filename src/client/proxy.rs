use std::fmt;
use std::net::IpAddr;
use std::str::FromStr;

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use crate::error::{Error, Result};

const MAX_CONNECT_RESPONSE: usize = 16 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProxyKind {
    Http,
    Socks5,
}

#[derive(Clone, PartialEq, Eq)]
pub struct Proxy {
    kind: ProxyKind,
    host: String,
    port: u16,
    credentials: Option<(String, String)>,
    url: String,
}

fn invalid(url: &str, why: &str) -> Error {
    Error::Other(format!("invalid proxy {}: {why}", redact(url)))
}

fn redact(url: &str) -> String {
    match (url.find("://"), url.rfind('@')) {
        (Some(scheme), Some(at)) if at > scheme => format!("{}://***@{}", &url[..scheme], &url[at + 1..]),
        _ => url.to_owned(),
    }
}

fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let decoded = (bytes[i] == b'%')
            .then(|| text.get(i + 1..i + 3))
            .flatten()
            .and_then(|hex| u8::from_str_radix(hex, 16).ok());
        match decoded {
            Some(byte) => {
                out.push(byte);
                i += 3;
            }
            None => {
                out.push(bytes[i]);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

impl Proxy {
    pub fn parse(url: &str) -> Result<Proxy> {
        let (scheme, rest) = url.split_once("://").ok_or_else(|| invalid(url, "missing scheme"))?;
        let kind = match scheme.to_ascii_lowercase().as_str() {
            "http" => ProxyKind::Http,
            "socks5" | "socks5h" => ProxyKind::Socks5,
            other => return Err(invalid(url, &format!("unsupported scheme {other}"))),
        };
        let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
        let (credentials, address) = match authority.rsplit_once('@') {
            Some((userinfo, address)) => {
                let (user, pass) = userinfo.split_once(':').unwrap_or((userinfo, ""));
                (Some((percent_decode(user), percent_decode(pass))), address)
            }
            None => (None, authority),
        };
        let default_port = if kind == ProxyKind::Http { 80 } else { 1080 };
        let (host, port) = if let Some(bracketed) = address.strip_prefix('[') {
            let (host, tail) = bracketed.split_once(']').ok_or_else(|| invalid(url, "unclosed ipv6 address"))?;
            let port = match tail.strip_prefix(':') {
                Some(port) => port.parse().map_err(|_| invalid(url, "bad port"))?,
                None => default_port,
            };
            (host.to_owned(), port)
        } else {
            match address.rsplit_once(':') {
                Some((host, port)) => (host.to_owned(), port.parse().map_err(|_| invalid(url, "bad port"))?),
                None => (address.to_owned(), default_port),
            }
        };
        if host.is_empty() {
            return Err(invalid(url, "missing host"));
        }
        Ok(Proxy { kind, host, port, credentials, url: url.to_owned() })
    }

    pub fn kind(&self) -> ProxyKind {
        self.kind
    }

    pub fn host(&self) -> &str {
        &self.host
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub(crate) fn for_http(&self) -> Result<reqwest::Proxy> {
        reqwest::Proxy::all(&self.url).map_err(|err| invalid(&self.url, &err.to_string()))
    }

    pub(crate) async fn connect(&self, host: &str, port: u16) -> Result<TcpStream> {
        let mut stream = TcpStream::connect((self.host.as_str(), self.port))
            .await
            .map_err(|err| Error::Other(format!("could not reach the proxy {}:{}: {err}", self.host, self.port)))?;
        stream.set_nodelay(true)?;
        match self.kind {
            ProxyKind::Http => self.http_connect(&mut stream, host, port).await?,
            ProxyKind::Socks5 => self.socks5_connect(&mut stream, host, port).await?,
        }
        Ok(stream)
    }

    async fn http_connect(&self, stream: &mut TcpStream, host: &str, port: u16) -> Result<()> {
        let target = if host.contains(':') { format!("[{host}]:{port}") } else { format!("{host}:{port}") };
        let mut request = format!("CONNECT {target} HTTP/1.1\r\nHost: {target}\r\nProxy-Connection: keep-alive\r\n");
        if let Some((user, pass)) = &self.credentials {
            request.push_str(&format!("Proxy-Authorization: Basic {}\r\n", STANDARD.encode(format!("{user}:{pass}"))));
        }
        request.push_str("\r\n");
        stream.write_all(request.as_bytes()).await?;

        let mut response = Vec::with_capacity(256);
        let mut chunk = [0u8; 512];
        let end = loop {
            let read = stream.read(&mut chunk).await?;
            if read == 0 {
                return Err(Error::Other("the proxy closed the connection during CONNECT".into()));
            }
            response.extend_from_slice(&chunk[..read]);
            if let Some(end) = response.windows(4).position(|w| w == b"\r\n\r\n") {
                break end + 4;
            }
            if response.len() > MAX_CONNECT_RESPONSE {
                return Err(Error::Other("the proxy sent an oversized CONNECT response".into()));
            }
        };
        if end != response.len() {
            return Err(Error::Other("the proxy sent data before the tunnel was ready".into()));
        }
        let status_line = String::from_utf8_lossy(&response[..end]).lines().next().unwrap_or_default().to_owned();
        let status = status_line.split_whitespace().nth(1).unwrap_or_default();
        match status {
            "200" => Ok(()),
            "407" => Err(Error::Other("the proxy rejected the credentials (407)".into())),
            _ => Err(Error::Other(format!("the proxy refused the tunnel: {status_line}"))),
        }
    }

    async fn socks5_connect(&self, stream: &mut TcpStream, host: &str, port: u16) -> Result<()> {
        let greeting: &[u8] = if self.credentials.is_some() { &[5, 2, 0, 2] } else { &[5, 1, 0] };
        stream.write_all(greeting).await?;
        let mut choice = [0u8; 2];
        stream.read_exact(&mut choice).await?;
        if choice[0] != 5 {
            return Err(Error::Other("the proxy does not speak SOCKS5".into()));
        }
        match (choice[1], &self.credentials) {
            (0, _) => {}
            (2, Some((user, pass))) => {
                if user.len() > 255 || pass.len() > 255 {
                    return Err(Error::Other("SOCKS5 credentials are limited to 255 bytes".into()));
                }
                let mut auth = vec![1, user.len() as u8];
                auth.extend_from_slice(user.as_bytes());
                auth.push(pass.len() as u8);
                auth.extend_from_slice(pass.as_bytes());
                stream.write_all(&auth).await?;
                let mut verdict = [0u8; 2];
                stream.read_exact(&mut verdict).await?;
                if verdict[1] != 0 {
                    return Err(Error::Other("the SOCKS5 proxy rejected the credentials".into()));
                }
            }
            (0xff, _) => return Err(Error::Other("the SOCKS5 proxy accepted none of our auth methods".into())),
            (method, _) => {
                return Err(Error::Other(format!("the SOCKS5 proxy chose unsupported auth method {method}")))
            }
        }

        let mut request = vec![5, 1, 0];
        match host.parse::<IpAddr>() {
            Ok(IpAddr::V4(ip)) => {
                request.push(1);
                request.extend_from_slice(&ip.octets());
            }
            Ok(IpAddr::V6(ip)) => {
                request.push(4);
                request.extend_from_slice(&ip.octets());
            }
            Err(_) => {
                if host.len() > 255 {
                    return Err(Error::Other("host name too long for SOCKS5".into()));
                }
                request.push(3);
                request.push(host.len() as u8);
                request.extend_from_slice(host.as_bytes());
            }
        }
        request.extend_from_slice(&port.to_be_bytes());
        stream.write_all(&request).await?;

        let mut head = [0u8; 4];
        stream.read_exact(&mut head).await?;
        if head[1] != 0 {
            let why = match head[1] {
                1 => "general failure",
                2 => "connection not allowed by ruleset",
                3 => "network unreachable",
                4 => "host unreachable",
                5 => "connection refused",
                6 => "TTL expired",
                7 => "command not supported",
                8 => "address type not supported",
                _ => "unknown error",
            };
            return Err(Error::Other(format!("the SOCKS5 proxy could not connect: {why}")));
        }
        let bound = match head[3] {
            1 => 4,
            4 => 16,
            3 => {
                let mut len = [0u8; 1];
                stream.read_exact(&mut len).await?;
                usize::from(len[0])
            }
            other => return Err(Error::Other(format!("the SOCKS5 proxy answered with address type {other}"))),
        };
        let mut skip = vec![0u8; bound + 2];
        stream.read_exact(&mut skip).await?;
        Ok(())
    }
}

impl FromStr for Proxy {
    type Err = Error;

    fn from_str(url: &str) -> Result<Self> {
        Proxy::parse(url)
    }
}

impl TryFrom<&str> for Proxy {
    type Error = Error;

    fn try_from(url: &str) -> Result<Self> {
        Proxy::parse(url)
    }
}

impl fmt::Debug for Proxy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Proxy")
            .field("kind", &self.kind)
            .field("host", &self.host)
            .field("port", &self.port)
            .field("authenticated", &self.credentials.is_some())
            .finish()
    }
}

impl fmt::Display for Proxy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&redact(&self.url))
    }
}

#[cfg(test)]
mod tests {
    use tokio::net::TcpListener;

    use super::*;

    #[test]
    fn parses_proxy_urls() {
        let socks = Proxy::parse("socks5h://bot:p%40ss@10.0.0.2:9050").unwrap();
        assert_eq!(socks.kind(), ProxyKind::Socks5);
        assert_eq!((socks.host(), socks.port()), ("10.0.0.2", 9050));
        assert_eq!(socks.credentials, Some(("bot".into(), "p@ss".into())));
        assert_eq!(socks.to_string(), "socks5h://***@10.0.0.2:9050");
        assert!(!format!("{socks:?}").contains("p@ss"));

        let http = Proxy::parse("http://[::1]").unwrap();
        assert_eq!((http.kind(), http.host(), http.port()), (ProxyKind::Http, "::1", 80));
        assert!(Proxy::parse("ftp://x").is_err());
        assert!(Proxy::parse("socks5://:1080").is_err());
    }

    async fn echo_server() -> u16 {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            while let Ok((mut socket, _)) = listener.accept().await {
                tokio::spawn(async move {
                    let mut buf = [0u8; 64];
                    while let Ok(n) = socket.read(&mut buf).await {
                        if n == 0 || socket.write_all(&buf[..n]).await.is_err() {
                            break;
                        }
                    }
                });
            }
        });
        port
    }

    async fn relay(mut client: TcpStream, host: &str, port: u16) {
        let mut upstream = TcpStream::connect((host, port)).await.unwrap();
        let _ = tokio::io::copy_bidirectional(&mut client, &mut upstream).await;
    }

    async fn socks5_session(mut socket: TcpStream, user: &str, pass: &str) {
        let mut head = [0u8; 2];
        socket.read_exact(&mut head).await.unwrap();
        let mut methods = vec![0u8; usize::from(head[1])];
        socket.read_exact(&mut methods).await.unwrap();
        assert!(methods.contains(&2));
        socket.write_all(&[5, 2]).await.unwrap();
        let mut version_and_len = [0u8; 2];
        socket.read_exact(&mut version_and_len).await.unwrap();
        let mut got_user = vec![0u8; usize::from(version_and_len[1])];
        socket.read_exact(&mut got_user).await.unwrap();
        let mut pass_len = [0u8; 1];
        socket.read_exact(&mut pass_len).await.unwrap();
        let mut got_pass = vec![0u8; usize::from(pass_len[0])];
        socket.read_exact(&mut got_pass).await.unwrap();
        let ok = got_user == user.as_bytes() && got_pass == pass.as_bytes();
        socket.write_all(&[1, if ok { 0 } else { 1 }]).await.unwrap();
        if !ok {
            return;
        }
        let mut request = [0u8; 4];
        socket.read_exact(&mut request).await.unwrap();
        assert_eq!(&request[..3], &[5, 1, 0]);
        let host = match request[3] {
            1 => {
                let mut ip = [0u8; 4];
                socket.read_exact(&mut ip).await.unwrap();
                std::net::Ipv4Addr::from(ip).to_string()
            }
            4 => {
                let mut ip = [0u8; 16];
                socket.read_exact(&mut ip).await.unwrap();
                std::net::Ipv6Addr::from(ip).to_string()
            }
            _ => {
                let mut len = [0u8; 1];
                socket.read_exact(&mut len).await.unwrap();
                let mut name = vec![0u8; usize::from(len[0])];
                socket.read_exact(&mut name).await.unwrap();
                String::from_utf8(name).unwrap()
            }
        };
        let mut target = [0u8; 2];
        socket.read_exact(&mut target).await.unwrap();
        socket.write_all(&[5, 0, 0, 1, 127, 0, 0, 1, 0, 0]).await.unwrap();
        relay(socket, &host, u16::from_be_bytes(target)).await;
    }

    async fn fake_socks5(user: &'static str, pass: &'static str) -> u16 {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            while let Ok((socket, _)) = listener.accept().await {
                tokio::spawn(socks5_session(socket, user, pass));
            }
        });
        port
    }

    async fn fake_http_proxy() -> u16 {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = Vec::new();
            let mut byte = [0u8; 1];
            while !request.ends_with(b"\r\n\r\n") {
                socket.read_exact(&mut byte).await.unwrap();
                request.push(byte[0]);
            }
            let text = String::from_utf8(request).unwrap();
            assert!(text.contains(&format!("Proxy-Authorization: Basic {}", STANDARD.encode("ana:secreto"))));
            let target: u16 = text.split_whitespace().nth(1).unwrap().rsplit(':').next().unwrap().parse().unwrap();
            socket.write_all(b"HTTP/1.1 200 Connection established\r\n\r\n").await.unwrap();
            relay(socket, "127.0.0.1", target).await;
        });
        port
    }

    async fn round_trip(mut stream: TcpStream) {
        stream.write_all(b"hola soblend").await.unwrap();
        let mut back = [0u8; 12];
        stream.read_exact(&mut back).await.unwrap();
        assert_eq!(&back, b"hola soblend");
    }

    #[tokio::test]
    async fn socks5_auth() {
        let echo = echo_server().await;
        let proxy =
            Proxy::parse(&format!("socks5://ana:secreto@127.0.0.1:{}", fake_socks5("ana", "secreto").await)).unwrap();
        round_trip(proxy.connect("localhost", echo).await.unwrap()).await;

        let wrong =
            Proxy::parse(&format!("socks5://ana:otra@127.0.0.1:{}", fake_socks5("ana", "secreto").await)).unwrap();
        assert!(wrong.connect("localhost", echo).await.is_err());
    }

    #[tokio::test]
    #[ignore = "connects to the real WhatsApp servers"]
    async fn live_qr_socks5() {
        let port = fake_socks5("ana", "secreto").await;
        let proxy = Proxy::parse(&format!("socks5://ana:secreto@127.0.0.1:{port}")).unwrap();
        let (client, mut events) = crate::Client::builder().proxy(proxy).build().await.unwrap();
        client.connect().await.unwrap();
        let qr = tokio::time::timeout(std::time::Duration::from_secs(30), async {
            loop {
                if let Some(crate::Event::Qr(code)) = events.recv().await {
                    break code;
                }
            }
        })
        .await
        .expect("no QR through the proxy within 30 s");
        assert!(qr.starts_with("https://wa.me/settings/linked_devices#"));
        client.disconnect().await;
    }

    #[tokio::test]
    async fn http_connect_tunnel() {
        let echo = echo_server().await;
        let proxy = Proxy::parse(&format!("http://ana:secreto@127.0.0.1:{}", fake_http_proxy().await)).unwrap();
        round_trip(proxy.connect("127.0.0.1", echo).await.unwrap()).await;
    }
}
