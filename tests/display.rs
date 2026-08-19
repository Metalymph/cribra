use cribra::{
    Confidence, Remediation, Rule, Scanner, Severity,
    transform::{ShareBundle, ShareMode, ShareModeKind},
};

#[test]
fn scalar_display_is_plain_and_stable() {
    assert_eq!(Severity::Critical.to_string(), "critical");
    assert_eq!(Confidence::High.to_string(), "high");
    assert_eq!(ShareModeKind::Template.to_string(), "template");
    assert_eq!(
        Remediation::RotatePassword.to_string(),
        Remediation::RotatePassword.message(),
    );
}

#[test]
fn finding_display_is_human_readable_without_exposing_secret_text() {
    let scanner = Scanner::builder()
        .rule(
            Rule::literal("example.secret", "SECRET", Severity::Critical)
                .with_remediation(Remediation::RevokeAndRotateCredential),
        )
        .build()
        .unwrap();

    let results = scanner.scan([("memory", "TOKEN=SECRET")]);
    let finding = &results.single_report().unwrap().findings()[0];
    let rendered = finding.to_string();

    assert!(rendered.contains("critical"));
    assert!(rendered.contains("high confidence"));
    assert!(rendered.contains("example.secret"));
    assert!(rendered.contains("Revoke and rotate credential"));
    assert!(!rendered.contains("SECRET"));
}

#[test]
fn report_display_uses_one_plain_text_diagnostic_per_finding() {
    let scanner = Scanner::builder()
        .rule(Rule::literal("alpha", "ALPHA", Severity::High))
        .rule(Rule::literal("beta", "BETA", Severity::Medium))
        .build()
        .unwrap();

    let results = scanner.scan([("memory", "ALPHA BETA")]);
    let rendered = results.single_report().unwrap().to_string();

    assert_eq!(rendered.lines().count(), 2);
    assert!(rendered.contains("alpha"));
    assert!(rendered.contains("beta"));
}

#[test]
fn manifest_display_composes_mode_and_existing_scan_summary() {
    let scanner = Scanner::builder()
        .rule(Rule::literal("secret", "SECRET", Severity::Critical))
        .build()
        .unwrap();

    let source = "SECRET";
    let results = scanner.scan([("memory", source)]);
    let bundle = ShareBundle::builder()
        .mode(ShareMode::Template)
        .build(&results, [source])
        .unwrap();

    let rendered = bundle.manifest().to_string();

    assert!(rendered.starts_with("transformation: template\n"));
    assert!(rendered.contains("sources: 1"));
    assert!(rendered.contains("findings: 1"));
}
