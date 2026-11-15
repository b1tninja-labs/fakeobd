//! The simulated CAN bus: the tester sends frames, the ECUs answer with time-stamped frames.
//!
//! Time is virtual. A returned frame carries the number of milliseconds after the tester's frame at which
//! it appears on the bus; nothing sleeps. The adapter decides, from those stamps and its timeout, what it
//! saw.

use crate::ecu::{Ctx, Ecu, Timed};
use crate::fault::{BusFault, Effects, FaultKind, FaultPlan};
use crate::profile::VehicleSpec;
use isotp::{CanFrame, FlowControl};
use obd_policy::{AuditLog, Policy};
use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BusError {
    /// The transmit failed (injected).
    CanError,
}

pub struct Bus {
    ecus: Vec<Ecu>,
    policy: Policy,
    audit: AuditLog,
    faults: Vec<BusFault>,
    s3_ms: u64,
    now_ms: u64,
    request_no: u32,
    emitted: HashMap<(u32, u32), u32>,
}

impl Bus {
    pub fn new(spec: &VehicleSpec, policy: Policy, audit: AuditLog) -> Bus {
        Bus {
            ecus: spec.ecus.iter().cloned().map(Ecu::new).collect(),
            policy,
            audit,
            faults: Vec::new(),
            s3_ms: spec.s3_ms,
            now_ms: 0,
            request_no: 0,
            emitted: HashMap::new(),
        }
    }

    pub fn set_faults(&mut self, plan: &FaultPlan) {
        self.faults = plan.bus.clone();
    }

    pub fn policy(&self) -> &Policy {
        &self.policy
    }

    pub fn audit(&self) -> &AuditLog {
        &self.audit
    }

    pub fn ecus(&self) -> impl Iterator<Item = &Ecu> {
        self.ecus.iter()
    }

    /// Let virtual time pass with no traffic (session timers run on it).
    pub fn advance(&mut self, ms: u64) {
        self.now_ms += ms;
    }

    pub fn now_ms(&self) -> u64 {
        self.now_ms
    }

    /// Requests seen so far (a request is a single frame or a first frame from the tester).
    pub fn request_count(&self) -> u32 {
        self.request_no
    }

    fn effects(&self, resp_id: u32) -> Effects {
        let mut fx = Effects::default();
        for f in self
            .faults
            .iter()
            .filter(|f| f.matches(self.request_no, resp_id))
        {
            match f.kind {
                FaultKind::Silent => fx.silent = true,
                FaultKind::Delay(ms) => fx.delay_ms += ms,
                FaultKind::Pending(ms) => fx.pending_ms = Some(ms),
                FaultKind::FcOverflow => fx.fc_overflow = true,
                FaultKind::FcWait(n) => fx.fc_wait = n,
                FaultKind::FcBlock { block_size, st_min } => {
                    fx.fc_block = Some(FlowControl {
                        status: isotp::FlowStatus::ContinueToSend,
                        block_size,
                        st_min,
                    })
                }
                FaultKind::DropFrame(_) | FaultKind::BadSequence(_) | FaultKind::CanError => {}
            }
        }
        fx
    }

    /// The tester puts one frame on the bus. Returns what the ECUs put on it in reply, in the order
    /// each ECU produced it (sort by `at_ms` for bus order).
    pub fn send(&mut self, frame: &CanFrame) -> Result<Vec<Timed>, BusError> {
        // A frame nobody acknowledges is a transmit error: a bench adapter with no vehicle sees this.
        if self.ecus.is_empty() {
            return Err(BusError::CanError);
        }
        let starts_request = matches!(frame.data.first().map(|b| b >> 4), Some(0) | Some(1));
        if starts_request {
            self.request_no += 1;
            let n = self.request_no;
            if self
                .faults
                .iter()
                .any(|f| matches!(f.kind, FaultKind::CanError) && f.matches(n, f.ecu.unwrap_or(0)))
            {
                return Err(BusError::CanError);
            }
        }
        let mut out = Vec::new();
        for i in 0..self.ecus.len() {
            let fx = self.effects(self.ecus[i].spec.resp_id);
            let mut ctx = Ctx {
                policy: &self.policy,
                audit: &mut self.audit,
                now_ms: self.now_ms,
                s3_ms: self.s3_ms,
                fx: &fx,
            };
            out.extend(self.ecus[i].on_frame(frame, &mut ctx));
        }
        Ok(self.apply_frame_faults(out))
    }

    fn apply_frame_faults(&mut self, frames: Vec<Timed>) -> Vec<Timed> {
        let mut out = Vec::with_capacity(frames.len());
        for mut t in frames {
            let k = self
                .emitted
                .entry((self.request_no, t.frame.id))
                .or_insert(0);
            let idx = *k;
            *k += 1;
            let mut drop_it = false;
            for f in self
                .faults
                .iter()
                .filter(|f| f.matches(self.request_no, t.frame.id))
            {
                match f.kind {
                    FaultKind::DropFrame(n) if n == idx => drop_it = true,
                    FaultKind::BadSequence(n) if n == idx && t.frame.data[0] >> 4 == 2 => {
                        t.frame.data[0] = 0x20 | ((t.frame.data[0] & 0x0F).wrapping_add(1) & 0x0F);
                    }
                    _ => {}
                }
            }
            if !drop_it {
                out.push(t);
            }
        }
        out
    }
}
