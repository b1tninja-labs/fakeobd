use elm::Flavor;
use fakeobd::{bind, serve, verify, ServeOptions, Sim};
use obd_conform::{capture, compare, escape, parse, render, replay, Tcp, Transport};
use obd_policy::Policy;
use std::io::Write;
use std::process::ExitCode;
use std::sync::{Arc, Mutex};
use vehicle::{FaultPlan, VehicleSpec};

const USAGE: &str = "\
fakeobd: a fake vehicle behind an ELM327/STN adapter, ELM327 over TCP

  fakeobd serve [--listen 127.0.0.1:35000] [--adapter elm327|stn] [--profile FILE] [--policy FILE]
                [--faults FILE] [--audit FILE] [--no-vehicle] [--realtime] [--banner] [--allow-non-loopback] [-v]
  fakeobd client HOST:PORT [CMD...]            send commands, print each reply with escapes
  fakeobd verify TRANSCRIPT...                 replay golden transcripts against an in-process simulator
  fakeobd replay TRANSCRIPT --connect HOST:PORT [--allow-remote]
  fakeobd capture SCRIPT --connect HOST:PORT --out FILE [--source sim|hardware] [--adapter elm327|stn]
                  [--meta key=value]... [--allow-remote]
  fakeobd compare EXPECTED ACTUAL              diff two transcripts
";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("fakeobd: {e}");
            ExitCode::from(2)
        }
    }
}

fn flag(args: &[String], name: &str) -> bool {
    args.iter().any(|a| a == name)
}

fn opt<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .map(String::as_str)
}

fn read(path: &str) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))
}

fn run(args: &[String]) -> Result<ExitCode, String> {
    let Some(cmd) = args.first() else {
        print!("{USAGE}");
        return Ok(ExitCode::from(2));
    };
    let rest = &args[1..];
    match cmd.as_str() {
        "serve" => {
            let flavor = match opt(rest, "--adapter") {
                None | Some("elm327") => Flavor::Elm327,
                Some("stn") => Flavor::Stn,
                Some(o) => return Err(format!("unknown adapter {o}")),
            };
            let mut sim = Sim::new(flavor);
            if let Some(p) = opt(rest, "--profile") {
                sim.spec = VehicleSpec::parse(&read(p)?)?;
            }
            if flag(rest, "--no-vehicle") {
                sim.spec.ecus.clear();
            }
            if let Some(p) = opt(rest, "--policy") {
                sim.policy = Policy::parse(&read(p)?).map_err(|e| e.to_string())?;
            }
            if let Some(p) = opt(rest, "--faults") {
                sim.faults = FaultPlan::parse(&read(p)?)?;
            }
            if let Some(p) = opt(rest, "--audit") {
                let f = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(p)
                    .map_err(|e| format!("{p}: {e}"))?;
                sim.audit_sink = Some(Arc::new(Mutex::new(Box::new(f))));
            }
            sim.config.banner_on_connect = flag(rest, "--banner");
            let addr = opt(rest, "--listen").unwrap_or("127.0.0.1:35000");
            let listener =
                bind(addr, flag(rest, "--allow-non-loopback")).map_err(|e| e.to_string())?;
            eprintln!(
                "fakeobd listening on {} ({:?}); policy refuses every state-changing service; SecurityAccess is always refused",
                listener.local_addr().map_err(|e| e.to_string())?,
                flavor
            );
            serve(
                listener,
                sim,
                ServeOptions {
                    realtime: flag(rest, "--realtime"),
                    verbose: flag(rest, "-v"),
                },
            )
            .map_err(|e| e.to_string())?;
            Ok(ExitCode::SUCCESS)
        }
        "client" => {
            let addr = rest.first().ok_or("client needs HOST:PORT")?;
            let mut tcp =
                Tcp::connect(addr, flag(rest, "--allow-remote")).map_err(|e| e.to_string())?;
            let cmds: Vec<&String> = rest[1..].iter().filter(|a| !a.starts_with("--")).collect();
            for c in cmds {
                let r = tcp.exchange(c).map_err(|e| e.to_string())?;
                println!("{c:?} -> {}", escape(&r));
            }
            Ok(ExitCode::SUCCESS)
        }
        "verify" => {
            let mut bad = false;
            for p in rest {
                let t = parse(&read(p)?)?;
                let r = verify(&t)?;
                println!("{p}: {}", r.render().lines().last().unwrap_or(""));
                if !r.ok() {
                    print!("{}", r.render());
                    bad = true;
                }
            }
            Ok(if bad {
                ExitCode::FAILURE
            } else {
                ExitCode::SUCCESS
            })
        }
        "replay" => {
            let t = parse(&read(rest.first().ok_or("replay needs a transcript")?)?)?;
            let addr = opt(rest, "--connect").ok_or("replay needs --connect")?;
            let mut tcp =
                Tcp::connect(addr, flag(rest, "--allow-remote")).map_err(|e| e.to_string())?;
            let r = replay(&t, &mut tcp).map_err(|e| e.to_string())?;
            print!("{}", r.render());
            Ok(if r.ok() {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            })
        }
        "capture" => {
            let script: Vec<String> = read(rest.first().ok_or("capture needs a script")?)?
                .lines()
                .map(str::trim)
                .filter(|l| !l.is_empty() && !l.starts_with('#'))
                .map(String::from)
                .collect();
            let addr = opt(rest, "--connect").ok_or("capture needs --connect")?;
            let out = opt(rest, "--out").ok_or("capture needs --out")?;
            let mut tcp =
                Tcp::connect(addr, flag(rest, "--allow-remote")).map_err(|e| e.to_string())?;
            let mut meta = vec![
                (
                    "source".to_string(),
                    opt(rest, "--source").unwrap_or("sim").to_string(),
                ),
                (
                    "adapter".to_string(),
                    opt(rest, "--adapter").unwrap_or("elm327").to_string(),
                ),
            ];
            for (i, a) in rest.iter().enumerate() {
                if a == "--meta" {
                    let kv = rest.get(i + 1).ok_or("--meta needs key=value")?;
                    let (k, v) = kv.split_once('=').ok_or("--meta needs key=value")?;
                    meta.push((k.to_string(), v.to_string()));
                }
            }
            let t = capture(&script, &mut tcp, meta).map_err(|e| e.to_string())?;
            std::fs::File::create(out)
                .and_then(|mut f| f.write_all(render(&t).as_bytes()))
                .map_err(|e| format!("{out}: {e}"))?;
            println!("{} entries written to {out}", t.entries.len());
            Ok(ExitCode::SUCCESS)
        }
        "compare" => {
            let (a, b) = (
                rest.first().ok_or("compare needs two files")?,
                rest.get(1).ok_or("compare needs two files")?,
            );
            let r = compare(&parse(&read(a)?)?, &parse(&read(b)?)?);
            print!("{}", r.render());
            Ok(if r.ok() {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            })
        }
        _ => {
            print!("{USAGE}");
            Ok(ExitCode::from(2))
        }
    }
}
