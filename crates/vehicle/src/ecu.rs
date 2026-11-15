//! One simulated ECU: an ISO-TP endpoint plus the OBD-II and UDS read services.
//!
//! Every complete request goes through the policy before any service code runs, and the decision is
//! written to the audit log. The ECU implements no write service: a state-changing request that a
//! configuration chose to allow still gets `serviceNotSupported`.

use crate::fault::Effects;
use crate::profile::{DtcKind, EcuSpec};
use isotp::{
    segment, st_min_us, CanFrame, FlowControl, FlowStatus, Reassembler, Rx, RxError, MAX_LEN,
};
use obd_policy::{AuditLog, Policy};
use std::collections::VecDeque;

/// The OBD-II functional (broadcast) request ID for 11-bit CAN.
pub const FUNCTIONAL_ID: u32 = 0x7DF;

/// Milliseconds between flow-control wait frames when the fault injects them.
const WAIT_STEP_MS: u32 = 50;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Timed {
    pub at_ms: u32,
    pub frame: CanFrame,
}

enum Reply {
    Pos(Vec<u8>),
    Neg(u8),
    None,
}

pub struct Ctx<'a> {
    pub policy: &'a Policy,
    pub audit: &'a mut AuditLog,
    pub now_ms: u64,
    pub s3_ms: u64,
    pub fx: &'a Effects,
}

pub struct Ecu {
    pub spec: EcuSpec,
    session: u8,
    last_ms: u64,
    rx: Reassembler,
    tx: Option<VecDeque<Vec<u8>>>,
    /// Block size this ECU asked for in its last flow control, and consecutive frames seen in the block.
    rx_block: (u8, u8),
}

impl Ecu {
    pub fn new(spec: EcuSpec) -> Ecu {
        Ecu {
            spec,
            session: 0x01,
            last_ms: 0,
            rx: Reassembler::new(),
            tx: None,
            rx_block: (0, 0),
        }
    }

    /// The active diagnostic session: 1 default, 3 extended.
    pub fn session(&self) -> u8 {
        self.session
    }

    fn frame(&self, at_ms: u32, mut data: Vec<u8>) -> Timed {
        if let Some(p) = self.spec.pad {
            data.resize(8, p);
        }
        Timed {
            at_ms,
            frame: CanFrame {
                id: self.spec.resp_id,
                data,
            },
        }
    }

    /// React to one frame from the tester.
    pub fn on_frame(&mut self, f: &CanFrame, ctx: &mut Ctx<'_>) -> Vec<Timed> {
        let functional = f.id == FUNCTIONAL_ID;
        if (functional && !self.spec.functional) || (!functional && f.id != self.spec.req_id) {
            return Vec::new();
        }
        let lat = self.spec.latency_ms + ctx.fx.delay_ms;
        match self.rx.push(&f.data, MAX_LEN) {
            Ok(Rx::Complete(pdu)) => {
                if ctx.fx.silent {
                    return Vec::new();
                }
                let replies = self.handle(f.id, &pdu, functional, ctx);
                self.emit(replies, lat)
            }
            Ok(Rx::NeedFlow { .. }) => {
                if functional || ctx.fx.silent {
                    self.rx.reset();
                    return Vec::new();
                }
                if ctx.fx.fc_overflow {
                    self.rx.reset();
                    let fc = FlowControl {
                        status: FlowStatus::Overflow,
                        block_size: 0,
                        st_min: 0,
                    };
                    return vec![self.frame(lat, fc.encode())];
                }
                let wait = FlowControl {
                    status: FlowStatus::Wait,
                    block_size: 0,
                    st_min: 0,
                };
                let mut out: Vec<Timed> = (0..ctx.fx.fc_wait)
                    .map(|i| self.frame(lat + i * WAIT_STEP_MS, wait.encode()))
                    .collect();
                let cts = ctx.fx.fc_block.unwrap_or(FlowControl::CTS);
                self.rx_block = (cts.block_size, 0);
                out.push(self.frame(lat + ctx.fx.fc_wait * WAIT_STEP_MS, cts.encode()));
                out
            }
            Ok(Rx::More) => {
                // With a block size, the receiver owes a new flow control after every block.
                let (bs, seen) = self.rx_block;
                if bs == 0 {
                    return Vec::new();
                }
                self.rx_block.1 = seen + 1;
                if self.rx_block.1 < bs {
                    return Vec::new();
                }
                self.rx_block.1 = 0;
                let cts = ctx.fx.fc_block.unwrap_or(FlowControl::CTS);
                vec![self.frame(lat, cts.encode())]
            }
            Ok(Rx::Flow(fc)) => self.on_flow(fc),
            Err(RxError::Overflow(_)) => {
                let fc = FlowControl {
                    status: FlowStatus::Overflow,
                    block_size: 0,
                    st_min: 0,
                };
                vec![self.frame(lat, fc.encode())]
            }
            Err(_) => Vec::new(),
        }
    }

