use std::io::Read;

use flate2::read::ZlibDecoder;
use prost::Message as _;

use super::Client;
use crate::error::{Error, Result};
use crate::events::Event;
use crate::jid::Jid;
use crate::proto::{HistorySync, HistorySyncNotification};

fn inflate(compressed: &[u8]) -> Result<Vec<u8>> {
    let mut out = Vec::with_capacity(compressed.len() * 4);
    ZlibDecoder::new(compressed).read_to_end(&mut out)?;
    Ok(out)
}

impl Client {
    pub async fn download_history(&self, notification: &HistorySyncNotification) -> Result<HistorySync> {
        let compressed = match &notification.initial_hist_bootstrap_inline_payload {
            Some(inline) => inline.clone(),
            None => self.download(notification).await?,
        };
        let inflated = tokio::task::spawn_blocking(move || inflate(&compressed))
            .await
            .map_err(|err| Error::other(err.to_string()))??;
        Ok(HistorySync::decode(inflated.as_slice())?)
    }

    pub(crate) fn process_history_sync(&self, notification: HistorySyncNotification) {
        let client = self.clone();
        tokio::spawn(async move {
            let history = match client.download_history(&notification).await {
                Ok(history) => history,
                Err(err) => {
                    tracing::warn!(%err, "could not download a history sync chunk");
                    return;
                }
            };
            for mapping in &history.phone_number_to_lid_mappings {
                let pn = mapping.pn_jid.as_deref().and_then(|j| j.parse::<Jid>().ok());
                let lid = mapping.lid_jid.as_deref().and_then(|j| j.parse::<Jid>().ok());
                if let (Some(pn), Some(lid)) = (pn, lid) {
                    client.remember_lid(&pn, &lid).await;
                }
            }
            for conversation in &history.conversations {
                for entry in &conversation.messages {
                    let Some(info) = &entry.message else { continue };
                    let Some(message) = &info.message else { continue };
                    let Some(id) = info.key.as_ref().and_then(|k| k.id.clone()) else { continue };
                    let Ok(chat) = conversation.id.parse::<Jid>() else { continue };
                    if super::polls::poll_of(message).is_some() {
                        let from_me = info.key.as_ref().and_then(|k| k.from_me).unwrap_or(false);
                        let creators = match (from_me, info.participant.as_deref().or(Some(conversation.id.as_str()))) {
                            (true, _) => [client.me(), client.lid()].into_iter().flatten().collect(),
                            (false, Some(author)) => author.parse::<Jid>().into_iter().collect(),
                            (false, None) => Vec::new(),
                        };
                        client.remember_poll(&id, &chat, message, creators).await;
                    }
                }
            }
            client.emit(Event::HistorySync(Box::new(history)));
        });
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use flate2::write::ZlibEncoder;
    use flate2::Compression;

    use super::*;
    use crate::proto::{Conversation, Pushname};

    #[test]
    fn inflates_and_decodes_history() {
        let history = HistorySync {
            sync_type: HistorySync::RECENT,
            conversations: vec![Conversation { id: "5215512345678@s.whatsapp.net".into(), ..Default::default() }],
            pushnames: vec![Pushname { id: Some("5215512345678@s.whatsapp.net".into()), pushname: Some("Ana".into()) }],
            ..Default::default()
        };
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&history.encode_to_vec()).unwrap();
        let decoded = HistorySync::decode(inflate(&encoder.finish().unwrap()).unwrap().as_slice()).unwrap();
        assert_eq!(decoded, history);
    }
}
