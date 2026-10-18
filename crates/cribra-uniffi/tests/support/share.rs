//! Direct public-Core oracle, independent of adapter bundle implementations.
#![allow(dead_code)]
use cribra::transform::{ShareBundle, ShareMode, TransformError};
use serde_json::{Value, json};
use std::time::{SystemTime, UNIX_EPOCH};

pub const KEY: &[u8; 32] = b"0123456789ABCDEF0123456789ABCDEF"; // PUBLIC test vector
pub fn batches() -> Vec<Value> {
    serde_json::from_str(include_str!("../fixtures/share.json")).unwrap()
}
pub fn scanner(builtins: bool) -> cribra::Scanner {
    if builtins {
        cribra::Scanner::default()
    } else {
        cribra::Scanner::builder()
            .rule(cribra::Rule::literal(
                "secret",
                "SECRET",
                cribra::Severity::High,
            ))
            .build()
            .unwrap()
    }
}
pub fn mode(index: usize) -> ShareMode {
    match index {
        0 => ShareMode::Redact,
        1 => ShareMode::Template,
        2 => ShareMode::Pseudonymize(
            cribra::transform::PseudonymizationOptions::new(*KEY)
                .prefix("p:")
                .digest_bytes(8),
        ),
        3 => ShareMode::Synthesize(cribra::transform::SynthesisOptions::new(*KEY).marker("demo")),
        _ => unreachable!(),
    }
}
pub fn error(error: TransformError) -> Value {
    use TransformError as E;
    match error {
        E::MissingShareMode => {
            json!({"kind":"bundle","code":"MissingShareMode","expected":null,"actual":null,"index":null,"detail":error.to_string()})
        }
        E::SourceCountMismatch { expected, actual } => {
            json!({"kind":"bundle","code":"SourceCountMismatch","expected":expected,"actual":actual,"index":null,"detail":error.to_string()})
        }
        E::SourceLengthMismatch {
            index,
            expected_bytes,
            actual_bytes,
        } => {
            json!({"kind":"bundle","code":"SourceLengthMismatch","expected":expected_bytes,"actual":actual_bytes,"index":index,"detail":error.to_string()})
        }
        _ => {
            let (code, first, second, length) = match &error {
                E::InvalidSpan {
                    start,
                    end,
                    source_len,
                } => (
                    "InvalidSpan",
                    Some(json!({"start":start,"end":end})),
                    None,
                    Some(*source_len),
                ),
                E::InvalidUtf8Boundary { start, end } => (
                    "InvalidUtf8Boundary",
                    Some(json!({"start":start,"end":end})),
                    None,
                    None,
                ),
                E::OverlappingSpans { first, second } => (
                    "OverlappingSpans",
                    Some(json!({"start":first.start,"end":first.end})),
                    Some(json!({"start":second.start,"end":second.end})),
                    None,
                ),
                _ => panic!("unexpected Core error"),
            };
            json!({"kind":"transform","code":code,"span":first,"otherSpan":second,"sourceLen":length,"detail":error.to_string()})
        }
    }
}
pub fn outcome(result: Result<ShareBundle<String>, TransformError>) -> Value {
    match result {
        Err(e) => json!({"error":error(e)}),
        Ok(b) => {
            let time = b
                .manifest()
                .generated_at()
                .duration_since(UNIX_EPOCH)
                .unwrap();
            assert!(time.as_secs() > 0 && time.subsec_nanos() < 1_000_000_000);
            assert!(b.manifest().generated_at() <= SystemTime::now());
            json!({"sources":b.sources().iter().map(|s|json!({"id":s.key(),"content":s.content()})).collect::<Vec<_>>(),
                "mode":b.manifest().mode().to_string(),"summary":b.summary()})
        }
    }
}
pub fn core_batch(case: &Value, mode_index: usize, atomic: bool) -> Value {
    let scanner = scanner(case["builtins"].as_bool().unwrap_or(false));
    let inputs: Vec<_> = case["sources"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| {
            (
                s["id"].as_str().unwrap().to_owned(),
                s["text"].as_str().unwrap(),
            )
        })
        .collect();
    let builder = ShareBundle::builder().mode(mode(mode_index));
    let result = if atomic {
        builder.scan_and_build(&scanner, inputs.clone())
    } else {
        let reports = scanner.scan(inputs.clone());
        builder.build(&reports, inputs.iter().map(|(_, text)| *text))
    };
    outcome(result)
}
/// Explicit finding fixtures cover malformed spans and overlap without rescanning.
pub fn core_explicit(case: &Value, mode_index: usize) -> Value {
    let findings: Vec<_> = case["findings"].as_array().unwrap().iter().map(|f| json!({
        "rule_id":f["id"],"location":{"start":f["start"],"end":f["end"],"line":f["line"].as_u64().unwrap_or(1),"column":f["column"].as_u64().unwrap_or(1)},
        "severity":f["severity"].as_str().unwrap_or("high"),"confidence":f["confidence"].as_str().unwrap_or("high")
    })).collect();
    let source = case["source"].as_str().unwrap();
    let results: cribra::ScanResults<String> = serde_json::from_value(json!({"entries":[{"key":"明示😀","source_bytes":source.len(),"report":{"findings":findings,"candidates":[]}}]})).unwrap();
    outcome(
        ShareBundle::builder()
            .mode(mode(mode_index))
            .build(&results, [source]),
    )
}
pub fn oracle() -> Value {
    let batch: Vec<_> = batches().iter().flat_map(|case| (0..4).flat_map(move |mode| [false,true].map(|atomic| {
        json!({"name":case["name"],"mode":mode,"atomic":atomic,"outcome":core_batch(case,mode,atomic)})
    }))).collect();
    let cases: Vec<Value> = serde_json::from_str(include_str!("../fixtures/flow.json")).unwrap();
    let explicit: Vec<_> = cases.iter().flat_map(|case| (0..4).map(move |mode| json!({"name":case["name"],"mode":mode,"outcome":core_explicit(case,mode)}))).collect();
    json!({"batch":batch,"explicit":explicit})
}
