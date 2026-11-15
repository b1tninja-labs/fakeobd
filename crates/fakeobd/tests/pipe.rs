use elm::Flavor;
use fakeobd::{Closed, Sim, SimPipe};

fn recv(rx: &std::sync::mpsc::Receiver<Vec<u8>>) -> String {
    String::from_utf8(rx.try_recv().expect("a reply chunk")).unwrap()
}

#[test]
fn a_pipe_answers_without_a_socket_one_chunk_per_line() {
    let mut p = SimPipe::new(&Sim::new(Flavor::Elm327));
    let rx = p.receiver().unwrap();
    assert!(p.receiver().is_none(), "the receiver is taken once");
    p.write(b"ATE0\rATL0\rATSP6\rATSH7E0\r").unwrap();
    for _ in 0..4 {
        rx.try_recv().unwrap();
    }
    p.write(b"01").unwrap();
    assert!(rx.try_recv().is_err(), "a partial line produces nothing");
    p.write(b"05\r").unwrap();
    assert_eq!(recv(&rx), "41 05 5A \r\r>");
    p.write(b"2701\r").unwrap();
    assert_eq!(recv(&rx), "7F 27 33 \r\r>");
    assert_eq!(p.elm().bus().audit().denied(), 1);
}

#[test]
fn close_is_idempotent_and_disconnects_the_receiver() {
    let mut p = SimPipe::new(&Sim::new(Flavor::Elm327));
    let rx = p.receiver().unwrap();
    p.close();
    p.close();
    assert_eq!(p.write(b"ATI\r"), Err(Closed));
    assert!(matches!(
        rx.try_recv(),
        Err(std::sync::mpsc::TryRecvError::Disconnected)
    ));
}

#[test]
fn an_adapter_drop_fault_closes_the_pipe() {
    let mut sim = Sim::new(Flavor::Elm327);
    sim.faults = vehicle::FaultPlan::parse("adapter drop_after=1").unwrap();
    let mut p = SimPipe::new(&sim);
    let rx = p.receiver().unwrap();
    p.write(b"ATI\r").unwrap();
    assert!(matches!(
        rx.try_recv(),
        Err(std::sync::mpsc::TryRecvError::Disconnected)
    ));
    assert_eq!(p.write(b"ATI\r"), Err(Closed));
}
