use std::time::Duration;

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use prost::Message as _;

use super::{Client, Outcome, Platform};
use crate::binary::Node;
use crate::crypto::{self, verify};
use crate::error::{Error, Result};
use crate::events::Event;
use crate::jid::Jid;
use crate::proto::{AdvDeviceIdentity, AdvSignedDeviceIdentity, AdvSignedDeviceIdentityHmac, ADV_ENCRYPTION_HOSTED};

const CROCKFORD: &[u8; 32] = b"123456789ABCDEFGHJKLMNPQRSTVWXYZ";
const PAIRING_ROUNDS: u32 = 2 << 16;
const FIRST_QR_TTL: Duration = Duration::from_secs(60);
const NEXT_QR_TTL: Duration = Duration::from_secs(20);

// The server checks this string. It has to look like "Chrome (Windows)", a browser and an OS it
// knows; anything else, like our device name, gets a 400 bad-request back. whatsmeow documents the
// same rule.
// TODO: Desktop shows up as Chrome here for now, find out what the desktop app sends.
fn pairing_display(platform: Platform) -> String {
    let browser = match platform {
        Platform::Desktop => "Chrome",
        other => other.display_name(),
    };
    let os = if cfg!(target_os = "windows") {
        "Windows"
    } else if cfg!(target_os = "macos") {
        "Mac OS"
    } else {
        "Linux"
    };
    format!("{browser} ({os})")
}

fn crockford(bytes: &[u8]) -> String {
    let mut out = String::new();
    let mut value: u64 = 0;
    let mut bits = 0;
    for byte in bytes {
        value = (value << 8) | u64::from(*byte);
        bits += 8;
        while bits >= 5 {
            out.push(char::from(CROCKFORD[((value >> (bits - 5)) & 31) as usize]));
            bits -= 5;
        }
    }
    if bits > 0 {
        out.push(char::from(CROCKFORD[((value << (5 - bits)) & 31) as usize]));
    }
    out
}

impl Client {
    pub(super) async fn on_pair_device(&self, node: &Node) -> Result<()> {
        let mut ack = Node::new("iq").attr("to", Jid::server()).attr("type", "result");
        if let Some(id) = node.get("id") {
            ack.set_attr("id", id.clone());
        }
        self.send_node(&ack)?;

        let refs: Vec<String> = node
            .child("pair-device")
            .map(|p| p.children_by("ref").filter_map(Node::content_string).collect())
            .unwrap_or_default();
        let creds = self.creds();
        let noise = STANDARD.encode(creds.noise_key.public);
        let identity = STANDARD.encode(creds.identity_key.public);
        let adv = STANDARD.encode(creds.adv_secret_key);
        let platform = self.inner.config.platform.companion_id();
        let generation = self.current_generation();

        let client = self.clone();
        tokio::spawn(async move {
            for (index, reference) in refs.into_iter().enumerate() {
                if client.current_generation() != generation || client.is_logged_in() {
                    return;
                }
                let code =
                    format!("https://wa.me/settings/linked_devices#{reference},{noise},{identity},{adv},{platform}");
                client.emit(Event::Qr(code));
                tokio::time::sleep(if index == 0 { FIRST_QR_TTL } else { NEXT_QR_TTL }).await;
            }
            if client.current_generation() == generation && !client.is_logged_in() {
                client.end_link(Outcome::QrTimeout);
            }
        });
        Ok(())
    }

    pub(super) async fn on_pair_success(&self, node: &Node) -> Result<()> {
        let id = node.attr_string("id").unwrap_or_default();
        match self.complete_pairing(node).await {
            Ok(reply) => {
                let reply = Node::new("iq")
                    .attr("to", Jid::server())
                    .attr("type", "result")
                    .attr("id", id)
                    .child_node(Node::new("pair-device-sign").child_node(reply));
                self.send_node(&reply)
            }
            Err(err) => {
                let reject = Node::new("iq")
                    .attr("to", Jid::server())
                    .attr("type", "error")
                    .attr("id", id)
                    .child_node(Node::new("error").attr("code", "401").attr("text", "not-authorized"));
                let _ = self.send_node(&reject);
                self.end_link(Outcome::Lost(format!("pairing failed: {err}")));
                Err(err)
            }
        }
    }

