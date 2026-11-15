//! `serial-bridge`: the simulator and the policy filter as [`serial_stream::ByteStream`]s.
//!
//! - [`SimStream`]: the simulated vehicle behind an ELM327/STN adapter, in process, no TCP.
//! - [`PolicyStream`]: wraps any backend `ByteStream` (the simulator, a TCP link to a real adapter, a
//!   fakegatt device) and applies [`obd_policy::Filter`] to each ELM line the frontend writes, so a relay is
//!   `frontend -> PolicyStream<backend>`.
//!
//! Limitation of [`PolicyStream`]: a refusal is answered locally as `7F <sid> <nrc>` or `?` followed by
//! `\r\r>`, without echo and without regard to the backend's linefeed setting. Apps that turn echo off
//! (`ATE0`, which the Voltage app does first) see the right bytes.

use fakeobd::{Sim, SimPipe};
use obd_policy::{Context, Filter, LineAction};
use serial_stream::{ByteStream, StreamError};
use std::sync::mpsc::{channel, Receiver, Sender};

pub struct SimStream {
    pipe: SimPipe,
    closed: bool,
}

impl SimStream {
    pub fn new(sim: &Sim) -> SimStream {
        SimStream {
            pipe: SimPipe::new(sim),
            closed: false,
        }
    }
}

impl ByteStream for SimStream {
    fn write(&mut self, bytes: &[u8]) -> Result<(), StreamError> {
        self.pipe.write(bytes).map_err(|_| StreamError::Closed)
    }
    fn receiver(&mut self) -> Option<Receiver<Vec<u8>>> {
        self.pipe.receiver()
    }
    fn close(&mut self) {
        self.closed = true;
        self.pipe.close();
    }
}

pub struct PolicyStream<S: ByteStream> {
    inner: S,
    filter: Filter,
    ctx: Context,
    pending: Vec<u8>,
    out_tx: Option<Sender<Vec<u8>>>,
    out_rx: Option<Receiver<Vec<u8>>>,
    pump: Option<std::thread::JoinHandle<()>>,
}

impl<S: ByteStream> PolicyStream<S> {
    pub fn new(mut inner: S, filter: Filter, origin: &str) -> PolicyStream<S> {
        let (tx, rx) = channel();
        let pump = inner.receiver().map(|from_backend| {
            let tx = tx.clone();
            std::thread::spawn(move || {
                while let Ok(chunk) = from_backend.recv() {
                    if tx.send(chunk).is_err() {
                        break;
                    }
                }
            })
        });
        PolicyStream {
            inner,
            filter,
            ctx: Context::new(origin),
            pending: Vec::new(),
            out_tx: Some(tx),
            out_rx: Some(rx),
            pump,
        }
    }

    pub fn filter(&self) -> &Filter {
        &self.filter
    }

    fn local(&self, text: &str) {
        if let Some(tx) = &self.out_tx {
            let _ = tx.send(format!("{text}\r\r>").into_bytes());
        }
    }
}

impl<S: ByteStream> ByteStream for PolicyStream<S> {
    fn write(&mut self, bytes: &[u8]) -> Result<(), StreamError> {
        if self.out_tx.is_none() {
            return Err(StreamError::Closed);
        }
        self.pending.extend_from_slice(bytes);
        while let Some(pos) = self.pending.iter().position(|b| *b == b'\r') {
            let line: Vec<u8> = self.pending.drain(..=pos).take(pos).collect();
            let text = String::from_utf8_lossy(&line).into_owned();
            match self.filter.decide_line(&text, &self.ctx) {
                LineAction::Forward => {
                    let mut l = line.clone();
                    l.push(b'\r');
                    self.inner.write(&l)?;
                }
                LineAction::Rewrite(new) => self.inner.write(format!("{new}\r").as_bytes())?,
                LineAction::Respond(r) => self.local(&r),
                LineAction::Refuse => self.local("?"),
            }
        }
        Ok(())
    }

    fn receiver(&mut self) -> Option<Receiver<Vec<u8>>> {
        self.out_rx.take()
    }

    fn close(&mut self) {
        self.inner.close();
        self.out_tx = None;
        if let Some(h) = self.pump.take() {
            let _ = h.join();
        }
    }

    fn idle_timeout(&self) -> Option<std::time::Duration> {
        self.inner.idle_timeout()
    }
}
