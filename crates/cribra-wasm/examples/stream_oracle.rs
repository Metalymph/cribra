//! Direct-Core streaming oracle for the generated Core and Flow runtime checks.
mod oracle_support;
use oracle_support::report;
use serde_json::{Value, json};

fn main() {
    let cases: Vec<Value> =
        serde_json::from_str(include_str!("../tests/runtime/stream-cases.json")).unwrap();
    let mut outcomes = Vec::new();
    for case in cases {
        let scanner = oracle_support::scanner(&case);
        for source in case["sources"].as_array().unwrap() {
            let source = source.as_str().unwrap();
            let whole = scanner.scan([((), source)]);
            let whole = whole.single_report().unwrap();
            let error = match scanner.stream() {
                Ok(mut session) => {
                    for byte in source.as_bytes() {
                        session.push(&[*byte]).unwrap();
                    }
                    assert_eq!(&session.finish().unwrap(), whole);
                    None
                }
                Err(cribra::StreamError::UnsupportedRuleSet) => Some("UnsupportedRuleSet"),
                Err(error) => panic!("unexpected admission error: {error}"),
            };
            outcomes.push(json!({"name":case["name"], "source":source, "error":error,
                "report":report(&scanner,whole,source.len())}));
        }
    }
    println!("{}", json!(outcomes));
}
