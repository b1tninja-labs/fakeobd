//! `isotp`: ISO-TP framing for 8-byte classic CAN frames and normal addressing.
//!
//! ISO 15765-2:2016 §9 is the transport layer protocol. Single-frame transmission is §9.2:
//! §9.2.1 when the transmit data length is 8, §9.2.2 when it is greater. §9.3 is multiple-frame
//! transmission, used once a message is longer than the single-frame maximum for that transmit
//! data length and addressing. With a transmit data length of 8 and normal addressing, that
//! maximum is 7 data bytes. Extended or mixed addressing, and any transmit data length above 8,
//! use a different maximum. This crate implements only the 8-byte normal-addressing case.
//!
//! PCI nibbles, flow-status values and STmin encoding were not re-read against §9.6. Those rows
//! stay `[unread]` in ROADMAP.md.

/// One classic CAN data frame with an 11-bit identifier.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CanFrame {
    pub id: u32,
    pub data: Vec<u8>,
}

/// Longest 12-bit first-frame length this crate sends. A longer message uses a wider length
/// field, which is not implemented.
pub const MAX_LEN: usize = 4095;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlowStatus {
    ContinueToSend,
    Wait,
    Overflow,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FlowControl {
    pub status: FlowStatus,
    /// Frames the sender may send before the next flow control; 0 means all the rest.
    pub block_size: u8,
    /// Raw STmin byte; see [`st_min_us`].
    pub st_min: u8,
}

impl FlowControl {
    pub const CTS: FlowControl = FlowControl {
        status: FlowStatus::ContinueToSend,
        block_size: 0,
        st_min: 0,
    };
    pub fn encode(self) -> Vec<u8> {
        let fs = match self.status {
            FlowStatus::ContinueToSend => 0,
            FlowStatus::Wait => 1,
            FlowStatus::Overflow => 2,
        };
        vec![0x30 | fs, self.block_size, self.st_min]
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Pdu {
    Single(Vec<u8>),
    First { len: usize, data: Vec<u8> },
    Consecutive { sn: u8, data: Vec<u8> },
    Flow(FlowControl),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DecodeError {
    Empty,
    /// PCI type nibble not in 0..=3.
    UnknownPci(u8),
    /// Single frame whose length field is 0 or exceeds the frame.
    BadSingleLength(usize),
    /// First frame shorter than 8 bytes or with a length that fits a single frame.
    BadFirstFrame,
    /// Flow control shorter than 3 bytes or with a reserved status.
    BadFlowControl,
}

/// Decode the PCI of one frame's data field.
pub fn decode(data: &[u8]) -> Result<Pdu, DecodeError> {
    let b0 = *data.first().ok_or(DecodeError::Empty)?;
    match b0 >> 4 {
        0 => {
            let len = usize::from(b0 & 0x0F);
            if len == 0 || len > data.len() - 1 || len > 7 {
                return Err(DecodeError::BadSingleLength(len));
            }
            Ok(Pdu::Single(data[1..1 + len].to_vec()))
        }
        1 => {
            if data.len() < 8 {
                return Err(DecodeError::BadFirstFrame);
            }
            let len = (usize::from(b0 & 0x0F) << 8) | usize::from(data[1]);
            if len < 8 {
                return Err(DecodeError::BadFirstFrame);
            }
            Ok(Pdu::First {
                len,
                data: data[2..8].to_vec(),
            })
        }
        2 => Ok(Pdu::Consecutive {
            sn: b0 & 0x0F,
            data: data[1..].to_vec(),
        }),
        3 => {
            if data.len() < 3 {
                return Err(DecodeError::BadFlowControl);
            }
            let status = match b0 & 0x0F {
                0 => FlowStatus::ContinueToSend,
                1 => FlowStatus::Wait,
                2 => FlowStatus::Overflow,
                _ => return Err(DecodeError::BadFlowControl),
            };
            Ok(Pdu::Flow(FlowControl {
                status,
                block_size: data[1],
                st_min: data[2],
            }))
        }
        n => Err(DecodeError::UnknownPci(n)),
    }
}

/// STmin in microseconds: 0x00..=0x7F are milliseconds, 0xF1..=0xF9 are 100..=900 microseconds, anything
/// else is reserved and treated as the longest millisecond value.
pub fn st_min_us(raw: u8) -> u32 {
    match raw {
        0x00..=0x7F => u32::from(raw) * 1000,
        0xF1..=0xF9 => u32::from(raw - 0xF0) * 100,
        _ => 127_000,
    }
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

/// Cut `payload` into frames for a transmit data length of 8 and normal addressing.
/// Seven data bytes or fewer are one single frame; eight or more start with a first frame.
/// `pad` fills every frame to 8 bytes when set.
pub fn segment(payload: &[u8], pad: Option<u8>) -> Result<Segments, SegmentError> {
    if payload.is_empty() {
        return Err(SegmentError::Empty);
    }
    if payload.len() > MAX_LEN {
        return Err(SegmentError::TooLong(payload.len()));
    }
    let padded = |mut v: Vec<u8>| {
        if let Some(p) = pad {
            v.resize(8, p);
        }
        v
    };
    if payload.len() <= 7 {
        let mut f = vec![payload.len() as u8];
        f.extend_from_slice(payload);
        return Ok(Segments {
            first: padded(f),
            rest: Vec::new(),
        });
    }
    let mut first = vec![
        0x10 | (payload.len() >> 8) as u8,
        (payload.len() & 0xFF) as u8,
    ];
    first.extend_from_slice(&payload[..6]);
    let rest = payload[6..]
        .chunks(7)
        .enumerate()
        .map(|(i, c)| {
            let mut f = vec![0x20 | ((i + 1) & 0x0F) as u8];
            f.extend_from_slice(c);
            padded(f)
        })
        .collect();
    Ok(Segments { first, rest })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RxError {
    /// A consecutive frame arrived with the wrong sequence number.
    BadSequence {
        expected: u8,
        got: u8,
    },
    /// A consecutive frame arrived with no first frame before it.
    Unexpected,
    /// The announced length exceeds the receiver's limit; answer with flow-control overflow.
    Overflow(usize),
    Decode(DecodeError),
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

/// Receiver side of one ISO-TP channel.
#[derive(Clone, Debug, Default)]
pub struct Reassembler {
    buf: Vec<u8>,
    expected: usize,
    next_sn: u8,
    active: bool,
}

impl Reassembler {
    pub fn new() -> Reassembler {
        Reassembler::default()
    }

    pub fn reset(&mut self) {
        *self = Reassembler::default();
    }

    pub fn in_progress(&self) -> bool {
        self.active
    }

    /// Feed one frame's data field. A receive error resets the channel. `limit` is the longest message
    /// this receiver will take.
    pub fn push(&mut self, data: &[u8], limit: usize) -> Result<Rx, RxError> {
        let pdu = match decode(data) {
            Ok(p) => p,
            Err(e) => {
                self.reset();
                return Err(RxError::Decode(e));
            }
        };
        match pdu {
            Pdu::Single(p) => {
                self.reset();
                Ok(Rx::Complete(p))
            }
            Pdu::First { len, data } => {
                self.reset();
                if len > limit {
                    return Err(RxError::Overflow(len));
                }
                self.buf = data;
                self.expected = len;
                self.next_sn = 1;
                self.active = true;
                Ok(Rx::NeedFlow { len })
            }
            Pdu::Consecutive { sn, data } => {
                if !self.active {
                    return Err(RxError::Unexpected);
                }
                if sn != self.next_sn {
                    let e = RxError::BadSequence {
                        expected: self.next_sn,
                        got: sn,
                    };
                    self.reset();
                    return Err(e);
                }
                self.next_sn = (self.next_sn + 1) & 0x0F;
                let want = self.expected - self.buf.len();
                self.buf.extend_from_slice(&data[..data.len().min(want)]);
                if self.buf.len() == self.expected {
                    let out = std::mem::take(&mut self.buf);
                    self.reset();
                    Ok(Rx::Complete(out))
                } else {
                    Ok(Rx::More)
                }
            }
            Pdu::Flow(fc) => Ok(Rx::Flow(fc)),
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
        // Transmit data length 8, normal addressing. Not the limit in §9.2.2 or with
        // extended or mixed addressing.
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
        assert_eq!(decode(&[0x33, 0, 0]), Err(DecodeError::BadFlowControl));
        assert_eq!(decode(&[0x30, 0]), Err(DecodeError::BadFlowControl));
    }

    #[test]
    fn malformed_frames_are_refused() {
        assert_eq!(decode(&[]), Err(DecodeError::Empty));
        assert_eq!(decode(&[0x00, 1]), Err(DecodeError::BadSingleLength(0)));
        assert_eq!(decode(&[0x05, 1, 2]), Err(DecodeError::BadSingleLength(5)));
        assert_eq!(
            decode(&[0x08, 1, 2, 3, 4, 5, 6, 7, 8]),
            Err(DecodeError::BadSingleLength(8))
        );
        assert_eq!(
            decode(&[0x10, 0x05, 1, 2, 3, 4, 5, 6]),
            Err(DecodeError::BadFirstFrame)
        );
        assert_eq!(decode(&[0x40]), Err(DecodeError::UnknownPci(4)));
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
