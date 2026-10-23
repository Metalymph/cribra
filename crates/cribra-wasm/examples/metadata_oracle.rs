//! Direct Core metadata and constructor errors, independent of WASM conversions.
#[allow(
    dead_code,
    reason = "shared oracle helpers; this example needs construction only"
)]
mod oracle_support;
use serde_json::{Value, json};

fn main() {
    let cases: Vec<Value> =
        serde_json::from_str(include_str!("../tests/runtime/configuration-cases.json")).unwrap();
    let outcomes:Vec<_>=cases.iter().map(|case| match oracle_support::scanner_result(case) {
        Ok(scanner)=>json!({"name":case["name"],"rules":scanner.rule_metadata().map(|r|json!({
            "ruleId":r.id(),"kind":format!("{:?}",r.kind()),"detectionMode":format!("{:?}",r.detection_mode()),
            "severity":format!("{:?}",r.severity()),"remediation":r.remediation().map(|r|format!("{r:?}"))
        })).collect::<Vec<_>>()}),
        Err(error)=>json!({"name":case["name"],"error":error}),
    }).collect();
    println!("{}", json!(outcomes));
}
