# Trial: the Voltage app against fakeobd, in the emulator

Run once, 2026-11-14, by hand. Nothing here is automated, and it is not a test.

**Setup.** `fakeobd serve --listen 127.0.0.1:35000 -v` on the host; the rooted lab emulator (`emulator-5554`,
never the physical phone; every `adb` call carried `-s emulator-5554`); the Voltage app (`io.tripovan.voltage`
2.3.0) in **WiFi** mode, Host `10.0.2.2`, Port `35000`, following volt's `docs/AVD-OBD-BRIDGE.md`. The app's
saved preferences file was copied aside before and restored byte for byte after (`cmp` identical, saved
`connection_method` back to `BLE`); the app was left on the BLE selection and re-connected.

**Result: the app connected and ran against the simulator.** The status turned **Connected**, the app
completed its adapter initialisation, and then probed and polled continuously (about 2,500 request lines in
a minute against the `elm327` profile). It never reached a state where it displayed vehicle data: it showed
"Scan was not successful. Please try again." and kept cycling. That is expected: the simulator has no module
at several of the addresses the app uses, and no VIN answer on the module the app asks first.

## What the app sent (both profiles)

First pass, in order, on the `elm327` profile (replies as the simulator gave them):

| Phase | Lines | Simulator reply |
|---|---|---|
| Reset and format | `ATZ`, `ATL0`, `ATH0`, `ATS0`, `ATSP6`, `ATE0`, `ATCAF1` | normal |
| Probe for STN | `STP63` | `?` on `elm327`, `OK` on `stn` |
| Identify (`stn` only) | `STI`, `ATRV` | `STN2230 vSIM`, `12.4V` |
| Single-wire CAN probe (`stn` only) | `STP61`, `STCSWM2`, `ATSH100` + raw `00CCCCCCCCCCCCCC`, `ATSH621` + raw `00FFFFFFFF000000` and `01FFFFFFFF000000`, `STCSWM3` | `OK`/`NO DATA`; `STCSWM` was `?` until this trial, now acknowledged |
| VIN-style read | `ATSH7E4`, `1A90` (GMLAN `$1A`), then `ATSH241`, `ATCRA641`, `1A90` | `7F 1A 31`; `NO DATA` (no module at `241`) |
| Flow-control override | `ATAL`, `ATFCSH241`, `ATFCSD300014`, `ATFCSM1`, `1ADF`, `ATAR`, `ATFCSM0` | `OK`, `NO DATA` |
| Functional probe | `ATSH7DF`, `ATH1`, `1AB0`; `ATSH101`, `ATCF600`, `ATCM700`, `ATCAF0`, raw `FE021AB0` | `NO DATA` |
| Module scan | `ATSTFF`, `ATSH7E0` / `7E1` / `7E4` / `7E7`, `1A99`, `1ACC`, `1A6F`, `1A6D`, `1201`, `2701`, mode `03` | mostly `7F 1A 31`, `NO DATA` |

Findings worth keeping:

- **`$27 01` (SecurityAccess request seed) was sent to `7E0` and `7E4`** during the first probe cycle. The
  policy refused both (`hard-floor`, recorded in the audit log); the app carried on. The simulator never
  produced a seed.
- **`$1A` is the app's first read** (GMLAN ReadDataByIdentifier, identifiers `90`, `99`, `CC`, `DF`, `B0`,
  `6F`, `6D`). The default profile answers `1A 90` on `7E0` only, with a placeholder.
- **Modules addressed that the simulator lacks:** `241`->`641` (the app sets `ATCRA641`, `ATFCSH241`,
  `ATFCSD300014`), `621`, `24C`->`64C`, `7E1`, `7E7`. The volt DID appendix lists these headers; it does not
  say which DIDs live behind them.
- **`$22` polling is heavy and concentrated on `7E4`:** 21 distinct DIDs (`40E9`, `41A3`, `41A6`, `41AD`,
  `41C5`, `41C6`, `432D`, `434F`, `4356`, `4368`, `4369`, `437D`, `4389`, `43A5`, `43A6`, `43AE`, `43AF`,
  `4470`, `45FF`, `8334`, `83E2`), repeated every cycle. This is evidence, from the app's own traffic, for
  which module the volt-catalogued DIDs are read from; the shipped profile still places them by guess.
  `7E0` is asked `002F`; `7E1` and `621` are asked `2889`; `7E7` `4181`; `241` `8005`, `8090`, `809C`, `8537`,
  `CB31`.
- With `ATH1` the app sent mode `03` on `7DF` and received `7E8 04 43 01 0A 7E 00 00 00` and the `7EC`
  answer: framing and padding worked.
- The app reset the adapter (`ATZ`) and re-initialised repeatedly; it did not give up.

## Follow-up in the profile

The addresses and identifiers in the findings above are now in `profiles/volt-lab.profile`, with synthetic
bytes. `$1A` `90`, `99`, `CC`, `DF`, `B0`, `6F` and `6D` are answered on `7E0`, `7E4`, `7E1`, `7E7`, `241`
and `24C`. `90` is the placeholder `SYNTH000000000FKE`, not a VIN. `DF` is multi-frame. `99`, `CC`, `B0`, `6F`
and `6D` are four-character single frames. `7DF` `$1A B0` is answered by HPCM2 and BECM, the two functional ECUs. No `42xx`
value is decoded. `621` is still absent: the DID appendix lists `ATSH621` and no `ATCRA` pair. The added
ECUs are not functional, so a `7DF` mode 01 broadcast is still only HPCM2 and BECM.

## Not verified

- Whether these answers get the app to a data screen. The trial above was not re-run.
- The app's behaviour on `STOPPED`, `BUFFER FULL`, `<DATA ERROR` or a slow adapter (`--realtime`).
- `adb reverse` (the `10.0.2.2` route was the one used).
- Any claim about a real adapter or a real car.
