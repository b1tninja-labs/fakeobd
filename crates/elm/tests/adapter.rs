use elm::{Config, Elm, Flavor};
use obd_policy::{AuditLog, Policy};
use vehicle::{Bus, FaultPlan, VehicleSpec};

fn elm(flavor: Flavor) -> Elm {
    let bus = Bus::new(
        &VehicleSpec::default_vehicle(),
        Policy::read_only(),
        AuditLog::new(64),
    );
    Elm::new(Config::new(flavor), bus)
}

/// Send one line, return the raw wire text.
fn tx(e: &mut Elm, line: &str) -> String {
    String::from_utf8(e.handle_line(line.as_bytes()).bytes).unwrap()
}

/// A bench with echo and linefeeds off, the way the Voltage app sets one up, protocol 6.
fn quiet() -> Elm {
    let mut e = elm(Flavor::Elm327);
    for c in ["ATE0", "ATL0", "ATSP6"] {
        assert!(tx(&mut e, c).contains("OK"));
    }
    e
}

#[test]
fn echo_and_linefeed_state_take_effect_after_their_own_line() {
    let mut e = elm(Flavor::Elm327);
    assert_eq!(
        tx(&mut e, "ATE0"),
        "ATE0\rOK\r\n\r\n>",
        "echo is on until ATE0 has been echoed"
    );
    assert_eq!(tx(&mut e, "ATL0"), "OK\r\r>");
    assert_eq!(tx(&mut e, "ATL1"), "OK\r\n\r\n>");
    assert_eq!(tx(&mut e, "ATE1"), "OK\r\n\r\n>");
    assert_eq!(tx(&mut e, "ATI"), "ATI\rELM327 v1.5\r\n\r\n>");
}

#[test]
fn atz_restores_power_on_state_and_prints_the_banner() {
    let mut e = quiet();
    tx(&mut e, "ATH1");
    assert_eq!(
        tx(&mut e, "ATZ"),
        "\r\nELM327 v1.5\r\n\r\n>",
        "the reboot prints with power-on settings"
    );
    assert_eq!(
        tx(&mut e, "ATI"),
        "ATI\rELM327 v1.5\r\n\r\n>",
        "echo and linefeeds are back on"
    );
    assert_eq!(
        tx(&mut e, "ATDPN"),
        "ATDPN\rA0\r\n\r\n>",
        "protocol is automatic again"
    );
}

#[test]
fn unknown_commands_and_st_on_a_plain_elm327_are_question_marks() {
    let mut e = quiet();
    assert_eq!(tx(&mut e, "ATXYZ"), "?\r\r>");
    assert_eq!(tx(&mut e, "STI"), "?\r\r>");
    assert_eq!(tx(&mut e, "ATSH12345"), "?\r\r>");
    assert_eq!(tx(&mut e, "ATSP"), "?\r\r>");
    assert_eq!(tx(&mut e, "ATSPD"), "?\r\r>");
    assert_eq!(tx(&mut e, "GARBAGE!"), "?\r\r>");
    assert_eq!(tx(&mut e, "ATE2"), "?\r\r>");
}

#[test]
fn stn_answers_the_st_family_and_identifies_as_an_elm_clone() {
    let mut e = elm(Flavor::Stn);
    tx(&mut e, "ATE0");
    assert_eq!(tx(&mut e, "ATI"), "ELM327 v1.4b\r\n\r\n>");
    assert_eq!(tx(&mut e, "STI"), "STN2230 vSIM\r\n\r\n>");
    assert_eq!(tx(&mut e, "STDI"), "FAKEOBD-SIM\r\n\r\n>");
    assert_eq!(tx(&mut e, "STCFCPA 7E0,7E8"), "OK\r\n\r\n>");
    assert_eq!(tx(&mut e, "STP 6"), "OK\r\n\r\n>");
    assert_eq!(tx(&mut e, "STNOPE"), "?\r\n\r\n>");
}

#[test]
fn spaces_and_case_in_commands_are_ignored() {
    let mut e = quiet();
    assert_eq!(tx(&mut e, "at sh 7e0"), "OK\r\r>");
    assert_eq!(tx(&mut e, "01 05"), "41 05 5A \r\r>");
}

