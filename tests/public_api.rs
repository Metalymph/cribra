use silens_scan::{Rule, Scanner, Severity, builtins};

#[test]
fn scanner_starts_with_zero_rules() {
    let scanner = Scanner::default();

    assert_eq!(scanner.rules_count(), 0);

    let report = scanner.scan("sk_live_should_not_match");

    assert!(report.is_empty());
    assert_eq!(report.len(), 0);
}

#[test]
fn scanner_with_selected_builtins_and_no_findings() {
    let scanner = Scanner::builder()
        .builtins([builtins::STRIPE_SECRET_KEY, builtins::GITHUB_PAT])
        .build()
        .expect("built-in rules must be valid");

    assert_eq!(scanner.rules_count(), 2);

    let report = scanner.scan("A long text containing\nnothing sensitive.");

    assert!(report.is_empty());
}

#[test]
fn literal_rule_matches_exact_text() {
    let scanner = Scanner::builder()
        .rule(Rule::literal(
            "custom.literal",
            "SUPER_SECRET",
            Severity::High,
        ))
        .build()
        .expect("literal rule must build");

    let source = "before SUPER_SECRET after";
    let report = scanner.scan(source);

    assert_eq!(report.len(), 1);

    let finding = &report.findings()[0];
    let location = finding.location();

    let start = source.find("SUPER_SECRET").unwrap();

    assert_eq!(finding.rule_id().as_str(), "custom.literal");
    assert_eq!(finding.severity(), Severity::High);
    assert_eq!(location.start(), start);
    assert_eq!(location.end(), start + "SUPER_SECRET".len());
    assert_eq!(location.line(), 1);
    assert_eq!(location.column(), 8);
}

#[test]
fn literal_rule_finds_multiple_occurrences() {
    let scanner = Scanner::builder()
        .rule(Rule::literal("custom.literal", "SECRET", Severity::High))
        .build()
        .expect("literal rule must build");

    let report = scanner.scan("SECRET middle SECRET");

    assert_eq!(report.len(), 2);

    assert_eq!(report.findings()[0].location().start(), 0);
    assert_eq!(
        report.findings()[1].location().start(),
        "SECRET middle ".len()
    );
}

#[test]
fn prefix_rule_matches_the_complete_token() {
    let scanner = Scanner::builder()
        .rule(Rule::prefix("acme.token", "acme_live_", Severity::Critical))
        .build()
        .expect("prefix rule must build");

    let source = "token=acme_live_abc123";
    let report = scanner.scan(source);

    assert_eq!(report.len(), 1);

    let finding = &report.findings()[0];
    let location = finding.location();
    let token = "acme_live_abc123";
    let start = source.find(token).unwrap();

    assert_eq!(finding.rule_id().as_str(), "acme.token");
    assert_eq!(location.start(), start);
    assert_eq!(location.end(), start + token.len());
}

#[test]
fn prefix_rule_requires_a_token_boundary_before_prefix() {
    let scanner = Scanner::builder()
        .rule(Rule::prefix("acme.token", "acme_live_", Severity::Critical))
        .build()
        .expect("prefix rule must build");

    let report = scanner.scan("prefixacme_live_abc123");

    assert!(report.is_empty());
}

#[test]
fn suffix_rule_matches_the_complete_token() {
    let scanner = Scanner::builder()
        .rule(Rule::suffix("acme.private", "_private", Severity::High))
        .build()
        .expect("suffix rule must build");

    let source = "token=abc123_private";
    let report = scanner.scan(source);

    assert_eq!(report.len(), 1);

    let finding = &report.findings()[0];
    let location = finding.location();
    let token = "abc123_private";
    let start = source.find(token).unwrap();

    assert_eq!(finding.rule_id().as_str(), "acme.private");
    assert_eq!(location.start(), start);
    assert_eq!(location.end(), start + token.len());
}

#[test]
fn suffix_rule_requires_a_token_boundary_after_suffix() {
    let scanner = Scanner::builder()
        .rule(Rule::suffix("acme.private", "_private", Severity::High))
        .build()
        .expect("suffix rule must build");

    let report = scanner.scan("abc_private_suffix");

    assert!(report.is_empty());
}

