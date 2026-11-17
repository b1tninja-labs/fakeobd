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

## Re-run 2026-11-17: a VIN answer is what gates the data screen

The follow-up question above ("whether these answers get the app to a data screen") now has an answer:
**yes, and the VIN is the gate.** Re-run on a different AVD (a Google Play Services image, same Android 14
fingerprint as the old lab; `io.tripovan.voltage` 2.3.0/14905, signed in), with one change to the setup: the
profile's VIN placeholders replaced by a real-shaped VIN on both paths the app reads it.

- **Setup.** `fakeobd serve --profile <copy of volt-lab.profile> --audit ... -v`, loopback as before. The copy
  is kept *outside* the repo deliberately, so the shipped profile stays all-synthetic and its header warning
  ("Do not read vehicle facts out of this file") still holds. `--profile` makes this a config change, not a
  code change; nothing in fakeobd needed editing.
- **App side.** WiFi mode, Host `10.0.2.2`, Port `35000`. The unsecure-adapter dialog is accepted by tapping
  OK and sets `ble_wifi_unsecure_acknowledged=true`; the host/port persist as `wifi_adapter_address` and
  `wifi_adapter_name`, both `10.0.2.2:35000`.
- **Result.** The app reached a live data screen: header shows **"2017 VOLT"** with an odometer reading, the
  Battery / EV Range / Fuel tiles render, and the button reads **Disconnect**. Status never went back to
  "Scan was not successful". One session ran ~33,000 audited requests before the app tore down with `ATAR`.
- **Which read supplies the VIN.** The app took it from **GMLAN `$1A 90` on `7E4`** (BECM), the *first* place
  it asks -- confirmed twice in the audit (`seq 1` and again at `seq 34`). It did **not** use mode `09` PID
  `02`, which was probed only by the host-side test client, not by the app. So a profile that answers `$1A 90`
  on `7E4` is sufficient; the earlier trial's failure to reach a data screen is explained by that one refusal.
- **Corollary for the profile.** Because `7E4` is answered first, the VIN must be correct *there*; a profile
  that puts it only on `7E0` will still look VIN-less to the app. The shipped placeholder is retained on every
  ECU so a scan is never a refusal.

Still not verified: whether a data screen plus a real VIN triggers the app's backend entitlement call (it did
not here -- see volt `docs/OBD-CONFIG.md`, "Re-tested on a fresh AVD"); and anything about a real adapter,
since fakeobd has still never been compared against one.
