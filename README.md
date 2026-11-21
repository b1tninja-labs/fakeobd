# fakeobd

A **fake vehicle behind an ELM327/STN adapter**, served as ELM327 over TCP, for testing diagnostic apps
without an adapter, a car or Bluetooth. Companion to [fakescan](../fakescan) and [fakeprint](../fakeprint),
in their style: a scriptable fake device, a profile, fault injection and a conformance harness.

Built first for the reverse-engineered Android app Voltage (see `../volt`): in the emulator, set the app to
**WiFi**, Host `10.0.2.2`, Port `35000`, and it connects to this simulator instead of a BLE adapter
([docs/AVD-TRIAL.md](docs/AVD-TRIAL.md) records what the app did).

## Why the name

`fakeobd`: the siblings are `fakescan` and `fakeprint`, named for the device class they fake, and this fakes
an OBD-II/UDS diagnostic stack. `fakecan` would promise a raw CAN bus (SocketCAN, `candump`); this is the
layer above and the interface is an adapter's line protocol, not frames. `fakeelm` would hide that STN
adapters and UDS are in scope. Checked: no `fakeobd` existed in `D:\code`.

## Language

Rust, a workspace of six crates with no external dependencies. The siblings are Rust workspaces
(`fakescan`, `fakeprint`, `gattmirror`, `obd-bridge`); the policy crate is meant to be linked into a
Rust relay later; a byte-exact protocol simulator with an ISO-TP state machine and no runtime is what
Rust is good at. Python would have been quicker for the line layer, and volt's `tools/fake_elm327.py` is
that, but it has no vehicle, no ISO-TP and nowhere to put a reusable policy.

## What it does

1. **ELM327-over-TCP server** (`fakeobd serve`): `AT` commands (echo, linefeeds, spaces, headers, protocol
   select and automatic search, `ATSH`, `ATCRA`/`ATCF`/`ATCM`, `ATST`, `ATFCSH`/`ATFCSD`/`ATFCSM`,
   `ATCAF0`, `ATZ`, `ATRV`, ...), the replies `?`, `OK`, `NO DATA`, `SEARCHING...`, `UNABLE TO CONNECT`,
   `CAN ERROR`, `BUFFER FULL`, `<DATA ERROR`, `STOPPED`, and an STN profile with the `ST` family.
   Where ELM327 and STN differ, the difference is a recorded gap: [docs/ground-truth-gaps.md](docs/ground-truth-gaps.md).
2. **A vehicle**: three ECUs on a simulated 11-bit 500 kbit/s HS-CAN (`7E0`/`7E8`, `7E4`/`7EC`, and a cluster
   at `244`/`644`) answering SAE J1979 modes 01, 02, 03, 06, 07, 09, 0A and UDS `$10`, `$19`, `$22`, `$3E`
   and GMLAN `$1A`, with ISO-TP segmentation and flow control in both directions. Data is from volt's
   documentation where it has any (arbitration IDs, module names, DID numbers, four demo values, three DTCs)
   and is otherwise synthetic. See [profiles/volt-lab.profile](profiles/volt-lab.profile).
3. **A safety policy** that runs before any service: read-only by default, `$27` refused under every
   configuration, every decision audited. It is a separate library, `obd-policy`, for reuse in a relay.
   [docs/SCOPE.md](docs/SCOPE.md).
4. **A conformance harness** (`obd-conform`): golden transcripts replayed against the simulator now and against
   a real known-good adapter later, a capture format and a comparison tool.
   [docs/transcript-format.md](docs/transcript-format.md).
5. **Fault injection**: dropped or mis-sequenced frames, wrong flow control, a slow or silent ECU, response
   pending, CAN error, an adapter that resets or hangs up mid-session. [docs/architecture.md](docs/architecture.md#fault-injection).

## Quick start

```bash
cargo run -p fakeobd -- serve                       # 127.0.0.1:35000, ELM327 profile
cargo run -p fakeobd -- serve --adapter stn -v      # STN profile, trace every line and refusal
cargo run -p fakeobd -- client 127.0.0.1:35000 ATZ ATSP6 ATSH7E0 0105 2701
```

```text
"ATZ" -> ATZ\r\r\nELM327 v1.5\r\n\r\n>
"0105" -> 41 05 5A \r\r>
"2701" -> 7F 27 33 \r\r>
```

Options: `--profile FILE`, `--policy FILE`, `--faults FILE`, `--audit FILE` (JSON lines), `--no-vehicle`
(an adapter on a desk), `--realtime`, `--banner`. The server binds loopback only; `--allow-non-loopback`
exists and is a decision, because the ELM327 stream has no authentication.

Replay the golden transcripts: `cargo run -p fakeobd -- verify transcripts/*.txt`.

## Honesty about what is verified

The simulator has **never been compared with a real adapter or a real vehicle**. ISO 15765-2:2016 and
ISO 14229-1:2013 are in `primarysource`, and the rules were checked against them where ROADMAP.md says so.
SAE J1979 and the ELM327/STN datasheets are not. Every row that depends on an unread document is `[unread]` in [ROADMAP.md](ROADMAP.md). The golden transcripts are written by the simulator
(`@ source: sim`) and reviewed by reading; they prove the simulator is stable, not that it is right. The
mechanism for finding out is built and waiting for hardware.

Every `[ok]` row was broken on purpose and watched to fail: [docs/BREAK-LOG.md](docs/BREAK-LOG.md),
reproducible with `python tools/break_check.py`.

## Layout

```
crates/obd-policy    policy engine, audit log, ELM line classifier
crates/isotp         ISO-TP framing, reassembly, flow control
crates/vehicle       profile parser, ECUs, bus, faults
crates/elm           the adapter
crates/obd-conform   transcripts, replay, capture, compare
crates/fakeobd       server, wiring, CLI
profiles/            vehicle profile
transcripts/         golden transcripts and the scripts that captured them
tools/break_check.py break-and-watch-fail
docs/                scope, architecture, gaps, transcript format, trial notes
```

License: MIT OR Apache-2.0.
