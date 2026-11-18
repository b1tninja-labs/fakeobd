use obd_policy::*;

/// The only service identifiers the default policy admits (with the sub-function rule for 10 below).
const ADMITTED: &[u8] = &[
    0x01, 0x02, 0x03, 0x05, 0x06, 0x07, 0x09, 0x0A, 0x10, 0x19, 0x1A, 0x22, 0x3E,
];

fn pdu(sid: u8) -> Vec<u8> {
    // Every service gets a sub-function/data byte 0x01 and a DID-like tail, so a request is never
    // refused for being short.
    vec![sid, 0x01, 0x02, 0x03]
}

#[test]
fn default_policy_admits_exactly_the_read_set_over_all_256_service_ids() {
    let p = Policy::read_only();
    for sid in 0..=255u8 {
        let d = p.evaluate(&pdu(sid));
        assert_eq!(
            d.allowed(),
            ADMITTED.contains(&sid),
            "service {sid:02X}: {d:?}"
        );
    }
}

#[test]
fn every_state_changing_service_is_refused_by_default() {
    let p = Policy::read_only();
    for &sid in STATE_CHANGING {
        let d = p.evaluate(&pdu(sid));
        assert!(!d.allowed(), "{sid:02X}");
        assert_eq!(d.class, Class::StateChange);
        assert!(d.nrc.is_some());
    }
}

#[test]
fn security_access_is_refused_under_every_policy_that_can_be_built() {
    let mut policies = vec![Policy::read_only(), Policy::deny_all()];
    // A policy that allows everything it is permitted to allow.
    let mut wide = Policy::deny_all();
    for sid in 0..=255u8 {
        let _ = wide.allow(sid, Subs::Any);
    }
    policies.push(wide);
    for p in &policies {
        for sub in 0..=255u8 {
            let d = p.evaluate(&[0x27, sub, 0xAA, 0xBB]);
            assert!(!d.allowed(), "27 {sub:02X}");
        }
        assert_eq!(
            p.evaluate(&[0x27, 0x01]).verdict,
            Verdict::Deny(Reason::HardFloor)
        );
    }
}

#[test]
fn allowing_security_access_is_an_error_not_a_silent_no_op() {
    let mut p = Policy::read_only();
    assert!(p.allow(0x27, Subs::Any).is_err());
    assert!(p.allow(0x27, Subs::Only(vec![1])).is_err());
    assert!(Policy::parse("allow 27").is_err());
}

#[test]
fn a_wide_policy_does_admit_a_write_when_the_owner_asks_for_it() {
    // The floor is one service, not a ban on configuration: a relay owner may allow the rest.
    let p = Policy::parse("allow 2E").unwrap();
    assert!(p.evaluate(&[0x2E, 0xD2, 0x1B, 0x00]).allowed());
    assert!(!p.evaluate(&[0x2F, 0xD2, 0x1B, 0x00]).allowed());
}

#[test]
fn session_control_admits_only_default_and_extended_for_all_256_sub_functions() {
    let p = Policy::read_only();
    for sub in 0..=255u8 {
        let d = p.evaluate(&[0x10, sub]);
        let base = sub & 0x7F;
        assert_eq!(d.allowed(), base == 0x01 || base == 0x03, "10 {sub:02X}");
        if !d.allowed() {
            assert_eq!(d.verdict, Verdict::Deny(Reason::SubfunctionNotAllowed));
            assert_eq!(d.nrc, Some(NRC_SUBFUNCTION_NOT_SUPPORTED));
        }
        assert_eq!(
            d.sub,
            Some(base),
            "the suppress bit is cleared in the decision"
        );
    }
    // A session request with no sub-function fails closed.
    assert!(!p.evaluate(&[0x10]).allowed());
}

#[test]
fn empty_request_is_refused() {
    let d = Policy::read_only().evaluate(&[]);
    assert_eq!(d.verdict, Verdict::Deny(Reason::Empty));
    assert_eq!(d.sid, None);
}

#[test]
fn deny_beats_allow() {
    let p = Policy::parse("deny 22").unwrap();
    assert_eq!(
        p.evaluate(&[0x22, 0xF1, 0x90]).verdict,
        Verdict::Deny(Reason::ExplicitDeny)
    );
    let p = Policy::parse("allow 10 01,02,03\ndeny 10 02").unwrap();
    assert!(p.evaluate(&[0x10, 0x03]).allowed());
    assert!(!p.evaluate(&[0x10, 0x02]).allowed());
}

