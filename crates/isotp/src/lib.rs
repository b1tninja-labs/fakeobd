//! `isotp`: the fake vehicle's server-side ISO-TP adapter over the `isotpkit` codec.
//!
//! The grounded ISO 15765-2:2016 codec — PCI framing, segmentation, the `ConsecutiveFrame`
//! sequence-number wrap, `STmin` decoding and `FlowControl` encode/decode — lives in the
//! first-party [`isotpkit`] crate, read clause by clause against the standard and sans-I/O. This
//! crate is the thin server layer that `isotpkit` deliberately leaves out: the simulated ECU's
//! receiver that owes the sender a `FlowControl` after a `FirstFrame`, a receive-buffer limit that
//! answers an over-long message with flow-control overflow, and the [`CanFrame`] type the bus moves
//! around. It adds no codec logic of its own; it names `isotpkit`'s primitives in the shape the
//! `elm` and `vehicle` crates consume.

use isotpkit::flowcontrol::SeparationTime;
use isotpkit::pci::{decode_frame, NPdu, FF_DL_12BIT_MAX};
use isotpkit::segment::{segment as kit_segment, Reassembler as KitReassembler, SegmentPdu};
use isotpkit::Error;

pub use isotpkit::flowcontrol::FlowStatus;

/// One classic CAN data frame with an 11-bit identifier. `isotpkit` is sans-I/O and has no frame
/// type; the simulated bus moves these.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CanFrame {
    pub id: u32,
    pub data: Vec<u8>,
}

/// Longest message ISO-TP can describe with a 12-bit length (`isotpkit`'s `FF_DL` 12-bit maximum).
pub const MAX_LEN: usize = FF_DL_12BIT_MAX;

/// A `FlowControl` N_PDU in the fields the server uses (`isotpkit` names them `flow_status`/`stmin`).
///
/// The on-wire encoding is delegated to [`isotpkit::pci::FlowControl`]; this type only carries the
/// three parameters in the shape the `elm`/`vehicle` servers expect.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FlowControl {
    pub status: FlowStatus,
    /// Frames the sender may send before the next flow control; 0 means all the rest.
    pub block_size: u8,
    /// Raw STmin byte; see [`st_min_us`].
    pub st_min: u8,
}

impl FlowControl {
    /// Clear-to-send, block size 0, STmin 0.
    pub const CTS: FlowControl = FlowControl {
        status: FlowStatus::ContinueToSend,
        block_size: 0,
        st_min: 0,
    };

    /// The three on-wire N_PCI bytes (`0x3<FS>`, `BS`, `STmin`), unpadded. The caller pads to 8.
    pub fn encode(self) -> Vec<u8> {
        isotpkit::pci::FlowControl::new(self.status, self.block_size, self.st_min)
            .encode_with(false, 0)
    }
}

fn from_kit_fc(fc: isotpkit::pci::FlowControl) -> FlowControl {
    FlowControl {
        status: fc.flow_status,
        block_size: fc.block_size,
        st_min: fc.stmin,
    }
}

/// One decoded N_PDU, in the shape the server matches on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Pdu {
    Single(Vec<u8>),
    First { len: usize, data: Vec<u8> },
    Consecutive { sn: u8, data: Vec<u8> },
    Flow(FlowControl),
}

/// Decode one frame's data field into its N_PDU, delegating the framing to `isotpkit`.
///
/// # Errors
/// Any [`isotpkit::Error`] its per-type decoder returns.
pub fn decode(data: &[u8]) -> Result<Pdu, Error> {
    Ok(match decode_frame(data)? {
        NPdu::Single(sf) => Pdu::Single(sf.data().to_vec()),
        NPdu::First(ff) => Pdu::First {
            len: ff.length(),
            data: ff.data().to_vec(),
        },
        NPdu::Consecutive(cf) => Pdu::Consecutive {
            sn: cf.sn(),
            data: cf.data().to_vec(),
        },
        NPdu::Flow(fc) => Pdu::Flow(from_kit_fc(fc)),
    })
}

/// STmin in microseconds, via [`isotpkit::flowcontrol::SeparationTime`]: 0x00..=0x7F are
/// milliseconds, 0xF1..=0xF9 are 100..=900 microseconds, and a reserved value is 127 ms.
pub fn st_min_us(raw: u8) -> u32 {
    SeparationTime::from_byte(raw).microseconds()
}

/// A message cut into frames.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Segments {
    pub first: Vec<u8>,
    /// Consecutive frames; empty for a single frame.
    pub rest: Vec<Vec<u8>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SegmentError {
    Empty,
    TooLong(usize),
}

fn encode_segment(pdu: &SegmentPdu, pad: Option<u8>) -> Vec<u8> {
    match (pdu, pad) {
        // A FirstFrame always fills all 8 bytes, so padding is a no-op.
        (SegmentPdu::First(ff), _) => ff.encode(),
        (SegmentPdu::Single(sf), Some(p)) => sf.encode_with(true, p),
        (SegmentPdu::Single(sf), None) => sf.encode_with(false, 0),
        (SegmentPdu::Consecutive(cf), Some(p)) => cf.encode_with(true, p),
        (SegmentPdu::Consecutive(cf), None) => cf.encode_with(false, 0),
    }
}

