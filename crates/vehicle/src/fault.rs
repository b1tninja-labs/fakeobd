//! Fault injection on the bus and the ECUs. The adapter-level faults (reset, dropped connection) are in
//! the `elm` crate; both share one text format, parsed by [`parse_fault_line`].

use isotp::FlowControl;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FaultKind {
    /// The ECU never answers.
    Silent,
    /// Every response frame arrives this many ms late.
    Delay(u32),
    /// The ECU first answers `7F <sid> 78` (response pending) and the real answer this many ms later.
    Pending(u32),
    /// Drop the k-th frame (0 = the single or first frame) of the response.
    DropFrame(u32),
    /// Corrupt the sequence number of the k-th frame of the response (k >= 1 is a consecutive frame).
    BadSequence(u32),
    /// The transmit itself fails: the adapter reports a CAN error.
    CanError,
    /// As receiver of a multi-frame request, answer the first frame with flow-control overflow.
    FcOverflow,
    /// As receiver of a multi-frame request, answer with n flow-control wait frames before clear-to-send.
    FcWait(u32),
    /// As receiver of a multi-frame request, ask for this block size and STmin.
    FcBlock { block_size: u8, st_min: u8 },
}

/// A fault and when it applies.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BusFault {
    /// 1-based index of the request on this connection; `None` for every request.
    pub request: Option<u32>,
    /// Response arbitration ID of the ECU the fault applies to; `None` for all ECUs.
    pub ecu: Option<u32>,
    pub kind: FaultKind,
}

impl BusFault {
    pub fn matches(&self, request_no: u32, resp_id: u32) -> bool {
        self.request.is_none_or(|r| r == request_no) && self.ecu.is_none_or(|e| e == resp_id)
    }
}

/// What an ECU does differently for the request in progress. Computed by the bus from the fault list.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Effects {
    pub silent: bool,
    pub delay_ms: u32,
    pub pending_ms: Option<u32>,
    pub fc_overflow: bool,
    pub fc_wait: u32,
    pub fc_block: Option<FlowControl>,
}

/// Adapter-level faults, applied by the `elm` crate.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AdapterFault {
    /// After this many lines have been handled, the adapter resets: state back to power-on, banner sent
    /// unprompted, the in-flight request unanswered.
    ResetAfter(u32),
    /// After this many lines, the adapter closes the connection.
    DropAfter(u32),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FaultPlan {
    pub bus: Vec<BusFault>,
    pub adapter: Vec<AdapterFault>,
}

impl FaultPlan {
    /// Parse a fault file, one fault per line:
    ///
    /// ```text
    /// bus request=2 ecu=7E8 delay_ms=500
    /// bus request=1 ecu=7E8 drop_frame=2
    /// bus bad_sequence=1
    /// bus request=3 can_error
    /// bus fc_wait=2          # ECU as receiver of a multi-frame request
    /// bus fc_block=4,0A      # block size 4, STmin 0x0A
    /// adapter reset_after=5
    /// adapter drop_after=9
    /// ```
    pub fn parse(text: &str) -> Result<FaultPlan, String> {
        let mut plan = FaultPlan::default();
        for (n, raw) in text.lines().enumerate() {
            let line = raw.split('#').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            parse_fault_line(line, &mut plan).map_err(|e| format!("fault line {}: {e}", n + 1))?;
        }
        Ok(plan)
    }
}

pub fn parse_fault_line(line: &str, plan: &mut FaultPlan) -> Result<(), String> {
    let mut w = line.split_whitespace();
    let scope = w.next().ok_or("empty")?;
    let toks: Vec<&str> = w.collect();
    let num = |v: &str| v.parse::<u32>().map_err(|_| format!("bad number {v}"));
    match scope {
        "adapter" => {
            let t = toks.first().ok_or("missing fault")?;
            let (k, v) = t.split_once('=').ok_or("adapter faults are key=value")?;
            plan.adapter.push(match k {
                "reset_after" => AdapterFault::ResetAfter(num(v)?),
                "drop_after" => AdapterFault::DropAfter(num(v)?),
                _ => return Err(format!("unknown adapter fault {k}")),
            });
            Ok(())
        }
        "bus" => {
            let (mut request, mut ecu, mut kind) = (None, None, None);
            for t in toks {
                let (k, v) = t.split_once('=').unwrap_or((t, ""));
                match k {
                    "request" => request = Some(num(v)?),
                    "ecu" => ecu = Some(u32::from_str_radix(v, 16).map_err(|_| "bad ecu id")?),
                    "silent" => kind = Some(FaultKind::Silent),
                    "delay_ms" => kind = Some(FaultKind::Delay(num(v)?)),
                    "pending_ms" => kind = Some(FaultKind::Pending(num(v)?)),
                    "drop_frame" => kind = Some(FaultKind::DropFrame(num(v)?)),
                    "bad_sequence" => kind = Some(FaultKind::BadSequence(num(v)?)),
                    "can_error" => kind = Some(FaultKind::CanError),
                    "fc_overflow" => kind = Some(FaultKind::FcOverflow),
                    "fc_wait" => kind = Some(FaultKind::FcWait(num(v)?)),
                    "fc_block" => {
                        let (b, s) = v.split_once(',').ok_or("fc_block=<bs>,<stmin hex>")?;
                        kind = Some(FaultKind::FcBlock {
                            block_size: b.parse().map_err(|_| "bad block size")?,
                            st_min: u8::from_str_radix(s, 16).map_err(|_| "bad stmin")?,
                        });
                    }
                    _ => return Err(format!("unknown bus option {k}")),
                }
            }
            plan.bus.push(BusFault {
                request,
                ecu,
                kind: kind.ok_or("no fault kind given")?,
            });
            Ok(())
        }
        _ => Err(format!("fault scope must be bus or adapter, not {scope}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_each_kind() {
        let p = FaultPlan::parse(
            "bus request=2 ecu=7E8 delay_ms=500\nbus drop_frame=2\nbus can_error\nbus fc_block=4,0A\nadapter reset_after=5\n# c\n",
        )
        .unwrap();
        assert_eq!(p.bus.len(), 4);
        assert_eq!(p.bus[0].request, Some(2));
        assert_eq!(p.bus[0].ecu, Some(0x7E8));
        assert_eq!(p.bus[0].kind, FaultKind::Delay(500));
        assert_eq!(
            p.bus[3].kind,
            FaultKind::FcBlock {
                block_size: 4,
                st_min: 0x0A
            }
        );
        assert_eq!(p.adapter, [AdapterFault::ResetAfter(5)]);
    }

    #[test]
    fn rejects_garbage_with_the_line_number() {
        assert!(FaultPlan::parse("bus request=2")
            .unwrap_err()
            .contains("line 1"));
        assert!(FaultPlan::parse("\nnope x=1")
            .unwrap_err()
            .contains("line 2"));
        assert!(FaultPlan::parse("adapter reset_after=x").is_err());
    }

    #[test]
    fn matching_honours_request_and_ecu() {
        let f = BusFault {
            request: Some(2),
            ecu: Some(0x7E8),
            kind: FaultKind::Silent,
        };
        assert!(f.matches(2, 0x7E8));
        assert!(!f.matches(1, 0x7E8));
        assert!(!f.matches(2, 0x7EC));
    }
}