#[test]
fn single_frame_formats_for_spaces_headers() {
    let mut e = quiet();
    tx(&mut e, "ATSH7E0");
    assert_eq!(tx(&mut e, "0105"), "41 05 5A \r\r>");
    tx(&mut e, "ATS0");
    assert_eq!(tx(&mut e, "0105"), "41055A\r\r>");
    tx(&mut e, "ATH1");
    assert_eq!(
        tx(&mut e, "0105"),
        "7E8034105 5A00000000\r\r>".replace(' ', ""),
        "id, raw frame with PCI and padding"
    );
    tx(&mut e, "ATS1");
    assert_eq!(tx(&mut e, "0105"), "7E8 03 41 05 5A 00 00 00 00 \r\r>");
}

#[test]
fn multi_frame_response_prints_a_length_line_and_numbered_rows_without_headers() {
    let mut e = quiet();
    tx(&mut e, "ATSH7E0");
    assert_eq!(
        tx(&mut e, "0902"),
        "014\r0: 49 02 01 31 47 31 \r1: 52 42 36 53 35 35 48 \r2: 55 30 30 30 30 30 30 \r\r>"
    );
    tx(&mut e, "ATS0");
    assert_eq!(
        tx(&mut e, "0902"),
        "014\r0:490201314731\r1:52423653353548\r2:55303030303030\r\r>".replace(' ', "")
    );
}

#[test]
fn multi_frame_response_with_headers_lists_every_raw_frame() {
    let mut e = quiet();
    tx(&mut e, "ATSH7E0");
    tx(&mut e, "ATH1");
    assert_eq!(
        tx(&mut e, "0902"),
        "7E8 10 14 49 02 01 31 47 31 \r7E8 21 52 42 36 53 35 35 48 \r7E8 22 55 30 30 30 30 30 30 \r\r>"
    );
}

#[test]
fn automatic_protocol_searches_once_then_remembers() {
    let mut e = elm(Flavor::Elm327);
    tx(&mut e, "ATE0");
    tx(&mut e, "ATL0");
    assert_eq!(tx(&mut e, "ATDPN"), "A0\r\r>");
    assert_eq!(tx(&mut e, "ATDP"), "AUTO\r\r>");
    assert_eq!(
        tx(&mut e, "0100"),
        "SEARCHING...\r41 00 08 18 80 01 \r41 00 00 00 00 01 \r\r>"
    );
    assert_eq!(tx(&mut e, "ATDPN"), "A6\r\r>");
    assert_eq!(tx(&mut e, "ATDP"), "AUTO, ISO 15765-4 (CAN 11/500)\r\r>");
    assert!(!tx(&mut e, "0100").contains("SEARCHING"));
}

#[test]
fn automatic_search_with_nothing_on_the_bus_is_unable_to_connect() {
    let mut e = elm(Flavor::Elm327);
    tx(&mut e, "ATE0");
    tx(&mut e, "ATL0");
    assert_eq!(tx(&mut e, "ATSH7E3"), "OK\r\r>");
    assert_eq!(
        tx(&mut e, "0100"),
        "SEARCHING...\r\rUNABLE TO CONNECT\r\r>".replace("\r\rUN", "\rUN")
    );
}

#[test]
fn protocols_other_than_can_11_500_cannot_connect() {
    let mut e = quiet();
    for p in ["ATSP3", "ATSP5", "ATSP7", "ATSP9"] {
        tx(&mut e, p);
        assert_eq!(tx(&mut e, "0100"), "UNABLE TO CONNECT\r\r>", "{p}");
    }
    assert_eq!(tx(&mut e, "ATSPA6"), "OK\r\r>");
    assert_eq!(tx(&mut e, "ATDPN"), "A0\r\r>");
    tx(&mut e, "ATSP6");
    assert_eq!(tx(&mut e, "ATDP"), "ISO 15765-4 (CAN 11/500)\r\r>");
}

