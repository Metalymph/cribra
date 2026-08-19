use cribra::{Rule, Scanner, Severity, builtins, transform};

#[test]
fn default_scanner_is_external_consumer_accessible() {
    let scanner = Scanner::default();

    let results = scanner.scan([(
        "config.env",
        "GITHUB_TOKEN=ghp_AbCdEf0123456789_AbCdEf0123456789",
    )]);

    assert!(
        !results
            .single_report()
            .expect("one source")
            .findings()
            .is_empty()
    );
}

#[test]
fn selectable_builtins_are_external_consumer_accessible() {
    let scanner = Scanner::builder()
        .builtins([
            builtins::GITHUB_CLASSIC_PAT,
            builtins::STRIPE_LIVE_SECRET_KEY,
        ])
        .build()
        .expect("selected built-ins should compile");

    let _ = scanner;
}

#[test]
fn custom_and_builtin_rules_are_composable() {
    let scanner = Scanner::builder()
        .builtins([builtins::GITHUB_CLASSIC_PAT])
        .rule(Rule::literal(
            "domain.internal-secret",
            "PRIVATE_VALUE",
            Severity::High,
        ))
        .build()
        .expect("mixed scanner should compile");

    let results = scanner.scan([("source", "PRIVATE_VALUE")]);

    assert_eq!(
        results
            .single_report()
            .expect("one source")
            .findings()
            .len(),
        1
    );
}

#[test]
fn transform_namespace_is_external_consumer_accessible() {
    let scanner = Scanner::builder()
        .rule(Rule::literal("credential", "SECRET", Severity::High))
        .build()
        .expect("scanner should compile");

    let source = "TOKEN=SECRET";
    let results = scanner.scan([("source", source)]);
    let report = results.single_report().expect("one source");

    assert_eq!(
        transform::redact(source, report).expect("redaction should succeed"),
        "TOKEN=[REDACTED]"
    );
}
