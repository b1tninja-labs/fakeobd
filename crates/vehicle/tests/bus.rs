use isotp::{decode, segment, CanFrame, FlowControl, FlowStatus, Pdu, Reassembler, Rx};
use obd_policy::{AuditLog, Policy};
use vehicle::{Bus, BusError, FaultPlan, Timed, VehicleSpec};

fn bus() -> Bus {
    Bus::new(
        &VehicleSpec::default_vehicle(),
        Policy::read_only(),
        AuditLog::new(64),
    )
}

fn frame(id: u32, data: &[u8]) -> CanFrame {
    CanFrame {
        id,
        data: data.to_vec(),
    }
}

/// Send one request PDU as the tester would and return every complete response per ECU ID, in arrival
/// order. Flow control is answered with clear-to-send.
fn request(bus: &mut Bus, id: u32, pdu: &[u8]) -> Vec<(u32, Vec<u8>)> {
    let seg = segment(pdu, Some(0)).unwrap();
    let mut inbox: Vec<Timed> = bus.send(&frame(id, &seg.first)).unwrap();
    if !seg.rest.is_empty() {
        // wait for the ECU's flow control, then send every consecutive frame
        let fc = inbox
            .iter()
            .position(|t| matches!(decode(&t.frame.data), Ok(Pdu::Flow(_))))
            .expect("ECU asked for flow control");
        inbox.remove(fc);
        for cf in &seg.rest {
            inbox.extend(bus.send(&frame(id, cf)).unwrap());
        }
    }
    collect(bus, id, inbox)
}

fn collect(bus: &mut Bus, req_id: u32, mut inbox: Vec<Timed>) -> Vec<(u32, Vec<u8>)> {
    let mut done = Vec::new();
    let mut chans: std::collections::BTreeMap<u32, Reassembler> = Default::default();
    let mut i = 0;
    inbox.sort_by_key(|t| t.at_ms);
    while i < inbox.len() {
        let t = inbox[i].clone();
        i += 1;
        let r = chans.entry(t.frame.id).or_default();
        match r.push(&t.frame.data, 4095) {
            Ok(Rx::Complete(p)) => done.push((t.frame.id, p)),
            Ok(Rx::NeedFlow { .. }) => {
                let target = if req_id == 0x7DF {
                    t.frame.id - 8
                } else {
                    req_id
                };
                inbox.extend(
                    bus.send(&frame(target, &FlowControl::CTS.encode()))
                        .unwrap(),
                );
            }
            Ok(_) | Err(_) => {}
        }
    }
    done
}

fn one(r: Vec<(u32, Vec<u8>)>) -> Vec<u8> {
    assert_eq!(r.len(), 1, "{r:02X?}");
    r.into_iter().next().unwrap().1
}

#[test]
fn supported_pid_bitmap_matches_the_profile() {
    let r = one(request(&mut bus(), 0x7E0, &[0x01, 0x00]));
    // PIDs 05, 0C, 0D, 11 in range 1..=20, and the chain bit for 21..=40 (PID 42 exists).
    assert_eq!(r, [0x41, 0x00, 0x08, 0x18, 0x80, 0x01]);
    let r = one(request(&mut bus(), 0x7E0, &[0x01, 0x40]));
    assert_eq!(r, [0x41, 0x40, 0x40, 0x00, 0x00, 0x00]);
}

#[test]
fn physical_request_is_answered_by_that_ecu_only() {
    let r = request(&mut bus(), 0x7E0, &[0x01, 0x05]);
    assert_eq!(r, [(0x7E8, vec![0x41, 0x05, 0x5A])]);
    // the cluster does not answer on 7E0
    assert!(request(&mut bus(), 0x244, &[0x01, 0x05]).is_empty());
}

#[test]
fn functional_request_is_answered_by_every_functional_ecu_and_not_the_cluster() {
    let mut b = bus();
    let mut r = request(&mut b, 0x7DF, &[0x01, 0x42]);
    r.sort();
    assert_eq!(
        r,
        [
            (0x7E8, vec![0x41, 0x42, 0x33, 0x90]),
            (0x7EC, vec![0x41, 0x42, 0x33, 0x90])
        ]
    );
}

