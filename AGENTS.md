# Instructions for coding agents

`fakeobd` is a Rust workspace: a fake vehicle behind an ELM327/STN adapter. Read `docs/SCOPE.md` before
changing anything: it lists what must never be built here. This file takes precedence over `D:\code\AGENTS.md`
for work in this repository.

## Commands

Run these from this directory (never from `D:\code`, never a formatter or `clippy --fix` outside it). All
must pass before a change is done.

    cargo fmt --check
    cargo clippy --workspace --all-targets -- -D warnings
    cargo test --workspace
    cargo run -p fakeobd -- verify transcripts/*.txt
    python tools/break_check.py          # every [ok] row: break the code, the named test must fail

## Scope (hard limits)

- **`$27` SecurityAccess is refused by `obd_policy::HARD_DENY` under every configuration.** Do not write a
  seed-to-key algorithm, a key generator, an entitlement check, or a table of keys, in any form or under any
  name, including as a "test fixture". Do not make `$27` configurable.
- No write services are implemented in an ECU (`$2E`, `$2F`, `$31`, `$11`, `$14`, `$28`, `$34`-`$37`,
  `$85`...). The policy refuses them; an ECU that is configured to let them through answers
  `serviceNotSupported`. Adding an implementation is a scope decision, not a feature.
- Never open a serial port, Bluetooth device or ADB connection from code or tests. The TCP client connects to
  loopback unless `--allow-remote`. The physical phone and any real adapter or vehicle are out of bounds; the
  emulator is `emulator-5554` and every `adb` call carries `-s emulator-5554`.
- No VINs, Bluetooth addresses, serial numbers or personal paths in the tree. The VIN-like string in the
  profile is a placeholder.

## Invariants

- **Most standards are not read.** `primarysource` has ISO 14229-1:2013, ISO 15765-2:2016 and ISO 15031-3:2004
  (`iso_*` tools: read a clause before writing a rule that depends on it). ISO 15765-2:2016 §9 is the transport
  layer protocol. Single-frame transmission is §9.2 (§9.2.1 when the transmit data length is 8, §9.2.2 when it
  is greater). §9.3 is multiple-frame transmission. A 7-byte single frame is the §9.2.1 normal-addressing case,
  not every case. That document does not define `ATSP6`, a `+0x400` response offset, or a bit rate. ISO 15031-3:2004
  is the diagnostic connector. SAE J1979:2002 is an unindexed scan: do not cite mode numbers from it, and do not
  change replies to match it. GMLAN and the ELM327/STN datasheets are not imported, and the `gm_*` corpus needs
  `primarysource gm import` before it answers. Do not cite a section or quote normative wording from memory; mark the row `[unread]` in
  ROADMAP.md and add or update the gap in `docs/ground-truth-gaps.md`. RFC questions go through the `rfc_*`
  tools.
- A row is `[ok]` only when a named test exists and failed when the code it covers was deliberately broken.
  The break is a numbered mutation in `tools/break_check.py`; add one when you add an `[ok]` row, and run it.
- Golden transcripts in `transcripts/` are reviewed by reading, then frozen. Do not regenerate one blindly
  after a behaviour change: read the diff of every changed reply. `fakeobd verify` replays them strictly
  (a `~ gap` annotation does not excuse a simulator difference).
- Any new `~ gap <id>` needs a heading `### <id>` in `docs/ground-truth-gaps.md` (a test checks).
- The policy runs before any ECU service. Keep `obd-policy` free of dependencies on the rest.
- First-party code only; say why in the commit message before adding a dependency.
- `unsafe` is forbidden by the lint table. Do not relax it.
- Write scratch output outside the repository. Never commit with `-A` when another agent is in the tree;
  check `git rev-parse --show-toplevel`.
- Commit messages carry no AI attribution trailer.
- Registration in projecthub/PORTFOLIO is done by the owner of the workspace root, not from here;
  see `docs/REGISTRATION.md`.
