//! Classify one ELM327/STN input line, so a relay can tell a diagnostic request (which the policy judges)
//! from an adapter setting (which it forwards) from a command that bypasses the request path (which it
//! refuses). The simulator's own parser is in the `elm` crate; this is the part a relay needs without it.

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Line {
    Empty,
    /// An `AT`/`ST` setting. Upper case, spaces removed.
    Adapter(String),
    /// An adapter command that can put bytes on the bus without passing through the request path, or
    /// reprogram the adapter. A relay refuses these by default.
    Bypass(String),
    /// A hex request. A trailing odd nibble is the ELM327 "stop after N responses" digit.
    Request {
        pdu: Vec<u8>,
        max_responses: Option<u8>,
    },
    Malformed,
}

/// Prefixes that bypass the request path: raw-frame mode (`ATCAF0`), programmable parameters (`ATPP`,
/// which write the adapter's flash), and the STN transmit-raw family (`STPX`). `[unread]`: recorded from
/// the adapter command sets in `obdlink-re/docs/COMMANDS.md`, not from a vendor manual.
const BYPASS_PREFIXES: &[&str] = &["ATCAF0", "ATPP", "STPX", "STSLX", "STBTPM"];

pub fn classify_line(raw: &str) -> Line {
    let cmd: String = raw
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>()
        .to_ascii_uppercase();
    if cmd.is_empty() {
        return Line::Empty;
    }
    if cmd.starts_with("AT") || cmd.starts_with("ST") {
        return if BYPASS_PREFIXES.iter().any(|p| cmd.starts_with(p)) {
            Line::Bypass(cmd)
        } else {
            Line::Adapter(cmd)
        };
    }
    if !cmd.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Line::Malformed;
    }
    let (body, max) = if cmd.len() % 2 == 1 {
        let (b, last) = cmd.split_at(cmd.len() - 1);
        (b, u8::from_str_radix(last, 16).ok())
    } else {
        (cmd.as_str(), None)
    };
    let pdu: Vec<u8> = (0..body.len() / 2)
        .map(|i| u8::from_str_radix(&body[2 * i..2 * i + 2], 16).unwrap_or(0))
        .collect();
    if pdu.is_empty() {
        return Line::Malformed;
    }
    Line::Request {
        pdu,
        max_responses: max,
    }
}