#[test]
fn an_unsupported_pid_gets_no_answer_and_several_pids_share_one_answer() {
    let mut b = bus();
    assert!(request(&mut b, 0x7E0, &[0x01, 0x0F]).is_empty());
    let r = one(request(&mut b, 0x7E0, &[0x01, 0x05, 0x0F, 0x0C]));
    assert_eq!(r, [0x41, 0x05, 0x5A, 0x0C, 0x1A, 0xF8]);
}

#[test]
fn mode_09_vin_is_multi_frame_and_needs_flow_control() {
    let mut b = bus();
    let seg = segment(&[0x09, 0x02], Some(0)).unwrap();
    let first = b.send(&frame(0x7E0, &seg.first)).unwrap();
    assert_eq!(first.len(), 1);
    assert_eq!(
        &first[0].frame.data[..2],
        [0x10, 0x14],
        "first frame announces 20 bytes"
    );
    // no consecutive frames until flow control arrives
    let after = b.send(&frame(0x7E0, &FlowControl::CTS.encode())).unwrap();
    assert_eq!(after.len(), 2);
    assert_eq!(after[0].frame.data[0], 0x21);
    assert_eq!(after[1].frame.data[0], 0x22);
    let mut b = bus();
    let r = one(request(&mut b, 0x7E0, &[0x09, 0x02]));
    assert_eq!(&r[..3], [0x49, 0x02, 0x01]);
    assert_eq!(&r[3..], b"SYNTH000000000FKE");
}

#[test]
fn block_size_and_stmin_from_the_tester_are_obeyed() {
    let mut b = bus();
    let seg = segment(&[0x22, 0x83, 0xE1], Some(0)).unwrap();
    b.send(&frame(0x244, &seg.first)).unwrap();
    let fc = FlowControl {
        status: FlowStatus::ContinueToSend,
        block_size: 2,
        st_min: 0x0A,
    };
    let blk = b.send(&frame(0x244, &fc.encode())).unwrap();
    assert_eq!(blk.len(), 2, "block size 2");
    assert_eq!(blk[1].at_ms - blk[0].at_ms, 10, "STmin 10 ms");
    // 47 bytes = 6 + 41 -> 6 consecutive frames; 4 left after the first block of 2
    let blk = b.send(&frame(0x244, &fc.encode())).unwrap();
    assert_eq!(blk.len(), 2);
    let blk = b.send(&frame(0x244, &FlowControl::CTS.encode())).unwrap();
    assert_eq!(blk.len(), 2, "the rest");
    assert!(b
        .send(&frame(0x244, &FlowControl::CTS.encode()))
        .unwrap()
        .is_empty());
}

#[test]
fn read_did_single_and_multi_frame_and_unknown() {
    let mut b = bus();
    assert_eq!(
        one(request(&mut b, 0x7E0, &[0x22, 0x41, 0xAD])),
        [0x62, 0x41, 0xAD, 0x00, 0x00]
    );
    assert_eq!(
        one(request(&mut b, 0x7E0, &[0x22, 0x00, 0x2F, 0x43, 0xAF])),
        [0x62, 0x00, 0x2F, 0x6C, 0x43, 0xAF, 0xF8, 0x00]
    );
    assert_eq!(
        one(request(&mut b, 0x7E0, &[0x22, 0xFF, 0xFF])),
        [0x7F, 0x22, 0x31]
    );
    let cluster = one(request(&mut b, 0x244, &[0x22, 0x83, 0xE1]));
    assert_eq!(cluster.len(), 3 + 44);
    assert_eq!(cluster[3 + 43], 0x2B);
    // transmit data length 8, normal addressing: longer than seven data bytes is multi-frame
    let r = one(request(
        &mut b,
        0x7E0,
        &[0x22, 0x41, 0xAD, 0x41, 0xC6, 0x43, 0xAF, 0x00, 0x2F],
    ));
    assert_eq!(r.len(), 1 + 4 + 6 + 4 + 3, "{r:02X?}");
    assert_eq!(request(&mut b, 0x7E0, &[0x22, 0x41]).len(), 1);
    assert_eq!(
        one(request(&mut b, 0x7E0, &[0x22, 0x41])),
        [0x7F, 0x22, 0x13]
    );
}

