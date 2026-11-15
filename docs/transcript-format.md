# Transcript format (version 1)

A transcript is plain text, one record per line, so a reviewer can read a diff. It pairs a request line with
the reply bytes, up to and including the `>` prompt.

```text
fakeobd-transcript 1
@ source: sim
@ scope: adapter
@ adapter: elm327
> ATI
< ATI\rELM327 v1.5\r\n\r\n>
~ gap G-IDENT
> ATRV
< *V\r\r>
~ match glob
```

| Line | Meaning |
|---|---|
| `fakeobd-transcript 1` | first line, required |
| `# ...` | comment |
| `@ key: value` | header, before or between entries; repeatable |
| `> text` | request as typed, without the carriage return; starts an entry |
| `< bytes` | the reply, escaped; exactly one per entry |
| `~ match normal\|exact\|glob` | how to compare (default `normal`) |
| `~ gap <id> [note]` | the entry is expected to differ on hardware; see [ground-truth-gaps.md](ground-truth-gaps.md) |

**Escapes** in `<`: `\r`, `\n`, `\\`, printable ASCII as is, anything else `\xHH`.

**Headers.** `source` is `sim` (written by this simulator, a statement about the simulator) or `hardware`
(captured from a real adapter, a statement about hardware). `scope` is `adapter` (answerable by an adapter
on a desk), `adapter-stn`, or `vehicle`. `adapter` is `elm327` or `stn` and selects the simulator profile
when `fakeobd verify` replays the transcript. `vehicle: none` removes every ECU. `fault: <line>` is
repeatable and injects a fault (the format of [fault files](architecture.md#fault-injection)).

**Match modes.**

- `normal`: split the reply on CR and LF, drop blank lines and the prompt, drop an echo of the request,
  trim, upper-case; compare line by line. Insensitive to echo, linefeed and trailing-space settings.
- `exact`: the bytes must be identical.
- `glob`: as `normal`, but `*` in a recorded line matches any run of characters; the line counts must agree.
  Use it for a reading that varies (`ATRV`).

**Gaps.** A mismatch on an entry with `~ gap` is reported `GAP`, not `FAIL`, when replaying against an
external target. A match on such an entry is reported `STALE` (the gap may have closed). `fakeobd verify`
treats both as failures or passes strictly: the simulator has no excuse for differing from itself.

## Commands

```text
fakeobd capture SCRIPT --connect HOST:PORT --out FILE [--source sim|hardware] [--adapter elm327|stn]
                       [--meta key=value]... [--allow-remote]
fakeobd replay  TRANSCRIPT --connect HOST:PORT [--allow-remote]      exit 1 on any FAIL
fakeobd compare EXPECTED ACTUAL                                      two captures, entry by entry
fakeobd verify  TRANSCRIPT...                                        replay against an in-process simulator
```

A capture script is one request line per line (`#` comments). `capture` and `replay` connect only to
loopback addresses unless `--allow-remote`: an ELM327 stream has no authentication, and these tools are
meant to be pointed at the simulator or at a local bridge.

## Validating the simulator against hardware (not yet done)

1. Put a known-good adapter behind a TCP bridge on loopback (volt's `docs/AVD-OBD-BRIDGE.md`), power it from
   a bench supply, nothing on the bus.
2. `fakeobd capture transcripts/scripts/bench.script --connect 127.0.0.1:35000 --source hardware --meta vehicle=none --out bench-hw.txt`
3. `fakeobd compare transcripts/bench.txt bench-hw.txt`. Every `FAIL` is a simulator bug or a gap to
   annotate; every `GAP` is a documented guess now measured.
4. Replace the sim transcript with the hardware one, remove the gap annotations that closed, change the
   simulator until `fakeobd verify` is green on the hardware transcript.

Vehicle-scope transcripts need a vehicle or a CAN simulator behind the adapter; their data will differ and
only the formats are comparable.
