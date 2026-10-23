//! Core metadata oracle for additive WASM custom-remediation configuration.
mod oracle_support;
use serde_json::{Value, json};

fn main() {
    let cases: Vec<Value> =
        serde_json::from_str(include_str!("../tests/runtime/remediation-cases.json")).unwrap();
    let outcomes: Vec<_> = cases.iter().map(|case| {
        let scanner = oracle_support::scanner(case);
        let source = case["source"].as_str().unwrap();
        let report = scanner.scan([((),source)]);
        json!({"name":case["name"],"report":oracle_support::report(&scanner,report.single_report().unwrap(),source.len())})
    }).collect();
    println!("{}", json!(outcomes));
}
