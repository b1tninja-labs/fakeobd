//! The audit log: one record for every request the policy sees, passed or refused.

use crate::policy::{Decision, Verdict};
use crate::service::payload_is_sensitive;
use std::collections::VecDeque;
use std::io::Write;

/// Bytes of a sensitive request kept in the log: the service, the sub-function and one more.
pub const SENSITIVE_KEEP: usize = 3;
/// Bytes of any other request kept in the log.
pub const PLAIN_KEEP: usize = 16;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuditEntry {
    pub seq: u64,
    /// Where the request was addressed, e.g. the arbitration ID as hex.
    pub origin: String,
    pub sid: Option<u8>,
    pub sub: Option<u8>,
    pub allowed: bool,
    pub reason: Option<&'static str>,
    pub nrc: Option<u8>,
    /// Hex of the request, truncated (see [`SENSITIVE_KEEP`], [`PLAIN_KEEP`]).
    pub request: String,
    /// Length of the whole request.
    pub len: usize,
}

impl AuditEntry {
    /// One JSON object, no trailing newline.
    pub fn to_json(&self) -> String {
        let opt_hex = |v: Option<u8>| v.map_or("null".to_string(), |b| format!("\"{b:02X}\""));
        format!(
            "{{\"seq\":{},\"origin\":\"{}\",\"sid\":{},\"sub\":{},\"verdict\":\"{}\",\"reason\":{},\"nrc\":{},\"req\":\"{}\",\"len\":{}}}",
            self.seq,
            self.origin.replace(['"', '\\'], "?"),
            opt_hex(self.sid),
            opt_hex(self.sub),
            if self.allowed { "allow" } else { "deny" },
            self.reason.map_or("null".to_string(), |r| format!("\"{r}\"")),
            opt_hex(self.nrc),
            self.request,
            self.len
        )
    }
}

/// A bounded in-memory log with an optional JSON-lines sink.
pub struct AuditLog {
    entries: VecDeque<AuditEntry>,
    cap: usize,
    seq: u64,
    sink: Option<Box<dyn Write + Send>>,
}

impl std::fmt::Debug for AuditLog {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "AuditLog({} entries, seq {})",
            self.entries.len(),
            self.seq
        )
    }
}

impl AuditLog {
    pub fn new(cap: usize) -> AuditLog {
        AuditLog {
            entries: VecDeque::new(),
            cap: cap.max(1),
            seq: 0,
            sink: None,
        }
    }

    /// Also write every entry to `sink` as one JSON line. Write errors are ignored: an audit sink must not
    /// be able to turn a refusal into a different outcome.
    pub fn with_sink(mut self, sink: Box<dyn Write + Send>) -> AuditLog {
        self.sink = Some(sink);
        self
    }

    pub fn record(&mut self, origin: &str, pdu: &[u8], d: &Decision) -> &AuditEntry {
        self.seq += 1;
        let keep = match d.sid {
            Some(s) if payload_is_sensitive(s) => SENSITIVE_KEEP,
            _ => PLAIN_KEEP,
        };
        let request = pdu.iter().take(keep).map(|b| format!("{b:02X}")).collect();
        let (allowed, reason) = match d.verdict {
            Verdict::Allow => (true, None),
            Verdict::Deny(r) => (false, Some(r.as_str())),
        };
        let entry = AuditEntry {
            seq: self.seq,
            origin: origin.to_string(),
            sid: d.sid,
            sub: d.sub,
            allowed,
            reason,
            nrc: d.nrc,
            request,
            len: pdu.len(),
        };
        if let Some(sink) = self.sink.as_mut() {
            let _ = writeln!(sink, "{}", entry.to_json());
            let _ = sink.flush();
        }
        if self.entries.len() == self.cap {
            self.entries.pop_front();
        }
        self.entries.push_back(entry);
        self.entries.back().expect("just pushed")
    }

    pub fn entries(&self) -> impl Iterator<Item = &AuditEntry> {
        self.entries.iter()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Total records ever made, including ones the ring has dropped.
    pub fn total(&self) -> u64 {
        self.seq
    }

    pub fn denied(&self) -> usize {
        self.entries.iter().filter(|e| !e.allowed).count()
    }
}
