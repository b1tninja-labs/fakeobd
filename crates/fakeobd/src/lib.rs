//! `fakeobd`: a fake vehicle behind an ELM327/STN adapter, served as ELM327 over TCP.
//!
//! This crate wires the pieces together: it builds an [`elm::Elm`] over a [`vehicle::Bus`] from a
//! [`Sim`] description, serves it on a socket, and verifies transcripts against an in-process server.

use elm::{Config, Elm, Flavor, Reply};
use obd_conform::{Report, Tcp, Transcript};
use obd_policy::{AuditLog, Policy};
use std::io::{self, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use vehicle::{parse_fault_line, Bus, FaultPlan, VehicleSpec};

/// Everything that defines one simulated bench. Cheap to clone; each connection gets its own bus.
#[derive(Clone)]
pub struct Sim {
    pub spec: VehicleSpec,
    pub policy: Policy,
    pub flavor: Flavor,
    pub faults: FaultPlan,
    pub config: Config,
    /// Where to append the audit log (JSON lines), shared by every connection.
    pub audit_sink: Option<Arc<Mutex<Box<dyn Write + Send>>>>,
}

impl Sim {
    pub fn new(flavor: Flavor) -> Sim {
        Sim {
            spec: VehicleSpec::default_vehicle(),
            policy: Policy::read_only(),
            flavor,
            faults: FaultPlan::default(),
            config: Config::new(flavor),
            audit_sink: None,
        }
    }

    pub fn build(&self) -> Elm {
        let mut audit = AuditLog::new(1024);
        if let Some(sink) = &self.audit_sink {
            audit = audit.with_sink(Box::new(SharedSink(sink.clone())));
        }
        let bus = Bus::new(&self.spec, self.policy.clone(), audit);
        let mut e = Elm::new(self.config.clone(), bus);
        e.set_faults(&self.faults);
        e
    }

    /// Configure from a transcript's header: `@ adapter: elm327|stn`, repeatable `@ fault: <line>` and
    /// `@ policy: <line>` (a line of the policy file format), and `@ vehicle: none` for a bench adapter with
    /// nothing on the bus.
    pub fn for_transcript(t: &Transcript) -> Result<Sim, String> {
        let flavor = match t.meta("adapter") {
            Some("stn") => Flavor::Stn,
            Some("elm327") | None => Flavor::Elm327,
            Some(other) => return Err(format!("unknown adapter {other:?}")),
        };
        let mut sim = Sim::new(flavor);
        if t.meta("vehicle") == Some("none") {
            sim.spec.ecus.clear();
        }
        for (k, v) in &t.meta {
            if k == "fault" {
                parse_fault_line(v, &mut sim.faults)?;
            }
        }
        let policy_text: Vec<&str> = t
            .meta
            .iter()
            .filter(|(k, _)| k == "policy")
            .map(|(_, v)| v.as_str())
            .collect();
        if !policy_text.is_empty() {
            sim.policy = Policy::parse(&policy_text.join("\n")).map_err(|e| e.to_string())?;
        }
        Ok(sim)
    }
}

struct SharedSink(Arc<Mutex<Box<dyn Write + Send>>>);
impl Write for SharedSink {
    fn write(&mut self, b: &[u8]) -> io::Result<usize> {
        self.0.lock().map_or(Ok(b.len()), |mut s| s.write(b))
    }
    fn flush(&mut self) -> io::Result<()> {
        self.0.lock().map_or(Ok(()), |mut s| s.flush())
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ServeOptions {
    /// Sleep for the adapter's virtual busy time, and answer a byte that arrives meanwhile with `STOPPED`.
    pub realtime: bool,
    /// Print every audit record, not only refusals, on stderr.
    pub verbose: bool,
}

/// Bind a listener. Refuses a non-loopback address unless `allow_non_loopback`.
pub fn bind(addr: &str, allow_non_loopback: bool) -> io::Result<TcpListener> {
    let l = TcpListener::bind(addr)?;
    if !allow_non_loopback && !l.local_addr()?.ip().is_loopback() {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "refusing a non-loopback address: an ELM327 stream has no authentication \
             (pass --allow-non-loopback to override)",
        ));
    }
    Ok(l)
}

/// Accept connections forever, one thread each.
pub fn serve(listener: TcpListener, sim: Sim, opts: ServeOptions) -> io::Result<()> {
    for conn in listener.incoming() {
        let stream = conn?;
        let sim = sim.clone();
        std::thread::spawn(move || {
            let peer = stream.peer_addr().ok();
            if let Err(e) = session(stream, &sim, opts) {
                if e.kind() != io::ErrorKind::ConnectionReset
                    && e.kind() != io::ErrorKind::BrokenPipe
                {
                    eprintln!("session {peer:?}: {e}");
                }
            }
        });
    }
    Ok(())
}

/// Serve one connection until it closes.
pub fn session(mut stream: TcpStream, sim: &Sim, opts: ServeOptions) -> io::Result<()> {
    stream.set_nodelay(true)?;
    let mut elm = sim.build();
    let banner = elm.banner();
    if !banner.is_empty() {
        stream.write_all(&banner)?;
    }
    let mut pending: Vec<u8> = Vec::new();
    let mut seen = 0u64;
    let mut idle_since = Instant::now();
    let mut buf = [0u8; 1024];
    loop {
        // a complete line is everything up to a carriage return
        while let Some(pos) = pending.iter().position(|b| *b == b'\r') {
            let line: Vec<u8> = pending.drain(..=pos).take(pos).collect();
            if opts.realtime {
                elm.bus_mut()
                    .advance(idle_since.elapsed().as_millis() as u64);
            }
            let reply: Reply = elm.handle_line(&line);
            if opts.verbose {
                eprintln!(
                    "<< {}
>> {}",
                    obd_conform::escape(&line),
                    obd_conform::escape(&reply.bytes)
                );
            }
            report_audit(&elm, &mut seen, opts.verbose);
            if opts.realtime && reply.busy_ms > 0 {
                stream.set_read_timeout(Some(Duration::from_millis(u64::from(reply.busy_ms))))?;
                match stream.read(&mut buf) {
                    Ok(0) => return Ok(()),
                    Ok(n) => {
                        // a byte arrived while the request was in progress: abandon it
                        stream.write_all(&elm.stopped())?;
                        pending.extend_from_slice(&buf[..n]);
                        stream.set_read_timeout(None)?;
                        continue;
                    }
                    Err(e)
                        if matches!(
                            e.kind(),
                            io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                        ) => {}
                    Err(e) => return Err(e),
                }
                stream.set_read_timeout(None)?;
            }
            if !reply.bytes.is_empty() {
                stream.write_all(&reply.bytes)?;
            }
            idle_since = Instant::now();
            if reply.close {
                return Ok(());
            }
        }
        let n = stream.read(&mut buf)?;
        if n == 0 {
            return Ok(());
        }
        pending.extend_from_slice(&buf[..n]);
    }
}

fn report_audit(elm: &Elm, seen: &mut u64, verbose: bool) {
    let last = *seen;
    for e in elm.bus().audit().entries().filter(|e| e.seq > last) {
        if verbose || !e.allowed {
            eprintln!("audit {}", e.to_json());
        }
        *seen = e.seq;
    }
}

/// Replay `t` against a fresh in-process server configured from the transcript's header.
pub fn verify(t: &Transcript) -> Result<Report, String> {
    let sim = Sim::for_transcript(t)?;
    let listener = bind("127.0.0.1:0", false).map_err(|e| e.to_string())?;
    let addr: SocketAddr = listener.local_addr().map_err(|e| e.to_string())?;
    std::thread::spawn(move || {
        if let Ok((s, _)) = listener.accept() {
            let _ = session(s, &sim, ServeOptions::default());
        }
    });
    let mut tcp = Tcp::connect(&addr.to_string(), false).map_err(|e| e.to_string())?;
    // Strict: a simulator replaying its own transcript has no excuse for a difference, annotated or not.
    obd_conform::replay(t, &mut tcp)
        .map(Report::strict)
        .map_err(|e| e.to_string())
}

/// An in-process connection to a fresh simulated bench: no socket, no thread. Write command bytes in,
/// take reply chunks from the receiver. The shape matches gattmirror's `serial_stream::ByteStream`
/// (`write`, take-once `receiver`, idempotent `close`); the adapter crate `crates/serial-bridge`
/// implements that trait over this type.
pub struct SimPipe {
    elm: Elm,
    pending: Vec<u8>,
    tx: Option<std::sync::mpsc::Sender<Vec<u8>>>,
    rx: Option<std::sync::mpsc::Receiver<Vec<u8>>>,
}

/// Writing to a closed pipe.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Closed;

impl SimPipe {
    pub fn new(sim: &Sim) -> SimPipe {
        let (tx, rx) = std::sync::mpsc::channel();
        let elm = sim.build();
        let banner = elm.banner();
        if !banner.is_empty() {
            let _ = tx.send(banner);
        }
        SimPipe {
            elm,
            pending: Vec::new(),
            tx: Some(tx),
            rx: Some(rx),
        }
    }

    /// Command bytes in. Each carriage-return-terminated line produces one reply chunk.
    pub fn write(&mut self, bytes: &[u8]) -> Result<(), Closed> {
        if self.tx.is_none() {
            return Err(Closed);
        }
        self.pending.extend_from_slice(bytes);
        while let Some(pos) = self.pending.iter().position(|b| *b == b'\r') {
            let line: Vec<u8> = self.pending.drain(..=pos).take(pos).collect();
            let reply = self.elm.handle_line(&line);
            if !reply.bytes.is_empty() {
                if let Some(tx) = &self.tx {
                    let _ = tx.send(reply.bytes);
                }
            }
            if reply.close {
                self.close();
                break;
            }
        }
        Ok(())
    }

    /// The reply stream; can be taken once. It disconnects when the pipe closes.
    pub fn receiver(&mut self) -> Option<std::sync::mpsc::Receiver<Vec<u8>>> {
        self.rx.take()
    }

    pub fn close(&mut self) {
        self.tx = None;
    }

    pub fn elm(&self) -> &Elm {
        &self.elm
    }
}
