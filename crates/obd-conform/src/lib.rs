//! `obd-conform`: golden transcripts for ELM327/STN adapters, and the tools to replay and compare them.
//!
//! A transcript is a list of (request line, reply bytes) pairs, kept as text so a person can read and
//! review it. The same transcript can be replayed against the simulator and, later, against a real
//! known-good adapter behind a TCP bridge; [`compare`] diffs two captures. A transcript written by the
//! simulator is a statement about the simulator. Only one captured from hardware is a statement about
//! hardware, and the `@ source:` header says which it is.
//!
//! The format is documented in `docs/transcript-format.md`.

use std::fmt::Write as _;
use std::io::{self, Read, Write};
use std::net::{IpAddr, TcpStream, ToSocketAddrs};
use std::time::Duration;

pub const MAGIC: &str = "fakeobd-transcript 1";

/// How a recorded reply is compared with a fresh one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Match {
    /// Same lines after [`normalize`]: echo, line endings, trailing blanks and the prompt ignored.
    Normal,
    /// Same bytes.
    Exact,
    /// Same number of normalized lines; `*` in a recorded line matches any run of characters.
    Glob,
}

impl Match {
    fn word(self) -> &'static str {
        match self {
            Match::Normal => "normal",
            Match::Exact => "exact",
            Match::Glob => "glob",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    /// The request line as typed, without the carriage return.
    pub request: String,
    /// The reply bytes, up to and including the `>` prompt.
    pub reply: Vec<u8>,
    pub mode: Match,
    /// Identifier of a documented difference between the recorded and the expected behaviour; see
    /// `docs/ground-truth-gaps.md`. A mismatch on an entry with a gap is reported as a known gap.
    pub gap: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Transcript {
    pub meta: Vec<(String, String)>,
    pub entries: Vec<Entry>,
}

impl Transcript {
    pub fn meta(&self, key: &str) -> Option<&str> {
        self.meta
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }
}

/// Escape bytes for a transcript line: `\r`, `\n`, `\\`, printable ASCII as is, the rest `\xHH`.
pub fn escape(bytes: &[u8]) -> String {
    let mut s = String::new();
    for &b in bytes {
        match b {
            b'\r' => s.push_str("\\r"),
            b'\n' => s.push_str("\\n"),
            b'\\' => s.push_str("\\\\"),
            0x20..=0x7E => s.push(b as char),
            _ => {
                let _ = write!(s, "\\x{b:02X}");
            }
        }
    }
    s
}

pub fn unescape(s: &str) -> Result<Vec<u8>, String> {
    let b = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if b[i] != b'\\' {
            out.push(b[i]);
            i += 1;
            continue;
        }
        match b.get(i + 1) {
            Some(b'r') => out.push(b'\r'),
            Some(b'n') => out.push(b'\n'),
            Some(b'\\') => out.push(b'\\'),
            Some(b'x') => {
                let h = s.get(i + 2..i + 4).ok_or("short \\x escape")?;
                out.push(u8::from_str_radix(h, 16).map_err(|_| format!("bad \\x escape {h:?}"))?);
                i += 2;
            }
            _ => return Err(format!("bad escape at byte {i}")),
        }
        i += 2;
    }
    Ok(out)
}

pub fn parse(text: &str) -> Result<Transcript, String> {
    let mut t = Transcript::default();
    let mut lines = text.lines().enumerate();
    match lines.next() {
        Some((_, l)) if l.trim_end() == MAGIC => {}
        _ => return Err(format!("first line must be {MAGIC:?}")),
    }
    for (n, line) in lines {
        let err = |m: &str| format!("line {}: {m}", n + 1);
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        let (tag, rest) = line.split_at(1);
        let rest = rest.strip_prefix(' ').unwrap_or(rest);
        match tag {
            "@" => {
                let (k, v) = rest
                    .split_once(':')
                    .ok_or_else(|| err("@ needs key: value"))?;
                t.meta.push((k.trim().to_string(), v.trim().to_string()));
            }
            ">" => t.entries.push(Entry {
                request: rest.to_string(),
                reply: Vec::new(),
                mode: Match::Normal,
                gap: None,
            }),
            "<" => {
                let e = t.entries.last_mut().ok_or_else(|| err("< before any >"))?;
                if !e.reply.is_empty() {
                    return Err(err("one < line per entry"));
                }
                e.reply = unescape(rest).map_err(|m| err(&m))?;
            }
            "~" => {
                let e = t.entries.last_mut().ok_or_else(|| err("~ before any >"))?;
                let mut w = rest.splitn(3, char::is_whitespace);
                match (w.next(), w.next()) {
                    (Some("match"), Some("normal")) => e.mode = Match::Normal,
                    (Some("match"), Some("exact")) => e.mode = Match::Exact,
                    (Some("match"), Some("glob")) => e.mode = Match::Glob,
                    (Some("gap"), Some(id)) => e.gap = Some(id.trim_end_matches(':').to_string()),
                    _ => return Err(err("~ is `match normal|exact|glob` or `gap <id> [note]`")),
                }
            }
            _ => return Err(err("line must start with @, >, <, ~ or #")),
        }
    }
    Ok(t)
}

pub fn render(t: &Transcript) -> String {
    let mut s = format!("{MAGIC}\n");
    for (k, v) in &t.meta {
        let _ = writeln!(s, "@ {k}: {v}");
    }
    for e in &t.entries {
        let _ = writeln!(s, "> {}", e.request);
        let _ = writeln!(s, "< {}", escape(&e.reply));
        if e.mode != Match::Normal {
            let _ = writeln!(s, "~ match {}", e.mode.word());
        }
        if let Some(g) = &e.gap {
            let _ = writeln!(s, "~ gap {g}");
        }
    }
    s
}

/// Reduce a reply to comparable lines: split on CR/LF, drop blanks and the prompt, drop an echo of the
/// request, trim, upper-case.
pub fn normalize(request: &str, reply: &[u8]) -> Vec<String> {
    let text = String::from_utf8_lossy(reply);
    let squash = |s: &str| {
        s.chars()
            .filter(|c| !c.is_whitespace())
            .collect::<String>()
            .to_ascii_uppercase()
    };
    let want_echo = squash(request);
    let mut out: Vec<String> = text
        .split(['\r', '\n'])
        .map(|l| l.trim().trim_end_matches('>').trim().to_ascii_uppercase())
        .filter(|l| !l.is_empty())
        .collect();
    if out.first().is_some_and(|l| squash(l) == want_echo) {
        out.remove(0);
    }
    out
}

fn glob(pat: &str, text: &str) -> bool {
    let parts: Vec<&str> = pat.split('*').collect();
    if parts.len() == 1 {
        return pat == text;
    }
    let mut rest = text;
    if !rest.starts_with(parts[0]) {
        return false;
    }
    rest = &rest[parts[0].len()..];
    for p in &parts[1..parts.len() - 1] {
        match rest.find(p) {
            Some(i) => rest = &rest[i + p.len()..],
            None => return false,
        }
    }
    rest.ends_with(parts[parts.len() - 1]) && rest.len() >= parts[parts.len() - 1].len()
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Verdict {
    Pass,
    /// Matches, but the entry is annotated with a gap that no longer shows: the annotation is stale.
    PassStaleGap(String),
    Fail {
        want: Vec<String>,
        got: Vec<String>,
    },
    KnownGap {
        id: String,
        want: Vec<String>,
        got: Vec<String>,
    },
}

impl Verdict {
    /// Not a failure (known gaps are documented differences).
    pub fn acceptable(&self) -> bool {
        !matches!(self, Verdict::Fail { .. })
    }
}

/// Compare one recorded entry with a fresh reply.
pub fn compare_entry(e: &Entry, got: &[u8]) -> Verdict {
    let (want_n, got_n) = (normalize(&e.request, &e.reply), normalize(&e.request, got));
    let same = match e.mode {
        Match::Exact => e.reply == got,
        Match::Normal => want_n == got_n,
        Match::Glob => {
            want_n.len() == got_n.len() && want_n.iter().zip(&got_n).all(|(p, t)| glob(p, t))
        }
    };
    match (same, &e.gap) {
        (true, None) => Verdict::Pass,
        (true, Some(g)) => Verdict::PassStaleGap(g.clone()),
        (false, Some(g)) => Verdict::KnownGap {
            id: g.clone(),
            want: want_n,
            got: got_n,
        },
        (false, None) => Verdict::Fail {
            want: want_n,
            got: got_n,
        },
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Report {
    pub results: Vec<(String, Verdict)>,
}

impl Report {
    pub fn count(&self, f: impl Fn(&Verdict) -> bool) -> usize {
        self.results.iter().filter(|(_, v)| f(v)).count()
    }
    pub fn failures(&self) -> usize {
        self.count(|v| matches!(v, Verdict::Fail { .. }))
    }
    pub fn known_gaps(&self) -> usize {
        self.count(|v| matches!(v, Verdict::KnownGap { .. }))
    }
    pub fn stale_gaps(&self) -> usize {
        self.count(|v| matches!(v, Verdict::PassStaleGap(_)))
    }
    pub fn ok(&self) -> bool {
        self.failures() == 0
    }

    /// Treat every known gap as a failure and drop stale-gap notes. A simulator replaying its own
    /// transcript has no excuse for a difference; gaps exist for comparing with hardware.
    pub fn strict(mut self) -> Report {
        for (_, v) in &mut self.results {
            *v = match std::mem::replace(v, Verdict::Pass) {
                Verdict::KnownGap { want, got, .. } => Verdict::Fail { want, got },
                Verdict::PassStaleGap(_) => Verdict::Pass,
                other => other,
            };
        }
        self
    }
    pub fn render(&self) -> String {
        let mut s = String::new();
        for (req, v) in &self.results {
            match v {
                Verdict::Pass => {}
                Verdict::PassStaleGap(g) => {
                    let _ = writeln!(
                        s,
                        "STALE  {req}: gap {g} is annotated but the replies match"
                    );
                }
                Verdict::KnownGap { id, want, got } => {
                    let _ = writeln!(
                        s,
                        "GAP    {req}: {id}\n         recorded {want:?}\n         got      {got:?}"
                    );
                }
                Verdict::Fail { want, got } => {
                    let _ = writeln!(
                        s,
                        "FAIL   {req}\n         recorded {want:?}\n         got      {got:?}"
                    );
                }
            }
        }
        let _ = writeln!(
            s,
            "{} entries: {} pass, {} known gaps, {} stale gap notes, {} FAIL",
            self.results.len(),
            self.count(|v| matches!(v, Verdict::Pass)),
            self.known_gaps(),
            self.stale_gaps(),
            self.failures()
        );
        s
    }
}

/// Anything that can take a request line and return the reply up to the prompt.
pub trait Transport {
    fn exchange(&mut self, request: &str) -> io::Result<Vec<u8>>;
}

/// Send every request of `t` in order and compare each reply with the recorded one.
pub fn replay(t: &Transcript, tr: &mut dyn Transport) -> io::Result<Report> {
    let mut r = Report::default();
    for e in &t.entries {
        let got = tr.exchange(&e.request)?;
        r.results.push((e.request.clone(), compare_entry(e, &got)));
    }
    Ok(r)
}

/// Send each request line and record what comes back.
pub fn capture(
    requests: &[String],
    tr: &mut dyn Transport,
    meta: Vec<(String, String)>,
) -> io::Result<Transcript> {
    let mut t = Transcript {
        meta,
        entries: Vec::new(),
    };
    for q in requests {
        let reply = tr.exchange(q)?;
        t.entries.push(Entry {
            request: q.clone(),
            reply,
            mode: Match::Normal,
            gap: None,
        });
    }
    Ok(t)
}

/// Diff two transcripts entry by entry. `expected` supplies the modes and gap annotations.
pub fn compare(expected: &Transcript, actual: &Transcript) -> Report {
    let mut r = Report::default();
    for (i, e) in expected.entries.iter().enumerate() {
        let v = match actual.entries.get(i) {
            Some(a) if a.request == e.request => compare_entry(e, &a.reply),
            Some(a) => Verdict::Fail {
                want: vec![format!("request {:?}", e.request)],
                got: vec![format!("request {:?}", a.request)],
            },
            None => Verdict::Fail {
                want: vec![format!("request {:?}", e.request)],
                got: vec!["(missing)".into()],
            },
        };
        r.results.push((e.request.clone(), v));
    }
    for extra in &actual.entries[expected.entries.len()..] {
        r.results.push((
            extra.request.clone(),
            Verdict::Fail {
                want: vec!["(missing)".into()],
                got: vec!["extra entry".into()],
            },
        ));
    }
    r
}

/// A TCP connection to an ELM327-over-TCP endpoint.
pub struct Tcp {
    stream: TcpStream,
}

impl Tcp {
    /// Connect to `addr`. Refuses a non-loopback address unless `allow_remote`: an ELM327 stream has no
    /// authentication, and this tool should only ever be pointed at a simulator or a local bridge.
    pub fn connect(addr: &str, allow_remote: bool) -> io::Result<Tcp> {
        let sa = addr
            .to_socket_addrs()?
            .next()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "no address"))?;
        if !allow_remote && !is_loopback(sa.ip()) {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("{addr} is not a loopback address (pass --allow-remote to override)"),
            ));
        }
        let stream = TcpStream::connect_timeout(&sa, Duration::from_secs(3))?;
        stream.set_nodelay(true)?;
        stream.set_read_timeout(Some(Duration::from_millis(150)))?;
        // Drain a banner sent on connect, if any.
        let mut sink = [0u8; 256];
        let _ = (&stream).read(&mut sink);
        stream.set_read_timeout(Some(Duration::from_secs(5)))?;
        Ok(Tcp { stream })
    }
}

