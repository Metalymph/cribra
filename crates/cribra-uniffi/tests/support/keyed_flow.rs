//! Public synthetic test keys and independent calls to keyed Core transforms.

use serde_json::{Value, json};

pub fn cases() -> Vec<Value> {
    let mut cases = super::support::cases();
    let extra: Vec<Value> =
        serde_json::from_str(include_str!("../fixtures/keyed-flow.json")).unwrap();
    cases.extend(extra);
    cases
}

pub fn options() -> Vec<Value> {
    serde_json::from_str(include_str!("../fixtures/keyed-options.json")).unwrap()
}

// Fixed PUBLIC test vectors, not generated runtime keys or live credentials.
pub fn test_key(selector: &str) -> [u8; 32] {
    match selector {
        "a" => *b"0123456789ABCDEF0123456789ABCDEF",
        "b" => *b"FEDCBA9876543210FEDCBA9876543210",
        _ => panic!("unknown public test vector selector"),
    }
}

pub fn core_outcomes(case: &Value, options: &Value) -> Value {
    let key = test_key(options["key"].as_str().unwrap());
    let source = case["source"].as_str().unwrap();
    let report = super::support::core_report(case);
    let mut pseudo = cribra::transform::PseudonymizationOptions::new(key);
    if let Some(prefix) = options["prefix"].as_str() {
        pseudo = pseudo.prefix(prefix);
    }
    if let Some(bytes) = options["digestBytes"].as_u64() {
        pseudo = pseudo.digest_bytes(usize::try_from(bytes).unwrap());
    }
    let mut synthesis = cribra::transform::SynthesisOptions::new(key);
    if let Some(marker) = options["marker"].as_str() {
        synthesis = synthesis.marker(marker);
    }
    json!({"name":case["name"],"options":options["name"],
        "pseudonymize":super::support::outcome(cribra::transform::pseudonymize(source,&report,&pseudo)),
        "synthesize":super::support::outcome(cribra::transform::synthesize(source,&report,&synthesis))})
}
