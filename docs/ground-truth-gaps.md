# Ground-truth gaps

Every place where the simulator's behaviour is a guess about a real adapter or a real ECU. No ELM327 or
STN datasheet is in `primarysource`. The behaviour was written from the command lists in
`obdlink-re/docs/COMMANDS.md`, the dialogue recorded by volt's `tools/fake_elm327.py`, the traffic the
Voltage app actually sends ([AVD-TRIAL.md](AVD-TRIAL.md)) and recollection. None of it has been compared
with hardware.

**Update 2026-11-15:** `primarysource` has ISO 14229-1:2013 and `gm_*` (registered, corpus not yet imported: `primarysource gm import`). UDS rows have been checked where noted below.

**Update 2026-11-16:** ISO 15765-2:2016 and ISO 15031-3:2004 are imported. In ISO 15765-2:2016, §9 is the transport layer protocol, §9.2 is single-frame transmission (§9.2.1 when the transmit data length is 8, §9.2.2 when it is greater), and §9.3 is multiple-frame transmission. The simulator's 7-byte single frame is the §9.2.1 normal-addressing case. A search of that document has no `ATSP6`, no `0x400`, and no bit rate. ISO 15031-3:2004 is the diagnostic connector. SAE J1979:2002 is an unindexed scan; mode numbers are not cited from it. Sequence-number and STmin rows were not re-read.

Each gap has an identifier. A transcript entry that carries `~ gap <id>` is expected to differ from a real
adapter; `fakeobd replay` against an external target reports such a difference as a **known gap** instead
of a failure, and `fakeobd verify` (the simulator replaying itself) counts it as a failure, so a gap
annotation can never hide a regression in the simulator. Close a gap by capturing the real behaviour,
changing the simulator, removing the annotation and recording the capture's `source: hardware`.

**Scope of a capture.** Anything that depends on a vehicle (`scope: vehicle`) can only be settled on a car
or a bench with a CAN simulator. `scope: adapter` transcripts (`elm-init`, `bench`, `stn`) are the ones a
bare adapter on a desk can answer; `bench.txt` is written for exactly that.

## Annotated in a transcript

### G-ATZ-BANNER
`ATZ` reply: the simulator prints a blank line, the version string and the prompt, with the line endings of
the power-on state (CR LF). A real chip's reply may carry more blank lines, a different delay, and prints
before the echo of the next command. Settle with: `ATZ` on a real adapter, `source: hardware`, `exact`.

### G-IDENT
`ATI` string. ELM327 profile `ELM327 v1.5`; STN profile `ELM327 v1.4b`. Both are recollection. Real
firmware versions differ per device.

### G-AT1
`AT@1` returns `OBDII to RS232 Interpreter`. Recollection of the chip's description string.

### G-DP-STRINGS
`ATDP` / `ATDPN` protocol names, including the `AUTO, ` prefix and `A6` form. Recollection.

### G-CAN-ERROR-BENCH
`ATSP6` then a request with nothing on the bus: the simulator says `CAN ERROR` because a frame nobody
acknowledges is a transmit error. A real adapter may say `CAN ERROR`, `NO DATA` or `UNABLE TO CONNECT`.
This is the most useful single capture: it needs only an adapter and a power supply.

### G-SEARCH
`SEARCHING...` line on the first request with `ATSP0`, then `UNABLE TO CONNECT` or the data; and
`ATDPN` showing `A6` once found. Real adapters print `SEARCHING...` with a different timing and may print
`BUS INIT: ...` for K-line protocols they try first.

### G-UNABLE-NONCAN
Every protocol except 6 answers `UNABLE TO CONNECT` immediately. Real adapters try the protocol, print
`BUS INIT:` lines for ISO 9141/14230, and answer `UNABLE TO CONNECT` after seconds. Protocols 7 to 9 and
A to C are 29-bit or other bit rates; the simulated bus only carries 11-bit 500 kbit/s, so they cannot
connect either.