#[test]
fn pattern_rule_matches_the_exact_pattern_span() {
    let rule = Rule::pattern("custom.pattern", r"tok_[A-Za-z0-9]{8}", Severity::Critical)
        .expect("pattern must be valid");

    let scanner = Scanner::builder()
        .rule(rule)
        .build()
        .expect("scanner must build");

    let source = "key=tok_a1b2c3d4;";
    let report = scanner.scan(source);

    assert_eq!(report.len(), 1);

    let finding = &report.findings()[0];
    let location = finding.location();
    let token = "tok_a1b2c3d4";
    let start = source.find(token).unwrap();

    assert_eq!(finding.rule_id().as_str(), "custom.pattern");
    assert_eq!(location.start(), start);
    assert_eq!(location.end(), start + token.len());
}

#[test]
fn pattern_rule_rejects_invalid_regex() {
    let rule = Rule::pattern("invalid.pattern", r"[unterminated", Severity::High);

    assert!(rule.is_err());
}

#[test]
fn scanner_supports_only_custom_rules() {
    let custom_rules = [
        Rule::prefix("acme.token", "acme_live_", Severity::Critical),
        Rule::suffix("acme.private", "_private", Severity::High),
    ];

    let scanner = Scanner::builder()
        .rules(custom_rules)
        .build()
        .expect("custom rules must build");

    let report = scanner.scan("first=acme_live_abc123 second=xyz_private");

    assert_eq!(report.len(), 2);
}

#[test]
fn scanner_supports_mixed_builtin_and_custom_rules() {
    let scanner = Scanner::builder()
        .builtin(builtins::STRIPE_SECRET_KEY)
        .rule(Rule::prefix("acme.token", "acme_live_", Severity::Critical))
        .build()
        .expect("mixed rules must build");

    let report = scanner.scan("stripe=sk_live_abc123 custom=acme_live_xyz789");

    assert_eq!(report.len(), 2);
}

#[test]
fn findings_are_sorted_by_source_position_not_rule_order() {
    let scanner = Scanner::builder()
        // Deliberately add the later match first.
        .rule(Rule::literal("later", "SECOND", Severity::High))
        .rule(Rule::literal("earlier", "FIRST", Severity::High))
        .build()
        .expect("rules must build");

    let report = scanner.scan("FIRST then SECOND");

    assert_eq!(report.len(), 2);
    assert_eq!(report.findings()[0].rule_id().as_str(), "earlier");
    assert_eq!(report.findings()[1].rule_id().as_str(), "later");
}

#[test]
fn location_uses_zero_based_bytes_and_one_based_unicode_line_column() {
    let scanner = Scanner::builder()
        .rule(Rule::prefix(
            "unicode.location",
            "sk_live_",
            Severity::Critical,
        ))
        .build()
        .expect("rule must build");

    let source = "αβ\nx sk_live_token";
    let token = "sk_live_token";

    let report = scanner.scan(source);

    assert_eq!(report.len(), 1);

    let location = report.findings()[0].location();
    let start = source.find(token).unwrap();

    // α and β occupy two UTF-8 bytes each.
    assert_eq!(location.start(), start);
    assert_eq!(location.end(), start + token.len());

    // Public coordinates are one-based.
    assert_eq!(location.line(), 2);

    // Line 2 is:
    // x[1] space[2] s[3]
    assert_eq!(location.column(), 3);
}

#[test]
fn location_column_counts_unicode_scalars_not_utf8_bytes() {
    let scanner = Scanner::builder()
        .rule(Rule::literal("unicode.column", "SECRET", Severity::High))
        .build()
        .expect("rule must build");

    let source = "àèì SECRET";
    let report = scanner.scan(source);

    assert_eq!(report.len(), 1);

    let location = report.findings()[0].location();

    // Three Unicode scalars + one space precede SECRET.
    assert_eq!(location.line(), 1);
    assert_eq!(location.column(), 5);

    // But byte offset is larger because à/è/ì are multibyte UTF-8.
    assert_eq!(location.start(), source.find("SECRET").unwrap());
    assert!(location.start() > location.column() - 1);
}
