//! Call 105's stand-in against the JIP's 105 vectors (docs/jip/vectors/jip8-core.jsonl), and
//! through `grumpkin_g1_msm`'s probe: present, absent, WHAT after an answer, HUH.

use curve_calls::stand_in::{set_modes, Mode};
use curve_calls::{grumpkin_g1_msm, used, Answer};
use std::sync::Mutex;

// the calls' probe state is global: one test at a time
static ONE: Mutex<()> = Mutex::new(());

fn field<'a>(line: &'a str, key: &str) -> &'a str {
    let at = line.find(&format!("\"{key}\": \"")).unwrap() + key.len() + 5;
    &line[at..at + line[at..].find('"').unwrap()]
}

fn unhex(s: &str) -> Vec<u8> {
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect()
}

fn vectors() -> Vec<(String, Vec<u8>, String, Vec<u8>)> {
    let f = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/jip/vectors/jip8-core.jsonl")).unwrap();
    f.lines()
        .filter(|l| field(l, "call") == "grumpkin_g1_msm")
        .map(|l| (field(l, "case").to_string(), unhex(field(l, "input")), field(l, "result").to_string(),
                  if field(l, "result") == "OK" { unhex(field(l, "output")) } else { vec![] }))
        .collect()
}

#[test]
fn the_jips_105_vectors() {
    let _one = ONE.lock().unwrap_or_else(|e| e.into_inner());
    let v = vectors();
    assert_eq!(v.len(), 25);
    for (case, input, result, output) in v {
        set_modes([Mode::Present; 5]);
        match (grumpkin_g1_msm(&input), result.as_str()) {
            (Answer::Value(o), "OK") => assert_eq!(o.to_vec(), output, "{case}"),
            (Answer::Malformed, "HUH") => {}
            _ => panic!("{case}: expected {result}"),
        }
        assert_eq!(used()[4], Some(true), "{case}");
    }
}

#[test]
fn the_probe() {
    let _one = ONE.lock().unwrap_or_else(|e| e.into_inner());
    let input = &vectors()[0].1;
    let msm = |modes: [Mode; 5]| { set_modes(modes); grumpkin_g1_msm(input) };
    let m = |last| [Mode::Present, Mode::Present, Mode::Present, Mode::Present, last];
    assert!(matches!(msm(m(Mode::Absent)), Answer::Absent));
    assert_eq!(used()[4], Some(false));
    assert!(matches!(grumpkin_g1_msm(input), Answer::Absent), "absent stays absent without calling again");
    assert!(matches!(msm(m(Mode::Huh)), Answer::Malformed));
    assert!(matches!(msm(m(Mode::WhatAfterFirst)), Answer::Value(_)));
    assert!(matches!(grumpkin_g1_msm(input), Answer::Broken), "WHAT after an answer is a broken host");
    // 101..104's modes leave 105 present
    set_modes([Mode::Absent; 4]);
    assert!(matches!(grumpkin_g1_msm(input), Answer::Value(_)));
}