#[test]
fn physical_header_without_a_matching_receive_filter_gets_nothing() {
    let mut e = quiet();
    tx(&mut e, "ATSH244");
    assert_eq!(
        tx(&mut e, "22 83 E1"),
        "NO DATA\r\r>",
        "644 is outside the default 7E8..7EF filter"
    );
    assert_eq!(tx(&mut e, "ATCRA644"), "OK\r\r>");
    let r = tx(&mut e, "22 83 E1");
    assert!(r.starts_with("02F\r0: 62 83 E1 00 01 02 "), "{r:?}");
    assert_eq!(tx(&mut e, "ATCRA"), "OK\r\r>");
    assert_eq!(
        tx(&mut e, "22 83 E1"),
        "NO DATA\r\r>",
        "ATCRA with no argument restores the default filter"
    );
}

#[test]
fn cf_and_cm_build_a_filter() {
    let mut e = quiet();
    tx(&mut e, "ATSH7DF");
    tx(&mut e, "ATCF7EC");
    tx(&mut e, "ATCM7FF");
    assert_eq!(
        tx(&mut e, "0142"),
        "41 42 33 90 \r\r>",
        "only the BECM passes"
    );
}

#[test]
fn functional_request_collects_every_answer_and_a_trailing_digit_stops_early() {
    let mut e = quiet();
    tx(&mut e, "ATSH7DF");
    let all = tx(&mut e, "0142");
    assert_eq!(all.matches("41 42 33 90").count(), 2);
    let r = e.handle_line(b"01421");
    let text = String::from_utf8(r.bytes).unwrap();
    assert_eq!(text.matches("41 42 33 90").count(), 1);
    let full = e.handle_line(b"0142");
    assert!(
        r.busy_ms < 100 && full.busy_ms >= 200,
        "stopping early is faster than waiting out the timeout"
    );
}

#[test]
fn st_sets_the_response_window_and_a_slow_ecu_misses_it() {
    let mut e = quiet();
    e.set_faults(&FaultPlan::parse("bus delay_ms=300").unwrap());
    tx(&mut e, "ATSH7E0");
    assert_eq!(tx(&mut e, "0105"), "NO DATA\r\r>", "default 200 ms window");
    assert_eq!(tx(&mut e, "ATST 96"), "OK\r\r>");
    assert_eq!(tx(&mut e, "0105"), "41 05 5A \r\r>", "0x96 * 4 ms = 600 ms");
    assert_eq!(tx(&mut e, "ATST00"), "OK\r\r>");
    assert_eq!(
        tx(&mut e, "0105"),
        "NO DATA\r\r>",
        "ATST00 restores the default"
    );
}

#[test]
fn response_pending_is_printed_and_the_real_answer_follows_if_in_time() {
    let mut e = quiet();
    e.set_faults(&FaultPlan::parse("bus pending_ms=100").unwrap());
    tx(&mut e, "ATSH7E0");
    assert_eq!(tx(&mut e, "22 41 AD"), "7F 22 78 \r62 41 AD 00 00 \r\r>");
    let mut e = quiet();
    e.set_faults(&FaultPlan::parse("bus pending_ms=1000").unwrap());
    tx(&mut e, "ATSH7E0");
    assert_eq!(
        tx(&mut e, "22 41 AD"),
        "7F 22 78 \r\r>",
        "the answer came after the window closed"
    );
}

#[test]
fn can_error_is_reported_once() {
    let mut e = quiet();
    e.set_faults(&FaultPlan::parse("bus request=1 can_error").unwrap());
    tx(&mut e, "ATSH7E0");
    assert_eq!(tx(&mut e, "0105"), "CAN ERROR\r\r>");
    assert_eq!(tx(&mut e, "0105"), "41 05 5A \r\r>");
}

#[test]
fn a_lost_consecutive_frame_prints_what_arrived_and_a_data_error() {
    let mut e = quiet();
    e.set_faults(&FaultPlan::parse("bus drop_frame=2").unwrap());
    tx(&mut e, "ATSH7E0");
    assert_eq!(tx(&mut e, "0902"), "<DATA ERROR\r\r>");
    let mut e = quiet();
    e.set_faults(&FaultPlan::parse("bus bad_sequence=1").unwrap());
    tx(&mut e, "ATSH7E0");
    assert_eq!(tx(&mut e, "0902"), "<DATA ERROR\r\r>");
}