    fn on_flow(&mut self, fc: FlowControl) -> Vec<Timed> {
        let Some(rest) = self.tx.as_mut() else {
            return Vec::new();
        };
        match fc.status {
            FlowStatus::Wait => Vec::new(),
            FlowStatus::Overflow => {
                self.tx = None;
                Vec::new()
            }
            FlowStatus::ContinueToSend => {
                let n = if fc.block_size == 0 {
                    rest.len()
                } else {
                    usize::from(fc.block_size).min(rest.len())
                };
                let step = st_min_us(fc.st_min);
                let frames: Vec<Vec<u8>> = rest.drain(..n).collect();
                if rest.is_empty() {
                    self.tx = None;
                }
                frames
                    .into_iter()
                    .enumerate()
                    .map(|(i, d)| self.frame(1 + (i as u32 * step) / 1000, d))
                    .collect()
            }
        }
    }

    fn emit(&mut self, replies: Vec<(Vec<u8>, u32)>, lat: u32) -> Vec<Timed> {
        let mut out = Vec::new();
        self.tx = None;
        for (pdu, extra) in replies {
            let seg = segment(&pdu, self.spec.pad).expect("replies are 1..=4095 bytes");
            out.push(self.frame(lat + extra, seg.first));
            if !seg.rest.is_empty() {
                self.tx = Some(seg.rest.into());
            }
        }
        out
    }

    fn handle(
        &mut self,
        req_id: u32,
        pdu: &[u8],
        functional: bool,
        ctx: &mut Ctx<'_>,
    ) -> Vec<(Vec<u8>, u32)> {
        if self.session != 0x01 && ctx.now_ms.saturating_sub(self.last_ms) > ctx.s3_ms {
            self.session = 0x01;
        }
        self.last_ms = ctx.now_ms;
        let sid = pdu[0];
        let decision = ctx.policy.evaluate(pdu);
        ctx.audit.record(&format!("{req_id:03X}"), pdu, &decision);
        let reply = match decision.nrc {
            Some(nrc) => Reply::Neg(nrc),
            None => self.dispatch(pdu),
        };
        let reply = match reply {
            // Functional requests are not answered with these three codes (the usual UDS rule; [unread]).
            Reply::Neg(0x11 | 0x12 | 0x31) if functional => Reply::None,
            r => r,
        };
        let bytes = match reply {
            Reply::Pos(p) => p,
            Reply::Neg(n) => vec![0x7F, sid, n],
            Reply::None => return Vec::new(),
        };
        match ctx.fx.pending_ms {
            Some(ms) => vec![(vec![0x7F, sid, 0x78], 0), (bytes, ms)],
            None => vec![(bytes, 0)],
        }
    }

    fn dispatch(&mut self, pdu: &[u8]) -> Reply {
        let sid = pdu[0];
        match sid {
            0x01 => self.mode01(pdu),
            0x02 => self.mode02(pdu),
            0x03 => self.dtc_list(0x43, DtcKind::Stored),
            0x07 => self.dtc_list(0x47, DtcKind::Pending),
            0x0A => self.dtc_list(0x4A, DtcKind::Permanent),
            0x06 => self.mode06(pdu),
            0x09 => self.mode09(pdu),
            0x05 => Reply::None,
            0x10 => self.session_control(pdu),
            0x19 => self.read_dtc(pdu),
            0x22 => self.read_did(pdu),
            0x1A => match pdu {
                [_, id] => match self.spec.lids.get(id) {
                    Some(d) => {
                        let mut out = vec![0x5A, *id];
                        out.extend_from_slice(d);
                        Reply::Pos(out)
                    }
                    None => Reply::Neg(0x31),
                },
                _ => Reply::Neg(0x13),
            },
            0x3E => match pdu {
                [_, s] if s & 0x7F == 0 => suppressable(*s, vec![0x7E, 0x00]),
                [_, _] => Reply::Neg(0x12),
                _ => Reply::Neg(0x13),
            },
            _ => Reply::Neg(0x11),
        }
    }

    fn mode01(&self, pdu: &[u8]) -> Reply {
        if pdu.len() < 2 || pdu.len() > 7 {
            return Reply::Neg(0x13);
        }
        let mut out = vec![0x41];
        for &pid in &pdu[1..] {
            append_pid(&mut out, pid, &self.spec.pids);
        }
        if out.len() == 1 {
            Reply::None
        } else {
            Reply::Pos(out)
        }
    }

    fn mode02(&self, pdu: &[u8]) -> Reply {
        match pdu {
            [_, pid, 0x00] => {
                let mut out = vec![0x42];
                append_pid(&mut out, *pid, &self.spec.freeze);
                // insert the frame number after the PID
                if out.len() == 1 {
                    return Reply::None;
                }
                out.insert(2, 0x00);
                Reply::Pos(out)
            }
            [_, _, _] => Reply::None,
            _ => Reply::Neg(0x13),
        }
    }

    fn dtc_list(&self, positive: u8, kind: DtcKind) -> Reply {
        let list: Vec<_> = self.spec.dtcs.iter().filter(|d| d.kind == kind).collect();
        let mut out = vec![positive, list.len() as u8];
        for d in list {
            out.extend_from_slice(&d.code);
        }
        Reply::Pos(out)
    }

