//! The policy engine: allow rules, deny rules, a hard floor, and a verdict for every request.

use crate::service::{classify, has_subfunction, Class};
use std::fmt;

/// Services no configuration can admit. `0x27` SecurityAccess is the one the project scope excludes
/// (volt `docs/OBD-CONFIG-SCOPE.md`): the seed-to-key material is protected and the unlock is the gate to
/// every coding write. Refusing it is not a default, it is the floor.
pub const HARD_DENY: &[u8] = &[0x27];

/// Why a request was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reason {
    /// Zero-length request.
    Empty,
    /// The service is in [`HARD_DENY`].
    HardFloor,
    /// An explicit deny rule matched.
    ExplicitDeny,
    /// A DID deny range matched.
    DidDenied,
    /// There is an allow rule for the service, but not for this sub-function (or it is missing).
    SubfunctionNotAllowed,
    /// No allow rule matched.
    NotAllowlisted,
}

impl Reason {
    pub fn as_str(self) -> &'static str {
        match self {
            Reason::Empty => "empty",
            Reason::HardFloor => "hard-floor",
            Reason::ExplicitDeny => "explicit-deny",
            Reason::DidDenied => "did-denied",
            Reason::SubfunctionNotAllowed => "subfunction-not-allowed",
            Reason::NotAllowlisted => "not-allowlisted",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    Allow,
    Deny(Reason),
}

/// The result of [`Policy::evaluate`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Decision {
    pub verdict: Verdict,
    pub sid: Option<u8>,
    /// Sub-function with the suppress-positive-response bit (0x80) cleared, when the service has one.
    pub sub: Option<u8>,
    pub class: Class,
    /// Negative response code to send when refused.
    pub nrc: Option<u8>,
}

impl Decision {
    pub fn allowed(&self) -> bool {
        self.verdict == Verdict::Allow
    }
}

/// Which sub-functions a rule covers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Subs {
    Any,
    Only(Vec<u8>),
}

/// A rule over a service identifier and optionally its sub-function.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rule {
    pub sid: u8,
    pub subs: Subs,
}

impl Rule {
    fn covers_sid(&self, sid: u8) -> bool {
        self.sid == sid
    }
    fn matches(&self, sid: u8, sub: Option<u8>) -> bool {
        self.covers_sid(sid)
            && match (&self.subs, sub) {
                (Subs::Any, _) => true,
                (Subs::Only(list), Some(s)) => list.contains(&s),
                (Subs::Only(_), None) => false,
            }
    }
}

/// An error building or parsing a policy.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PolicyError(pub String);

impl fmt::Display for PolicyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for PolicyError {}

/// NRC for a refused state-changing service. `0x33` securityAccessDenied. A simulator choice: a real
/// module would answer according to its own gating. `[unread]`.
pub const NRC_SECURITY_ACCESS_DENIED: u8 = 0x33;
/// NRC for a service not in the table. `0x11` serviceNotSupported. `[unread]`.
pub const NRC_SERVICE_NOT_SUPPORTED: u8 = 0x11;
/// NRC for a sub-function not allowed. `0x12` subFunctionNotSupported. `[unread]`.
pub const NRC_SUBFUNCTION_NOT_SUPPORTED: u8 = 0x12;
/// NRC for a DID range that is denied. `0x31` requestOutOfRange: the code the volt capture shows for `22`.
pub const NRC_REQUEST_OUT_OF_RANGE: u8 = 0x31;

/// Allow rules, deny rules and DID deny ranges. Deny wins over allow; anything not allowed is denied.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Policy {
    allow: Vec<Rule>,
    deny: Vec<Rule>,
    deny_dids: Vec<(u16, u16)>,
    /// Exact requests admitted on top of the service rules. A different payload of the same service
    /// stays refused.
    allow_pdus: Vec<Vec<u8>>,
    /// Overrides the NRC for refusals when set.
    nrc_override: Option<u8>,
}

impl Default for Policy {
    fn default() -> Self {
        Policy::read_only()
    }
}

impl Policy {
    /// Allow nothing. Every request is refused.
    pub fn deny_all() -> Policy {
        Policy {
            allow: Vec::new(),
            deny: Vec::new(),
            deny_dids: Vec::new(),
            allow_pdus: Vec::new(),
            nrc_override: None,
        }
    }

