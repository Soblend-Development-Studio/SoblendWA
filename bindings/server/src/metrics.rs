use std::fmt::Write as _;
use std::time::Duration;

use soblendwa::{Spread, Stats};

pub(crate) struct Family {
    name: &'static str,
    kind: &'static str,
    help: &'static str,
    read: fn(&Stats) -> Option<f64>,
}

const fn family(name: &'static str, kind: &'static str, help: &'static str, read: fn(&Stats) -> Option<f64>) -> Family {
    Family { name, kind, help, read }
}

fn seconds(duration: Duration) -> f64 {
    duration.as_secs_f64()
}

pub(crate) const FAMILIES: &[Family] = &[
    family("soblendwa_connected", "gauge", "1 while the session is connected", |s| {
        Some(f64::from(u8::from(s.connected)))
    }),
    family("soblendwa_uptime_seconds", "gauge", "Seconds since the client started", |s| Some(seconds(s.uptime))),
    family("soblendwa_messages_received_total", "counter", "Messages received", |s| Some(s.received as f64)),
    family("soblendwa_messages_sent_total", "counter", "Messages sent", |s| Some(s.sent as f64)),
    family("soblendwa_messages_undecryptable_total", "counter", "Messages that failed to decrypt", |s| {
        Some(s.undecryptable as f64)
    }),
    family("soblendwa_messages_recovered_total", "counter", "Failed messages that arrived after a retry", |s| {
        Some(s.recovered as f64)
    }),
    family(
        "soblendwa_messages_lost",
        "gauge",
        "Failed messages that never came back (drops if one shows up late)",
        |s| Some(s.lost as f64),
    ),
    family("soblendwa_messages_awaiting_retry", "gauge", "Failed messages still waiting for a retry", |s| {
        Some(s.awaiting_recovery as f64)
    }),
    family("soblendwa_messages_duplicate_total", "counter", "Duplicates dropped on arrival", |s| {
        Some(s.duplicates as f64)
    }),
    family("soblendwa_reconnects_total", "counter", "Reconnections", |s| Some(s.reconnects as f64)),
    family("soblendwa_rate_limited_total", "counter", "Sends refused by the send limit", |s| {
        Some(s.rate_limited as f64)
    }),
    family("soblendwa_media_uploaded_total", "counter", "Media uploads", |s| Some(s.media_uploaded as f64)),
    family("soblendwa_media_reused_total", "counter", "Media sent again without uploading", |s| {
        Some(s.media_reused as f64)
    }),
    family("soblendwa_outbox_resent_total", "counter", "Messages resent from the outbox after a drop", |s| {
        Some(s.outbox_resent as f64)
    }),
    family("soblendwa_panics_total", "counter", "Handler panics that were contained", |s| Some(s.panics as f64)),
    family("soblendwa_latency_seconds", "gauge", "Last measured round trip to WhatsApp", |s| s.latency.map(seconds)),
];

fn escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n")
}

fn phases(stats: &Stats) -> [(&'static str, Option<&Spread>); 6] {
    let timings = &stats.send_timings;
    [
        ("queued", Some(&timings.queued)),
        ("upload", timings.upload.as_ref()),
        ("prepare", Some(&timings.prepare)),
        ("encrypt", Some(&timings.encrypt)),
        ("server", timings.server.as_ref()),
        ("total", Some(&timings.total)),
    ]
}

pub(crate) fn render(sessions: &[(String, Stats)], resident_bytes: Option<u64>) -> String {
    let mut out = String::new();
    for family in FAMILIES {
        let _ = writeln!(out, "# HELP {} {}", family.name, family.help);
        let _ = writeln!(out, "# TYPE {} {}", family.name, family.kind);
        for (name, stats) in sessions {
            if let Some(value) = (family.read)(stats) {
                let _ = writeln!(out, "{}{{session=\"{}\"}} {value}", family.name, escape(name));
            }
        }
    }
    let _ = writeln!(out, "# HELP soblendwa_send_phase_seconds Time per send phase over the last 512 sends");
    let _ = writeln!(out, "# TYPE soblendwa_send_phase_seconds gauge");
    for (name, stats) in sessions {
        if stats.send_timings.samples == 0 {
            continue;
        }
        for (phase, spread) in phases(stats) {
            let Some(spread) = spread else { continue };
            for (quantile, value) in [("0.5", spread.p50), ("0.95", spread.p95), ("1", spread.max)] {
                let _ = writeln!(
                    out,
                    "soblendwa_send_phase_seconds{{session=\"{}\",phase=\"{phase}\",quantile=\"{quantile}\"}} {}",
                    escape(name),
                    seconds(value)
                );
            }
        }
    }
    if let Some(bytes) = resident_bytes {
        let _ = writeln!(out, "# HELP soblendwa_process_resident_memory_bytes Resident memory of the server");
        let _ = writeln!(out, "# TYPE soblendwa_process_resident_memory_bytes gauge");
        let _ = writeln!(out, "soblendwa_process_resident_memory_bytes {bytes}");
    }
    out
}

pub(crate) fn resident_bytes() -> Option<u64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    let line = status.lines().find(|line| line.starts_with("VmRSS:"))?;
    let kb: u64 = line.split_whitespace().nth(1)?.parse().ok()?;
    Some(kb * 1024)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prometheus_text() {
        let stats = Stats { received: 7, connected: true, ..Default::default() };
        let text = render(&[("ventas \"1\"".to_owned(), stats)], Some(4096));
        assert!(text.contains("# TYPE soblendwa_messages_received_total counter\n"), "{text}");
        assert!(text.contains("soblendwa_messages_received_total{session=\"ventas \\\"1\\\"\"} 7\n"), "{text}");
        assert!(text.contains("soblendwa_connected{session=\"ventas \\\"1\\\"\"} 1\n"), "{text}");
        assert!(!text.contains("soblendwa_latency_seconds{"), "no latency measured yet, so no sample");
        assert!(!text.contains("soblendwa_send_phase_seconds{"), "no sends yet");
        assert!(text.ends_with("soblendwa_process_resident_memory_bytes 4096\n"), "{text}");
    }
}
