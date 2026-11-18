"""Break-and-watch-fail: for every `[ok]` ROADMAP row, change the code it covers, run the named test, and
require the test to fail; then restore the file byte for byte.

    python tools/break_check.py                 # run every mutation, print a table, exit 1 if any survived
    python tools/break_check.py M07 M12         # only these
    python tools/break_check.py --write-log     # also rewrite docs/BREAK-LOG.md

A mutation that does not change the file (the `old` text is not found exactly once) is an error, not a
pass: the table would otherwise rot as the code moves. Run from the repository root.
"""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

# (id, file, old, new, cargo test arguments that must fail)
MUTATIONS: list[tuple[str, str, str, str, list[str]]] = [
    ("M01", "crates/obd-policy/src/policy.rs", "0x0A, 0x19, 0x1A, 0x22, 0x3E,", "0x0A, 0x19, 0x1A, 0x22, 0x2E, 0x3E,",
     ["-p", "obd-policy", "--test", "policy", "default_policy_admits_exactly"]),
    ("M02", "crates/obd-policy/src/policy.rs", "pub const HARD_DENY: &[u8] = &[0x27];", "pub const HARD_DENY: &[u8] = &[];",
     ["-p", "obd-policy", "--test", "policy", "security_access"]),
    ("M03", "crates/obd-policy/src/policy.rs", "subs: Subs::Only(vec![0x01, 0x03]),", "subs: Subs::Only(vec![0x01, 0x02, 0x03]),",
     ["-p", "obd-policy", "--test", "policy", "session_control_admits"]),
    ("M04", "crates/obd-policy/src/policy.rs", "if self.deny.iter().any(|r| r.matches(sid, sub)) {", "if false && self.deny.iter().any(|r| r.matches(sid, sub)) {",
     ["-p", "obd-policy", "--test", "policy", "deny_beats_allow"]),
    ("M05", "crates/obd-policy/src/audit.rs", "pub const SENSITIVE_KEEP: usize = 3;", "pub const SENSITIVE_KEEP: usize = 7;",
     ["-p", "obd-policy", "--test", "policy", "audit_does_not_keep_the_body"]),
    ("M06", "crates/obd-policy/src/policy.rs", "(lo..=hi).contains(&d)", "(lo..hi).contains(&d)",
     ["-p", "obd-policy", "--test", "policy", "did_deny_ranges"]),
    ("M07", "crates/obd-policy/src/line.rs", '"ATCAF0", "ATPP", "STPX", "STSLX", "STBTPM"', '"ATCAF0", "ATPP", "STSLX", "STBTPM"',
     ["-p", "obd-policy", "--test", "policy", "line_classifier"]),
    ("M08", "crates/isotp/src/lib.rs", "if payload.len() <= 7 {", "if payload.len() <= 8 {",
     ["-p", "isotp", "seven_bytes_is_single"]),
    ("M09", "crates/isotp/src/lib.rs", "0x20 | ((i + 1) & 0x0F) as u8", "0x20 | ((i + 1) & 0x1F) as u8",
     ["-p", "isotp", "sequence_number_wraps"]),
    ("M10", "crates/isotp/src/lib.rs", "if sn != self.next_sn {", "if false && sn != self.next_sn {",
     ["-p", "isotp", "wrong_sequence_number"]),
    ("M11", "crates/isotp/src/lib.rs", "0xF1..=0xF9 => u32::from(raw - 0xF0) * 100,", "0xF1..=0xF9 => u32::from(raw - 0xF0) * 1000,",
     ["-p", "isotp", "st_min_decoding"]),
    ("M12", "crates/vehicle/src/ecu.rs", "v |= 1 << (32 - off);", "v |= 1 << (31 - off);",
     ["-p", "vehicle", "bitmap_sets_the_msb"]),
    ("M13", "crates/vehicle/src/ecu.rs", "if (functional && !self.spec.functional)", "if (false && functional && !self.spec.functional)",
     ["-p", "vehicle", "--test", "bus", "a_physical_only_ecu_ignores"]),
    ("M14", "crates/vehicle/src/ecu.rs", "> ctx.s3_ms {", "> ctx.s3_ms + 1_000_000 {",
     ["-p", "vehicle", "--test", "bus", "extended_session_unlocks"]),
    ("M15", "crates/vehicle/src/ecu.rs", "Some(nrc) => Reply::Neg(nrc),", "Some(_) => self.dispatch(pdu),",
     ["-p", "vehicle", "--test", "bus", "every_state_changing_request_is_refused"]),
    ("M16", "crates/vehicle/src/ecu.rs", "usize::from(fc.block_size).min(rest.len())", "rest.len()",
     ["-p", "vehicle", "--test", "bus", "block_size_and_stmin"]),
    ("M17", "crates/vehicle/src/bus.rs", "FaultKind::DropFrame(n) if n == idx => drop_it = true,", "FaultKind::DropFrame(n) if n == idx + 100 => drop_it = true,",
     ["-p", "vehicle", "--test", "bus", "fault_drop_frame"]),
    ("M18", "crates/vehicle/src/ecu.rs", "Some(ms) => vec![(vec![0x7F, sid, 0x78], 0), (bytes, ms)],", "Some(_) => vec![(bytes, 0)],",
     ["-p", "vehicle", "--test", "bus", "fault_delay_and_pending"]),
    ("M19", "crates/vehicle/src/bus.rs", "if self.ecus.is_empty() {", "if false && self.ecus.is_empty() {",
     ["-p", "fakeobd", "--test", "golden", "every_golden_transcript_replays_clean"]),
    ("M20", "crates/elm/src/lib.rs", "let echo = self.st.echo.then_some(typed.clone());", "let echo: Option<String> = None;",
     ["-p", "elm", "--test", "adapter", "echo_and_linefeed_state"]),
    ("M21", "crates/elm/src/exchange.rs", "id & self.st.rx_mask == self.st.rx_filter & self.st.rx_mask", "true",
     ["-p", "elm", "--test", "adapter", "physical_header_without"]),
    ("M22", "crates/elm/src/lib.rs", "u32::from(self.st) * 4", "u32::from(self.st) * 40",
     ["-p", "elm", "--test", "adapter", "st_sets_the_response_window"]),
    ("M23", "crates/elm/src/exchange.rs", 'out.push(format!("{:03X}", m.pdu.len()));', 'out.push(format!("{:04X}", m.pdu.len()));',
     ["-p", "elm", "--test", "adapter", "multi_frame_response_prints"]),
    ("M24", "crates/elm/src/exchange.rs", "stopped_early = true;", "stopped_early = false;",
     ["-p", "elm", "--test", "adapter", "functional_request_collects"]),
    ("M25", "crates/elm/src/exchange.rs", "data_error = true;\n        }\n        Outcome::Msgs", "data_error = false;\n        }\n        Outcome::Msgs",
     ["-p", "elm", "--test", "adapter", "a_lost_consecutive_frame"]),
    ("M26", "crates/elm/src/exchange.rs", 'return Outcome::Status("BUFFER FULL", tm.at_ms);', 'return Outcome::Status("BUFFER EMPTY", tm.at_ms);',
     ["-p", "elm", "--test", "adapter", "longer_than_the_receive_buffer"]),
    ("M27", "crates/elm/src/exchange.rs", "(1 | 2, Some(d)) => d.clone(),", "(9, Some(d)) => d.clone(),",
     ["-p", "elm", "--test", "adapter", "flow_control_data_that_is_not"]),
    ("M28", "crates/elm/src/lib.rs", "AdapterFault::ResetAfter(n) if n == self.lines => Some(false),", "AdapterFault::ResetAfter(n) if n + 1 == self.lines => Some(false),",
     ["-p", "elm", "--test", "adapter", "adapter_reset_mid_session"]),
    ("M29", "crates/elm/src/lib.rs", 'lines.push("SEARCHING...".to_string());', 'lines.push("SEARCHING".to_string());',
     ["-p", "elm", "--test", "adapter", "automatic_protocol_searches_once"]),
    ("M30", "crates/obd-conform/src/lib.rs", "if out.first().is_some_and(|l| squash(l) == want_echo) {", "if false && out.first().is_some_and(|l| squash(l) == want_echo) {",
     ["-p", "obd-conform", "normalize_drops_echo"]),
    ("M31", "crates/obd-conform/src/lib.rs", "want_n.len() == got_n.len() &&", "true &&",
     ["-p", "obd-conform", "glob_wildcards"]),
    ("M32", "crates/obd-conform/src/lib.rs", "(true, Some(g)) => Verdict::PassStaleGap(g.clone()),", "(true, Some(_)) => Verdict::Pass,",
     ["-p", "obd-conform", "a_mismatch_on_an_annotated_entry"]),
    ("M33", "crates/obd-conform/src/lib.rs", "if !allow_remote && !is_loopback(sa.ip()) {", "if false && !allow_remote && !is_loopback(sa.ip()) {",
     ["-p", "obd-conform", "tcp_refuses"]),
    ("M34", "crates/fakeobd/src/lib.rs", "if !allow_non_loopback && !l.local_addr()?.ip().is_loopback() {", "if false && !allow_non_loopback && !l.local_addr()?.ip().is_loopback() {",
     ["-p", "fakeobd", "--test", "server", "non_loopback_bind"]),
    ("M35", "crates/obd-policy/src/audit.rs", "let _ = writeln!(sink, \"{}\", entry.to_json());", "let _ = &sink;",
     ["-p", "obd-policy", "--test", "policy", "audit_sink_receives"]),
    ("M36", "crates/obd-policy/src/filter.rs", "if let Verdict::Deny(_) = d.verdict {", "if false {",
     ["-p", "obd-policy", "--test", "filter", "decide_agrees"]),
    ("M37", "crates/obd-policy/src/filter.rs", "Verdict::Allow => Action::Rewrite(new),", "_ => Action::Rewrite(new),",
     ["-p", "obd-policy", "--test", "filter", "a_rewriter_is_judged_again"]),
    ("M38", "crates/obd-policy/src/filter.rs", "Line::Bypass(_) | Line::Malformed => LineAction::Refuse,", "Line::Bypass(_) | Line::Malformed => LineAction::Forward,",
     ["-p", "obd-policy", "--test", "filter", "lines_are_forwarded"]),
    ("M39", "crates/fakeobd/src/lib.rs", "while let Some(pos) = self.pending.iter().position(|b| *b == b'\\r') {", "while let Some(pos) = self.pending.iter().position(|b| *b == b'\\n') {",
     ["-p", "fakeobd", "--test", "pipe", "a_pipe_answers"]),
    ("M40", "crates/fakeobd/src/lib.rs", "pub fn close(&mut self) {\n        self.tx = None;", "pub fn close(&mut self) {\n        let _ = &self.tx;",
     ["-p", "fakeobd", "--test", "pipe", "close_is_idempotent"]),
    ("M41", "crates/serial-bridge/src/lib.rs", 'LineAction::Refuse => self.local("?"),', "LineAction::Refuse => {}",
     ["--manifest-path", "crates/serial-bridge/Cargo.toml", "a_relay_between"]),
    ("M42", "profiles/volt-lab.profile", "ecu M241 req=241 resp=641", "ecu M241 req=242 resp=642",
     ["-p", "vehicle", "--test", "bus", "trial_addresses_answer"]),
    ("M43", "crates/elm/src/lib.rs", "0x63 => Self::IsoTp { ext: false },",
     "0x63 => return None,",
     ["-p", "elm", "--test", "adapter", "stn_single_wire_can_presets"]),
]


