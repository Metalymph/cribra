use silens_scan::{Scanner, builtins, Rule, Severity};

#[test]
fn scanner_with_no_rule() {
    let scanner = Scanner::default();
    let scan_report = scanner.scan("A long text containing\nNOTHING");
    assert!(scan_report.findings().is_empty());    
}

#[test]
fn scanner_with_no_findings() {
    let scanner = Scanner::builder()
        .builtins([builtins::STRIPE_SECRET_KEY, builtins::GITHUB_PAT])
        .build();

    let scanner = scanner.unwrap();
    let scan_report = scanner.scan("A long text containing\nNOTHING");
    assert!(scan_report.findings().is_empty());
}

#[test]
fn scanner_with_only_custom_rules() {
    let custom_rules = [
        Rule::prefix(
            "acme.token",
            "acme_live_",
            Severity::Critical,
        ),
        Rule::suffix(
            "acme.private",
            "_private",
            Severity::High,
        ),
    ];
    
    let scanner = Scanner::builder()
        .rules(custom_rules)
        .build();

    let scanner = scanner.unwrap();
    let scan_report = scanner.scan("");
    assert!(!scan_report.findings().is_empty());
}

#[test]
fn scanner_with_mixed_rules() {
    let custom_rules = [
        Rule::prefix(
            "acme.token",
            "acme_live_",
            Severity::Critical,
        ),
        Rule::suffix(
            "acme.private",
            "_private",
            Severity::High,
        ),
    ];
    
    let scanner = Scanner::builder()
        .builtins([
            builtins::STRIPE_SECRET_KEY,
            builtins::GITHUB_PAT,
        ])
        .rules(custom_rules)
        .build();

    let scanner = scanner.unwrap();
    let scan_report = scanner.scan("");
    assert!(!scan_report.findings().is_empty());
}
