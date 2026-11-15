//! One request/response exchange between the adapter and the simulated bus: ISO-TP in both directions,
//! flow control, the receive filter and the response timeout.

use crate::{Elm, Flavor};
use isotp::{
    decode, segment, st_min_us, CanFrame, FlowControl, FlowStatus, Pdu, Reassembler, Rx, RxError,
};
use vehicle::{BusError, Timed};

/// One message as the adapter saw it.
#[derive(Clone, Debug)]
pub(crate) struct Msg {
    pub id: u32,
    pub pdu: Vec<u8>,
    /// Raw frame data fields, in arrival order (what `ATH1` prints).
    pub frames: Vec<Vec<u8>>,
}

pub(crate) enum Outcome {
    /// Messages seen, whether any frame sequence was bad or incomplete, and the milliseconds the
    /// adapter was busy.
    Msgs {
        msgs: Vec<Msg>,
        data_error: bool,
        busy_ms: u32,
    },
    /// A status line instead of data.
    Status(&'static str, u32),
}

fn shift(frames: Vec<Timed>, by: u32) -> impl Iterator<Item = Timed> {
    frames.into_iter().map(move |mut t| {
        t.at_ms += by;
        t
    })
}

/// Remove and return the earliest flow-control frame in the queue.
fn take_flow(q: &mut Vec<Timed>) -> Option<(FlowControl, u32)> {
    let mut best: Option<usize> = None;
    for (i, t) in q.iter().enumerate() {
        if matches!(decode(&t.frame.data), Ok(Pdu::Flow(_)))
            && best.is_none_or(|b| t.at_ms < q[b].at_ms)
        {
            best = Some(i);
        }
    }
    let t = q.remove(best?);
    match decode(&t.frame.data) {
        Ok(Pdu::Flow(fc)) => Some((fc, t.at_ms)),
        _ => None,
    }
}

impl Elm {
    fn accepts(&self, id: u32) -> bool {
        id & self.st.rx_mask == self.st.rx_filter & self.st.rx_mask
    }

    /// Arbitration ID and data of the flow control this adapter sends for a first frame from `resp_id`.
    fn flow_control_for(&self, resp_id: u32) -> (u32, Vec<u8>) {
        let st = &self.st;
        let paired = st
            .stn_pairs
            .iter()
            .find(|(_, rx)| *rx == resp_id)
            .map(|(tx, _)| *tx);
        let id = match (paired, st.fc_mode, st.fc_header) {
            (Some(tx), _, _) => tx,
            (None, 1, Some(h)) => h,
            _ if st.tx_id == vehicle::FUNCTIONAL_ID => resp_id.wrapping_sub(8),
            _ => st.tx_id,
        };
        let mut data = match (st.fc_mode, &st.fc_data) {
            (1 | 2, Some(d)) => d.clone(),
            _ => FlowControl::CTS.encode(),
        };
        if let Some(p) = self.cfg.tx_pad {
            data.resize(8, p);
        }
        (id, data)
    }

    /// Send `bytes` as a request and gather the responses.
    pub(crate) fn exchange(&mut self, bytes: &[u8], max_responses: Option<u8>) -> Outcome {
        let timeout = self.st.timeout_ms();
        let tx_id = self.st.tx_id;
        let mut q: Vec<Timed> = Vec::new();
        let mut t: u32 = 0;

        if !self.st.caf {
            // Raw mode: the host supplies the whole data field and sees whole frames back.
            if bytes.len() > 8 {
                return Outcome::Status("?", 0);
            }
            match self.bus.send(&CanFrame {
                id: tx_id,
                data: bytes.to_vec(),
            }) {
                Ok(r) => q.extend(r),
                Err(BusError::CanError) => return Outcome::Status("CAN ERROR", 0),
            }
            return self.collect_raw(q, timeout, max_responses);
        }

        let Ok(seg) = segment(bytes, self.cfg.tx_pad) else {
            return Outcome::Status("?", 0);
        };
        match self.bus.send(&CanFrame {
            id: tx_id,
            data: seg.first.clone(),
        }) {
            Ok(r) => q.extend(r),
            Err(BusError::CanError) => return Outcome::Status("CAN ERROR", 0),
        }
        let mut sent = 0usize;
        let mut waits = 0u32;
        while sent < seg.rest.len() {
            let Some((fc, at)) = take_flow(&mut q) else {
                return Outcome::Status("NO DATA", timeout);
            };
            if at.saturating_sub(t) > timeout {
                return Outcome::Status("NO DATA", timeout);
            }
            t = at;
            match fc.status {
                FlowStatus::Wait => {
                    waits += 1;
                    if waits > self.cfg.max_fc_waits {
                        return Outcome::Status("CAN ERROR", t);
                    }
                    continue;
                }
                FlowStatus::Overflow => return Outcome::Status("CAN ERROR", t),
                FlowStatus::ContinueToSend => {}
            }
            let left = seg.rest.len() - sent;
            let n = if fc.block_size == 0 {
                left
            } else {
                usize::from(fc.block_size).min(left)
            };
            let step = st_min_us(fc.st_min);
            for k in 0..n {
                let at = t + 1 + (k as u32 * step) / 1000;
                match self.bus.send(&CanFrame {
                    id: tx_id,
                    data: seg.rest[sent].clone(),
                }) {
                    Ok(r) => q.extend(shift(r, at)),
                    Err(BusError::CanError) => return Outcome::Status("CAN ERROR", at),
                }
                sent += 1;
                t = at;
            }
        }
        self.collect(q, t, timeout, max_responses)
    }

