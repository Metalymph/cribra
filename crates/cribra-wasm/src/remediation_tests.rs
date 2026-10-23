//! Custom remediation is a Core configuration projection, never classification.
use super::*;

fn rule(kind: usize, id: &str, value: &str) -> Rule {
    match kind {
        0 => Rule::literal(id, value, Severity::High),
        1 => Rule::prefix(id, value, Severity::High),
        2 => Rule::suffix(id, value, Severity::High),
        3 => Rule::pattern(id, value, Severity::High).unwrap(),
        _ => unreachable!(),
    }
}

#[test]
fn every_family_and_remediation_matches_direct_core_metadata_and_findings() {
    let values = ["SECRET", "pre_", "_END", "code[0-9]{2}"];
    let source = "é🙂 SECRET pre_ABC token_END code42\nABCD-EFGH-IJKL-MNOP";
    for (projection, remediation) in [
        (None, None),
        (Some(RemediationKind::None), None),
        (
            Some(RemediationKind::RevokeAndRotateCredential),
            Some(Remediation::RevokeAndRotateCredential),
        ),
        (
            Some(RemediationKind::RotateCredential),
            Some(Remediation::RotateCredential),
        ),
        (
            Some(RemediationKind::RotatePassword),
            Some(Remediation::RotatePassword),
        ),
        (
            Some(RemediationKind::ReplacePrivateKey),
            Some(Remediation::ReplacePrivateKey),
        ),
        (
            Some(RemediationKind::RemoveSensitiveValue),
            Some(Remediation::RemoveSensitiveValue),
        ),
        (
            Some(RemediationKind::ReviewSensitiveHash),
            Some(Remediation::ReviewSensitiveHash),
        ),
        (
            Some(RemediationKind::ReviewPasswordVerifier),
            Some(Remediation::ReviewPasswordVerifier),
        ),
    ] {
        assert_eq!(remediation_core(projection).unwrap(), remediation);
        for (kind, value) in values.into_iter().enumerate() {
            let mut builder = ScanEngineBuilder::new(false);
            if kind == 3 {
                builder
                    .add_pattern_with_remediation_core(
                        "custom/clé-🔑",
                        value,
                        FindingSeverity::High,
                        projection,
                    )
                    .unwrap();
            } else {
                builder
                    .push_remediated_rule(rule(kind, "custom/clé-🔑", value), projection)
                    .unwrap();
            }
            let core_rule = rule(kind, "custom/clé-🔑", value);
            let core_rule = match remediation {
                Some(r) => core_rule.with_remediation(r),
                None => core_rule,
            };
            let core = Scanner::builder().rule(core_rule).build().unwrap();
            let engine = builder.build_core().unwrap();
            let actual = engine.scan(source);
            let expected = core.scan([((), source)]);
            assert_eq!(&actual.report, expected.single_report().unwrap());
            assert_eq!((actual.finding_count(), actual.candidate_count()), (1, 1));
            assert_eq!(
                actual.finding_at(0).unwrap().remediation(),
                RemediationKind::from(remediation)
            );
            let metadata = engine.scanner.rule_metadata().next().unwrap();
            assert_eq!(metadata.remediation(), remediation);
            assert_eq!(
                metadata.detection_mode(),
                cribra::DetectionMode::MatcherOnly
            );
        }
    }
}

#[test]
fn unknown_remediation_and_invalid_patterns_do_not_stage_rules() {
    for (kind, value) in ["SECRET", "pre_", "_END", "code42"].into_iter().enumerate() {
        let mut builder = ScanEngineBuilder::new(false);
        assert!(
            builder
                .push_remediated_rule(rule(kind, "custom", value), Some(RemediationKind::Unknown))
                .is_err()
        );
        assert_eq!(builder.custom_rule_count(), 0);
    }
    let mut builder = ScanEngineBuilder::new(false);
    for pattern in ["[", "a*", r"\b"] {
        let expected = Rule::pattern("invalid", pattern, Severity::High)
            .unwrap_err()
            .to_string();
        assert_eq!(
            builder
                .add_pattern_with_remediation_core(
                    "invalid",
                    pattern,
                    FindingSeverity::High,
                    Some(RemediationKind::Unknown)
                )
                .unwrap_err(),
            expected
        );
        assert_eq!(builder.custom_rule_count(), 0);
    }
}

#[test]
fn remediation_preserves_core_builder_validation_and_error_precedence() {
    for kind in 0..3 {
        for (id, value) in [("", "SECRET"), ("empty", "")] {
            let mut builder = ScanEngineBuilder::new(false);
            builder
                .push_remediated_rule(
                    rule(kind, id, value),
                    Some(RemediationKind::RotateCredential),
                )
                .unwrap();
            let expected = Scanner::builder()
                .rule(rule(kind, id, value).with_remediation(Remediation::RotateCredential))
                .build()
                .unwrap_err()
                .to_string();
            assert_eq!(builder.build_core().err().unwrap(), expected);
        }
    }
    let mut builder = ScanEngineBuilder::new(false);
    builder
        .push_remediated_rule(
            rule(0, "same", "SECRET"),
            Some(RemediationKind::RotatePassword),
        )
        .unwrap();
    builder.add_literal("same", "SECOND", FindingSeverity::Low);
    assert!(
        builder
            .build_core()
            .err()
            .unwrap()
            .contains("duplicate rule identifier")
    );
}