    /// The default: the read side only.
    ///
    /// OBD-II modes 01, 02, 03, 05, 06, 07, 09, 0A; UDS `10` with sub-function 01 or 03 (default and
    /// extended session), `19`, `22`, `3E`; GMLAN `1A`. Nothing else.
    pub fn read_only() -> Policy {
        let mut p = Policy::deny_all();
        for sid in [
            0x01, 0x02, 0x03, 0x05, 0x06, 0x07, 0x09, 0x0A, 0x19, 0x1A, 0x22, 0x3E,
        ] {
            p.allow.push(Rule {
                sid,
                subs: Subs::Any,
            });
        }
        p.allow.push(Rule {
            sid: 0x10,
            subs: Subs::Only(vec![0x01, 0x03]),
        });
        p
    }

    /// Add an allow rule. Fails for a service in [`HARD_DENY`].
    pub fn allow(&mut self, sid: u8, subs: Subs) -> Result<(), PolicyError> {
        if HARD_DENY.contains(&sid) {
            return Err(PolicyError(format!(
                "service {sid:02X} is refused unconditionally and cannot be allowed"
            )));
        }
        self.allow.push(Rule { sid, subs });
        Ok(())
    }

    /// Admit this exact request and no other payload of the same service.
    ///
    /// Fails for an empty request or a service in [`HARD_DENY`]. A lab file cannot
    /// use this to let `$27` through.
    pub fn allow_pdu(&mut self, pdu: &[u8]) -> Result<(), PolicyError> {
        let Some(&sid) = pdu.first() else {
            return Err(PolicyError("a request needs a service identifier".into()));
        };
        if HARD_DENY.contains(&sid) {
            return Err(PolicyError(format!(
                "service {sid:02X} is refused unconditionally and cannot be allowed"
            )));
        }
        self.allow_pdus.push(pdu.to_vec());
        Ok(())
    }

    /// Add a deny rule. Deny rules win over allow rules.
    pub fn deny(&mut self, sid: u8, subs: Subs) {
        self.deny.push(Rule { sid, subs });
    }

    /// Refuse reads of DIDs in `lo..=hi` through `22` and `1A`.
    pub fn deny_did_range(&mut self, lo: u16, hi: u16) {
        self.deny_dids.push((lo.min(hi), lo.max(hi)));
    }

    pub fn set_refusal_nrc(&mut self, nrc: u8) {
        self.nrc_override = Some(nrc);
    }

    /// Parse a policy file.
    ///
    /// ```text
    /// # comment
    /// preset read-only | none     # must come first if present; default is read-only
    /// allow 22                    # any sub-function
    /// allow 10 01,03              # only these sub-functions
    /// allow-pdu AE3BFF01010101   # this request only
    /// deny 3E
    /// deny-did F100-F1FF
    /// nrc 22
    /// ```
    pub fn parse(text: &str) -> Result<Policy, PolicyError> {
        let mut policy = Policy::read_only();
        for (n, raw) in text.lines().enumerate() {
            let line = raw.split('#').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            let err = |m: &str| PolicyError(format!("line {}: {m}", n + 1));
            let mut it = line.split_whitespace();
            let verb = it.next().unwrap_or("");
            match verb {
                "preset" => match it.next() {
                    Some("read-only") => policy = Policy::read_only(),
                    Some("none") => policy = Policy::deny_all(),
                    _ => return Err(err("preset must be read-only or none")),
                },
                "allow" | "deny" => {
                    let sid = hex_u8(it.next().ok_or_else(|| err("missing service"))?)
                        .ok_or_else(|| err("service is not two hex digits"))?;
                    let subs = match it.next() {
                        None => Subs::Any,
                        Some(list) => Subs::Only(
                            list.split(',')
                                .map(hex_u8)
                                .collect::<Option<Vec<_>>>()
                                .ok_or_else(|| err("bad sub-function list"))?,
                        ),
                    };
                    if verb == "allow" {
                        policy.allow(sid, subs).map_err(|e| err(&e.0))?;
                    } else {
                        policy.deny(sid, subs);
                    }
                }
                "allow-pdu" => {
                    let hex = it.next().ok_or_else(|| err("missing request"))?;
                    let bytes = hex_bytes(hex).ok_or_else(|| err("request is not even hex"))?;
                    policy.allow_pdu(&bytes).map_err(|e| err(&e.0))?;
                }
                "deny-did" => {
                    let range = it.next().ok_or_else(|| err("missing range"))?;
                    let (lo, hi) = range.split_once('-').unwrap_or((range, range));
                    let lo = u16::from_str_radix(lo, 16).map_err(|_| err("bad DID"))?;
                    let hi = u16::from_str_radix(hi, 16).map_err(|_| err("bad DID"))?;
                    policy.deny_did_range(lo, hi);
                }
                "nrc" => {
                    let v = hex_u8(it.next().ok_or_else(|| err("missing code"))?)
                        .ok_or_else(|| err("bad code"))?;
                    policy.set_refusal_nrc(v);
                }
                _ => return Err(err("unknown directive")),
            }
        }
        Ok(policy)
    }