    fn collect_raw(&mut self, mut q: Vec<Timed>, timeout: u32, max: Option<u8>) -> Outcome {
        q.sort_by_key(|t| t.at_ms);
        let mut msgs = Vec::new();
        let mut last = 0;
        for t in q {
            if !self.accepts(t.frame.id) {
                continue;
            }
            if t.at_ms.saturating_sub(last) > timeout {
                break;
            }
            last = t.at_ms;
            msgs.push(Msg {
                id: t.frame.id,
                pdu: t.frame.data.clone(),
                frames: vec![t.frame.data],
            });
            if Some(msgs.len()) == max.map(usize::from) {
                return Outcome::Msgs {
                    msgs,
                    data_error: false,
                    busy_ms: last,
                };
            }
        }
        Outcome::Msgs {
            msgs,
            data_error: false,
            busy_ms: last + timeout,
        }
    }

    fn collect(&mut self, mut q: Vec<Timed>, start: u32, timeout: u32, max: Option<u8>) -> Outcome {
        q.sort_by_key(|t| t.at_ms);
        let mut chans: std::collections::BTreeMap<u32, (Reassembler, Vec<Vec<u8>>)> =
            Default::default();
        // Block size we advertised per responder, and consecutive frames since our last flow control.
        let mut blocks: std::collections::BTreeMap<u32, (u8, u8)> = Default::default();
        let mut msgs: Vec<Msg> = Vec::new();
        let mut data_error = false;
        let mut last = start;
        let mut stopped_early = false;
        let mut i = 0;
        while i < q.len() {
            let tm = q[i].clone();
            i += 1;
            if !self.accepts(tm.frame.id) {
                continue;
            }
            if tm.at_ms.saturating_sub(last) > timeout {
                break;
            }
            last = tm.at_ms;
            let (r, frames) = chans.entry(tm.frame.id).or_default();
            frames.push(tm.frame.data.clone());
            let mut flow_to_send = false;
            match r.push(&tm.frame.data, self.cfg.rx_limit) {
                Ok(Rx::Complete(pdu)) => {
                    msgs.push(Msg {
                        id: tm.frame.id,
                        pdu,
                        frames: std::mem::take(frames),
                    });
                    if Some(msgs.len()) == max.map(usize::from) {
                        stopped_early = true;
                        break;
                    }
                }
                Ok(Rx::NeedFlow { .. }) => {
                    flow_to_send = true;
                    let bs = self
                        .flow_control_for(tm.frame.id)
                        .1
                        .get(1)
                        .copied()
                        .unwrap_or(0);
                    blocks.insert(tm.frame.id, (bs, 0));
                }
                Ok(Rx::More) => {
                    if let Some((bs, seen)) = blocks.get_mut(&tm.frame.id) {
                        *seen += 1;
                        if *bs != 0 && *seen >= *bs {
                            *seen = 0;
                            flow_to_send = true;
                        }
                    }
                }
                Ok(Rx::Flow(_)) => {}
                Err(RxError::Overflow(_)) => {
                    let (id, _) = self.flow_control_for(tm.frame.id);
                    let fc = FlowControl {
                        status: FlowStatus::Overflow,
                        block_size: 0,
                        st_min: 0,
                    };
                    let mut data = fc.encode();
                    if let Some(p) = self.cfg.tx_pad {
                        data.resize(8, p);
                    }
                    let _ = self.bus.send(&CanFrame { id, data });
                    return Outcome::Status("BUFFER FULL", tm.at_ms);
                }
                Err(_) => {
                    data_error = true;
                    frames.clear();
                }
            }
            if flow_to_send {
                let (id, data) = self.flow_control_for(tm.frame.id);
                match self.bus.send(&CanFrame { id, data }) {
                    Ok(extra) => {
                        for e in shift(extra, tm.at_ms) {
                            let pos = i + q[i..].partition_point(|x| x.at_ms <= e.at_ms);
                            q.insert(pos, e);
                        }
                    }
                    Err(BusError::CanError) => return Outcome::Status("CAN ERROR", tm.at_ms),
                }
            }
        }
        if chans.values().any(|(r, _)| r.in_progress()) {
            data_error = true;
        }
        Outcome::Msgs {
            msgs,
            data_error,
            busy_ms: last + if stopped_early { 0 } else { timeout },
        }
    }
}

/// Render the messages the way the adapter prints them.
pub(crate) fn format_msgs(elm: &Elm, msgs: &[Msg]) -> Vec<String> {
    let st = &elm.st;
    let sep = if st.spaces { " " } else { "" };
    let tail = if st.spaces && elm.cfg.trailing_space {
        " "
    } else {
        ""
    };
    let hex = |b: &[u8]| {
        b.iter()
            .map(|x| format!("{x:02X}"))
            .collect::<Vec<_>>()
            .join(sep)
    };
    let mut out = Vec::new();
    for m in msgs {
        if st.headers {
            for f in &m.frames {
                out.push(format!("{:03X}{}{}{}", m.id, sep, hex(f), tail));
            }
        } else if !st.caf || m.frames.len() == 1 {
            let body = if st.caf { &m.pdu } else { &m.frames[0] };
            out.push(format!("{}{}", hex(body), tail));
        } else {
            out.push(format!("{:03X}", m.pdu.len()));
            let mut rows: Vec<&[u8]> = vec![&m.pdu[..m.pdu.len().min(6)]];
            if m.pdu.len() > 6 {
                rows.extend(m.pdu[6..].chunks(7));
            }
            for (n, row) in rows.iter().enumerate() {
                out.push(format!(
                    "{:X}:{}{}{}",
                    n % 16,
                    if st.spaces { " " } else { "" },
                    hex(row),
                    tail
                ));
            }
        }
    }
    out
}

impl Flavor {
    pub(crate) fn is_stn(self) -> bool {
        self == Flavor::Stn
    }
}
