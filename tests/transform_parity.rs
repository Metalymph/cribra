//! Parity invariants for transformations that operate on finding metadata.

use cribra::{
    Rule, Scanner, Severity,
    transform::{PseudonymizationOptions, pseudonymize, redact, template},
};

#[test]
fn non_synthesis_transforms_are_independent_of_rule_family() {
    let scanner = Scanner::builder()
        .rule(Rule::literal(
            "github.classic-pat",
            "GITHUB_SECRET_VALUE",
            Severity::Critical,
        ))
        .rule(Rule::literal(
            "wireguard.private-key",
            "WIREGUARD_SECRET_VALUE",
            Severity::Critical,
        ))
        .rule(Rule::literal(
            "generic.database-connection-password",
            "DATABASE_SECRET_VALUE",
            Severity::Critical,
        ))
        .build()
        .expect("representative rule identifiers should compile");

    let source = "github=GITHUB_SECRET_VALUE\nwireguard=WIREGUARD_SECRET_VALUE\ndatabase=DATABASE_SECRET_VALUE";
    let results = scanner.scan([("memory", source)]);
    let report = results.single_report().expect("one source was scanned");

    let redacted = redact(source, report).expect("redaction should use finding spans");
    assert!(!redacted.contains("GITHUB_SECRET_VALUE"));
    assert!(!redacted.contains("WIREGUARD_SECRET_VALUE"));
    assert!(!redacted.contains("DATABASE_SECRET_VALUE"));

    let templated = template(source, report).expect("templating should use finding metadata");
    assert!(templated.contains("<CRIBRA:github.classic-pat>"));
    assert!(templated.contains("<CRIBRA:wireguard.private-key>"));
    assert!(templated.contains("<CRIBRA:generic.database-connection-password>"));
    assert!(!templated.contains("GITHUB_SECRET_VALUE"));

    let pseudonymized = pseudonymize(source, report, &PseudonymizationOptions::new([23; 32]))
        .expect("pseudonymization should use finding spans");
    assert!(!pseudonymized.contains("GITHUB_SECRET_VALUE"));
    assert!(!pseudonymized.contains("WIREGUARD_SECRET_VALUE"));
    assert!(!pseudonymized.contains("DATABASE_SECRET_VALUE"));
}