def run(m) -> tuple[str, str]:
    mid, rel, old, new, args = m
    path = ROOT / rel
    original = path.read_bytes()
    text = original.decode("utf8")
    if text.count(old) != 1:
        return "ERROR", f"`old` found {text.count(old)} times in {rel}"
    try:
        path.write_bytes(text.replace(old, new).encode("utf8"))
        # first make sure it still compiles: a mutation that breaks the build proves nothing
        build = subprocess.run(["cargo", "test", "--no-run", *args[:2]], cwd=ROOT, capture_output=True, text=True)
        if build.returncode != 0:
            return "ERROR", "mutant does not compile: " + build.stderr.strip().splitlines()[-1]
        r = subprocess.run(["cargo", "test", *args], cwd=ROOT, capture_output=True, text=True)
        ran = "running 0 tests" in r.stdout and "test result: ok. 0 passed" in r.stdout and r.returncode == 0
        if r.returncode == 0:
            return ("ERROR", "the filter matched no test") if ran else ("SURVIVED", "the test passed on the broken code")
        failed = [l.split()[1] for l in r.stdout.splitlines() if l.startswith("test ") and l.endswith("FAILED")]
        return "KILLED", ", ".join(failed) or "test binary failed"
    finally:
        path.write_bytes(original)


def main() -> int:
    argv = [a for a in sys.argv[1:] if not a.startswith("--")]
    write_log = "--write-log" in sys.argv
    chosen = [m for m in MUTATIONS if not argv or m[0] in argv]
    rows, bad = [], 0
    for m in chosen:
        status, detail = run(m)
        bad += status != "KILLED"
        rows.append((m[0], m[1], status, detail))
        print(f"{m[0]} {status:8} {m[1]}: {detail}", flush=True)
    if write_log:
        out = ["# Break log", "", "Each row: the code was changed (`tools/break_check.py` holds the exact edit), the named test was run, "
               "and it failed; the file was then restored. Regenerate with `python tools/break_check.py --write-log`.", "",
               "| Id | File | Result | Tests that failed |", "|---|---|---|---|"]
        out += [f"| {a} | `{b}` | {c} | {d} |" for a, b, c, d in rows]
        (ROOT / "docs" / "BREAK-LOG.md").write_text("\n".join(out) + "\n", encoding="utf8")
    print(f"{len(rows) - bad}/{len(rows)} killed")
    return 1 if bad else 0


if __name__ == "__main__":
    raise SystemExit(main())
