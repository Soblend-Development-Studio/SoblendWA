use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use dashmap::DashMap;
use parking_lot::Mutex;
use serde_json::Value;
use tokio::sync::Notify;

const ORDERED: &[&str] = &[
    "sendText",
    "reply",
    "react",
    "sendMedia",
    "sendFile",
    "sendPoll",
    "votePoll",
    "edit",
    "revoke",
    "sendInteractive",
    "sendButtons",
    "sendList",
    "sendContacts",
    "sendMentionAll",
    "sendGroupInvite",
    "sendProto",
];

#[derive(Default)]
struct Line {
    issued: u64,
    serving: u64,
    abandoned: BTreeSet<u64>,
    bells: BTreeMap<u64, Arc<Notify>>,
}

#[derive(Default)]
struct Gate {
    line: Mutex<Line>,
}

#[derive(Default, Clone)]
pub(crate) struct Turnstiles {
    gates: Arc<DashMap<String, Arc<Gate>>>,
}

pub struct Turn {
    gates: Arc<DashMap<String, Arc<Gate>>>,
    key: String,
    gate: Arc<Gate>,
    bell: Arc<Notify>,
    number: u64,
}

impl Turnstiles {
    pub(crate) fn take(&self, key: String) -> Turn {
        let gate = self.gates.entry(key.clone()).or_default().clone();
        let bell = Arc::new(Notify::new());
        let number = {
            let mut line = gate.line.lock();
            let number = line.issued;
            line.issued += 1;
            line.bells.insert(number, bell.clone());
            number
        };
        Turn { gates: self.gates.clone(), key, gate, bell, number }
    }

    #[cfg(test)]
    fn open_gates(&self) -> usize {
        self.gates.len()
    }
}

impl Turn {
    pub async fn ready(&self) {
        while self.gate.line.lock().serving != self.number {
            self.bell.notified().await;
        }
    }
}

impl Drop for Turn {
    fn drop(&mut self) {
        let (next, idle) = {
            let mut guard = self.gate.line.lock();
            let line = &mut *guard;
            line.bells.remove(&self.number);
            if line.serving == self.number {
                line.serving += 1;
                while line.abandoned.remove(&line.serving) {
                    line.serving += 1;
                }
            } else {
                line.abandoned.insert(self.number);
            }
            (line.bells.get(&line.serving).cloned(), line.serving == line.issued)
        };
        // wake only the next turn. The first version used notify_waiters() and 10k queued sends to one
        // chat turned into 100M wakeups (545 msg/s).
        if let Some(bell) = next {
            bell.notify_one();
        }
        if idle {
            self.gates.remove_if(&self.key, |_, gate| Arc::strong_count(gate) <= 2 && Arc::ptr_eq(gate, &self.gate));
        }
    }
}

pub(crate) fn chat_of(method: &str, params: &Value) -> Option<String> {
    if !ORDERED.contains(&method) {
        return None;
    }
    let field = if method == "sendGroupInvite" { "to" } else { "chat" };
    params[field].as_str().map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn call_order() {
        let turnstiles = Turnstiles::default();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let mut tasks = Vec::new();
        for i in 0..200u64 {
            let turn = turnstiles.take("chat".into());
            let seen = seen.clone();
            tasks.push(tokio::spawn(async move {
                if i % 7 == 0 {
                    tokio::time::sleep(Duration::from_micros(200)).await;
                }
                turn.ready().await;
                seen.lock().push(i);
                drop(turn);
            }));
        }
        for task in tasks {
            task.await.unwrap();
        }
        assert_eq!(*seen.lock(), (0..200).collect::<Vec<_>>());
        assert_eq!(turnstiles.open_gates(), 0, "idle chats must not keep a gate around");
    }

    #[tokio::test]
    async fn abandoned_turn() {
        let turnstiles = Turnstiles::default();
        let first = turnstiles.take("chat".into());
        let skipped = turnstiles.take("chat".into());
        let last = turnstiles.take("chat".into());
        drop(skipped);
        drop(first);
        tokio::time::timeout(Duration::from_secs(1), last.ready()).await.unwrap();
        let other = turnstiles.take("otro".into());
        tokio::time::timeout(Duration::from_secs(1), other.ready()).await.unwrap();
    }
}
