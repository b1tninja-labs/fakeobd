"""Smoke tests for the fakeobd Python bindings.

Build the extension first (``maturin develop`` from ``py/``, or build the cdylib and place it as
``fakeobd.pyd`` / ``fakeobd.so`` on the path), then run ``pytest``.
"""

import os

import pytest

import fakeobd


def setup_can(bench):
    """Turn echo off and select the 11-bit/500k CAN protocol and the engine ECU header."""
    bench.send("ATE0")
    bench.send("ATSP6")
    bench.send("ATSH7E0")


def test_reset_reports_identity():
    bench = fakeobd.Bench("elm327")
    reply = bench.send("ATZ")
    assert b"ELM327 v1.5" in reply


def test_mode01_read_round_trips():
    bench = fakeobd.Bench("elm327")
    setup_can(bench)
    reply = bench.send("0105")  # Mode 01 PID 05, engine coolant temperature
    assert b"41 05" in reply  # positive response: 0x40 + mode, echoed PID


def test_security_access_is_refused():
    bench = fakeobd.Bench("elm327")
    setup_can(bench)
    reply = bench.send("2701")  # SecurityAccess requestSeed
    assert b"7F 27" in reply  # negative response to service 0x27


def test_feed_splits_lines_and_concatenates_replies():
    bench = fakeobd.Bench("elm327")
    # Two complete lines plus a trailing partial line (held until the next feed).
    out = bench.feed(b"ATE0\rATI\rAT")
    assert out.count(b">") == 2  # one prompt per complete line
    assert b"ELM327 v1.5" in out  # ATI identity
    rest = bench.feed(b"Z\r")  # completes "ATZ"
    assert b"ELM327 v1.5" in rest


def test_banner_only_when_requested():
    assert fakeobd.Bench("elm327").banner() == b""
    assert b"ELM327 v1.5" in fakeobd.Bench("elm327", banner=True).banner()


def test_no_vehicle_has_nothing_on_the_bus():
    bench = fakeobd.Bench("elm327", no_vehicle=True)
    setup_can(bench)
    reply = bench.send("0100")
    # With nothing on the bus no ECU answers; the adapter reports a no-response status.
    assert any(s in reply for s in (b"NO DATA", b"UNABLE TO CONNECT", b"CAN ERROR"))


def test_stn_adapter_answers_st_commands():
    elm = fakeobd.Bench("elm327")
    stn = fakeobd.Bench("stn")
    # A plain ELM327 rejects the ST family with "?"; an STN adapter answers it.
    assert b"?" in elm.send("STI")
    assert b"?" not in stn.send("STI")


def test_default_profile_is_nonempty_text():
    text = fakeobd.default_profile()
    assert isinstance(text, str)
    assert len(text) > 0


def test_unknown_adapter_raises():
    with pytest.raises(ValueError):
        fakeobd.Bench("j2534")


def test_bad_profile_raises():
    with pytest.raises(ValueError):
        fakeobd.Bench("elm327", profile="this is not a valid profile line\n")


def test_verify_transcript_against_a_golden():
    path = os.path.join(os.path.dirname(__file__), "..", "..", "transcripts", "bench.txt")
    if not os.path.exists(path):
        pytest.skip("golden transcript not available")
    # newline="" preserves the file's exact bytes (no universal-newline translation), so the parsed
    # entry count matches `fakeobd verify` reading the same file with std::fs::read_to_string.
    with open(path, encoding="utf-8", newline="") as f:
        ok, report = fakeobd.verify_transcript(f.read())
    assert ok, report