### G-RAW-MODE
`ATCAF0`: the data field is sent as typed (up to 8 bytes) and every accepted frame is printed whole.
Real behaviour around `ATCAF0` with `ATH0` (do headers still show, is the length byte shown) is unverified.

### G-DATA-ERROR
A multi-frame response that loses a consecutive frame or arrives with a bad sequence number: the
simulator prints the lines it assembled (none, if the message never completed) and `<DATA ERROR`.
The string, whether partial lines are printed, and whether the adapter retries are recollection.

### G-BUFFER-FULL
Two triggers for `BUFFER FULL`: an input line over 256 characters, and a first frame announcing more
bytes than the receive limit (the adapter then also sends flow control overflow). Real chips have a
smaller or larger buffer and may use `BUFFER FULL` for only one of the two.

### G-MULTI-FORMAT
Multi-frame replies with headers off: a three-digit hex length line, then rows `0:`, `1:`, ... of seven
bytes (six in the first row). With headers on: every raw frame on its own line, no length line, padding
bytes shown. Recollection of the ELM327 format; STN firmware may differ.

### G-HDR-PAD
`ATH1` single frames print the PCI byte and the padding the ECU sent (here 0x00). Real ECUs pad with
0x00, 0x55 or 0xAA or not at all; the adapter may or may not strip it.

### G-NO-RESPONSE
An unsupported PID gets silence, so the adapter says `NO DATA`. A real ECU may answer with a negative
response instead.

