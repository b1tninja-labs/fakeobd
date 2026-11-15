//! Every golden transcript in `transcripts/` replays clean against a fresh in-process simulator.
//!
//! These are simulator-authored (`@ source: sim`): the test proves the simulator still says what it said
//! when the transcript was reviewed, not that a real adapter says the same.

use obd_conform::{parse, render};
use std::path::PathBuf;

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../transcripts")
}

fn transcripts() -> Vec<(String, String)> {
    let mut v: Vec<_> = std::fs::read_dir(dir())
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|x| x == "txt"))
        .map(|e| {
            (
                e.file_name().to_string_lossy().into_owned(),
                std::fs::read_to_string(e.path()).unwrap(),
            )
        })
        .collect();
    v.sort();
    v
}

#[test]
fn there_are_golden_transcripts() {
    assert!(transcripts().len() >= 7);
}

#[test]
fn every_golden_transcript_replays_clean() {
    for (name, text) in transcripts() {
        let t = parse(&text).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert!(!t.entries.is_empty(), "{name}");
        assert!(
            t.meta("source").is_some() && t.meta("scope").is_some(),
            "{name} lacks source/scope"
        );
        let r = fakeobd::verify(&t).unwrap();
        assert!(r.ok(), "{name}:\n{}", r.render());
    }
}

#[test]
fn goldens_are_in_canonical_form() {
    for (name, text) in transcripts() {
        let t = parse(&text).unwrap();
        let again = parse(&render(&t)).unwrap();
        assert_eq!(t.entries, again.entries, "{name}");
    }
}

#[test]
fn a_changed_reply_is_a_failure_not_a_pass() {
    // the harness must be able to fail: tamper with one recorded reply and replay
    let (_, text) = transcripts()
        .into_iter()
        .find(|(n, _)| n == "obd2-read.txt")
        .unwrap();
    let tampered = text.replace("< 41 05 5A \\r\\r>", "< 41 05 5B \\r\\r>");
    assert_ne!(tampered, text);
    let r = fakeobd::verify(&parse(&tampered).unwrap()).unwrap();
    assert!(!r.ok());
    assert!(r.failures() >= 1);
}

#[test]
fn every_state_changing_request_in_the_policy_transcript_is_a_refusal() {
    let (_, text) = transcripts()
        .into_iter()
        .find(|(n, _)| n == "policy.txt")
        .unwrap();
    let t = parse(&text).unwrap();
    let refused = t
        .entries
        .iter()
        .filter(|e| e.reply.starts_with(b"7F "))
        .count();
    assert!(refused >= 15, "{refused}");
    for e in &t.entries {
        let req = e.request.replace(' ', "");
        if req.starts_with("22") || req.starts_with("AT") {
            continue;
        }
        assert!(
            !e.reply.starts_with(b"6") && !e.reply.starts_with(b"4") && !e.reply.starts_with(b"5"),
            "{}: {:?}",
            e.request,
            e.reply
        );
    }
}

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn every_gap_id_in_a_transcript_is_documented() {
    let doc = std::fs::read_to_string(root().join("docs/ground-truth-gaps.md")).unwrap();
    let mut used = 0;
    for (name, text) in transcripts() {
        for line in text.lines().filter(|l| l.starts_with("~ gap ")) {
            let id = line["~ gap ".len()..].split_whitespace().next().unwrap();
            used += 1;
            assert!(
                doc.contains(&format!(
                    "### {id}
"
                )),
                "{name}: gap {id} has no `### {id}` heading in docs/ground-truth-gaps.md"
            );
        }
    }
    assert!(used > 20, "{used}");
}

fn rust_sources(dir: &std::path::Path, out: &mut String) {
    for e in std::fs::read_dir(dir).unwrap().flatten() {
        let p = e.path();
        if p.is_dir() {
            if p.file_name().is_some_and(|n| n != "target") {
                rust_sources(&p, out);
            }
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push_str(&std::fs::read_to_string(&p).unwrap());
        }
    }
}

#[test]
fn roadmap_names_only_tests_that_exist() {
    let roadmap = std::fs::read_to_string(root().join("ROADMAP.md")).unwrap();
    let mut src = String::new();
    rust_sources(&root().join("crates"), &mut src);
    let mut checked = 0;
    for chunk in roadmap.split('`').skip(1).step_by(2) {
        if !chunk.contains("::") || chunk.starts_with("crate::") {
            continue;
        }
        let last = chunk.rsplit("::").next().unwrap();
        if last.is_empty()
            || !last
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        {
            continue;
        }
        checked += 1;
        assert!(
            src.contains(&format!("fn {last}(")),
            "ROADMAP.md names `{chunk}` but no `fn {last}(` exists"
        );
    }
    assert!(checked > 80, "{checked}");
}
