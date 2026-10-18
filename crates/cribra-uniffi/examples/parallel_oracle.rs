//! Direct Core oracle; no adapter scanning or conversion is used.
#[path = "../tests/support/parallel.rs"]
#[allow(dead_code)]
mod support;
use serde_json::json;
fn main() {
    let mut outcomes = Vec::new();
    for mode in support::MODES {
        let scanner = support::core(mode);
        for (name, sources) in support::batches() {
            let inputs = || sources.iter().map(|s| (&s.id, s.text.as_str()));
            let sequential = scanner.scan(inputs());
            let parallel = scanner.parallel_scan(inputs());
            assert_eq!(parallel, sequential);
            outcomes.push(json!({"mode":mode,"batch":name,"outcome":support::snapshot(&parallel)}));
        }
    }
    println!("{}", json!(outcomes));
}
