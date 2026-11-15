use obd_policy::*;
use std::sync::{Arc, Mutex};

struct Collect(Arc<Mutex<Vec<AuditEntry>>>);
impl AuditSink for Collect {
    fn record(&mut self, e: &AuditEntry) {
        self.0.lock().unwrap().push(e.clone());
    }
}

fn ctx() -> Context {
    Context::new("7E0")
}

#[test]
fn decide_agrees_with_the_policy_for_all_256_service_ids() {
    let p = Policy::read_only();
    let mut f = Filter::new(p.clone());
    for sid in 0..=255u8 {
        let req = [sid, 1, 2, 3];
        let d = p.evaluate(&req);
        match f.decide(&req, &ctx()) {
            Action::Allow => assert!(d.allowed(), "{sid:02X}"),
            Action::Deny { sid: s, nrc } => {
                assert!(!d.allowed(), "{sid:02X}");
                assert_eq!(s, Some(sid));
                assert_eq!(Some(nrc), d.nrc);
            }
            Action::Rewrite(_) => panic!("no rewriter installed"),
        }
    }
    assert_eq!(f.audit().total(), 256);
}

#[test]
fn a_deny_carries_the_negative_response() {
    let mut f = Filter::new(Policy::read_only());
    let a = f.decide(&[0x27, 0x01], &ctx());
    assert_eq!(a.negative_response(), Some(vec![0x7F, 0x27, 0x33]));
    assert_eq!(f.decide(&[], &ctx()).negative_response(), None);
    assert!(matches!(
        f.decide(&[], &ctx()),
        Action::Deny { sid: None, .. }
    ));
}

#[test]
fn sinks_see_every_decision_with_the_origin() {
    let got = Arc::new(Mutex::new(Vec::new()));
    let mut f = Filter::new(Policy::read_only()).with_sink(Box::new(Collect(got.clone())));
    f.decide(&[0x22, 0xF1, 0x90], &Context::new("7E0"));
    f.decide(&[0x2E, 0xF1, 0x90, 0x01], &Context::new("7E4"));
    let v = got.lock().unwrap();
    assert_eq!(v.len(), 2);
    assert_eq!((v[0].origin.as_str(), v[0].allowed), ("7E0", true));
    assert_eq!((v[1].origin.as_str(), v[1].allowed), ("7E4", false));
}

#[test]
fn a_rewriter_is_judged_again_and_cannot_smuggle_a_refused_service() {
    let mut f = Filter::new(Policy::read_only()).with_rewriter(Box::new(|req, _| {
        if req == [0x22, 0x00, 0x01] {
            Some(vec![0x22, 0x00, 0x02])
        } else if req == [0x22, 0x00, 0x03] {
            Some(vec![0x27, 0x01])
        } else {
            None
        }
    }));
    assert_eq!(
        f.decide(&[0x22, 0, 1], &ctx()),
        Action::Rewrite(vec![0x22, 0, 2])
    );
    assert_eq!(f.decide(&[0x22, 0, 9], &ctx()), Action::Allow);
    assert!(matches!(
        f.decide(&[0x22, 0, 3], &ctx()),
        Action::Deny {
            sid: Some(0x27),
            ..
        }
    ));
}

#[test]
fn lines_are_forwarded_answered_or_refused() {
    let mut f = Filter::new(Policy::read_only());
    assert_eq!(f.decide_line("ATSH7E0", &ctx()), LineAction::Forward);
    assert_eq!(f.decide_line("", &ctx()), LineAction::Forward);
    assert_eq!(f.decide_line("22 41 AD", &ctx()), LineAction::Forward);
    assert_eq!(
        f.decide_line("27 01", &ctx()),
        LineAction::Respond("7F 27 33".into())
    );
    assert_eq!(f.decide_line("ATCAF0", &ctx()), LineAction::Refuse);
    assert_eq!(
        f.decide_line("STPX h:7E0, d:2701", &ctx()),
        LineAction::Refuse
    );
    assert_eq!(f.decide_line("nonsense!", &ctx()), LineAction::Refuse);
}

#[test]
fn a_rewritten_line_keeps_the_response_count_digit() {
    let mut f = Filter::new(Policy::read_only()).with_rewriter(Box::new(|r, _| {
        (r == [0x01, 0x00]).then(|| vec![0x01, 0x20])
    }));
    assert_eq!(
        f.decide_line("0100 1", &ctx()),
        LineAction::Rewrite("01201".into())
    );
}
