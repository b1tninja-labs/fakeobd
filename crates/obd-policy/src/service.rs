//! Classification of diagnostic service identifiers.
//!
//! Service numbers below `0x10` are SAE J1979 (OBD-II) modes. Numbers from `0x10` up are UDS (ISO 14229)
//! or GMLAN services. The two ranges do not overlap, which is what lets one table cover a request whose
//! dialect is not known from the bytes alone.
//!
//! **Provenance.** Neither ISO 14229 nor SAE J1979 is in `primarysource`, so no row here is cited from a
//! standard. The set of identifiers is the one the Voltage app is documented to use (volt
//! `docs/OBD-CONFIG-DID-APPENDIX.md`: `10`, `11`, `14`, `19`, `22`, `27`, `2E`, `31`, `1A`) plus the
//! services any gateway has to have an opinion about. Every row is `[unread]` in ROADMAP.md.

/// What a service does to the module or the vehicle. The class never grants anything: it labels the audit
/// log and picks the negative response code. Admission is decided by [`crate::Policy`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Class {
    /// An OBD-II mode that only reads (01, 02, 03, 05, 06, 07, 09, 0A).
    ObdRead,
    /// A UDS/GMLAN service that only reads (19, 1A, 22, 23, 24).
    UdsRead,
    /// DiagnosticSessionControl (10): changes the module's session, not its data.
    Session,
    /// TesterPresent (3E).
    KeepAlive,
    /// Writes, resets, clears, routines, I/O control, programming, comms control, DTC-setting control.
    StateChange,
    /// SecurityAccess (27). Always refused: see [`crate::HARD_DENY`].
    Security,
    /// Not in the table. Refused by every default policy.
    Unknown,
}

/// Service identifiers that change module or vehicle state, or open a path that can.
///
/// Sorted. `04` and `08` are OBD-II modes (clear emission DTCs; control an on-board system); the rest are
/// UDS or GMLAN. `3B`, `20`, `A5`, `AE` are GMLAN rather than UDS and are recorded from memory.
pub const STATE_CHANGING: &[u8] = &[
    0x04, 0x08, 0x11, 0x14, 0x20, 0x28, 0x2A, 0x2C, 0x2E, 0x2F, 0x31, 0x34, 0x35, 0x36, 0x37, 0x38,
    0x3B, 0x3D, 0x83, 0x84, 0x85, 0x86, 0x87, 0xA5, 0xAE,
];

/// Classify a service identifier.
pub fn classify(sid: u8) -> Class {
    match sid {
        0x01 | 0x02 | 0x03 | 0x05 | 0x06 | 0x07 | 0x09 | 0x0A => Class::ObdRead,
        0x10 => Class::Session,
        0x3E => Class::KeepAlive,
        0x19 | 0x1A | 0x22 | 0x23 | 0x24 => Class::UdsRead,
        0x27 => Class::Security,
        s if STATE_CHANGING.contains(&s) => Class::StateChange,
        _ => Class::Unknown,
    }
}

/// Services whose request body can carry a secret or a payload worth not copying into a log.
pub fn payload_is_sensitive(sid: u8) -> bool {
    matches!(classify(sid), Class::StateChange | Class::Security)
}

/// Whether byte 1 of the request is a sub-function (low 7 bits) rather than data.
pub fn has_subfunction(sid: u8) -> bool {
    matches!(
        sid,
        0x10 | 0x11 | 0x19 | 0x27 | 0x28 | 0x31 | 0x3E | 0x83 | 0x85 | 0x86 | 0x87
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_changing_list_is_sorted_and_unique() {
        let mut v = STATE_CHANGING.to_vec();
        v.sort_unstable();
        v.dedup();
        assert_eq!(v, STATE_CHANGING);
    }

    #[test]
    fn every_listed_state_changer_classifies_as_one() {
        for &s in STATE_CHANGING {
            assert_eq!(classify(s), Class::StateChange, "{s:02X}");
        }
    }

    #[test]
    fn security_access_is_its_own_class() {
        assert_eq!(classify(0x27), Class::Security);
    }
}