    async fn complete_pairing(&self, node: &Node) -> Result<Node> {
        let invalid = |why: &str| Error::Handshake(format!("pair-success rejected: {why}"));
        let pair = node.child("pair-success").ok_or_else(|| invalid("missing body"))?;
        let identity_blob = pair.child_bytes("device-identity").ok_or_else(|| invalid("missing device identity"))?;
        let device = pair.child("device").ok_or_else(|| invalid("missing device"))?;
        let jid = device.attr_jid("jid").ok_or_else(|| invalid("missing jid"))?;
        let lid = device.attr_jid("lid");
        let platform = pair.child("platform").and_then(|p| p.attr_string("name"));
        let business_name = pair.child("biz").and_then(|b| b.attr_string("name"));

        let creds = self.creds();
        let container = AdvSignedDeviceIdentityHmac::decode(identity_blob)?;
        let details = container.details.unwrap_or_default();
        let hosted = container.account_type == Some(ADV_ENCRYPTION_HOSTED);
        let prefix: &[u8] = if hosted { &[6, 5] } else { &[] };
        let expected = crypto::hmac_sha256(&creds.adv_secret_key, &[prefix, &details]);
        if !crypto::constant_time_eq(&expected, container.hmac.as_deref().unwrap_or_default()) {
            return Err(invalid("hmac mismatch"));
        }

        let mut account = AdvSignedDeviceIdentity::decode(details.as_slice())?;
        let device_details = account.details.clone().unwrap_or_default();
        let identity = AdvDeviceIdentity::decode(device_details.as_slice())?;
        let account_key: [u8; 32] = account
            .account_signature_key
            .as_deref()
            .and_then(|k| k.try_into().ok())
            .ok_or_else(|| invalid("missing account signature key"))?;
        let account_signature: [u8; 64] = account
            .account_signature
            .as_deref()
            .and_then(|s| s.try_into().ok())
            .ok_or_else(|| invalid("missing account signature"))?;

        let account_prefix: &[u8] = if identity.device_type == Some(ADV_ENCRYPTION_HOSTED) { &[6, 5] } else { &[6, 0] };
        let account_message = [account_prefix, &device_details, &creds.identity_key.public].concat();
        if !verify(&account_key, &account_message, &account_signature) {
            return Err(invalid("account signature does not verify"));
        }

        let device_message = [&[6u8, 1][..], &device_details, &creds.identity_key.public, &account_key].concat();
        account.device_signature = Some(creds.identity_key.sign(&device_message).to_vec());

        let stored = account.encode_to_vec();
        let mut reply_account = account.clone();
        reply_account.account_signature_key = None;

        self.update_creds(|c| {
            c.me = Some(jid.clone());
            c.lid = lid.clone();
            c.platform = platform.clone();
            c.business_name = business_name.clone();
            c.account = Some(stored);
            c.registered = true;
            c.pairing_code = None;
        })
        .await?;
        self.emit(Event::PairSuccess { id: jid, lid, platform, business_name });

        Ok(Node::new("device-identity")
            .attr("key-index", identity.key_index.unwrap_or_default())
            .bytes(reply_account.encode_to_vec()))
    }

    pub async fn request_pair_code(&self, phone: &str) -> Result<String> {
        if self.is_logged_in() {
            return Err(Error::other("this session is already paired"));
        }
        let me = Jid::phone(phone);
        if me.user.is_empty() {
            return Err(Error::InvalidJid(phone.to_owned()));
        }
        let code = crockford(&crypto::random_array::<5>());
        let creds = self.creds();

        let salt = crypto::random_array::<32>();
        let iv = crypto::random_array::<16>();
        let key = derive_pairing_key(&code, &salt).await?;
        let wrapped = [&salt[..], &iv, &crypto::aes_ctr(&key, &iv, &creds.pairing_ephemeral_key.public)?].concat();

        self.update_creds(|c| {
            c.pairing_code = Some(code.clone());
            c.me = Some(me.clone());
        })
        .await?;

        let platform = self.inner.config.platform;
        let display = pairing_display(platform);
        let request = Node::new("iq").attr("to", Jid::server()).attr("type", "set").attr("xmlns", "md").child_node(
            Node::new("link_code_companion_reg")
                .attr("jid", me)
                .attr("stage", "companion_hello")
                .attr("should_show_push_notification", "true")
                .children([
                    Node::new("link_code_pairing_wrapped_companion_ephemeral_pub").bytes(wrapped),
                    Node::new("companion_server_auth_key_pub").bytes(creds.noise_key.public.to_vec()),
                    Node::new("companion_platform_id").text(platform.companion_id().to_string()),
                    Node::new("companion_platform_display").text(display),
                    Node::new("link_code_pairing_nonce").text("0"),
                ]),
        );
        self.query(request).await?;
        Ok(code)
    }

