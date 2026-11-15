//! `vehicle`: a simulated vehicle on an 11-bit HS-CAN bus.
//!
//! ECUs answer SAE J1979 modes 01, 02, 03, 06, 07, 09, 0A and the UDS read services `$10`, `$19`, `$22`,
//! `$3E`, with ISO-TP segmentation and flow control. Every request passes the [`obd_policy::Policy`]
//! first. Data comes from a [`VehicleSpec`]; the shipped one is synthetic.
//!
//! No service here writes anything, and none ever computes a SecurityAccess key.

mod bus;
mod ecu;
mod fault;
mod profile;

pub use bus::{Bus, BusError};
pub use ecu::{bitmap, Ecu, Timed, FUNCTIONAL_ID};
pub use fault::{parse_fault_line, AdapterFault, BusFault, Effects, FaultKind, FaultPlan};
pub use profile::{
    dtc_code, dtc_text, DidSpec, DtcKind, DtcSpec, EcuSpec, VehicleSpec, DEFAULT_PROFILE,
};