#[test]
fn a_response_longer_than_the_receive_buffer_is_buffer_full() {
    let bus = Bus::new(
        &VehicleSpec::default_vehicle(),
        Policy::read_only(),
        AuditLog::new(8),
    );
    let mut cfg = Config::new(Flavor::Elm327);
    cfg.rx_limit = 16;
    let mut e = Elm::new(cfg, bus);
    for c in ["ATE0", "ATL0", "ATSP6", "ATSH7E0"] {
        tx(&mut e, c);
    }
    assert_eq!(tx(&mut e, "0902"), "BUFFER FULL\r\r>");
}

#[test]
fn an_over_long_input_line_is_buffer_full() {
    let mut e = quiet();
    let long = "0".repeat(300);
    assert_eq!(tx(&mut e, &long), "BUFFER FULL\r\r>");
    assert_eq!(
        tx(&mut e, "ATI"),
        "ELM327 v1.5\r\r>",
        "the adapter is still usable"
    );
}

#[test]
fn a_bare_carriage_return_repeats_the_last_command() {
    let mut e = quiet();
    tx(&mut e, "ATSH7E0");
    assert_eq!(tx(&mut e, "0105"), "41 05 5A \r\r>");
    assert_eq!(tx(&mut e, ""), "41 05 5A \r\r>");
    let mut fresh = elm(Flavor::Elm327);
    let r = tx(&mut fresh, "");
    assert!(
        r.ends_with('>') && !r.contains("OK") && !r.contains('?'),
        "{r:?}"
    );
}

#[test]
fn stopped_follows_the_linefeed_setting() {
    let mut e = elm(Flavor::Elm327);
    assert_eq!(e.stopped(), b"STOPPED\r\n\r\n>");
    tx(&mut e, "ATL0");
    assert_eq!(e.stopped(), b"STOPPED\r\r>");
}

#[test]
fn uds_reads_pass_the_adapter_and_state_changes_come_back_refused() {
    let mut e = quiet();
    tx(&mut e, "ATSH7E0");
    assert_eq!(tx(&mut e, "1003"), "50 03 00 32 01 F4 \r\r>");
    assert_eq!(tx(&mut e, "225005"), "62 50 05 DE AD \r\r>");
    assert_eq!(tx(&mut e, "2701"), "7F 27 33 \r\r>");
    assert_eq!(tx(&mut e, "2ED21B0102"), "7F 2E 33 \r\r>");
    assert_eq!(tx(&mut e, "3101"), "7F 31 33 \r\r>");
    assert_eq!(e.bus().audit().denied(), 3);
}

#[test]
fn flow_control_data_from_atfcsd_is_what_the_ecu_sees() {
    let mut e = quiet();
    tx(&mut e, "ATSH7E0");
    // default flow control: the whole VIN
    assert!(tx(&mut e, "0902").starts_with("014\r"));
    // user flow control with a block size and an STmin makes the response slower, not different
    let base = e.handle_line(b"0902").busy_ms;
    tx(&mut e, "ATFCSM1");
    tx(&mut e, "ATFCSH7E0");
    assert_eq!(tx(&mut e, "ATFCSD300014"), "OK\r\r>");
    let slow = e.handle_line(b"0902");
    assert!(
        String::from_utf8_lossy(&slow.bytes).starts_with("014\r"),
        "{:?}",
        slow
    );
    assert!(
        slow.busy_ms > base,
        "STmin 0x14 = 20 ms between frames: {} vs {base}",
        slow.busy_ms
    );
    // a block size of 1 means the adapter must send a flow control after every frame; the answer is whole
    assert_eq!(tx(&mut e, "ATFCSD300100"), "OK\r\r>");
    assert!(tx(&mut e, "0902").starts_with(
        "014\r0: 49 02 01 31 47 31 \r1: 52 42 36 53 35 35 48 \r2: 55 30 30 30 30 30 30 "
    ));
}