    /// Decide one request. `pdu` is the application-layer request: service identifier first.
    pub fn evaluate(&self, pdu: &[u8]) -> Decision {
        let Some(&sid) = pdu.first() else {
            return Decision {
                verdict: Verdict::Deny(Reason::Empty),
                sid: None,
                sub: None,
                class: Class::Unknown,
                nrc: Some(self.nrc_override.unwrap_or(NRC_SERVICE_NOT_SUPPORTED)),
            };
        };
        let class = classify(sid);
        let sub = if has_subfunction(sid) {
            pdu.get(1).map(|b| b & 0x7F)
        } else {
            None
        };
        let verdict = self.verdict(sid, sub, pdu);
        let nrc = match verdict {
            Verdict::Allow => None,
            Verdict::Deny(r) => Some(self.nrc_override.unwrap_or(match r {
                Reason::SubfunctionNotAllowed => NRC_SUBFUNCTION_NOT_SUPPORTED,
                Reason::DidDenied => NRC_REQUEST_OUT_OF_RANGE,
                Reason::NotAllowlisted if class == Class::Unknown => NRC_SERVICE_NOT_SUPPORTED,
                _ => NRC_SECURITY_ACCESS_DENIED,
            })),
        };
        Decision {
            verdict,
            sid: Some(sid),
            sub,
            class,
            nrc,
        }
    }

    fn verdict(&self, sid: u8, sub: Option<u8>, pdu: &[u8]) -> Verdict {
        if HARD_DENY.contains(&sid) {
            return Verdict::Deny(Reason::HardFloor);
        }
        if self.deny.iter().any(|r| r.matches(sid, sub)) {
            return Verdict::Deny(Reason::ExplicitDeny);
        }
        if matches!(sid, 0x22 | 0x1A) && !self.deny_dids.is_empty() {
            // 22 carries two-byte DIDs; 1A carries one-byte local identifiers, widened here.
            let hit = if sid == 0x22 {
                pdu[1..]
                    .chunks(2)
                    .filter(|c| c.len() == 2)
                    .map(|c| u16::from_be_bytes([c[0], c[1]]))
                    .any(|d| {
                        self.deny_dids
                            .iter()
                            .any(|&(lo, hi)| (lo..=hi).contains(&d))
                    })
            } else {
                pdu[1..].iter().any(|&b| {
                    self.deny_dids
                        .iter()
                        .any(|&(lo, hi)| (lo..=hi).contains(&u16::from(b)))
                })
            };
            if hit {
                return Verdict::Deny(Reason::DidDenied);
            }
        }
        if self.allow.iter().any(|r| r.matches(sid, sub)) {
            return Verdict::Allow;
        }
        if self.allow_pdus.iter().any(|p| p == pdu) {
            return Verdict::Allow;
        }
        if self.allow.iter().any(|r| r.covers_sid(sid)) {
            return Verdict::Deny(Reason::SubfunctionNotAllowed);
        }
        Verdict::Deny(Reason::NotAllowlisted)
    }
}

fn hex_u8(s: &str) -> Option<u8> {
    if s.len() == 2 {
        u8::from_str_radix(s, 16).ok()
    } else {
        None
    }
}

fn hex_bytes(s: &str) -> Option<Vec<u8>> {
    if s.is_empty() || !s.len().is_multiple_of(2) {
        return None;
    }
    (0..s.len() / 2)
        .map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).ok())
        .collect()
}
