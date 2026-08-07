//! Public deterministic pseudonymization tests.

use silens_scan::{
    Rule, Scanner, Severity,
    transform::{PseudonymizationOptions, pseudonymize},
};

#[test]
fn same_secret_across_sources_has_same_pseudonym() {
    let scanner = Scanner::builder()
        .rule(Rule::literal("secret", "SECRET", Severity::High))
        .build()
        .expect("scanner should build");

    let key = [42; 32];
    let options = PseudonymizationOptions::new(key);

    let first_source = "A=SECRET";
    let second_source = "B=SECRET";

    let first_results = scanner.scan([("first", first_source)]);
    let second_results = scanner.scan([("second", second_source)]);

    let first = pseudonymize(
        first_source,
        first_results.single_report().unwrap(),
        &options,
    )
    .unwrap();
    let second = pseudonymize(
        second_source,
        second_results.single_report().unwrap(),
        &options,
    )
    .unwrap();

    assert_eq!(
        first.strip_prefix("A=").unwrap(),
        second.strip_prefix("B=").unwrap(),
    );
}

#[test]
fn changing_pseudonymization_key_breaks_cross_run_linkability() {
    let scanner = Scanner::builder()
        .rule(Rule::literal("secret", "SECRET", Severity::High))
        .build()
        .expect("scanner should build");

    let source = "TOKEN=SECRET";
    let results = scanner.scan([("memory", source)]);
    let report = results.single_report().unwrap();

    let first = pseudonymize(source, report, &PseudonymizationOptions::new([1; 32])).unwrap();
    let second = pseudonymize(source, report, &PseudonymizationOptions::new([2; 32])).unwrap();

    assert_ne!(first, second);
}

#[test]
fn pseudonymization_does_not_embed_rule_or_secret() {
    let scanner = Scanner::builder()
        .rule(Rule::literal(
            "provider.secret.rule",
            "SECRET",
            Severity::Critical,
        ))
        .build()
        .expect("scanner should build");

    let source = "TOKEN=SECRET";
    let results = scanner.scan([("memory", source)]);
    let output = pseudonymize(
        source,
        results.single_report().unwrap(),
        &PseudonymizationOptions::new([7; 32]),
    )
    .unwrap();

    assert!(!output.contains("SECRET"));
    assert!(!output.contains("provider.secret.rule"));
    assert!(output.contains("silens_pseudo_"));
}