#[test]
fn flow_control_data_that_is_not_a_flow_control_frame_stalls_the_transfer() {
    let mut e = quiet();
    tx(&mut e, "ATSH7E0");
    tx(&mut e, "ATFCSM1");
    tx(&mut e, "ATFCSD4F0000");
    assert_eq!(tx(&mut e, "0902"), "<DATA ERROR\r\r>");
    // and with the mode back to default the same request works again
    tx(&mut e, "ATFCSM0");
    assert!(tx(&mut e, "0902").starts_with("014\r"));
}

#[test]
fn a_multi_frame_request_obeys_the_ecus_flow_control() {
    let mut e = quiet();
    tx(&mut e, "ATSH7E0");
    let long = "22 41AD 41C6 43AF 002F";
    let ok = tx(&mut e, long);
    assert_eq!(
        ok,
        "62 41 AD 00 00 41 C6 00 00 00 00 43 AF F8 00 00 2F 6C \r\r>".replace(
            "62 41 AD 00 00 41 C6 00 00 00 00 43 AF F8 00 00 2F 6C \r\r>",
            "012\r0: 62 41 AD 00 00 41 \r1: C6 00 00 00 00 43 AF \r2: F8 00 00 2F 6C \r\r>"
        )
    );
    for (fault, want) in [
        ("bus fc_overflow", "CAN ERROR\r\r>"),
        ("bus fc_wait=3", "012\r"),
        ("bus fc_wait=20", "CAN ERROR\r\r>"),
        ("bus fc_block=1,05", "012\r"),
        ("bus silent", "NO DATA\r\r>"),
    ] {
        let mut e = quiet();
        e.set_faults(&FaultPlan::parse(fault).unwrap());
        tx(&mut e, "ATSH7E0");
        assert!(tx(&mut e, long).starts_with(want), "{fault}");
    }
}

#[test]
fn adapter_reset_mid_session_forgets_state_and_leaves_the_request_unanswered() {
    let mut e = quiet();
    e.set_faults(&FaultPlan::parse("adapter reset_after=5").unwrap());
    tx(&mut e, "ATSH7E0"); // line 4
    assert_eq!(tx(&mut e, "0105"), "\r\rELM327 v1.5\r\r>");
    assert_eq!(
        tx(&mut e, "ATDPN"),
        "ATDPN\rA0\r\n\r\n>",
        "echo and protocol are back to power-on"
    );
}

#[test]
fn adapter_can_drop_the_connection() {
    let mut e = quiet();
    e.set_faults(&FaultPlan::parse("adapter drop_after=4").unwrap());
    let r = e.handle_line(b"ATSH7E0");
    assert!(r.close && r.bytes.is_empty());
}

#[test]
fn raw_mode_sends_the_data_field_as_given_and_prints_whole_frames() {
    let mut e = quiet();
    tx(&mut e, "ATSH7E0");
    assert_eq!(tx(&mut e, "ATCAF0"), "OK\r\r>");
    assert_eq!(tx(&mut e, "02 01 05"), "03 41 05 5A 00 00 00 00 \r\r>");
    assert_eq!(tx(&mut e, "ATCAF1"), "OK\r\r>");
}

#[test]
fn session_timers_run_on_virtual_time_across_requests() {
    let mut e = quiet();
    tx(&mut e, "ATSH7E0");
    tx(&mut e, "1003");
    assert_eq!(tx(&mut e, "225005"), "62 50 05 DE AD \r\r>");
    e.bus_mut().advance(6000);
    assert_eq!(tx(&mut e, "225005"), "7F 22 31 \r\r>");
}

#[test]
fn stn_flow_control_pairs_choose_the_flow_control_address() {
    let mut e = elm(Flavor::Stn);
    for c in [
        "ATE0",
        "ATL0",
        "ATSP6",
        "ATSH7DF",
        "STCFCPA 7E0,7E8",
        "ATCRA7E8",
    ] {
        tx(&mut e, c);
    }
    // functional request, one responder passes the filter, flow control goes to 7E0 by pair
    assert!(tx(&mut e, "0902").starts_with("014\r"));
}
