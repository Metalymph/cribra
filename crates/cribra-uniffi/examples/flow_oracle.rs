//! Independent direct-Core oracle for the real Swift Flow consumer.

#[path = "../tests/support/keyed_flow.rs"]
mod keyed;
#[path = "../tests/support/flow.rs"]
mod support;

fn main() {
    let cases = support::cases();
    // Also exercise the fixture-to-DTO helper to keep this shared test module checked.
    assert!(cases.iter().any(|case| !support::findings(case).is_empty()));
    let outcomes: Vec<_> = cases.iter().map(support::core_outcomes).collect();
    let mut keyed_outcomes = Vec::new();
    for case in keyed::cases() {
        for options in keyed::options() {
            keyed_outcomes.push(keyed::core_outcomes(&case, &options));
        }
    }
    // No keys or options DTOs are serialized into the output oracle.
    println!(
        "{}",
        serde_json::json!({"redactTemplate":outcomes,"keyed":keyed_outcomes})
    );
}