#[test]
fn extended_session_unlocks_the_extended_only_did_and_the_s3_timer_takes_it_back() {
    let mut b = bus();
    assert_eq!(
        one(request(&mut b, 0x7E0, &[0x22, 0x50, 0x05])),
        [0x7F, 0x22, 0x31]
    );
    assert_eq!(
        one(request(&mut b, 0x7E0, &[0x10, 0x03])),
        [0x50, 0x03, 0x00, 0x32, 0x01, 0xF4]
    );
    assert_eq!(
        one(request(&mut b, 0x7E0, &[0x22, 0x50, 0x05])),
        [0x62, 0x50, 0x05, 0xDE, 0xAD]
    );
    b.advance(4000);
    assert_eq!(one(request(&mut b, 0x7E0, &[0x3E, 0x00])), [0x7E, 0x00]);
    b.advance(4000);
    assert_eq!(
        one(request(&mut b, 0x7E0, &[0x22, 0x50, 0x05])).len(),
        5,
        "TesterPresent kept the session alive across 8 s"
    );
    b.advance(6000);
    assert_eq!(
        one(request(&mut b, 0x7E0, &[0x22, 0x50, 0x05])),
        [0x7F, 0x22, 0x31],
        "S3 expired"
    );
}

#[test]
fn session_control_variants() {
    let mut b = bus();
    assert!(
        request(&mut b, 0x7E0, &[0x10, 0x83]).is_empty(),
        "suppress positive response"
    );
    assert_eq!(
        one(request(&mut b, 0x7E0, &[0x10, 0x02])),
        [0x7F, 0x10, 0x12],
        "refused by policy"
    );
    assert!(request(&mut b, 0x7E0, &[0x3E, 0x80]).is_empty());
}

#[test]
fn dtc_services() {
    let mut b = bus();
    assert_eq!(
        one(request(&mut b, 0x7E0, &[0x03])),
        [0x43, 0x01, 0x0A, 0x7E]
    );
    assert_eq!(
        one(request(&mut b, 0x7E0, &[0x07])),
        [0x47, 0x01, 0x0A, 0xE3]
    );
    assert_eq!(one(request(&mut b, 0x7E0, &[0x0A])), [0x4A, 0x00]);
    assert_eq!(
        one(request(&mut b, 0x7E4, &[0x19, 0x02, 0xFF])),
        [0x59, 0x02, 0xFF, 0x0A, 0x0D, 0x00, 0x08]
    );
    assert_eq!(
        one(request(&mut b, 0x7E4, &[0x19, 0x01, 0x08])),
        [0x59, 0x01, 0xFF, 0x01, 0x00, 0x01]
    );
    assert_eq!(
        one(request(&mut b, 0x7E4, &[0x19, 0x01, 0x04])),
        [0x59, 0x01, 0xFF, 0x01, 0x00, 0x00]
    );
}

#[test]
fn mode_06_and_mode_02() {
    let mut b = bus();
    let r = one(request(&mut b, 0x7E0, &[0x06, 0xA1]));
    assert_eq!(&r[..2], [0x46, 0xA1]);
    assert_eq!(r.len(), 2 + 16);
    assert_eq!(
        one(request(&mut b, 0x7E0, &[0x06, 0xA0])),
        [0x46, 0xA0, 0x80, 0, 0, 0]
    );
    assert_eq!(
        one(request(&mut b, 0x7E0, &[0x02, 0x05, 0x00])),
        [0x42, 0x05, 0x00, 0x5A]
    );
    assert_eq!(
        one(request(&mut b, 0x7E0, &[0x09, 0x00])),
        [0x49, 0x00, 0x50, 0x40, 0x00, 0x00]
    );
}

#[test]
fn every_state_changing_request_is_refused_and_audited_and_changes_nothing() {
    let mut b = bus();
    let before = one(request(&mut b, 0x7E0, &[0x22, 0x41, 0xAD]));
    let attacks: &[&[u8]] = &[
        &[0x27, 0x01],
        &[0x27, 0x02, 0xAA, 0xBB, 0xCC, 0xDD, 0xEE],
        &[0x2E, 0xD2, 0x1B, 0x01, 0x02],
        &[0x2F, 0x01, 0x02, 0x03],
        &[0x31, 0x01, 0x52, 0x47],
        &[0x11, 0x01],
        &[0x14, 0xFF, 0xFF, 0xFF],
        &[0x04],
        &[0x28, 0x03, 0x01],
        &[0x34, 0x00, 0x44, 0, 0, 0, 0],
        &[0x85, 0x02],
        &[0x10, 0x02],
    ];
    for a in attacks {
        let r = one(request(&mut b, 0x7E0, a));
        assert_eq!(r[0], 0x7F, "{a:02X?}");
        assert_eq!(r[1], a[0]);
        let nrc = if a[0] == 0x10 { 0x12 } else { 0x33 };
        assert_eq!(
            r[2], nrc,
            "the refusal comes from the policy, not from the ECU: {a:02X?}"
        );
    }
    assert_eq!(one(request(&mut b, 0x7E0, &[0x22, 0x41, 0xAD])), before);
    let audit = b.audit();
    assert_eq!(
        audit.total(),
        2 + attacks.len() as u64,
        "every request is recorded"
    );
    assert_eq!(audit.denied(), attacks.len());
    assert!(audit
        .entries()
        .filter(|e| e.sid == Some(0x27))
        .all(|e| e.reason == Some("hard-floor")));
}

