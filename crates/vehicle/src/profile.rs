//! The vehicle description: ECUs, their addresses and their data. A plain-text format, parsed here.
//!
//! ```text
//! # comment
//! ecu HPCM2 req=7E0 resp=7E8 functional=yes pad=00 latency=3
//! pid 05 5A                    # mode 01 PID, data bytes after the PID
//! freeze 05 5A                 # mode 02 PID (frame 0)
//! info 02 ascii ABC            # mode 09 PID; `ascii` prefixes the item count 01
//! info 04 hex 01 AA BB
//! mode06 01 hex 01 8C 0A 00 10 00 00 00 FF
//! did 41AD 0000 [ext]          # UDS $22 DID; `ext` = readable only in the extended session
//! lid 90 ascii ABC             # GMLAN $1A identifier (one byte); data after the identifier
//! dtc stored P0A0D status=08   # stored | pending | permanent
//! ```
//!
//! Every value in the shipped profile is synthetic. The volt documentation catalogs which DIDs exist, not
//! what they hold; the few values it records from the app's Demo mode are marked in the profile.

use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DidSpec {
    pub data: Vec<u8>,
    pub extended_only: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DtcKind {
    Stored,
    Pending,
    Permanent,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DtcSpec {
    /// Two-byte SAE J2012 code, as carried by modes 03/07/0A.
    pub code: [u8; 2],
    pub kind: DtcKind,
    /// UDS status byte reported by `$19`.
    pub status: u8,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EcuSpec {
    pub name: String,
    pub req_id: u32,
    pub resp_id: u32,
    pub functional: bool,
    pub pad: Option<u8>,
    /// Milliseconds from request to first response frame.
    pub latency_ms: u32,
    pub pids: BTreeMap<u8, Vec<u8>>,
    pub freeze: BTreeMap<u8, Vec<u8>>,
    pub info: BTreeMap<u8, Vec<u8>>,
    pub mode06: BTreeMap<u8, Vec<u8>>,
    pub dids: BTreeMap<u16, DidSpec>,
    /// GMLAN `$1A` one-byte data identifiers.
    pub lids: BTreeMap<u8, Vec<u8>>,
    pub dtcs: Vec<DtcSpec>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VehicleSpec {
    pub ecus: Vec<EcuSpec>,
    /// Session timeout: a non-default session falls back after this long without a request.
    pub s3_ms: u64,
}

impl Default for VehicleSpec {
    fn default() -> Self {
        VehicleSpec {
            ecus: Vec::new(),
            s3_ms: 5000,
        }
    }
}

/// The profile shipped in the binary.
pub const DEFAULT_PROFILE: &str = include_str!("../../../profiles/volt-lab.profile");

impl VehicleSpec {
    pub fn default_vehicle() -> VehicleSpec {
        VehicleSpec::parse(DEFAULT_PROFILE).expect("the shipped profile parses")
    }

    pub fn parse(text: &str) -> Result<VehicleSpec, String> {
        let mut spec = VehicleSpec::default();
        for (n, raw) in text.lines().enumerate() {
            let line = raw.split('#').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            let err = |m: String| format!("profile line {}: {m}", n + 1);
            let mut w = line.split_whitespace();
            let verb = w.next().unwrap_or("");
            let rest: Vec<&str> = w.collect();
            if verb == "s3" {
                spec.s3_ms = rest
                    .first()
                    .and_then(|v| v.parse().ok())
                    .ok_or_else(|| err("s3 takes milliseconds".into()))?;
                continue;
            }
            if verb == "ecu" {
                let name = rest.first().ok_or_else(|| err("ecu needs a name".into()))?;
                let mut e = EcuSpec {
                    name: (*name).to_string(),
                    pad: Some(0x00),
                    latency_ms: 3,
                    ..Default::default()
                };
                for kv in &rest[1..] {
                    let (k, v) = kv
                        .split_once('=')
                        .ok_or_else(|| err(format!("bad option {kv}")))?;
                    match k {
                        "req" => {
                            e.req_id =
                                id11(v).ok_or_else(|| err("req is not an 11-bit hex id".into()))?
                        }
                        "resp" => {
                            e.resp_id =
                                id11(v).ok_or_else(|| err("resp is not an 11-bit hex id".into()))?
                        }
                        "functional" => e.functional = v == "yes",
                        "pad" => {
                            e.pad = if v == "none" {
                                None
                            } else {
                                Some(u8::from_str_radix(v, 16).map_err(|_| err("bad pad".into()))?)
                            }
                        }
                        "latency" => {
                            e.latency_ms = v.parse().map_err(|_| err("bad latency".into()))?
                        }
                        _ => return Err(err(format!("unknown ecu option {k}"))),
                    }
                }
                if e.req_id == 0 || e.resp_id == 0 {
                    return Err(err("ecu needs req= and resp=".into()));
                }
                spec.ecus.push(e);
                continue;
            }
            let ecu = spec
                .ecus
                .last_mut()
                .ok_or_else(|| err(format!("{verb} before any ecu line")))?;
            match verb {
                "pid" | "freeze" | "mode06" => {
                    let id = hex_byte(rest.first().copied()).ok_or_else(|| err("bad id".into()))?;
                    let mut tail = &rest[1..];
                    if tail.first() == Some(&"hex") {
                        tail = &tail[1..];
                    }
                    let data = hex_bytes(tail).ok_or_else(|| err("bad hex data".into()))?;
                    match verb {
                        "pid" => ecu.pids.insert(id, data),
                        "freeze" => ecu.freeze.insert(id, data),
                        _ => ecu.mode06.insert(id, data),
                    };
                }
                "info" => {
                    let id =
                        hex_byte(rest.first().copied()).ok_or_else(|| err("bad pid".into()))?;
                    let data = match rest.get(1).copied() {
                        Some("ascii") => {
                            let text = rest[2..].join(" ");
                            let mut d = vec![0x01];
                            d.extend_from_slice(text.as_bytes());
                            d
                        }
                        Some("hex") => {
                            hex_bytes(&rest[2..]).ok_or_else(|| err("bad hex".into()))?
                        }
                        _ => return Err(err("info needs ascii or hex".into())),
                    };
                    ecu.info.insert(id, data);
                }
                "lid" => {
                    let id =
                        hex_byte(rest.first().copied()).ok_or_else(|| err("bad lid".into()))?;
                    let data = match rest.get(1).copied() {
                        Some("ascii") => rest[2..].join(" ").into_bytes(),
                        _ => hex_bytes(&rest[1..]).ok_or_else(|| err("bad hex data".into()))?,
                    };
                    ecu.lids.insert(id, data);
                }
                "did" => {
                    let did = rest
                        .first()
                        .filter(|d| d.len() == 4)
                        .and_then(|d| u16::from_str_radix(d, 16).ok())
                        .ok_or_else(|| err("did is not four hex digits".into()))?;
                    let extended_only = rest.last() == Some(&"ext");
                    let end = if extended_only {
                        rest.len() - 1
                    } else {
                        rest.len()
                    };
                    let data =
                        hex_bytes(&rest[1..end]).ok_or_else(|| err("bad hex data".into()))?;
                    ecu.dids.insert(
                        did,
                        DidSpec {
                            data,
                            extended_only,
                        },
                    );
                }
                "dtc" => {
                    let kind = match rest.first().copied() {
                        Some("stored") => DtcKind::Stored,
                        Some("pending") => DtcKind::Pending,
                        Some("permanent") => DtcKind::Permanent,
                        _ => return Err(err("dtc kind is stored, pending or permanent".into())),
                    };
                    let code = dtc_code(rest.get(1).copied().unwrap_or(""))
                        .ok_or_else(|| err("bad DTC, expected like P0A0D".into()))?;
                    let default_status = match kind {
                        DtcKind::Pending => 0x04,
                        _ => 0x08,
                    };
                    let status = match rest.get(2).and_then(|s| s.strip_prefix("status=")) {
                        Some(s) => {
                            u8::from_str_radix(s, 16).map_err(|_| err("bad status".into()))?
                        }
                        None => default_status,
                    };
                    ecu.dtcs.push(DtcSpec { code, kind, status });
                }
                _ => return Err(err(format!("unknown directive {verb}"))),
            }
        }
        if spec.ecus.is_empty() {
            return Err("profile defines no ecu".into());
        }
        Ok(spec)
    }
}

fn id11(s: &str) -> Option<u32> {
    u32::from_str_radix(s, 16)
        .ok()
        .filter(|v| *v > 0 && *v <= 0x7FF)
}

fn hex_byte(s: Option<&str>) -> Option<u8> {
    s.filter(|s| s.len() == 2)
        .and_then(|s| u8::from_str_radix(s, 16).ok())
}

/// Hex tokens of any grouping (`AABB`, `AA BB`) to bytes; an odd digit count is an error.
fn hex_bytes(tokens: &[&str]) -> Option<Vec<u8>> {
    let all: String = tokens.concat();
    if !all.len().is_multiple_of(2) {
        return None;
    }
    (0..all.len() / 2)
        .map(|i| u8::from_str_radix(all.get(2 * i..2 * i + 2)?, 16).ok())
        .collect()
}

/// `P0A0D` to the two-byte J2012 encoding: two type bits, two digit bits, three hex nibbles.
pub fn dtc_code(s: &str) -> Option<[u8; 2]> {
    let b = s.as_bytes();
    if b.len() != 5 {
        return None;
    }
    let ty = match b[0].to_ascii_uppercase() {
        b'P' => 0u16,
        b'C' => 1,
        b'B' => 2,
        b'U' => 3,
        _ => return None,
    };
    let d = u16::from(b[1].checked_sub(b'0').filter(|d| *d < 4)?);
    let rest = u16::from_str_radix(&s[2..], 16).ok()?;
    let v = (ty << 14) | (d << 12) | rest;
    Some(v.to_be_bytes())
}

/// The inverse of [`dtc_code`].
pub fn dtc_text(code: [u8; 2]) -> String {
    let v = u16::from_be_bytes(code);
    let ty = ['P', 'C', 'B', 'U'][usize::from(v >> 14)];
    format!("{ty}{}{:03X}", (v >> 12) & 3, v & 0xFFF)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dtc_text_round_trips_every_code() {
        for v in 0..=u16::MAX {
            let t = dtc_text(v.to_be_bytes());
            assert_eq!(dtc_code(&t), Some(v.to_be_bytes()), "{t}");
        }
    }

    #[test]
    fn known_dtc_encodings() {
        assert_eq!(dtc_code("P0A0D"), Some([0x0A, 0x0D]));
        assert_eq!(dtc_code("U0184"), Some([0xC1, 0x84]));
        assert_eq!(dtc_code("P1000"), Some([0x10, 0x00]));
        assert_eq!(dtc_code("X0000"), None);
        assert_eq!(dtc_code("P4000"), None);
    }

    #[test]
    fn the_shipped_profile_parses_and_has_the_cluster() {
        let v = VehicleSpec::default_vehicle();
        assert_eq!(v.ecus.len(), 7);
        assert!(v
            .ecus
            .iter()
            .any(|e| e.req_id == 0x244 && e.resp_id == 0x644 && !e.functional));
        assert!(v
            .ecus
            .iter()
            .any(|e| e.req_id == 0x241 && e.resp_id == 0x641 && !e.functional));
    }

    #[test]
    fn errors_name_the_line() {
        let e = VehicleSpec::parse("ecu A req=7E0 resp=7E8\nbogus 1").unwrap_err();
        assert!(e.contains("line 2"), "{e}");
        assert!(VehicleSpec::parse("pid 05 5A")
            .unwrap_err()
            .contains("before any ecu"));
        assert!(VehicleSpec::parse("ecu A req=800 resp=7E8").is_err());
        assert!(VehicleSpec::parse("").is_err());
    }
}