    fn mode06(&self, pdu: &[u8]) -> Reply {
        let [_, mid] = pdu else {
            return Reply::Neg(0x13);
        };
        let mut out = vec![0x46, *mid];
        if mid % 0x20 == 0 {
            if *mid != 0 && !self.spec.mode06.keys().any(|k| k > mid) {
                return Reply::None;
            }
            out.extend_from_slice(&bitmap(self.spec.mode06.keys().copied(), *mid));
        } else if let Some(d) = self.spec.mode06.get(mid) {
            out.extend_from_slice(d);
        } else {
            return Reply::None;
        }
        Reply::Pos(out)
    }

    fn mode09(&self, pdu: &[u8]) -> Reply {
        let [_, pid] = pdu else {
            return Reply::Neg(0x13);
        };
        let mut out = vec![0x49, *pid];
        if pid % 0x20 == 0 {
            if *pid != 0 && !self.spec.info.keys().any(|k| k > pid) {
                return Reply::None;
            }
            out.extend_from_slice(&bitmap(self.spec.info.keys().copied(), *pid));
        } else if let Some(d) = self.spec.info.get(pid) {
            out.extend_from_slice(d);
        } else {
            return Reply::None;
        }
        Reply::Pos(out)
    }

    fn session_control(&mut self, pdu: &[u8]) -> Reply {
        let [_, s] = pdu else {
            return Reply::Neg(0x13);
        };
        match s & 0x7F {
            sub @ (0x01 | 0x03) => {
                self.session = sub;
                // sub-function, P2 = 50 ms, P2* = 5000 ms in 10 ms units. [unread]
                suppressable(*s, vec![0x50, sub, 0x00, 0x32, 0x01, 0xF4])
            }
            _ => Reply::Neg(0x12),
        }
    }

    fn read_dtc(&self, pdu: &[u8]) -> Reply {
        let (Some(&sub), Some(&mask)) = (pdu.get(1), pdu.get(2)) else {
            return Reply::Neg(0x13);
        };
        let matching: Vec<_> = self
            .spec
            .dtcs
            .iter()
            .filter(|d| d.status & mask != 0)
            .collect();
        match sub & 0x7F {
            0x01 => {
                let n = matching.len() as u16;
                let mut out = vec![0x59, 0x01, 0xFF, 0x01];
                out.extend_from_slice(&n.to_be_bytes());
                suppressable(sub, out)
            }
            0x02 => {
                let mut out = vec![0x59, 0x02, 0xFF];
                for d in matching {
                    out.extend_from_slice(&[d.code[0], d.code[1], 0x00, d.status]);
                }
                suppressable(sub, out)
            }
            _ => Reply::Neg(0x12),
        }
    }

    fn read_did(&self, pdu: &[u8]) -> Reply {
        if pdu.len() < 3 || pdu.len().is_multiple_of(2) {
            return Reply::Neg(0x13);
        }
        let mut out = vec![0x62];
        for c in pdu[1..].chunks(2) {
            let did = u16::from_be_bytes([c[0], c[1]]);
            match self.spec.dids.get(&did) {
                Some(d) if !d.extended_only || self.session == 0x03 => {
                    out.extend_from_slice(&did.to_be_bytes());
                    out.extend_from_slice(&d.data);
                }
                _ => return Reply::Neg(0x31),
            }
        }
        Reply::Pos(out)
    }
}

fn suppressable(sub: u8, bytes: Vec<u8>) -> Reply {
    if sub & 0x80 != 0 {
        Reply::None
    } else {
        Reply::Pos(bytes)
    }
}

/// The 4-byte supported-IDs bitmap for the 0x20-wide range starting after `base`.
pub fn bitmap(keys: impl Iterator<Item = u8>, base: u8) -> [u8; 4] {
    let mut v: u32 = 0;
    for k in keys {
        let off = i32::from(k) - i32::from(base);
        if (1..=0x20).contains(&off) {
            v |= 1 << (32 - off);
        } else if off > 0x20 {
            v |= 1;
        }
    }
    v.to_be_bytes()
}

fn append_pid(out: &mut Vec<u8>, pid: u8, table: &std::collections::BTreeMap<u8, Vec<u8>>) {
    if pid.is_multiple_of(0x20) {
        if pid != 0 && !table.keys().any(|k| *k > pid) {
            return;
        }
        out.push(pid);
        out.extend_from_slice(&bitmap(table.keys().copied(), pid));
    } else if let Some(d) = table.get(&pid) {
        out.push(pid);
        out.extend_from_slice(d);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bitmap_sets_the_msb_for_the_first_pid_and_chains_to_the_next_range() {
        assert_eq!(bitmap([0x01u8].into_iter(), 0), [0x80, 0, 0, 0]);
        assert_eq!(
            bitmap([0x05u8, 0x0C, 0x0D].into_iter(), 0),
            [0x08, 0x18, 0, 0]
        );
        assert_eq!(bitmap([0x20u8].into_iter(), 0), [0, 0, 0, 0x01]);
        assert_eq!(bitmap([0x42u8].into_iter(), 0), [0, 0, 0, 0x01]);
        assert_eq!(bitmap([0x42u8].into_iter(), 0x40), [0x40, 0, 0, 0]);
    }
}
