# fakeobd (Python bindings)

Python bindings for [`fakeobd`](../), the fake vehicle behind an ELM327/STN adapter. The simulator is
implemented in Rust; this package exposes its in-process driving surface to Python via
[pyo3](https://pyo3.rs), so a test harness can stand the simulator up and exchange ELM327/ISO-TP
traffic with it without a socket, an adapter, a car or Bluetooth.

The bindings are a **separate, excluded crate**: the core workspace forbids `unsafe` and its five Rust
gates run `cargo … --workspace`, so these bindings (which pyo3's macros need `unsafe` for) live outside
that workspace and have their own CI job. The Rust core carries no pyo3 dependency.

## Build

From this `py/` directory, with [maturin](https://www.maturin.rs):

```
pip install maturin
maturin develop        # build and install into the active environment
pytest                 # run tests/test_fakeobd.py
```

`maturin build --out dist` produces an abi3 wheel (CPython 3.9+). Alternatively, `cargo build` produces
the cdylib directly; rename it to `fakeobd.pyd` (Windows) / `fakeobd.so` (Unix) on the import path.

## Use

```python
import fakeobd

bench = fakeobd.Bench("elm327")          # or "stn"; optional profile=/policy=/faults=/no_vehicle=/banner=

bench.send("ATE0")                        # b'...OK...>'  (turn echo off)
bench.send("ATSP6")                       # b'OK...>'     (ISO 15765-4 CAN 11/500)
bench.send("ATSH7E0")                     # b'OK...>'
bench.send("0105")                        # b'41 05 5A ...>'  (engine coolant temperature)
bench.send("2701")                        # b'7F 27 ...>'     (SecurityAccess is always refused)

# Or replay a raw byte script of carriage-return-terminated lines:
bench.feed(b"ATZ\r0100\r")                # b'...ELM327 v1.5...>...>'

fakeobd.default_profile()                 # the built-in volt-lab profile as text
ok, report = fakeobd.verify_transcript(open("../transcripts/bench.txt").read())
```

`Bench(..., banner=True)` makes `bench.banner()` return the adapter's connect banner (empty otherwise).

An unknown adapter name, or profile/policy/fault text that does not parse, raises `ValueError`. The
bindings add no capability the simulator does not already have: `$27` is refused and no write service
is implemented, under every configuration.
