use elm::Flavor;
use fakeobd::Sim;
use obd_policy::{Filter, Policy};
use serial_bridge::{PolicyStream, SimStream};
use serial_stream::ByteStream;
use std::time::Duration;

fn recv(rx: &std::sync::mpsc::Receiver<Vec<u8>>) -> String {
    String::from_utf8(rx.recv_timeout(Duration::from_secs(2)).expect("reply")).unwrap()
}

#[test]
fn the_simulator_is_a_byte_stream() {
    let mut s = SimStream::new(&Sim::new(Flavor::Elm327));
    let rx = s.receiver().unwrap();
    s.write(b"ATE0\rATL0\rATSP6\rATSH7E0\r0105\r").unwrap();
    for _ in 0..4 {
        recv(&rx);
    }
    assert_eq!(recv(&rx), "41 05 5A \r\r>");
    s.close();
    s.close();
    assert!(s.write(b"ATI\r").is_err());
}

#[test]
fn a_relay_between_two_streams_refuses_what_the_policy_refuses() {
    // The backend here is the simulator with a policy that would answer anything; the relay's own policy
    // is what refuses.
    let mut sim = Sim::new(Flavor::Elm327);
    sim.policy = Policy::parse("allow 2E").unwrap();
    let mut relay = PolicyStream::new(SimStream::new(&sim), Filter::new(Policy::read_only()), "relay");
    let rx = relay.receiver().unwrap();
    relay.write(b"ATE0\rATL0\rATSP6\rATSH7E0\r").unwrap();
    for _ in 0..4 {
        recv(&rx);
    }
    relay.write(b"0105\r").unwrap();
    assert_eq!(recv(&rx), "41 05 5A \r\r>");
    relay.write(b"2E F1 90 01\r").unwrap();
    assert_eq!(recv(&rx), "7F 2E 33\r\r>", "answered by the relay, never forwarded");
    relay.write(b"2701\r").unwrap();
    assert_eq!(recv(&rx), "7F 27 33\r\r>");
    relay.write(b"ATCAF0\r").unwrap();
    assert_eq!(recv(&rx), "?\r\r>");
    assert_eq!(relay.filter().audit().denied(), 2);
    // the backend never saw the refused requests
    relay.close();
}
