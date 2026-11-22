//! Python bindings for `fakeobd`, the ELM327/STN OBD-II/UDS simulator.
//!
//! The compiled module is imported as `fakeobd`. Build the wheel with maturin (see `pyproject.toml`),
//! or build the extension directly with `cargo build` and rename the cdylib to `fakeobd.pyd`
//! (Windows) / `fakeobd.so` (Unix).
//!
//! These bindings wrap the existing public surface of the Rust core: a [`Bench`] builds a simulated
//! vehicle behind an adapter (`fakeobd::Sim` + `elm::Elm`) and drives it line by line, exactly as the
//! TCP server and in-process `SimPipe` do. No socket, no thread, no new behaviour — a Python test
//! harness can stand the simulator up and exchange ELM327/ISO-TP traffic with it in-process.

use elm::Flavor;
use fakeobd::Sim;
use obd_policy::Policy;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyBytes;
use vehicle::{FaultPlan, VehicleSpec};

fn to_py_err(err: String) -> PyErr {
    PyValueError::new_err(err)
}

/// The built-in `volt-lab` vehicle profile, as profile-file text.
///
/// Pass it (or your own edited copy) as the `profile=` argument to [`Bench`]; it is also what a
/// `Bench` built with `profile=None` uses.
#[pyfunction]
fn default_profile() -> &'static str {
    vehicle::DEFAULT_PROFILE
}

/// Replay a golden transcript against a fresh in-process simulator configured from its header.
///
/// This mirrors `fakeobd verify`: the transcript's `@ adapter`, `@ vehicle`, `@ fault` and `@ policy`
/// header lines configure the bench, and every request/response pair is replayed strictly. Returns
/// `(ok, report)` where `ok` is whether every exchange matched and `report` is the rendered diff.
/// Raises `ValueError` for a malformed transcript or header.
#[pyfunction]
fn verify_transcript(text: &str) -> PyResult<(bool, String)> {
    let transcript = obd_conform::parse(text).map_err(to_py_err)?;
    let report = fakeobd::verify(&transcript).map_err(to_py_err)?;
    Ok((report.ok(), report.render()))
}

/// A simulated bench: one fake vehicle behind one ELM327/STN adapter, driven in-process.
///
/// Construct it with an adapter flavor and, optionally, a vehicle profile, a safety policy and a
/// fault plan (each as the text of the corresponding file format). Then drive it with [`Bench::send`]
/// (one command line) or [`Bench::feed`] (a raw byte stream of carriage-return-terminated lines),
/// taking the adapter's reply bytes back exactly as they would appear on the wire.
#[pyclass(unsendable)]
struct Bench {
    elm: elm::Elm,
    pending: Vec<u8>,
}

#[pymethods]
impl Bench {
    /// Build a bench.
    ///
    /// * `adapter` — `"elm327"` (default) or `"stn"`.
    /// * `profile` — vehicle profile text; `None` uses the built-in `volt-lab` profile.
    /// * `policy` — policy-file text; `None` uses the read-only default (`$27` is always refused).
    /// * `faults` — fault-plan text; `None` injects no faults.
    /// * `no_vehicle` — if true, clear the bus so the adapter answers with nothing on it.
    /// * `banner` — if true, the adapter prints its identity banner on connect (see [`Bench::banner`]).
    ///
    /// Raises `ValueError` for an unknown adapter or a profile/policy/fault text that does not parse.
    #[new]
    #[pyo3(signature = (adapter="elm327", *, profile=None, policy=None, faults=None, no_vehicle=false, banner=false))]
    fn new(
        adapter: &str,
        profile: Option<String>,
        policy: Option<String>,
        faults: Option<String>,
        no_vehicle: bool,
        banner: bool,
    ) -> PyResult<Self> {
        let flavor = match adapter {
            "elm327" => Flavor::Elm327,
            "stn" => Flavor::Stn,
            other => {
                return Err(PyValueError::new_err(format!(
                    "unknown adapter {other:?} (expected \"elm327\" or \"stn\")"
                )))
            }
        };
        let mut sim = Sim::new(flavor);
        if let Some(text) = profile {
            sim.spec = VehicleSpec::parse(&text).map_err(to_py_err)?;
        }
        if no_vehicle {
            sim.spec.ecus.clear();
        }
        if let Some(text) = policy {
            sim.policy = Policy::parse(&text).map_err(|e| to_py_err(e.to_string()))?;
        }
        if let Some(text) = faults {
            sim.faults = FaultPlan::parse(&text).map_err(to_py_err)?;
        }
        sim.config.banner_on_connect = banner;
        Ok(Bench {
            elm: sim.build(),
            pending: Vec::new(),
        })
    }

    /// The bytes a client sees on connect, before any command.
    ///
    /// Empty unless the bench was built with `banner=True`. A harness that models a real connection
    /// reads this once, then begins sending commands.
    fn banner<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new(py, &self.elm.banner())
    }

    /// Send one command line (no carriage return) and return the adapter's reply bytes.
    ///
    /// The line is an `AT`/`ST` command (`"ATZ"`, `"ATSP6"`, `"ATSH7E0"`, ...) or a hex request
    /// (`"0105"`, `"2701"`). The reply ends in the `>` prompt, exactly as the TCP server would send it.
    fn send<'py>(&mut self, py: Python<'py>, line: &str) -> Bound<'py, PyBytes> {
        let reply = self.elm.handle_line(line.as_bytes());
        PyBytes::new(py, &reply.bytes)
    }

    /// Feed a raw byte stream and return the concatenated replies for every complete line in it.
    ///
    /// Bytes are buffered across calls; each carriage-return-terminated line is dispatched and its
    /// reply appended to the returned chunk. A trailing partial line is held until the next call. This
    /// mirrors `fakeobd`'s in-process `SimPipe`: a harness can replay a captured byte script verbatim.
    fn feed<'py>(&mut self, py: Python<'py>, data: &[u8]) -> Bound<'py, PyBytes> {
        self.pending.extend_from_slice(data);
        let mut out: Vec<u8> = Vec::new();
        while let Some(pos) = self.pending.iter().position(|b| *b == b'\r') {
            let line: Vec<u8> = self.pending.drain(..=pos).take(pos).collect();
            let reply = self.elm.handle_line(&line);
            out.extend_from_slice(&reply.bytes);
        }
        PyBytes::new(py, &out)
    }
}

/// The `fakeobd` Python module.
#[pymodule]
#[pyo3(name = "fakeobd")]
fn bindings(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add(
        "__doc__",
        "Drive the fakeobd ELM327/STN OBD-II/UDS simulator in-process from Python.",
    )?;
    m.add_class::<Bench>()?;
    m.add_function(wrap_pyfunction!(default_profile, m)?)?;
    m.add_function(wrap_pyfunction!(verify_transcript, m)?)?;
    Ok(())
}
