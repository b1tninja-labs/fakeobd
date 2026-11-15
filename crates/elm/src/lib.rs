//! `elm`: an ELM327/STN adapter in front of a simulated bus.
//!
//! The adapter takes one input line at a time (a command or a hex request) and returns the bytes that go
//! back on the wire, ending in the `>` prompt, plus how long the adapter was busy in virtual
//! milliseconds. It owns a [`vehicle::Bus`]; nothing here touches a socket, so the whole layer is tested
//! in-process.
//!
//! **Provenance.** No ELM327 or STN datasheet is in `primarysource`. Behaviour comes from the command
//! lists recovered in `obdlink-re/docs/COMMANDS.md`, the verified dialogue in volt's `fake_elm327.py`,
//! and recollection. Every place the two chip families, or a real adapter, may differ is listed in
//! `docs/ground-truth-gaps.md`, and the replay harness exists to settle them against hardware.

mod exchange;

use exchange::{format_msgs, Outcome};
use vehicle::{AdapterFault, Bus, FaultPlan};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Flavor {
    /// A plain ELM327: `ST` commands answer `?`.
    Elm327,
    /// An STN-based adapter: the ELM327 command set plus the `ST` family.
    Stn,
}

#[derive(Clone, Debug)]
pub struct Config {
    pub flavor: Flavor,
    /// Reply to `ATI`.
    pub ident: String,
    pub voltage: String,
    /// Data lines end in a space when spaces are on, as ELM327 chips print them. `[open]`.
    pub trailing_space: bool,
    /// Padding for frames the adapter sends; `None` sends them unpadded.
    pub tx_pad: Option<u8>,
    /// Longest ISO-TP message the adapter will receive.
    pub rx_limit: usize,
    /// Longest input line, in characters, before `BUFFER FULL`.
    pub input_limit: usize,
    /// How many flow-control wait frames the adapter tolerates.
    pub max_fc_waits: u32,
    /// Print the banner as soon as a client connects (real adapters do not; the volt fake did).
    pub banner_on_connect: bool,
}