#[test]
fn did_deny_ranges_apply_to_every_did_in_a_multi_did_read() {
    let p = Policy::parse("deny-did 42A0-42FF").unwrap();
    assert!(p.evaluate(&[0x22, 0x41, 0xAD]).allowed());
    assert!(p.evaluate(&[0x22, 0x42, 0x9F, 0x41, 0xAD]).allowed());
    let d = p.evaluate(&[0x22, 0x41, 0xAD, 0x42, 0xA0]);
    assert_eq!(d.verdict, Verdict::Deny(Reason::DidDenied));
    assert_eq!(d.nrc, Some(NRC_REQUEST_OUT_OF_RANGE));
    assert!(!p.evaluate(&[0x22, 0x42, 0xFF]).allowed());
}

#[test]
fn nrc_follows_the_kind_of_refusal_and_can_be_overridden() {
    let p = Policy::read_only();
    assert_eq!(
        p.evaluate(&[0x2E, 1, 2]).nrc,
        Some(NRC_SECURITY_ACCESS_DENIED)
    );
    assert_eq!(p.evaluate(&[0xF7, 1]).nrc, Some(NRC_SERVICE_NOT_SUPPORTED));
    let p = Policy::parse("nrc 22").unwrap();
    assert_eq!(p.evaluate(&[0x2E, 1, 2]).nrc, Some(0x22));
}

#[test]
fn parse_rejects_what_it_does_not_understand() {
    for bad in [
        "allow",
        "allow 2",
        "allow ZZ",
        "allow 10 0G",
        "allow-pdu",
        "allow-pdu ZZ",
        "allow-pdu AE3",
        "frob 22",
        "preset wide",
        "nrc",
        "deny-did X",
    ] {
        assert!(Policy::parse(bad).is_err(), "{bad}");
    }
    assert!(Policy::parse("# only a comment\n\n").is_ok());
}

#[test]
fn preset_none_refuses_everything() {
    let p = Policy::parse("preset none").unwrap();
    for sid in 0..=255u8 {
        assert!(!p.evaluate(&pdu(sid)).allowed());
    }
}

#[test]
fn audit_records_passed_and_refused_requests_in_order() {
    let p = Policy::read_only();
    let mut log = AuditLog::new(8);
    for req in [
        &[0x22u8, 0xF1, 0x90][..],
        &[0x2E, 0xD2, 0x1B, 0x55, 0x66],
        &[0x27, 0x01],
    ] {
        let d = p.evaluate(req);
        log.record("7E0", req, &d);
    }
    let v: Vec<_> = log.entries().collect();
    assert_eq!(v.len(), 3);
    assert_eq!(v.iter().map(|e| e.seq).collect::<Vec<_>>(), [1, 2, 3]);
    assert!(v[0].allowed && !v[1].allowed && !v[2].allowed);
    assert_eq!(v[2].reason, Some("hard-floor"));
    assert_eq!(log.denied(), 2);
}

#[test]
fn audit_does_not_keep_the_body_of_a_state_changing_request() {
    let p = Policy::read_only();
    let mut log = AuditLog::new(4);
    let req = [0x2E, 0xD2, 0x1B, 0xDE, 0xAD, 0xBE, 0xEF];
    let e = log.record("7E0", &req, &p.evaluate(&req)).clone();
    assert_eq!(e.request, "2ED21B");
    assert_eq!(e.len, 7);
    assert!(!e.to_json().to_uppercase().contains("DEADBEEF"));
    // a read keeps more
    let rd = [0x22u8, 0x41, 0xAD, 0x41, 0xC6];
    assert_eq!(
        log.record("7E0", &rd, &p.evaluate(&rd)).request,
        "2241AD41C6"
    );
}

#[test]
fn audit_ring_drops_the_oldest_but_keeps_counting() {
    let p = Policy::read_only();
    let mut log = AuditLog::new(2);
    for _ in 0..5 {
        log.record("7E0", &[0x01, 0x00], &p.evaluate(&[0x01, 0x00]));
    }
    assert_eq!(log.len(), 2);
    assert_eq!(log.total(), 5);
    assert_eq!(log.entries().next().unwrap().seq, 4);
}

