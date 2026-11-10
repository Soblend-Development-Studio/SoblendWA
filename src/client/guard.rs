use std::collections::VecDeque;
use std::future::Future;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use dashmap::DashMap;
use parking_lot::Mutex;

use super::Client;
use crate::error::{Error, Result};
use crate::jid::Jid;

const PRUNE_ABOVE: usize = 4096;

tokio::task_local! {
    static LANE: Jid;
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SendLimit {
    pub per_minute: u32,
    pub burst: u32,
    pub per_chat_per_minute: u32,
    pub per_chat_burst: u32,
}

impl Default for SendLimit {
    fn default() -> Self {
        SendLimit { per_minute: 240, burst: 60, per_chat_per_minute: 40, per_chat_burst: 15 }
    }
}

#[derive(Clone, Copy, Debug)]
struct Bucket {
    tokens: f64,
    updated: Instant,
}

impl Bucket {
    fn full(burst: u32) -> Self {
        Bucket { tokens: f64::from(burst), updated: Instant::now() }
    }

    fn refill(&mut self, per_minute: u32, burst: u32) {
        let now = Instant::now();
        let earned = now.duration_since(self.updated).as_secs_f64() * f64::from(per_minute) / 60.0;
        self.tokens = (self.tokens + earned).min(f64::from(burst));
        self.updated = now;
    }

    fn wait_for_one(&self, per_minute: u32) -> Duration {
        let missing = (1.0 - self.tokens).max(0.0);
        Duration::from_secs_f64(missing * 60.0 / f64::from(per_minute.max(1)))
    }
}

#[derive(Default)]
pub(crate) struct Limiter {
    global: Mutex<Option<Bucket>>,
    chats: DashMap<Jid, Bucket>,
}

impl Limiter {
    fn admit(&self, limit: &SendLimit, chat: &Jid) -> std::result::Result<(), Duration> {
        let mut global = self.global.lock();
        let global = global.get_or_insert_with(|| Bucket::full(limit.burst));
        global.refill(limit.per_minute, limit.burst);
        let mut chat_bucket = *self.chats.entry(chat.clone()).or_insert_with(|| Bucket::full(limit.per_chat_burst));
        chat_bucket.refill(limit.per_chat_per_minute, limit.per_chat_burst);
        if global.tokens < 1.0 || chat_bucket.tokens < 1.0 {
            let wait = global.wait_for_one(limit.per_minute).max(chat_bucket.wait_for_one(limit.per_chat_per_minute));
            self.chats.insert(chat.clone(), chat_bucket);
            return Err(wait);
        }
        global.tokens -= 1.0;
        chat_bucket.tokens -= 1.0;
        self.chats.insert(chat.clone(), chat_bucket);
        if self.chats.len() > PRUNE_ABOVE {
            let idle = Duration::from_secs(60 * 60);
            self.chats.retain(|_, bucket| bucket.updated.elapsed() < idle);
        }
        Ok(())
    }
}

#[derive(Default)]
pub(crate) struct Counters {
    pub received: AtomicU64,
    pub sent: AtomicU64,
    pub undecryptable: AtomicU64,
    pub recovered: AtomicU64,
    pub duplicates: AtomicU64,
    pub reconnects: AtomicU64,
    pub rate_limited: AtomicU64,
    pub media_uploaded: AtomicU64,
    pub media_reused: AtomicU64,
    pub outbox_resent: AtomicU64,
    pub panics: AtomicU64,
    pub lost: AtomicU64,
}

impl Counters {
    pub(crate) fn bump(counter: &AtomicU64) {
        counter.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn drop_one(counter: &AtomicU64) {
        let mut current = counter.load(Ordering::Relaxed);
        while current > 0 {
            match counter.compare_exchange_weak(current, current - 1, Ordering::Relaxed, Ordering::Relaxed) {
                Ok(_) => break,
                Err(actual) => current = actual,
            }
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    pub connected: bool,
    pub uptime: Duration,
    pub received: u64,
    pub sent: u64,
    pub undecryptable: u64,
    pub recovered: u64,
    pub lost: u64,
    pub duplicates: u64,
    pub awaiting_recovery: usize,
    pub reconnects: u64,
    pub rate_limited: u64,
    pub media_uploaded: u64,
    pub media_reused: u64,
    pub outbox_resent: u64,
    pub panics: u64,
    pub cached_devices: usize,
    pub cached_groups: usize,
    pub pending_queries: usize,
    pub open_lanes: usize,
    pub latency: Option<Duration>,
    pub send_timings: TimingSummary,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Spread {
    pub p50: Duration,
    pub p95: Duration,
    pub max: Duration,
}

impl Spread {
    fn of(mut values: Vec<Duration>) -> Option<Spread> {
        let last = values.len().checked_sub(1)?;
        let max = values.iter().copied().max()?;
        let p95 = *values.select_nth_unstable(last * 95 / 100).1;
        let p50 = *values.select_nth_unstable(last * 50 / 100).1;
        Some(Spread { p50, p95, max })
    }
}

#[derive(Default)]
pub(crate) struct TimingWindow {
    samples: VecDeque<SendTimings>,
    summary: Option<TimingSummary>,
}

impl TimingWindow {
    pub(crate) fn push(&mut self, timings: SendTimings, capacity: usize) {
        if self.samples.len() >= capacity {
            self.samples.pop_front();
        }
        self.samples.push_back(timings);
        self.summary = None;
    }

    pub(crate) fn summary(&mut self) -> TimingSummary {
        let samples = &mut self.samples;
        *self.summary.get_or_insert_with(|| TimingSummary::of(samples.make_contiguous()))
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TimingSummary {
    pub samples: usize,
    pub queued: Spread,
    pub upload: Option<Spread>,
    pub prepare: Spread,
    pub encrypt: Spread,
    pub server: Option<Spread>,
    pub total: Spread,
}

impl TimingSummary {
    pub(crate) fn of(samples: &[SendTimings]) -> TimingSummary {
        let pick = |field: fn(&SendTimings) -> Option<Duration>| {
            Spread::of(samples.iter().filter_map(field).collect::<Vec<_>>())
        };
        TimingSummary {
            samples: samples.len(),
            queued: pick(|t| Some(t.queued)).unwrap_or_default(),
            upload: pick(|t| t.upload),
            prepare: pick(|t| Some(t.prepare)).unwrap_or_default(),
            encrypt: pick(|t| Some(t.encrypt)).unwrap_or_default(),
            server: pick(|t| t.server),
            total: pick(|t| Some(t.total())).unwrap_or_default(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SendTimings {
    pub queued: Duration,
    pub upload: Option<Duration>,
    pub prepare: Duration,
    pub encrypt: Duration,
    pub server: Option<Duration>,
}

impl SendTimings {
    pub fn total(&self) -> Duration {
        self.queued + self.upload.unwrap_or_default() + self.prepare + self.encrypt + self.server.unwrap_or_default()
    }
}

impl Client {
    pub fn stats(&self) -> Stats {
        let counters = &self.inner.counters;
        let read = |counter: &AtomicU64| counter.load(Ordering::Relaxed);
        let latency = self.inner.latency_micros.load(Ordering::Relaxed);
        Stats {
            connected: self.is_connected(),
            uptime: self.inner.started.elapsed(),
            received: read(&counters.received),
            sent: read(&counters.sent),
            undecryptable: read(&counters.undecryptable),
            recovered: read(&counters.recovered),
            lost: read(&counters.lost),
            duplicates: read(&counters.duplicates),
            awaiting_recovery: self.inner.awaiting_retry.len(),
            reconnects: read(&counters.reconnects),
            rate_limited: read(&counters.rate_limited),
            media_uploaded: read(&counters.media_uploaded),
            media_reused: read(&counters.media_reused),
            outbox_resent: read(&counters.outbox_resent),
            panics: read(&counters.panics),
            cached_devices: self.inner.devices.len(),
            cached_groups: self.inner.groups.len(),
            pending_queries: self.inner.pending.lock().len(),
            open_lanes: self.inner.chat_lanes.len(),
            latency: (latency > 0).then(|| Duration::from_micros(latency)),
            send_timings: self.inner.timing_window.lock().summary(),
        }
    }

    pub(crate) fn admit_send(&self, chat: &Jid) -> Result<()> {
        let Some(limit) = &self.inner.config.send_limit else { return Ok(()) };
        if chat.is_status_broadcast() {
            return Ok(());
        }
        match self.inner.limiter.admit(limit, chat) {
            Ok(()) => Ok(()),
            Err(retry_after) => {
                Counters::bump(&self.inner.counters.rate_limited);
                tracing::warn!(%chat, ?retry_after, "send refused by the send limit");
                Err(Error::RateLimited { chat: chat.to_string(), retry_after })
            }
        }
    }

    pub(crate) async fn in_chat_lane<T>(&self, chat: &Jid, work: impl Future<Output = T>) -> (T, Duration) {
        let chat = chat.to_non_device();
        if LANE.try_with(|held| *held == chat).unwrap_or(false) {
            return (work.await, Duration::ZERO);
        }
        let lane = self.inner.chat_lanes.entry(chat.clone()).or_insert_with(|| Arc::new(Default::default())).clone();
        let waiting = Instant::now();
        let guard = lane.lock_owned().await;
        let queued = waiting.elapsed();
        let result = LANE.scope(chat.clone(), work).await;
        drop(guard);
        self.inner.chat_lanes.remove_if(&chat, |_, lane| Arc::strong_count(lane) == 1);
        (result, queued)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percentiles() {
        let mut seed = 7u64;
        for len in [1usize, 2, 3, 20, 99, 512] {
            let values: Vec<Duration> = (0..len)
                .map(|_| {
                    seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                    Duration::from_micros(seed >> 44)
                })
                .collect();
            let mut sorted = values.clone();
            sorted.sort_unstable();
            let at = |percent: usize| sorted[(len - 1) * percent / 100];
            assert_eq!(Spread::of(values), Some(Spread { p50: at(50), p95: at(95), max: at(100) }), "len {len}");
        }
        assert_eq!(Spread::of(Vec::new()), None);

        let mut window = TimingWindow::default();
        let sample = |micros| SendTimings { encrypt: Duration::from_micros(micros), ..Default::default() };
        window.push(sample(5), 2);
        assert_eq!(window.summary().encrypt.max, Duration::from_micros(5));
        window.push(sample(9), 2);
        window.push(sample(1), 2);
        let summary = window.summary();
        assert_eq!(
            (summary.samples, summary.encrypt.max, summary.encrypt.p50),
            (2, Duration::from_micros(9), Duration::from_micros(1))
        );
    }

    #[test]
    fn runaway_loop() {
        let limiter = Limiter::default();
        let limit = SendLimit { per_minute: 600, burst: 50, per_chat_per_minute: 60, per_chat_burst: 5 };
        let spammed = Jid::phone("5219900000001");
        let admitted = (0..100).filter(|_| limiter.admit(&limit, &spammed).is_ok()).count();
        assert_eq!(admitted, 5);
        let wait = limiter.admit(&limit, &spammed).unwrap_err();
        assert!(wait > Duration::ZERO && wait <= Duration::from_secs(1), "{wait:?}");
        let others = (0..40).filter(|i| limiter.admit(&limit, &Jid::phone(&format!("52155{i:08}"))).is_ok()).count();
        assert_eq!(others, 40);
        let more = (0..40).filter(|i| limiter.admit(&limit, &Jid::phone(&format!("52166{i:08}"))).is_ok()).count();
        assert_eq!(more, 5);
    }

    #[tokio::test]
    async fn per_chat_send_order() {
        let (client, _events) = Client::builder().build().await.unwrap();
        let slow_chat = Jid::phone("5215500000001");
        let other_chat = Jid::phone("5215500000002");
        let order = Arc::new(Mutex::new(Vec::new()));

        let video = {
            let (client, chat, order) = (client.clone(), slow_chat.clone(), order.clone());
            tokio::spawn(async move {
                client
                    .in_chat_lane(&chat, async {
                        tokio::time::sleep(Duration::from_millis(150)).await;
                        order.lock().push("video");
                    })
                    .await
            })
        };
        tokio::time::sleep(Duration::from_millis(20)).await;
        let text_same_chat = {
            let (client, chat, order) = (client.clone(), slow_chat.clone(), order.clone());
            tokio::spawn(
                async move { client.in_chat_lane(&chat, async { order.lock().push("text after video") }).await },
            )
        };
        let started = Instant::now();
        client.in_chat_lane(&other_chat, async { order.lock().push("text elsewhere") }).await;
        assert!(started.elapsed() < Duration::from_millis(100));

        video.await.unwrap();
        let ((), queued) = text_same_chat.await.unwrap();
        assert!(queued >= Duration::from_millis(100), "{queued:?}");
        assert_eq!(*order.lock(), vec!["text elsewhere", "video", "text after video"]);
        let ((), nested) = client.in_chat_lane(&slow_chat, client.in_chat_lane(&slow_chat, async {})).await.0;
        assert_eq!(nested, Duration::ZERO);
        assert_eq!(client.stats().open_lanes, 0);
    }

    #[tokio::test]
    async fn handler_panic() {
        let (client, _events) = Client::builder().build().await.unwrap();
        client.contained("a test handler", async { panic!("bug in a command") }).await;
        client.contained("a test handler", async {}).await;
        assert_eq!(client.stats().panics, 1);
    }
}