fn is_loopback(ip: IpAddr) -> bool {
    ip.is_loopback()
}

impl Transport for Tcp {
    fn exchange(&mut self, request: &str) -> io::Result<Vec<u8>> {
        self.stream.write_all(request.as_bytes())?;
        self.stream.write_all(b"\r")?;
        let mut out = Vec::new();
        let mut buf = [0u8; 1024];
        while !out.ends_with(b">") {
            let n = self.stream.read(&mut buf)?;
            if n == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "connection closed before the prompt",
                ));
            }
            out.extend_from_slice(&buf[..n]);
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Canned(Vec<Vec<u8>>);
    impl Transport for Canned {
        fn exchange(&mut self, _: &str) -> io::Result<Vec<u8>> {
            Ok(self.0.remove(0))
        }
    }

    fn entry(req: &str, reply: &str, mode: Match, gap: Option<&str>) -> Entry {
        Entry {
            request: req.into(),
            reply: reply.as_bytes().to_vec(),
            mode,
            gap: gap.map(String::from),
        }
    }

    #[test]
    fn escape_round_trips_every_byte() {
        let all: Vec<u8> = (0..=255).collect();
        assert_eq!(unescape(&escape(&all)).unwrap(), all);
        assert_eq!(escape(b"A\r\n>\\"), "A\\r\\n>\\\\");
        assert!(unescape("\\q").is_err());
        assert!(unescape("\\x4").is_err());
    }

    #[test]
    fn render_parse_round_trip() {
        let t = Transcript {
            meta: vec![("source".into(), "sim".into())],
            entries: vec![
                entry("ATI", "ELM327 v1.5\r\n\r\n>", Match::Normal, None),
                entry("ATRV", "*V\r\r>", Match::Glob, Some("G-RV")),
                entry("0100", "41 00\r\r>", Match::Exact, None),
            ],
        };
        assert_eq!(parse(&render(&t)).unwrap(), t);
    }

    #[test]
    fn parse_errors_name_the_line() {
        assert!(parse("nope").is_err());
        let e = parse(&format!("{MAGIC}\n< x")).unwrap_err();
        assert!(e.contains("line 2"), "{e}");
        assert!(parse(&format!("{MAGIC}\n> a\n~ frob")).is_err());
        assert!(parse(&format!("{MAGIC}\n> a\n< x\n< y")).is_err());
    }

    #[test]
    fn normalize_drops_echo_prompt_blanks_and_case() {
        assert_eq!(
            normalize("ATI", b"ATI\rELM327 v1.5\r\n\r\n>"),
            ["ELM327 V1.5"]
        );
        assert_eq!(normalize("01 05", b"0105\r41 05 5a \r\r>"), ["41 05 5A"]);
        assert_eq!(normalize("ATE0", b"OK\r\r>"), ["OK"]);
    }

    #[test]
    fn linefeed_and_echo_differences_are_not_failures_in_normal_mode() {
        let e = entry("ATI", "ELM327 v1.5\r\n\r\n>", Match::Normal, None);
        assert_eq!(compare_entry(&e, b"ATI\rELM327 v1.5\r\r>"), Verdict::Pass);
        assert!(matches!(
            compare_entry(&e, b"ELM327 v2.1\r\r>"),
            Verdict::Fail { .. }
        ));
    }

    #[test]
    fn exact_mode_sees_the_bytes() {
        let e = entry("ATI", "ELM327 v1.5\r\n\r\n>", Match::Exact, None);
        assert_eq!(compare_entry(&e, b"ELM327 v1.5\r\n\r\n>"), Verdict::Pass);
        assert!(matches!(
            compare_entry(&e, b"ELM327 v1.5\r\r>"),
            Verdict::Fail { .. }
        ));
    }

    #[test]
    fn glob_wildcards() {
        assert!(glob("*", "anything"));
        assert!(glob("ELM327 V*", "ELM327 V1.5"));
        assert!(glob("A*C*E", "ABCDE"));
        assert!(!glob("A*C*E", "ABDE"));
        assert!(!glob("ELM*", "XELM"));
        assert!(glob("12.*V", "12.4V"));
        assert!(!glob("12.*V", "12.4"));
        let e = entry("ATRV", "*V\r\r>", Match::Glob, None);
        assert_eq!(compare_entry(&e, b"12.4V\r\r>"), Verdict::Pass);
        assert!(
            matches!(
                compare_entry(&e, b"12.4V\r12.4V\r\r>"),
                Verdict::Fail { .. }
            ),
            "line counts must agree"
        );
    }

    #[test]
    fn a_mismatch_on_an_annotated_entry_is_a_known_gap_and_a_match_makes_the_note_stale() {
        let e = entry("ATI", "ELM327 v1.5\r\r>", Match::Normal, Some("G-IDENT"));
        assert!(matches!(
            compare_entry(&e, b"STN\r\r>"),
            Verdict::KnownGap { .. }
        ));
        assert_eq!(
            compare_entry(&e, b"ELM327 v1.5\r\r>"),
            Verdict::PassStaleGap("G-IDENT".into())
        );
        assert!(compare_entry(&e, b"STN\r\r>").acceptable());
    }

    #[test]
    fn strict_turns_known_gaps_into_failures() {
        let e = entry("ATI", "ELM327 v1.5\r\r>", Match::Normal, Some("G"));
        let r = Report {
            results: vec![("ATI".into(), compare_entry(&e, b"STN\r\r>"))],
        };
        assert!(r.ok() && r.known_gaps() == 1);
        let s = r.strict();
        assert!(!s.ok() && s.known_gaps() == 0);
        let stale = Report {
            results: vec![("ATI".into(), compare_entry(&e, b"ELM327 v1.5\r\r>"))],
        };
        assert_eq!(stale.stale_gaps(), 1);
        assert_eq!(stale.strict().stale_gaps(), 0);
    }

    #[test]
    fn replay_counts_failures_and_gaps() {
        let t = Transcript {
            meta: vec![],
            entries: vec![
                entry("A", "x\r>", Match::Normal, None),
                entry("B", "y\r>", Match::Normal, None),
                entry("C", "z\r>", Match::Normal, Some("G")),
            ],
        };
        let r = replay(
            &t,
            &mut Canned(vec![
                b"x\r>".to_vec(),
                b"nope\r>".to_vec(),
                b"q\r>".to_vec(),
            ]),
        )
        .unwrap();
        assert_eq!((r.failures(), r.known_gaps(), r.stale_gaps()), (1, 1, 0));
        assert!(!r.ok());
        assert!(r.render().contains("FAIL   B"));
    }

    #[test]
    fn compare_detects_reordering_and_missing_entries() {
        let a = Transcript {
            meta: vec![],
            entries: vec![
                entry("A", "x\r>", Match::Normal, None),
                entry("B", "y\r>", Match::Normal, None),
            ],
        };
        let mut b = a.clone();
        assert!(compare(&a, &b).ok());
        b.entries.swap(0, 1);
        assert_eq!(compare(&a, &b).failures(), 2);
        b.entries.pop();
        assert!(compare(&a, &b).failures() >= 1);
    }

    #[test]
    fn tcp_refuses_a_non_loopback_address_by_default() {
        let e = Tcp::connect("192.0.2.1:35000", false)
            .err()
            .expect("refused");
        assert_eq!(e.kind(), io::ErrorKind::PermissionDenied);
    }
}
