//! `obd-policy`: what a diagnostic request is allowed to do.
//!
//! The library judges an application-layer request (the bytes after ISO-TP reassembly, service identifier
//! first) and records the decision. It has no dependency on a CAN bus, an adapter or a simulator, so the
//! same engine sits inside the fake vehicle today and can sit in a relay between an app and a real
//! adapter later. Read-only is the default, `$27` SecurityAccess is refused whatever the configuration,
//! and every decision, passed or refused, lands in the [`AuditLog`].
//!
//! The library implements no seed-to-key algorithm and no key generator, and never will; see the scope
//! limits in the repository's `docs/SCOPE.md`.

mod audit;
mod filter;
mod line;
mod policy;
mod service;

pub use audit::{AuditEntry, AuditLog, PLAIN_KEEP, SENSITIVE_KEEP};
pub use filter::{Action, AuditSink, Context, Filter, LineAction, Rewriter};
pub use line::{classify_line, Line};
pub use policy::{
    Decision, Policy, PolicyError, Reason, Rule, Subs, Verdict, HARD_DENY,
    NRC_REQUEST_OUT_OF_RANGE, NRC_SECURITY_ACCESS_DENIED, NRC_SERVICE_NOT_SUPPORTED,
    NRC_SUBFUNCTION_NOT_SUPPORTED,
};
pub use service::{classify, has_subfunction, payload_is_sensitive, Class, STATE_CHANGING};