#[test]
fn a_policy_that_allows_a_write_still_gets_service_not_supported_from_the_ecu() {
    let policy = Policy::parse("allow 2E\nallow 11").unwrap();
    let mut b = Bus::new(&VehicleSpec::default_vehicle(), policy, AuditLog::new(8));
    assert_eq!(
        one(request(&mut b, 0x7E0, &[0x2E, 0xD2, 0x1B, 0x00])),
        [0x7F, 0x2E, 0x11]
    );
    assert_eq!(
        one(request(&mut b, 0x7E0, &[0x11, 0x01])),
        [0x7F, 0x11, 0x11]
    );
    assert_eq!(b.audit().denied(), 0);
}

#[test]
fn functional_requests_do_not_get_the_suppressed_negative_responses() {
    let mut b = bus();
    assert!(request(&mut b, 0x7DF, &[0x22, 0xFF, 0xFF]).is_empty());
    assert!(request(&mut b, 0x7DF, &[0xF7, 0x00]).is_empty());
    // a refusal with the "security access denied" code is still sent
    assert_eq!(request(&mut b, 0x7DF, &[0x2E, 0xD2, 0x1B]).len(), 2);
}

fn faulted(text: &str) -> Bus {
    let mut b = bus();
    b.set_faults(&FaultPlan::parse(text).unwrap());
    b
}

#[test]
fn fault_silent_and_request_scoping() {
    let mut b = faulted("bus request=2 ecu=7E8 silent");
    assert_eq!(request(&mut b, 0x7E0, &[0x01, 0x05]).len(), 1);
    assert!(request(&mut b, 0x7E0, &[0x01, 0x05]).is_empty());
    assert_eq!(request(&mut b, 0x7E0, &[0x01, 0x05]).len(), 1);
}

#[test]
fn fault_drop_frame_loses_a_consecutive_frame_so_reassembly_never_completes() {
    let mut b = faulted("bus drop_frame=2");
    assert!(request(&mut b, 0x7E0, &[0x09, 0x02]).is_empty());
    // dropping the first frame loses the whole message
    let mut b = faulted("bus drop_frame=0");
    assert!(request(&mut b, 0x7E0, &[0x01, 0x05]).is_empty());
}

#[test]
fn fault_bad_sequence_breaks_reassembly() {
    let mut b = faulted("bus bad_sequence=1");
    assert!(request(&mut b, 0x7E0, &[0x09, 0x02]).is_empty());
}

#[test]
fn fault_delay_and_pending() {
    let mut b = faulted("bus delay_ms=500");
    let seg = segment(&[0x01, 0x05], Some(0)).unwrap();
    let t = b.send(&frame(0x7E0, &seg.first)).unwrap();
    assert_eq!(t[0].at_ms, 503);
    let mut b = faulted("bus pending_ms=1200");
    let t = b.send(&frame(0x7E0, &seg.first)).unwrap();
    assert_eq!(t.len(), 2);
    assert_eq!(&t[0].frame.data[..4], [0x03, 0x7F, 0x01, 0x78]);
    assert_eq!(t[1].at_ms - t[0].at_ms, 1200);
}

#[test]
fn fault_can_error_fails_the_transmit_for_one_request_only() {
    let mut b = faulted("bus request=1 can_error");
    let seg = segment(&[0x01, 0x05], Some(0)).unwrap();
    assert_eq!(b.send(&frame(0x7E0, &seg.first)), Err(BusError::CanError));
    assert!(b.send(&frame(0x7E0, &seg.first)).is_ok());
}