### G-UDS-SESSION-RESP
**Partly closed 2026-11-15** against ISO 14229-1:2013 (now in `primarysource` as `iso:14229-1@2013`): the layout is confirmed (sub-function, then P2Server_max in 1 ms units and P2*Server_max in 10 ms units, two bytes each; the standard's own example uses 50 ms). The values are still the simulator's.
`$10` positive response carries P2 = 50 ms and P2* = 5000 ms (`00 32 01 F4`). Format and values from memory
of ISO 14229-1; a real module reports its own timings.

### G-19-FORMAT
**Partly closed 2026-11-15** against ISO 14229-1:2013: the `$19` 01 response layout and the statusOfDTC bit names (0 testFailed, 2 pendingDTC, 3 confirmedDTC) are confirmed. The `$19` 02 layout and the failure-type byte were not re-read.
`$19` sub-functions 01 and 02: availability mask `FF`, DTC format identifier `01`, a failure-type byte of
`00`, status bits 0x08 confirmed and 0x04 pending. From memory of ISO 14229-1.

### G-UDS-NRC
**Partly closed 2026-11-15** against ISO 14229-1:2013: 0x11 serviceNotSupported, 0x12 subFunctionNotSupported, 0x13 incorrectMessageLengthOrInvalidFormat, 0x31 requestOutOfRange and 0x33 securityAccessDenied are confirmed by number and name.
Negative response codes `31` (request out of range), `13` (incorrect length), `12` (sub-function not
supported), `11` (service not supported). `31` is the one volt's capture shows for `$22`; the rest are
from memory.

### G-REFUSAL-NRC
A refused state-changing service is answered `7F <sid> 33` (securityAccessDenied) and an unknown service
`7F <sid> 11`. This is the simulator's policy, not a recording of any module: a real module would answer
according to its own gating, and over a relay the refusal might better be silence or a distinct code.
Configurable with `nrc <hex>` in the policy file.

### G-STN-STRINGS
`STI`, `STDI`, `STMFR`, `STSN` answer fixed synthetic strings (`STN2230 vSIM`, `FAKEOBD-SIM`, ...). Real
strings are device specific.

### G-ST-ACK
`STP`, `STCFCPA`, `STCFCPC` and the `STCSW`/`STSL`/`STBT` families are acknowledged with `OK`. `STCFCPA`
pairs do choose the flow-control address; the rest have no effect. `STP63`, which the Voltage app sends,
is acknowledged without knowing what protocol 63 means.

## Not annotated (no transcript entry pins it, or it is a default)

| Id | Simulator does | Unknown about hardware |
|---|---|---|
| G-DEFAULT-LF | Power-on state is echo on, linefeeds on, spaces on, headers off | ELM327 power-on linefeed default; STN may differ |
| G-TRAILING-SPACE | ELM327 profile prints a space after every data line when spaces are on; STN profile does not | Both unverified; normal-mode comparison ignores it, `exact` mode sees it |
| G-CRA-DEFAULT | Default receive filter accepts 7E8..7EF regardless of `ATSH` | Whether a real adapter derives a filter from the header |
| G-FC-DERIVE | Flow control goes to `ATSH` (physical) or response ID minus 8 (functional); `ATFCSH`/`ATFCSD` apply only in `ATFCSM` 1 and 2; `STCFCPA` pairs win | Real derivation, and which of the settings the Voltage app relies on |
| G-FC-OVERFLOW | ECU flow control overflow or too many waits gives `CAN ERROR` | Real adapter reaction |
| G-REPEAT-CR | A bare carriage return repeats the previous line | Real adapters repeat the last command only for some commands |
| G-STOPPED | In `--realtime`, a byte during a request abandons it with `STOPPED` | Which bytes and when |
| G-TIMEOUT | `ATST hh` is `hh * 4` ms, `ATST00` is the default 0x32; `ATAT` is acknowledged but timing stays fixed | Adaptive timing, STN differences |
| G-ACK-ONLY | `ATAL`, `ATAR`, `ATNL`, `ATM`, `ATR`, `ATCFC`, `ATCEA`, ... answer `OK` with no effect | Effect of each on a real adapter |
| G-FUNC-NRC | Functional requests are not answered with NRC `11`, `12`, `31` | **Confirmed against ISO 14229-1:2013 section 7.5.3.3** (they are always suppressed, with the in-active-session variants `7E`/`7F` as well, which the simulator does not model); real ECUs' behaviour is still unmeasured |
| G-PERIODIC | The simulator never originates traffic; no periodic or unsolicited frames | Real vehicles broadcast constantly; a real adapter's monitor mode would see it |
| G-29BIT | `ATSH` with 6 or 8 digits is accepted; nothing ever answers | 29-bit addressing is not modelled |
| G-ECU-DATA | Every value in the shipped profile is synthetic. The identifiers the 2026-11-14 trial logged are placed on the header the app used; that is the app's addressing, not a vehicle capture. Lengths other than the four demo values and the 44-byte `83E1` block were not recorded | Needs a vehicle |
| G-ECU-ABSENT | `621` still has no ECU: the DID appendix lists `ATSH621` and no response filter, so a response ID would be a guess. `241`/`641`, `24C`/`64C`, `7E1`/`7E9` and `7E7`/`7EF` are present with synthetic data | Wake frames on `621` and any real reply are unmeasured |
| G-GMLAN-AE | The lab policy admits nine exact service-`AE` frames and the ECU answers each with the single byte `EE`. The default policy answers `7F AE 33` (not allowlisted). An `AE` payload that is not one of the nine stays on that refusal. If a configuration allows the service and the payload is still unknown, the ECU answers `7F AE 11` and stores nothing. ISO 14229-1:2013 §7.3.2 Table 2 (read 2026-11-18) does not name service `AE`. §12.2 names service `0x2F`. `EE` sets bit 6 of `AE`, which §7.3.2 describes for a positive response of a service that table defines. GMLAN is not imported. `[unread]` | A captured positive reply for these frames |

## ELM327 versus STN (as modelled)

| Behaviour | `--adapter elm327` | `--adapter stn` |
|---|---|---|
| `ATI` | `ELM327 v1.5` | `ELM327 v1.4b` |
| `ST...` commands | `?` | the known ones answer, others `?` |
| Trailing space after data | yes | no |
| `STCFCPA` flow-control pairs | `?` | applied |
| `STP n` | `?` | one digit = `ATSP n`; longer acknowledged |

The Voltage app reacts to the difference: against `elm327` it sends `STP63` (answered `?`) and carries on;
against `stn` it additionally runs a single-wire CAN probe (`STP61`, `STCSWM2`, wake frames on `ATSH100`
and `ATSH621`, see AVD-TRIAL.md).
