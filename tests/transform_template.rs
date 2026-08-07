//! Public semantic-template transformation tests.

use silens_scan::{
    Rule, Scanner, Severity,
    transform::{TemplateOptions, template, template_with},
};

#[test]
fn public_template_uses_rule_identity_not_secret_value() {
    let scanner = Scanner::builder()
        .rule(Rule::literal(
            "example.credential",
            "SECRET",
            Severity::High,
        ))
        .build()
        .expect("scanner should build");

    let source = "TOKEN=SECRET";
    let results = scanner.scan([("memory", source)]);
    let report = results.single_report().expect("one report");

    let transformed = template(source, report).unwrap();

    assert_eq!(transformed, "TOKEN=<SILENS:example.credential>",);
    assert!(!transformed.contains("SECRET"));
}

#[test]
fn public_template_can_number_repeated_semantic_findings() {
    let scanner = Scanner::builder()
        .rule(Rule::literal("secret", "SECRET", Severity::High))
        .build()
        .expect("scanner should build");

    let source = "A=SECRET B=SECRET";
    let results = scanner.scan([("memory", source)]);
    let report = results.single_report().expect("one report");
    let options = TemplateOptions::new().numbered(true);

    assert_eq!(
        template_with(source, report, &options).unwrap(),
        "A=<SILENS:secret:1> B=<SILENS:secret:2>",
    );
}

#[test]
fn public_template_rejects_ambiguous_overlaps() {
    let scanner = Scanner::builder()
        .rule(Rule::literal("short", "github", Severity::Low))
        .rule(Rule::literal("long", "github_pat", Severity::High))
        .build()
        .expect("scanner should build");

    let source = "github_pat_value";
    let results = scanner.scan([("memory", source)]);
    let report = results.single_report().expect("one report");

    assert!(template(source, report).is_err());
}