#[test]
fn flow_control_faults_when_the_ecu_receives_a_multi_frame_request() {
    let req = [0x22, 0x41, 0xAD, 0x41, 0xC6, 0x43, 0xAF, 0x00, 0x2F];
    let seg = segment(&req, Some(0)).unwrap();
    let ff = |b: &mut Bus| b.send(&frame(0x7E0, &seg.first)).unwrap();

    let t = ff(&mut faulted("bus fc_overflow"));
    assert_eq!(t[0].frame.data[..3], [0x32, 0, 0]);

    let t = ff(&mut faulted("bus fc_wait=2"));
    let kinds: Vec<u8> = t.iter().map(|x| x.frame.data[0]).collect();
    assert_eq!(kinds, [0x31, 0x31, 0x30]);
    assert!(t[2].at_ms > t[1].at_ms);

    let t = ff(&mut faulted("bus fc_block=4,0A"));
    assert_eq!(t[0].frame.data[..3], [0x30, 4, 0x0A]);
}

#[test]
fn a_tester_overflow_flow_control_abandons_the_transfer() {
    let mut b = bus();
    let seg = segment(&[0x09, 0x02], Some(0)).unwrap();
    b.send(&frame(0x7E0, &seg.first)).unwrap();
    let fc = FlowControl {
        status: FlowStatus::Overflow,
        block_size: 0,
        st_min: 0,
    };
    assert!(b.send(&frame(0x7E0, &fc.encode())).unwrap().is_empty());
    assert!(
        b.send(&frame(0x7E0, &FlowControl::CTS.encode()))
            .unwrap()
            .is_empty(),
        "nothing left to send"
    );
}

#[test]
fn malformed_frames_are_ignored_not_answered() {
    let mut b = bus();
    for data in [
        &[][..],
        &[0x00, 1][..],
        &[0x0F, 1, 2][..],
        &[0x40, 1][..],
        &[0x25, 1][..],
    ] {
        assert!(
            b.send(&frame(0x7E0, data)).unwrap().is_empty(),
            "{data:02X?}"
        );
    }
}

#[test]
fn gmlan_read_data_by_identifier_answers_from_the_lid_table() {
    let mut b = bus();
    let r = one(request(&mut b, 0x7E0, &[0x1A, 0x90]));
    assert_eq!(&r[..2], [0x5A, 0x90]);
    assert_eq!(&r[2..], b"SYNTH000000000FKE");
    let r99 = one(request(&mut b, 0x7E0, &[0x1A, 0x99]));
    assert_eq!(&r99[..2], [0x5A, 0x99]);
    assert_eq!(&r99[2..], b"SY99");
    assert_eq!(
        one(request(&mut b, 0x7E0, &[0x1A, 0xFE])),
        [0x7F, 0x1A, 0x31]
    );
    let becm = one(request(&mut b, 0x7E4, &[0x1A, 0x90]));
    assert_eq!(
        &becm[2..],
        b"SYNTH000000000FKE",
        "the app asks BECM for $1A 90 first"
    );
    assert_eq!(one(request(&mut b, 0x7E0, &[0x1A])), [0x7F, 0x1A, 0x13]);
}

#[test]
fn a_physical_only_ecu_ignores_the_functional_id() {
    let mut b = bus();
    // The cluster has no mode 01 PIDs, but PID 00 is still a single-frame bitmap, so a functional
    // request would show its response id if it answered. 83E1 would not: it is multi-frame, and the
    // test helper's flow control for 7DF goes to the response id minus 8, which is not 244.
    // 4368 is also on BECM, so it no longer isolates the cluster.
    let ids: Vec<u32> = request(&mut b, 0x7DF, &[0x01, 0x00])
        .into_iter()
        .map(|(id, _)| id)
        .collect();
    assert!(
        !ids.contains(&0x644),
        "the cluster answered a functional request: {ids:03X?}"
    );
    assert_eq!(request(&mut b, 0x244, &[0x22, 0x43, 0x68]).len(), 1);
}