/// Cut `payload` into frames. `pad` fills every frame to 8 bytes when set. The segmentation
/// (single vs first-plus-consecutive, the sequence-number wrap) is `isotpkit`'s.
///
/// # Errors
/// [`SegmentError::Empty`] for an empty payload, [`SegmentError::TooLong`] above [`MAX_LEN`].
pub fn segment(payload: &[u8], pad: Option<u8>) -> Result<Segments, SegmentError> {
    let pdus = kit_segment(payload).map_err(|e| match e {
        Error::EmptyMessage => SegmentError::Empty,
        Error::MessageTooLong(n) => SegmentError::TooLong(n),
        // `segment` only reports the two above; keep the length for any future variant.
        _ => SegmentError::TooLong(payload.len()),
    })?;
    let mut frames = pdus.iter().map(|p| encode_segment(p, pad));
    let first = frames.next().expect("segment yields at least one PDU");
    let rest = frames.collect();
    Ok(Segments { first, rest })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RxError {
    /// A consecutive frame arrived with the wrong sequence number.
    BadSequence { expected: u8, got: u8 },
    /// A consecutive frame arrived with no first frame before it.
    Unexpected,
    /// The announced length exceeds the receiver's limit; answer with flow-control overflow.
    Overflow(usize),
    /// The frame did not decode as a valid N_PDU.
    Decode(Error),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Rx {
    /// A whole message.
    Complete(Vec<u8>),
    /// A first frame was accepted; the receiver owes the sender a flow control.
    NeedFlow { len: usize },
    /// A consecutive frame was accepted; more to come.
    More,
    /// A flow-control frame: not part of reassembly, handed back for the sender side.
    Flow(FlowControl),
}

/// Receiver side of one ISO-TP channel. The buffer, sequence-number validation and
/// completion are `isotpkit`'s [`Reassembler`](isotpkit::segment::Reassembler); this wrapper adds
/// the server's flow-control-owed signal and the receive-buffer limit.
#[derive(Clone, Debug, Default)]
pub struct Reassembler {
    inner: KitReassembler,
}

impl Reassembler {
    pub fn new() -> Reassembler {
        Reassembler {
            inner: KitReassembler::new(),
        }
    }

    pub fn reset(&mut self) {
        self.inner.reset();
    }

    pub fn in_progress(&self) -> bool {
        self.inner.in_progress()
    }

    /// Feed one frame's data field. A receive error resets the channel. `limit` is the longest
    /// message this receiver will take.
    ///
    /// # Errors
    /// [`RxError::Decode`] for a malformed frame, [`RxError::Overflow`] for a first frame whose
    /// length exceeds `limit`, [`RxError::Unexpected`] for a consecutive frame with no first frame
    /// before it, and [`RxError::BadSequence`] for a consecutive frame out of order.
    pub fn push(&mut self, data: &[u8], limit: usize) -> Result<Rx, RxError> {
        let npdu = match decode_frame(data) {
            Ok(p) => p,
            Err(e) => {
                self.inner.reset();
                return Err(RxError::Decode(e));
            }
        };
        match npdu {
            NPdu::Single(sf) => {
                self.inner.reset();
                Ok(Rx::Complete(sf.data().to_vec()))
            }
            NPdu::First(ff) => {
                self.inner.reset();
                let len = ff.length();
                if len > limit {
                    return Err(RxError::Overflow(len));
                }
                // A FirstFrame on a just-reset reassembler opens the session and cannot fail.
                let _ = self.inner.feed(&SegmentPdu::First(ff));
                Ok(Rx::NeedFlow { len })
            }
            NPdu::Consecutive(cf) => {
                if !self.inner.in_progress() {
                    return Err(RxError::Unexpected);
                }
                match self.inner.feed(&SegmentPdu::Consecutive(cf)) {
                    Ok(Some(message)) => Ok(Rx::Complete(message)),
                    Ok(None) => Ok(Rx::More),
                    // `feed` resets itself on a wrong sequence number (N_WRONG_SN).
                    Err(Error::WrongSequenceNumber { expected, got }) => {
                        Err(RxError::BadSequence { expected, got })
                    }
                    Err(e) => {
                        self.inner.reset();
                        Err(RxError::Decode(e))
                    }
                }
            }
            NPdu::Flow(fc) => Ok(Rx::Flow(from_kit_fc(fc))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roundtrip(payload: &[u8], pad: Option<u8>) {
        let s = segment(payload, pad).unwrap();
        let mut r = Reassembler::new();
        let mut got = None;
        for f in std::iter::once(&s.first).chain(s.rest.iter()) {
            match r.push(f, MAX_LEN).unwrap() {
                Rx::Complete(p) => got = Some(p),
                Rx::NeedFlow { .. } | Rx::More => {}
                Rx::Flow(_) => panic!(),
            }
        }
        assert_eq!(got.as_deref(), Some(payload), "len {}", payload.len());
    }

    #[test]
    fn every_length_round_trips_padded_and_unpadded() {
        for len in 1..=300 {
            let p: Vec<u8> = (0..len).map(|i| (i * 7 + 3) as u8).collect();
            roundtrip(&p, None);
            roundtrip(&p, Some(0xAA));
        }
        roundtrip(&vec![0x5A; MAX_LEN], None);
    }

    #[test]
    fn the_limits_are_enforced() {
        assert_eq!(segment(&[], None), Err(SegmentError::Empty));
        assert_eq!(
            segment(&vec![0; MAX_LEN + 1], None),
            Err(SegmentError::TooLong(MAX_LEN + 1))
        );
    }

    #[test]
    fn seven_bytes_is_single_and_eight_is_first_plus_consecutive() {
        assert!(segment(&[1; 7], None).unwrap().rest.is_empty());
        let s = segment(&[1; 8], None).unwrap();
        assert_eq!(s.first, [0x10, 0x08, 1, 1, 1, 1, 1, 1]);
        assert_eq!(s.rest, [vec![0x21, 1, 1]]);
    }

    #[test]
    fn sequence_number_wraps_from_f_to_0() {
        let s = segment(&[9; 6 + 7 * 16], None).unwrap();
        assert_eq!(s.rest.len(), 16);
        assert_eq!(s.rest[14][0], 0x2F);
        assert_eq!(s.rest[15][0], 0x20);
    }

    #[test]
    fn padding_fills_frames_to_eight_bytes() {
        let s = segment(&[1; 3], Some(0xAA)).unwrap();
        assert_eq!(s.first, [0x03, 1, 1, 1, 0xAA, 0xAA, 0xAA, 0xAA]);
        let s = segment(&[2; 8], Some(0xAA)).unwrap();
        assert_eq!(s.rest[0], [0x21, 2, 2, 0xAA, 0xAA, 0xAA, 0xAA, 0xAA]);
    }

    #[test]
    fn a_first_frame_is_accepted_and_owes_a_flow_control() {
        let s = segment(&[7; 30], None).unwrap();
        assert_eq!(
            Reassembler::new().push(&s.first, MAX_LEN),
            Ok(Rx::NeedFlow { len: 30 })
        );
    }

    #[test]
    fn a_wrong_sequence_number_resets_the_channel() {
        let s = segment(&[7; 30], None).unwrap();
        let mut r = Reassembler::new();
        r.push(&s.first, MAX_LEN).unwrap();
        assert_eq!(
            r.push(&s.rest[1], MAX_LEN),
            Err(RxError::BadSequence {
                expected: 1,
                got: 2
            })
        );
        assert!(!r.in_progress());
        assert_eq!(r.push(&s.rest[1], MAX_LEN), Err(RxError::Unexpected));
    }

    #[test]
    fn a_first_frame_over_the_limit_is_an_overflow() {
        let s = segment(&[7; 100], None).unwrap();
        assert_eq!(
            Reassembler::new().push(&s.first, 50),
            Err(RxError::Overflow(100))
        );
    }

    #[test]
    fn a_new_first_frame_restarts_reassembly() {
        let a = segment(&[1; 20], None).unwrap();
        let b = segment(&[2; 9], None).unwrap();
        let mut r = Reassembler::new();
        r.push(&a.first, MAX_LEN).unwrap();
        r.push(&b.first, MAX_LEN).unwrap();
        assert_eq!(r.push(&b.rest[0], MAX_LEN), Ok(Rx::Complete(vec![2; 9])));
    }

    #[test]
    fn flow_control_encodes_and_decodes() {
        let fc = FlowControl {
            status: FlowStatus::Wait,
            block_size: 8,
            st_min: 0x14,
        };
        assert_eq!(fc.encode(), [0x31, 8, 0x14]);
        assert_eq!(decode(&fc.encode()), Ok(Pdu::Flow(fc)));
        assert!(decode(&[0x33, 0, 0]).is_err());
        assert!(decode(&[0x30, 0]).is_err());
    }

    #[test]
    fn malformed_frames_are_refused() {
        assert!(decode(&[]).is_err());
        assert!(decode(&[0x00, 1]).is_err());
        assert!(decode(&[0x05, 1, 2]).is_err());
        assert!(decode(&[0x08, 1, 2, 3, 4, 5, 6, 7, 8]).is_err());
        assert!(decode(&[0x10, 0x05, 1, 2, 3, 4, 5, 6]).is_err());
        assert!(decode(&[0x40]).is_err());
    }

    #[test]
    fn st_min_decoding() {
        assert_eq!(st_min_us(0), 0);
        assert_eq!(st_min_us(0x14), 20_000);
        assert_eq!(st_min_us(0x7F), 127_000);
        assert_eq!(st_min_us(0xF1), 100);
        assert_eq!(st_min_us(0xF9), 900);
        assert_eq!(st_min_us(0x80), 127_000);
        assert_eq!(st_min_us(0xFA), 127_000);
    }
}
