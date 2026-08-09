use silens_scan::{Remediation, Rule, Scanner, Severity, builtins};

#[test]
fn custom_rules_have_no_remediation_by_default() {
    let scanner = Scanner::builder()
        .rule(Rule::literal("custom.secret", "SECRET", Severity::High))
        .build()
        .unwrap();

    let results = scanner.scan([("memory", "SECRET")]);
    let finding = &results.single_report().unwrap().findings()[0];

    assert_eq!(finding.remediation(), None);
}

#[test]
fn custom_rules_can_attach_remediation() {
    let scanner = Scanner::builder()
        .rule(
            Rule::literal("custom.secret", "SECRET", Severity::High)
                .with_remediation(Remediation::RemoveSensitiveValue),
        )
        .build()
        .unwrap();

    let results = scanner.scan([("memory", "SECRET")]);
    let finding = &results.single_report().unwrap().findings()[0];

    assert_eq!(
        finding.remediation(),
        Some(Remediation::RemoveSensitiveValue),
    );
}

#[test]
fn builtin_remediation_survives_the_scan_pipeline() {
    let scanner = Scanner::builder()
        .rule(builtins::GITHUB_CLASSIC_PAT.to_rule().unwrap())
        .build()
        .unwrap();

    let source = "ghp_abcdefghijklmnopqrstuvwxyzABCDEFGHIJ";
    let results = scanner.scan([("memory", source)]);
    let report = results.single_report().unwrap();

    assert_eq!(report.len(), 1);
    assert_eq!(
        report.findings()[0].remediation(),
        Some(Remediation::RevokeAndRotateCredential),
    );
}