#[test]
fn trial_addresses_answer_the_requests_the_app_sent() {
    let mut b = bus();
    let vin = b"SYNTH000000000FKE";

    let becm = request(&mut b, 0x7E4, &[0x1A, 0x90]);
    assert_eq!(becm.len(), 1);
    assert_eq!(becm[0].0, 0x7EC);
    assert_eq!(&becm[0].1[2..], vin);

    let m241 = request(&mut b, 0x241, &[0x1A, 0x90]);
    assert_eq!(m241.len(), 1);
    assert_eq!(m241[0].0, 0x641);
    assert_eq!(&m241[0].1[2..], vin);

    let df = one(request(&mut b, 0x241, &[0x1A, 0xDF]));
    assert_eq!(&df[..2], [0x5A, 0xDF]);
    assert!(
        df.len() > 7,
        "the app sets a flow-control override before $1A DF: {df:02X?}"
    );

    for did in [0x8005u16, 0x8090, 0x809C, 0x8537, 0xCB31] {
        let r = one(request(&mut b, 0x241, &[0x22, (did >> 8) as u8, did as u8]));
        assert_eq!(&r[..3], [0x62, (did >> 8) as u8, did as u8], "{did:04X}");
    }
    for did in [
        0x40E9u16, 0x41A3, 0x41A6, 0x41AD, 0x41C5, 0x41C6, 0x432D, 0x434F, 0x4356, 0x4368, 0x4369,
        0x437D, 0x4389, 0x43A5, 0x43A6, 0x43AE, 0x43AF, 0x4470, 0x45FF, 0x8334, 0x83E2,
    ] {
        let r = one(request(&mut b, 0x7E4, &[0x22, (did >> 8) as u8, did as u8]));
        assert_eq!(r[0], 0x62, "{did:04X} {r:02X?}");
    }
    assert_eq!(
        one(request(&mut b, 0x7E4, &[0x22, 0x41, 0xAD])),
        [0x62, 0x41, 0xAD, 0x00, 0x00]
    );
    assert_eq!(
        one(request(&mut b, 0x7E4, &[0x22, 0x41, 0xC6])),
        [0x62, 0x41, 0xC6, 0x00, 0x00, 0x00, 0x00]
    );
    assert_eq!(
        one(request(&mut b, 0x7E4, &[0x22, 0x43, 0xAF])),
        [0x62, 0x43, 0xAF, 0xF8, 0x00]
    );

    let m7e1 = request(&mut b, 0x7E1, &[0x22, 0x28, 0x89]);
    assert_eq!(m7e1.len(), 1);
    assert_eq!(m7e1[0].0, 0x7E9);
    assert_eq!(&m7e1[0].1[..3], [0x62, 0x28, 0x89]);
    let m7e7 = request(&mut b, 0x7E7, &[0x22, 0x41, 0x81]);
    assert_eq!(m7e7.len(), 1);
    assert_eq!(m7e7[0].0, 0x7EF);
    assert_eq!(&m7e7[0].1[..3], [0x62, 0x41, 0x81]);
    let scan_lids = [
        (0x90u8, &b"SYNTH000000000FKE"[..]),
        (0x99, b"SY99"),
        (0xCC, b"SYCC"),
        (0xDF, b"SYNTH1ADF0000001"),
        (0xB0, b"SYB0"),
        (0x6F, b"SY6F"),
        (0x6D, b"SY6D"),
    ];
    for addr in [0x7E0u32, 0x7E4, 0x7E1, 0x7E7, 0x241, 0x24C] {
        for (id, data) in scan_lids {
            let r = one(request(&mut b, addr, &[0x1A, id]));
            assert_eq!(&r[..2], [0x5A, id], "{addr:03X} {id:02X}");
            assert_eq!(&r[2..], data, "{addr:03X} {id:02X}");
        }
    }
    let broadcast = request(&mut b, 0x7DF, &[0x1A, 0xB0]);
    assert_eq!(broadcast.len(), 2, "{broadcast:02X?}");
    assert!(broadcast.iter().all(|(_, p)| &p[2..] == b"SYB0"));

    let m24c = request(&mut b, 0x24C, &[0x3E, 0x00]);
    assert_eq!(m24c.len(), 1);
    assert_eq!(m24c[0].0, 0x64C);
    assert_eq!(m24c[0].1, [0x7E, 0x00]);
    // No response id is recorded for 621. An identifier the profile does not list stays negative.
    assert!(request(&mut b, 0x621, &[0x22, 0x28, 0x89]).is_empty());
    assert_eq!(
        one(request(&mut b, 0x7E0, &[0x1A, 0xFE])),
        [0x7F, 0x1A, 0x31]
    );
    // The added ECUs are not on the functional broadcast.
    assert_eq!(request(&mut b, 0x7DF, &[0x01, 0x00]).len(), 2);
    assert_eq!(request(&mut b, 0x7DF, &[0x2E, 0xD2, 0x1B]).len(), 2);
}