#[test]
fn audit_sink_receives_one_json_line_per_decision() {
    use std::sync::{Arc, Mutex};
    #[derive(Clone)]
    struct Buf(Arc<Mutex<Vec<u8>>>);
    impl std::io::Write for Buf {
        fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(b);
            Ok(b.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let buf = Buf(Arc::default());
    let mut log = AuditLog::new(4).with_sink(Box::new(buf.clone()));
    let p = Policy::read_only();
    log.record(
        "7E0",
        &[0x14, 0xFF, 0xFF, 0xFF],
        &p.evaluate(&[0x14, 0xFF, 0xFF, 0xFF]),
    );
    let text = String::from_utf8(buf.0.lock().unwrap().clone()).unwrap();
    assert_eq!(text.lines().count(), 1);
    assert!(text.contains("\"verdict\":\"deny\"") && text.contains("\"sid\":\"14\""));
}

const LAB_POLICY: &str = include_str!("../../../profiles/lab-control.policy");

fn frames_in(text: &str) -> Vec<Vec<u8>> {
    text.lines()
        .filter_map(|raw| {
            let line = raw.split('#').next().unwrap_or("").trim();
            let mut it = line.split_whitespace();
            if it.next() != Some("allow-pdu") {
                return None;
            }
            let hex = it.next().expect("allow-pdu has a request");
            Some(
                (0..hex.len() / 2)
                    .map(|i| u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).unwrap())
                    .collect(),
            )
        })
        .collect()
}

#[test]
fn lab_file_keeps_security_access_on_the_hard_floor() {
    let frames = frames_in(LAB_POLICY);
    assert_eq!(frames.len(), 9);
    let window = [0xAE, 0x3B, 0xFF, 0x01, 0x01, 0x01, 0x01];
    let door = [0xAE, 0x01, 0x01, 0x01, 0x00, 0x00, 0x00];
    let port = [0xAE, 0x47, 0x02, 0x02, 0x00, 0x00, 0x00];
    for required in [&window[..], &door[..], &port[..]] {
        assert!(
            frames.iter().any(|f| f == required),
            "lab file is missing {required:02X?}"
        );
    }
    let lab = Policy::parse(LAB_POLICY).unwrap();
    let plain = Policy::read_only();
    for frame in &frames {
        assert_eq!(
            plain.evaluate(frame).verdict,
            Verdict::Deny(Reason::NotAllowlisted),
            "{frame:02X?}"
        );
        assert!(lab.evaluate(frame).allowed(), "{frame:02X?}");
    }
    let unknown = [0xAE, 0x10, 0x80, 0x80, 0x80, 0x80, 0x80];
    assert_eq!(
        lab.evaluate(&unknown).verdict,
        Verdict::Deny(Reason::NotAllowlisted)
    );
    assert!(lab.evaluate(&[0x3E, 0x00]).allowed());
    assert_eq!(
        lab.evaluate(&[0x27, 0x01]).verdict,
        Verdict::Deny(Reason::HardFloor)
    );
    assert!(Policy::parse("allow-pdu 27").is_err());
    assert!(Policy::parse("allow-pdu 2701").is_err());
    assert!(Policy::parse(&format!("{LAB_POLICY}\nallow 27\n")).is_err());
    assert!(Policy::parse(&format!("{LAB_POLICY}\nallow-pdu 2701\n")).is_err());
    let mut parsed = Policy::parse(LAB_POLICY).unwrap();
    assert!(parsed.allow(0x27, Subs::Any).is_err());
    assert!(parsed.allow_pdu(&[0x27, 0x01]).is_err());
    assert_eq!(
        parsed.evaluate(&[0x27, 0x01]).verdict,
        Verdict::Deny(Reason::HardFloor)
    );
}

#[test]
fn line_classifier_separates_requests_settings_and_bypasses() {
    assert_eq!(classify_line(""), Line::Empty);
    assert_eq!(classify_line("atsh 7e0"), Line::Adapter("ATSH7E0".into()));
    assert_eq!(
        classify_line("01 00"),
        Line::Request {
            pdu: vec![1, 0],
            max_responses: None
        }
    );
    assert_eq!(
        classify_line("0100 1"),
        Line::Request {
            pdu: vec![1, 0],
            max_responses: Some(1)
        }
    );
    assert!(matches!(classify_line("ATCAF0"), Line::Bypass(_)));
    assert!(matches!(classify_line("ATPP 0C SV 20"), Line::Bypass(_)));
    assert!(matches!(
        classify_line("STPX h:7E0, d:2701"),
        Line::Bypass(_)
    ));
    assert_eq!(classify_line("ATCAF1"), Line::Adapter("ATCAF1".into()));
    assert_eq!(classify_line("GARBAGE"), Line::Malformed);
    assert_eq!(classify_line("1"), Line::Malformed);
}
