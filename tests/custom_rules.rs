//! Public regression contract for custom rules.
//!
//! v0.2.5 keeps custom rules deliberately small: caller/domain knowledge is
//! expressed through the same declarative rule families as the core engine.
//! A custom rule is authoritative for the span it classifies and therefore
//! remains `MatcherOnly`; it does not gain access to private built-in
//! validators.

use silens_scan::{
    DetectionMode, Explanation, Remediation, Rule, RuleKind, Scanner, ScannerBuildError, Severity,
};

const RECOVERY_LIKE: &str = "ABCD-EFGH-IJKL-MNOP";

#[test]
fn custom_literal_promotes_domain_knowledge_to_a_finding() {
    let scanner = Scanner::builder()
        .rule(
            Rule::literal("acme.recovery-code", RECOVERY_LIKE, Severity::Critical)
                .with_remediation(Remediation::RotateCredential),
        )
        .build()
        .expect("custom scanner should compile");

    let results = scanner.scan([("account.txt", RECOVERY_LIKE)]);
    let report = results.single_report().expect("one source was scanned");

    assert_eq!(report.findings().len(), 1);
    assert_eq!(report.candidate_len(), 0);

    let finding = &report.findings()[0];

    assert_eq!(finding.rule_id().as_str(), "acme.recovery-code");
    assert_eq!(finding.severity(), Severity::Critical);
    assert_eq!(
        finding.explanation(&scanner),
        Some(Explanation::Classified(DetectionMode::MatcherOnly))
    );
}

#[test]
fn custom_rule_metadata_is_introspectable_before_and_after_compilation() {
    let rule = Rule::prefix("acme.service-token", "acme_live_", Severity::Critical)
        .with_remediation(Remediation::RotateCredential);

    let metadata = rule.metadata();

    assert_eq!(metadata.id(), "acme.service-token");
    assert_eq!(metadata.kind(), RuleKind::Prefix);
    assert_eq!(metadata.detection_mode(), DetectionMode::MatcherOnly);
    assert_eq!(metadata.severity(), Severity::Critical);
    assert_eq!(metadata.remediation(), Some(Remediation::RotateCredential));

    let scanner = Scanner::builder()
        .rule(rule)
        .build()
        .expect("custom scanner should compile");

    let compiled = scanner
        .rule_metadata()
        .find(|metadata| metadata.id() == "acme.service-token")
        .expect("compiled metadata should contain custom rule");

    assert_eq!(compiled.kind(), RuleKind::Prefix);
    assert_eq!(compiled.detection_mode(), DetectionMode::MatcherOnly);
    assert_eq!(compiled.severity(), Severity::Critical);
    assert_eq!(compiled.remediation(), Some(Remediation::RotateCredential));
}

#[test]
fn custom_pattern_classifies_only_its_matched_span() {
    let scanner = Scanner::builder()
        .rule(
            Rule::pattern(
                "acme.recovery-code",
                r"\b[A-Z2-9]{4}(?:-[A-Z2-9]{4}){3}\b",
                Severity::High,
            )
            .expect("pattern should compile"),
        )
        .build()
        .expect("custom scanner should compile");

    let source = format!("recovery={RECOVERY_LIKE}\nmode=production");
    let results = scanner.scan([("config.txt", source.as_str())]);
    let report = results.single_report().expect("one source was scanned");

    assert_eq!(report.findings().len(), 1);
    assert_eq!(report.candidate_len(), 0);

    let finding = &report.findings()[0];
    assert_eq!(
        &source[finding.location().start()..finding.location().end()],
        RECOVERY_LIKE
    );
}

#[test]
fn multiple_custom_rules_compose_without_private_validator_access() {
    let scanner = Scanner::builder()
        .rules([
            Rule::literal("acme.recovery-code", RECOVERY_LIKE, Severity::Critical),
            Rule::prefix("acme.service-token", "acme_live_", Severity::High),
        ])
        .build()
        .expect("custom scanner should compile");

    assert_eq!(scanner.rules_count(), 2);

    for metadata in scanner.rule_metadata() {
        assert_eq!(metadata.detection_mode(), DetectionMode::MatcherOnly);
    }

    let results = scanner.scan([("config.txt", "ABCD-EFGH-IJKL-MNOP\nacme_live_1234567890")]);
    let report = results.single_report().expect("one source was scanned");

    assert_eq!(report.findings().len(), 2);
    assert_eq!(report.candidate_len(), 0);
}

#[test]
fn custom_rules_can_be_combined_with_borrowed_builtin_catalog() {
    let scanner = Scanner::builder()
        .builtins(silens_scan::builtins::CURRENT)
        .rule(Rule::literal(
            "acme.recovery-code",
            RECOVERY_LIKE,
            Severity::Critical,
        ))
        .build()
        .expect("mixed scanner should compile");

    assert!(scanner.rules_count() > 1);

    let custom = scanner
        .rule_metadata()
        .find(|metadata| metadata.id() == "acme.recovery-code")
        .expect("custom metadata should be present");

    assert_eq!(custom.detection_mode(), DetectionMode::MatcherOnly);
}

#[test]
fn duplicate_custom_rule_ids_are_rejected_deterministically() {
    let error = Scanner::builder()
        .rules([
            Rule::literal("acme.shared", "FIRST", Severity::High),
            Rule::literal("acme.shared", "SECOND", Severity::Critical),
        ])
        .build()
        .expect_err("duplicate custom rule identifiers should fail");

    assert!(matches!(
        error,
        ScannerBuildError::DuplicateRuleId { ref rule_id }
            if rule_id.as_str() == "acme.shared"
    ));
}

#[test]
fn custom_rule_cannot_shadow_builtin_rule_identity() {
    let builtin = silens_scan::builtins::CURRENT[0];

    let error = Scanner::builder()
        .builtin(builtin)
        .rule(Rule::literal(
            builtin.id(),
            "CUSTOM_VALUE",
            Severity::Critical,
        ))
        .build()
        .expect_err("custom rule should not shadow built-in identity");

    assert!(matches!(
        error,
        ScannerBuildError::DuplicateRuleId { ref rule_id }
            if rule_id.as_str() == builtin.id()
    ));
}

#[test]
fn custom_rule_order_is_preserved_in_metadata() {
    let scanner = Scanner::builder()
        .rules([
            Rule::literal("acme.first", "FIRST", Severity::Low),
            Rule::literal("acme.second", "SECOND", Severity::Medium),
            Rule::literal("acme.third", "THIRD", Severity::High),
        ])
        .build()
        .expect("custom scanner should compile");

    let ids = scanner
        .rule_metadata()
        .map(|metadata| metadata.id().to_owned())
        .collect::<Vec<_>>();

    assert_eq!(ids, ["acme.first", "acme.second", "acme.third"]);
}

#[test]
fn custom_rule_ids_and_matchers_are_validated_at_build_boundary() {
    let empty_id = Scanner::builder()
        .rule(Rule::literal("", "SECRET", Severity::High))
        .build()
        .expect_err("empty rule id should fail");

    assert!(empty_id.to_string().contains("identifier cannot be empty"));

    let empty_matcher = Scanner::builder()
        .rule(Rule::literal("acme.empty", "", Severity::High))
        .build()
        .expect_err("empty matcher should fail");

    assert!(empty_matcher.to_string().contains("empty matcher"));
}