    pub(super) async fn on_link_code_notification(&self, node: &Node) -> Result<()> {
        let missing = |what: &str| Error::Handshake(format!("link code notification is missing {what}"));
        let registration = node.child("link_code_companion_reg").ok_or_else(|| missing("registration"))?;
        let reference = registration.child_bytes("link_code_pairing_ref").ok_or_else(|| missing("ref"))?.to_vec();
        let primary_identity: [u8; 32] = registration
            .child_bytes("primary_identity_pub")
            .and_then(|k| k.try_into().ok())
            .ok_or_else(|| missing("primary identity"))?;
        let wrapped = registration
            .child_bytes("link_code_pairing_wrapped_primary_ephemeral_pub")
            .filter(|w| w.len() >= 80)
            .ok_or_else(|| missing("primary ephemeral key"))?;

        let creds = self.creds();
        let code = creds.pairing_code.clone().ok_or_else(|| missing("a pending pairing code"))?;
        let me = creds.me.clone().ok_or_else(|| missing("the phone number"))?;

        let key = derive_pairing_key(&code, &wrapped[..32]).await?;
        let primary_ephemeral: [u8; 32] = crypto::aes_ctr(&key, &wrapped[32..48], &wrapped[48..80])?
            .try_into()
            .map_err(|_| missing("a valid primary ephemeral key"))?;

        let companion_shared = creds.pairing_ephemeral_key.shared_secret(&primary_ephemeral);
        let random = crypto::random_array::<32>();
        let bundle_salt = crypto::random_array::<32>();
        let bundle_key: [u8; 32] =
            crypto::hkdf_array(&companion_shared, Some(&bundle_salt), b"link_code_pairing_key_bundle_encryption_key");
        let bundle_iv = crypto::random_array::<12>();
        let bundle_plain = [&creds.identity_key.public[..], &primary_identity, &random].concat();
        let bundle = crypto::aes_gcm_encrypt(&bundle_key, &bundle_iv, &[], &bundle_plain)?;
        let wrapped_bundle = [&bundle_salt[..], &bundle_iv, &bundle].concat();

        let identity_shared = creds.identity_key.shared_secret(&primary_identity);
        let adv_input = [&companion_shared[..], &identity_shared, &random].concat();
        let adv_secret: [u8; 32] = crypto::hkdf_array(&adv_input, None, b"adv_secret");
        self.update_creds(|c| c.adv_secret_key = adv_secret).await?;

        let finish = Node::new("iq").attr("to", Jid::server()).attr("type", "set").attr("xmlns", "md").child_node(
            Node::new("link_code_companion_reg").attr("jid", me).attr("stage", "companion_finish").children([
                Node::new("link_code_pairing_wrapped_key_bundle").bytes(wrapped_bundle),
                Node::new("companion_identity_public").bytes(creds.identity_key.public.to_vec()),
                Node::new("link_code_pairing_ref").bytes(reference),
            ]),
        );
        self.query(finish).await?;
        Ok(())
    }
}

async fn derive_pairing_key(code: &str, salt: &[u8]) -> Result<[u8; 32]> {
    let code = code.as_bytes().to_vec();
    let salt = salt.to_vec();
    tokio::task::spawn_blocking(move || crypto::pbkdf2_sha256(&code, &salt, PAIRING_ROUNDS))
        .await
        .map_err(|err| Error::other(err.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pairing_display_format() {
        for platform in
            [Platform::Chrome, Platform::Firefox, Platform::Safari, Platform::Edge, Platform::Opera, Platform::Desktop]
        {
            let display = pairing_display(platform);
            let (browser, os) = display.trim_end_matches(')').split_once(" (").unwrap();
            assert!(["Chrome", "Firefox", "Safari", "Edge", "Opera"].contains(&browser), "{display}");
            assert!(["Windows", "Mac OS", "Linux"].contains(&os), "{display}");
        }
    }

    #[test]
    fn pair_code_chars() {
        let code = crockford(&[0xff, 0x00, 0xaa, 0x55, 0x12]);
        assert_eq!(code.len(), 8);
        assert!(code.bytes().all(|c| CROCKFORD.contains(&c)));
        assert_eq!(crockford(&[0, 0, 0, 0, 0]), "11111111");
    }
}
