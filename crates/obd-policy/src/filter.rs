//! The stable relay-facing API: `Filter::decide(request_bytes, context) -> Action`, an audit sink, and a
//! line-level wrapper for ELM327 text streams.
//!
//! A relay sits between two byte streams (an app side and an adapter side). For each request it calls
//! [`Filter::decide`] (or [`Filter::decide_line`] for ELM text) and then forwards, answers itself, or
//! forwards a rewritten request. The filter never does I/O.

use crate::audit::{AuditEntry, AuditLog};
use crate::line::{classify_line, Line};
use crate::policy::{Decision, Policy, Verdict};

/// Where a request came from and where it is going; recorded in the audit log.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Context {
    /// Free-form label, e.g. the arbitration ID as hex ("7E0") or a stream name.
    pub origin: String,
}

impl Context {
    pub fn new(origin: impl Into<String>) -> Context {
        Context {
            origin: origin.into(),
        }
    }
}

/// The outcome for one application-layer request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    /// Forward unchanged.
    Allow,
    /// Do not forward; answer the requester with `7F <sid> <nrc>`.
    Deny { sid: Option<u8>, nrc: u8 },
    /// Forward these bytes instead.
    Rewrite(Vec<u8>),
}

impl Action {
    /// The negative response PDU for a `Deny`, `None` for the others or for an empty request (nothing to
    /// answer).
    pub fn negative_response(&self) -> Option<Vec<u8>> {
        match self {
            Action::Deny { sid: Some(s), nrc } => Some(vec![0x7F, *s, *nrc]),
            _ => None,
        }
    }
}

/// What to do with one ELM327/STN text line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LineAction {
    /// Forward the line as typed (adapter settings, allowed requests, blank lines).
    Forward,
    /// Do not forward; answer the app with these response bytes (hex with spaces, no prompt).
    Respond(String),
    /// Do not forward and answer `?`: a command that bypasses the request path, or malformed input.
    Refuse,
    /// Forward this line instead of the one received.
    Rewrite(String),
}

/// Somewhere decisions are recorded. Implement for a file, a channel, a socket.
pub trait AuditSink: Send {
    fn record(&mut self, entry: &AuditEntry);
}

/// A rewriter may replace an allowed request. Return `None` to leave it alone.
pub type Rewriter = Box<dyn FnMut(&[u8], &Context) -> Option<Vec<u8>> + Send>;

pub struct Filter {
    policy: Policy,
    log: AuditLog,
    sinks: Vec<Box<dyn AuditSink>>,
    rewriter: Option<Rewriter>,
}

impl Filter {
    pub fn new(policy: Policy) -> Filter {
        Filter {
            policy,
            log: AuditLog::new(1024),
            sinks: Vec::new(),
            rewriter: None,
        }
    }

    pub fn with_sink(mut self, sink: Box<dyn AuditSink>) -> Filter {
        self.sinks.push(sink);
        self
    }

    /// Install a rewriter. A rewritten request is judged again: a rewriter cannot smuggle a refused service
    /// past the policy.
    pub fn with_rewriter(mut self, r: Rewriter) -> Filter {
        self.rewriter = Some(r);
        self
    }

    pub fn audit(&self) -> &AuditLog {
        &self.log
    }

    pub fn policy(&self) -> &Policy {
        &self.policy
    }

    fn judge(&mut self, pdu: &[u8], ctx: &Context) -> Decision {
        let d = self.policy.evaluate(pdu);
        let e = self.log.record(&ctx.origin, pdu, &d).clone();
        for s in &mut self.sinks {
            s.record(&e);
        }
        d
    }

    /// Decide one application-layer request (service identifier first).
    pub fn decide(&mut self, request: &[u8], ctx: &Context) -> Action {
        let d = self.judge(request, ctx);
        if let Verdict::Deny(_) = d.verdict {
            return Action::Deny {
                sid: d.sid,
                nrc: d.nrc.unwrap_or(crate::NRC_SECURITY_ACCESS_DENIED),
            };
        }
        if let Some(rw) = self.rewriter.as_mut() {
            if let Some(new) = rw(request, ctx) {
                let d2 = self.judge(&new, ctx);
                return match d2.verdict {
                    Verdict::Allow => Action::Rewrite(new),
                    Verdict::Deny(_) => Action::Deny {
                        sid: d2.sid,
                        nrc: d2.nrc.unwrap_or(crate::NRC_SECURITY_ACCESS_DENIED),
                    },
                };
            }
        }
        Action::Allow
    }

    /// Decide one ELM327/STN text line.
    pub fn decide_line(&mut self, line: &str, ctx: &Context) -> LineAction {
        match classify_line(line) {
            Line::Empty | Line::Adapter(_) => LineAction::Forward,
            Line::Bypass(_) | Line::Malformed => LineAction::Refuse,
            Line::Request { pdu, max_responses } => match self.decide(&pdu, ctx) {
                Action::Allow => LineAction::Forward,
                Action::Rewrite(new) => {
                    let mut s: String = new.iter().map(|b| format!("{b:02X}")).collect();
                    if let Some(m) = max_responses {
                        s.push_str(&format!("{m:X}"));
                    }
                    LineAction::Rewrite(s)
                }
                a @ Action::Deny { .. } => match a.negative_response() {
                    Some(nr) => LineAction::Respond(
                        nr.iter()
                            .map(|b| format!("{b:02X}"))
                            .collect::<Vec<_>>()
                            .join(" "),
                    ),
                    None => LineAction::Refuse,
                },
            },
        }
    }
}
