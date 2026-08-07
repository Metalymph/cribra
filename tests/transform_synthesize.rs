//! Public provider-aware synthesis tests.

use silens_scan::{
    Rule, Scanner, Severity,
    transform::{SynthesisOptions, synthesize},
};

#[test]
fn public_synthesis_preserves_known_provider_identity_without_original_value() {
    let scanner = Scanner::builder()
        .rule(Rule::prefix(
            "stripe.live-secret-key",
            "sk_live_",
            Severity::Critical,
        ))
        .build()
        .expect("scanner should build");

    let source = "sk_live_1234567890abcdefghijkl";
    let results = scanner.scan([("memory", source)]);
    let report = results.single_report().expect("one report");

    let output = synthesize(source, report, &SynthesisOptions::new([42; 32])).unwrap();

    assert_eq!(output.len(), source.len());
    assert!(output.starts_with("sk_live_!"));
    assert_ne!(output, source);
}

#[test]
fn public_synthesis_is_reproducible() {
    let scanner = Scanner::builder()
        .rule(Rule::literal(
            "custom.secret",
            "SECRET_VALUE",
            Severity::High,
        ))
        .build()
        .expect("scanner should build");

    let source = "TOKEN=SECRET_VALUE";
    let results = scanner.scan([("memory", source)]);
    let report = results.single_report().expect("one report");
    let options = SynthesisOptions::new([7; 32]);

    assert_eq!(
        synthesize(source, report, &options).unwrap(),
        synthesize(source, report, &options).unwrap(),
    );
}

#[test]
fn public_synthesis_rejects_ambiguous_overlaps() {
    let scanner = Scanner::builder()
        .rule(Rule::literal("short", "github", Severity::Low))
        .rule(Rule::literal("long", "github_pat", Severity::High))
        .build()
        .expect("scanner should build");

    let source = "github_pat_value";
    let results = scanner.scan([("memory", source)]);

    assert!(
        synthesize(
            source,
            results.single_report().unwrap(),
            &SynthesisOptions::new([1; 32]),
        )
        .is_err()
    );
}
