use elm::Flavor;
use fakeobd::{bind, serve, ServeOptions, Sim};
use obd_conform::{Tcp, Transport};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::{Arc, Mutex};
use std::time::Duration;

fn start(sim: Sim, opts: ServeOptions) -> String {
    let l = bind("127.0.0.1:0", false).unwrap();
    let addr = l.local_addr().unwrap().to_string();
    std::thread::spawn(move || {
        let _ = serve(l, sim, opts);
    });
    addr
}

fn ex(t: &mut Tcp, c: &str) -> String {
    String::from_utf8(t.exchange(c).unwrap()).unwrap()
}

#[test]
fn a_client_sees_the_full_dialogue_over_tcp() {
    let addr = start(Sim::new(Flavor::Elm327), ServeOptions::default());
    let mut t = Tcp::connect(&addr, false).unwrap();
    assert_eq!(ex(&mut t, "ATZ"), "ATZ\r\r\nELM327 v1.5\r\n\r\n>");
    assert_eq!(ex(&mut t, "ATE0"), "ATE0\rOK\r\n\r\n>");
    ex(&mut t, "ATSP6");
    ex(&mut t, "ATSH7E0");
    ex(&mut t, "ATL0");
    assert_eq!(ex(&mut t, "0105"), "41 05 5A \r\r>");
    assert_eq!(ex(&mut t, "2701"), "7F 27 33 \r\r>");
}

#[test]
fn connections_are_independent_vehicles() {
    let addr = start(Sim::new(Flavor::Elm327), ServeOptions::default());
    let (mut a, mut b) = (
        Tcp::connect(&addr, false).unwrap(),
        Tcp::connect(&addr, false).unwrap(),
    );
    for t in [&mut a, &mut b] {
        for c in ["ATE0", "ATL0", "ATSP6", "ATSH7E0"] {
            ex(t, c);
        }
    }
    assert_eq!(ex(&mut a, "1003"), "50 03 00 32 01 F4 \r\r>");
    assert_eq!(ex(&mut a, "225005"), "62 50 05 DE AD \r\r>");
    assert_eq!(
        ex(&mut b, "225005"),
        "7F 22 31 \r\r>",
        "the other connection is still in the default session"
    );
}

#[test]
fn the_banner_is_sent_on_connect_only_when_asked_for() {
    let mut sim = Sim::new(Flavor::Elm327);
    sim.config.banner_on_connect = true;
    let addr = start(sim, ServeOptions::default());
    let mut s = TcpStream::connect(&addr).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
    let mut buf = [0u8; 64];
    let n = s.read(&mut buf).unwrap();
    assert_eq!(&buf[..n], b"\r\nELM327 v1.5\r\n\r\n>");
}

#[test]
fn the_audit_sink_gets_one_line_per_request_and_refusals_are_marked() {
    #[derive(Clone)]
    struct Buf(Arc<Mutex<Vec<u8>>>);
    impl Write for Buf {
        fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(b);
            Ok(b.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let buf = Buf(Arc::default());
    let mut sim = Sim::new(Flavor::Elm327);
    sim.audit_sink = Some(Arc::new(Mutex::new(Box::new(buf.clone()))));
    let addr = start(sim, ServeOptions::default());
    let mut t = Tcp::connect(&addr, false).unwrap();
    for c in ["ATE0", "ATSP6", "ATSH7E0", "0105", "2701", "2EF1900102"] {
        ex(&mut t, c);
    }
    let text = String::from_utf8(buf.0.lock().unwrap().clone()).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 3, "{text}");
    assert!(lines[0].contains("\"verdict\":\"allow\""));
    assert!(lines[1].contains("\"reason\":\"hard-floor\""));
    assert!(
        lines[2].contains("\"verdict\":\"deny\"") && !lines[2].contains("0102"),
        "payload is not logged: {}",
        lines[2]
    );
}

#[test]
fn a_non_loopback_bind_is_refused_by_default() {
    let e = bind("0.0.0.0:0", false).expect_err("refused");
    assert_eq!(e.kind(), std::io::ErrorKind::PermissionDenied);
    assert!(bind("0.0.0.0:0", true).is_ok());
}

#[test]
fn the_adapter_can_drop_the_connection_mid_session() {
    let mut sim = Sim::new(Flavor::Elm327);
    sim.faults = vehicle::FaultPlan::parse("adapter drop_after=2").unwrap();
    let addr = start(sim, ServeOptions::default());
    let mut t = Tcp::connect(&addr, false).unwrap();
    ex(&mut t, "ATE0");
    assert!(t.exchange("ATSP6").is_err());
}

#[test]
fn realtime_mode_answers_stopped_when_a_byte_arrives_during_a_request() {
    let mut sim = Sim::new(Flavor::Elm327);
    sim.faults = vehicle::FaultPlan::parse("bus delay_ms=800").unwrap();
    let addr = start(
        sim,
        ServeOptions {
            realtime: true,
            verbose: false,
        },
    );
    let mut s = TcpStream::connect(&addr).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    s.write_all(b"ATE0\rATL0\rATSP6\rATSH7E0\r").unwrap();
    let mut seen = Vec::new();
    let mut buf = [0u8; 256];
    while String::from_utf8_lossy(&seen).matches('>').count() < 4 {
        let n = s.read(&mut buf).unwrap();
        seen.extend_from_slice(&buf[..n]);
    }
    // the request is busy for the response window; a byte during it abandons the request
    s.write_all(b"0105\r").unwrap();
    std::thread::sleep(Duration::from_millis(50));
    s.write_all(b"\r").unwrap();
    let mut out = Vec::new();
    while !String::from_utf8_lossy(&out).contains("STOPPED") {
        let n = s.read(&mut buf).unwrap();
        assert!(n > 0, "closed: {:?}", String::from_utf8_lossy(&out));
        out.extend_from_slice(&buf[..n]);
    }
    assert!(String::from_utf8_lossy(&out).starts_with("STOPPED"));
}