impl Config {
    pub fn new(flavor: Flavor) -> Config {
        Config {
            flavor,
            ident: match flavor {
                Flavor::Elm327 => "ELM327 v1.5".into(),
                Flavor::Stn => "ELM327 v1.4b".into(),
            },
            voltage: "12.4V".into(),
            trailing_space: flavor == Flavor::Elm327,
            tx_pad: Some(0x00),
            rx_limit: isotp::MAX_LEN,
            input_limit: 256,
            max_fc_waits: 10,
            banner_on_connect: false,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct State {
    pub echo: bool,
    pub linefeed: bool,
    pub spaces: bool,
    pub headers: bool,
    pub caf: bool,
    /// Selected protocol 0..=12; 0 is automatic.
    pub protocol: u8,
    /// Automatic search landed here.
    pub auto_found: Option<u8>,
    pub tx_id: u32,
    pub rx_filter: u32,
    pub rx_mask: u32,
    /// `ATST` byte; timeout is four milliseconds per unit.
    pub st: u8,
    pub fc_mode: u8,
    pub fc_header: Option<u32>,
    pub fc_data: Option<Vec<u8>>,
    pub stn_pairs: Vec<(u32, u32)>,
}

impl State {
    fn power_on() -> State {
        State {
            echo: true,
            linefeed: true,
            spaces: true,
            headers: false,
            caf: true,
            protocol: 0,
            auto_found: None,
            tx_id: vehicle::FUNCTIONAL_ID,
            rx_filter: 0x7E8,
            rx_mask: 0x7F8,
            st: 0x32,
            fc_mode: 0,
            fc_header: None,
            fc_data: None,
            stn_pairs: Vec::new(),
        }
    }

    pub fn timeout_ms(&self) -> u32 {
        u32::from(self.st) * 4
    }
}

/// What one input line produced.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reply {
    /// Bytes for the wire, ending in `>` unless the adapter went away.
    pub bytes: Vec<u8>,
    /// Virtual milliseconds the adapter spent before it could print the reply.
    pub busy_ms: u32,
    /// The connection should be closed after the bytes are sent.
    pub close: bool,
}

pub struct Elm {
    pub(crate) cfg: Config,
    pub(crate) st: State,
    pub(crate) bus: Bus,
    adapter_faults: Vec<AdapterFault>,
    lines: u32,
    last_line: Option<String>,
}

impl Elm {
    pub fn new(cfg: Config, bus: Bus) -> Elm {
        Elm {
            cfg,
            st: State::power_on(),
            bus,
            adapter_faults: Vec::new(),
            lines: 0,
            last_line: None,
        }
    }

    pub fn set_faults(&mut self, plan: &FaultPlan) {
        self.bus.set_faults(plan);
        self.adapter_faults = plan.adapter.clone();
    }

    pub fn bus(&self) -> &Bus {
        &self.bus
    }

    pub fn bus_mut(&mut self) -> &mut Bus {
        &mut self.bus
    }

    pub fn config(&self) -> &Config {
        &self.cfg
    }

    fn eol(&self) -> &'static str {
        if self.st.linefeed {
            "\r\n"
        } else {
            "\r"
        }
    }

    /// What a client sees on connect, if anything.
    pub fn banner(&self) -> Vec<u8> {
        if !self.cfg.banner_on_connect {
            return Vec::new();
        }
        format!("\r\n{}\r\n\r\n>", self.cfg.ident).into_bytes()
    }

    /// The reply when a byte arrives while a request is still being answered.
    pub fn stopped(&self) -> Vec<u8> {
        format!("STOPPED{}{}>", self.eol(), self.eol()).into_bytes()
    }

    fn reset(&mut self) {
        self.st = State::power_on();
    }

    fn wire(&self, echo: Option<&str>, lines: &[String]) -> Vec<u8> {
        let eol = self.eol();
        let mut s = String::new();
        if let Some(e) = echo {
            s.push_str(e);
            s.push('\r');
        }
        for l in lines {
            s.push_str(l);
            s.push_str(eol);
        }
        s.push_str(eol);
        s.push('>');
        s.into_bytes()
    }

    /// Handle one input line (the bytes before the carriage return).
    pub fn handle_line(&mut self, raw: &[u8]) -> Reply {
        self.lines += 1;
        let fault = self.adapter_faults.iter().find_map(|f| match *f {
            AdapterFault::ResetAfter(n) if n == self.lines => Some(false),
            AdapterFault::DropAfter(n) if n == self.lines => Some(true),
            _ => None,
        });
        match fault {
            Some(true) => {
                return Reply {
                    bytes: Vec::new(),
                    busy_ms: 0,
                    close: true,
                }
            }
            Some(false) => {
                self.reset();
                let bytes = format!("\r\r{}\r\r>", self.cfg.ident).into_bytes();
                return Reply {
                    bytes,
                    busy_ms: 0,
                    close: false,
                };
            }
            None => {}
        }

        let typed: String = String::from_utf8_lossy(raw)
            .chars()
            .filter(|c| *c != '\n')
            .collect();
        let echo = self.st.echo.then_some(typed.clone());
        let work = if typed.trim().is_empty() {
            match &self.last_line {
                Some(l) => l.clone(),
                None => {
                    return Reply {
                        bytes: self.wire(echo.as_deref(), &[]),
                        busy_ms: 0,
                        close: false,
                    }
                }
            }
        } else {
            self.last_line = Some(typed.clone());
            typed.clone()
        };
        let (lines, busy_ms) = if work.chars().count() > self.cfg.input_limit {
            (vec!["BUFFER FULL".to_string()], 0)
        } else {
            self.dispatch(&work)
        };
        let bytes = self.wire(echo.as_deref(), &lines);
        self.bus.advance(u64::from(busy_ms));
        Reply {
            bytes,
            busy_ms,
            close: false,
        }
    }

    fn dispatch(&mut self, line: &str) -> (Vec<String>, u32) {
        let cmd: String = line
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect::<String>()
            .to_ascii_uppercase();
        let q = || (vec!["?".to_string()], 0);
        if let Some(rest) = cmd.strip_prefix("AT") {
            return match self.at(rest) {
                Some(l) => (l, 0),
                None => q(),
            };
        }
        if let Some(rest) = cmd.strip_prefix("ST") {
            if !self.cfg.flavor.is_stn() {
                return q();
            }
            return match self.stn(rest) {
                Some(l) => (l, 0),
                None => q(),
            };
        }
        self.request(&cmd)
    }

    fn at(&mut self, c: &str) -> Option<Vec<String>> {
        let ok = || Some(vec!["OK".to_string()]);
        let one = |s: &str| Some(vec![s.to_string()]);
        let flag = |c: &str| match c.chars().last() {
            Some('0') => Some(false),
            Some('1') => Some(true),
            _ => None,
        };
        match c {
            "Z" | "WS" => {
                self.reset();
                return Some(vec![String::new(), self.cfg.ident.clone()]);
            }
            "D" => {
                self.reset();
                return ok();
            }
            "I" => return one(&self.cfg.ident.clone()),
            "@1" => return one("OBDII to RS232 Interpreter"),
            "RV" => return one(&self.cfg.voltage.clone()),
            "IGN" => return one("ON"),
            "DPN" => return one(&self.protocol_number()),
            "DP" => return one(&self.protocol_name()),
            "PC" => return ok(),
            _ => {}
        }
        if c.len() == 2 && c.starts_with('E') {
            self.st.echo = flag(c)?;
            return ok();
        }
        if c.len() == 2 && c.starts_with('L') {
            self.st.linefeed = flag(c)?;
            return ok();
        }
        if c.len() == 2 && c.starts_with('S') && c != "ST" {
            self.st.spaces = flag(c)?;
            return ok();
        }
        if c.len() == 2 && c.starts_with('H') {
            self.st.headers = flag(c)?;
            return ok();
        }
        if let Some(v) = c.strip_prefix("CAF") {
            self.st.caf = flag(v)?;
            return ok();
        }
        if let Some(v) = c.strip_prefix("SP").or_else(|| c.strip_prefix("TP")) {
            // "ATSP6" selects protocol 6; "ATSPA6" is automatic with 6 tried first (treated as plain
            // automatic here); "ATSP0" is automatic.
            let digit = |d: &str| {
                u8::from_str_radix(d, 16)
                    .ok()
                    .filter(|p| (1..=12).contains(p))
            };
            self.st.protocol = match v.len() {
                1 if v == "0" => 0,
                1 => digit(v)?,
                2 if v.starts_with('A') => {
                    digit(&v[1..])?;
                    0
                }
                _ => return None,
            };
            self.st.auto_found = None;
            return ok();
        }
        if let Some(v) = c.strip_prefix("SH") {
            self.st.tx_id = header(v)?;
            return ok();
        }
        if let Some(v) = c.strip_prefix("CRA") {
            if v.is_empty() {
                (self.st.rx_filter, self.st.rx_mask) = (0x7E8, 0x7F8);
            } else {
                self.st.rx_filter = header(v)?;
                self.st.rx_mask = 0x7FF;
            }
            return ok();
        }
        if let Some(v) = c.strip_prefix("CF") {
            if !v.starts_with('C') {
                self.st.rx_filter = header(v)?;
                return ok();
            }
        }
        if let Some(v) = c.strip_prefix("CM") {
            self.st.rx_mask = header(v)?;
            return ok();
        }
        if let Some(v) = c.strip_prefix("ST") {
            let n = u8::from_str_radix(v, 16)
                .ok()
                .filter(|_| (1..=2).contains(&v.len()))?;
            self.st.st = if n == 0 { 0x32 } else { n };
            return ok();
        }
        if let Some(v) = c.strip_prefix("FCSH") {
            self.st.fc_header = Some(header(v)?);
            return ok();
        }
        if let Some(v) = c.strip_prefix("FCSD") {
            self.st.fc_data = Some(hex_bytes(v).filter(|b| (1..=5).contains(&b.len()))?);
            return ok();
        }
        if let Some(v) = c.strip_prefix("FCSM") {
            self.st.fc_mode = v.parse().ok().filter(|m| *m <= 2)?;
            return ok();
        }
        // Accepted and acknowledged; their effect is not modelled (see docs/ground-truth-gaps.md).
        const ACK: &[&str] = &[
            "M", "AT", "NL", "AL", "AR", "CFC", "D0", "D1", "R", "CEA", "CP", "TA", "SR", "SW",
            "V", "CV", "BI", "KW", "IB", "IIA", "WM", "BRD", "PB",
        ];
        if ACK.iter().any(|p| c.starts_with(p)) {
            return ok();
        }
        None
    }

    fn stn(&mut self, c: &str) -> Option<Vec<String>> {
        let one = |s: &str| Some(vec![s.to_string()]);
        match c {
            "I" => return one("STN2230 vSIM"),
            "DI" | "DIX" => return one("FAKEOBD-SIM"),
            "MFR" => return one("fakeobd"),
            "SN" => return one("000000000001"),
            "CFCPC" => {
                self.st.stn_pairs.clear();
                return one("OK");
            }
            _ => {}
        }
        if let Some(v) = c.strip_prefix("CFCPA") {
            let (tx, rx) = v.split_once(',')?;
            self.st.stn_pairs.push((header(tx)?, header(rx)?));
            return one("OK");
        }
        if let Some(v) = c.strip_prefix('P') {
            if !v.is_empty() && v.chars().all(|c| c.is_ascii_hexdigit()) && !c.starts_with("PBR") {
                // A one-digit STN protocol number is the same as ATSP; longer forms (the Voltage app
                // sends "STP63") are acknowledged without effect. Unknown semantics, see the gaps doc.
                return if v.len() == 1 {
                    self.at(&format!("SP{v}"))
                } else {
                    Some(vec!["OK".to_string()])
                };
            }
        }
        const ACK: &[&str] = &[
            "PBR", "CSWM", "CSEGT", "BC", "UIL", "SL", "BT", "FAP", "FCP", "FAC", "CAF", "PO",
            "SCSM",
        ];
        if ACK.iter().any(|p| c.starts_with(p)) {
            return one("OK");
        }
        None
    }

    fn protocol_number(&self) -> String {
        let p = self.st.auto_found.unwrap_or(self.st.protocol);
        if self.st.protocol == 0 {
            format!("A{:X}", p)
        } else {
            format!("{p:X}")
        }
    }

    fn protocol_name(&self) -> String {
        const NAMES: [&str; 13] = [
            "AUTO",
            "SAE J1850 PWM",
            "SAE J1850 VPW",
            "ISO 9141-2",
            "ISO 14230-4 (KWP 5BAUD)",
            "ISO 14230-4 (KWP FAST)",
            "ISO 15765-4 (CAN 11/500)",
            "ISO 15765-4 (CAN 29/500)",
            "ISO 15765-4 (CAN 11/250)",
            "ISO 15765-4 (CAN 29/250)",
            "SAE J1939 (CAN 29/250)",
            "USER1 (CAN 11/125)",
            "USER2 (CAN 11/50)",
        ];
        match (self.st.protocol, self.st.auto_found) {
            (0, None) => "AUTO".into(),
            (0, Some(p)) => format!("AUTO, {}", NAMES[usize::from(p)]),
            (p, _) => NAMES[usize::from(p)].into(),
        }
    }

    fn request(&mut self, hex: &str) -> (Vec<String>, u32) {
        if !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return (vec!["?".into()], 0);
        }
        let (body, max) = if hex.len() % 2 == 1 {
            let (b, d) = hex.split_at(hex.len() - 1);
            (b, u8::from_str_radix(d, 16).ok().filter(|n| *n > 0))
        } else {
            (hex, None)
        };
        let Some(bytes) = hex_bytes(body).filter(|b| !b.is_empty()) else {
            return (vec!["?".into()], 0);
        };
        let auto_search = self.st.protocol == 0 && self.st.auto_found.is_none();
        let proto = if self.st.protocol == 0 {
            6
        } else {
            self.st.protocol
        };
        let mut lines = Vec::new();
        if auto_search {
            lines.push("SEARCHING...".to_string());
        }
        if proto != 6 {
            lines.push("UNABLE TO CONNECT".into());
            return (lines, 0);
        }
        let (more, busy) = match self.exchange(&bytes, max) {
            Outcome::Status(s, busy) => {
                if auto_search && (s == "NO DATA" || s == "CAN ERROR") {
                    (vec!["UNABLE TO CONNECT".to_string()], busy)
                } else {
                    (vec![s.to_string()], busy)
                }
            }
            Outcome::Msgs {
                msgs,
                data_error,
                busy_ms,
            } => {
                if msgs.is_empty() && !data_error {
                    let s = if auto_search {
                        "UNABLE TO CONNECT"
                    } else {
                        "NO DATA"
                    };
                    (vec![s.to_string()], busy_ms)
                } else {
                    if auto_search && !msgs.is_empty() {
                        self.st.auto_found = Some(6);
                    }
                    let mut l = format_msgs(self, &msgs);
                    if data_error {
                        l.push("<DATA ERROR".into());
                    }
                    (l, busy_ms)
                }
            }
        };
        lines.extend(more);
        (lines, busy)
    }
}

fn hex_bytes(s: &str) -> Option<Vec<u8>> {
    if s.is_empty() || !s.len().is_multiple_of(2) || !s.is_ascii() {
        return None;
    }
    (0..s.len() / 2)
        .map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).ok())
        .collect()
}

/// `ATSH`/`ATCRA`-style identifier: 1 to 3 hex digits is an 11-bit ID, 8 digits a 29-bit ID.
fn header(s: &str) -> Option<u32> {
    if !s.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    match s.len() {
        1..=3 => u32::from_str_radix(s, 16).ok(),
        // 29-bit identifiers are accepted but the simulated bus carries only 11-bit frames, so nothing
        // ever answers them; the high bit marks them as extended.
        6 | 8 => u32::from_str_radix(s, 16).ok().map(|v| v | 0x8000_0000),
        _ => None,
    }
}
