//! Native metadata authority and structured category checks; JS errors run in WASM.
use super::*;

#[test]
fn metadata_preserves_core_order_for_all_catalogs_and_custom_families() {
    for mask in 0..8 {
        for custom in [false, true] {
            let mut builder = ScanEngineBuilder::new(mask & 1 != 0);
            let mut core = Scanner::builder();
            if mask & 1 != 0 {
                core = core.builtins(cribra::builtins::CURRENT);
            }
            if mask & 2 != 0 {
                builder.add_financial_builtins();
                core = core.builtins(cribra::builtins::financial::CURRENT);
            }
            if mask & 4 != 0 {
                builder.add_personal_builtins();
                core = core.builtins(cribra::builtins::personal::CURRENT);
            }
            if custom {
                builder.add_literal("custom/clé-🔑", "SECRET", FindingSeverity::High);
                builder
                    .push_remediated_rule_structured(
                        Rule::prefix("prefix", "pre_", Severity::High),
                        Some(RemediationKind::RotatePassword),
                    )
                    .unwrap();
                builder.add_suffix("suffix", "_END", FindingSeverity::Low);
                builder
                    .add_pattern_structured(
                        "pattern",
                        "code[0-9]{2}",
                        FindingSeverity::Critical,
                        Some(RemediationKind::ReviewPasswordVerifier),
                    )
                    .unwrap();
                core = core
                    .rule(Rule::literal("custom/clé-🔑", "SECRET", Severity::High))
                    .rule(
                        Rule::prefix("prefix", "pre_", Severity::High)
                            .with_remediation(Remediation::RotatePassword),
                    )
                    .rule(Rule::suffix("suffix", "_END", Severity::Low))
                    .rule(
                        Rule::pattern("pattern", "code[0-9]{2}", Severity::Critical)
                            .unwrap()
                            .with_remediation(Remediation::ReviewPasswordVerifier),
                    );
            }
            let core = core.build().unwrap();
            let engine = builder.build_structured().unwrap();
            let expected: Vec<_> = core.rule_metadata().map(RuleMetadataView::from).collect();
            assert_eq!(engine.active_rules(), expected);
            assert_eq!(engine.active_rules().len(), engine.rules_count());
            if custom {
                let first = &engine.active_rules()[0];
                assert_eq!(first.rule_id(), "custom/clé-🔑");
                assert_eq!(first.kind(), RuleKind::Literal);
                assert_eq!(first.detection_mode(), DetectionModeKind::MatcherOnly);
                assert_eq!(first.remediation(), RemediationKind::None);
            }
        }
    }
    assert_eq!(
        ScanEngine::new().active_rules(),
        Scanner::default()
            .rule_metadata()
            .map(RuleMetadataView::from)
            .collect::<Vec<_>>()
    );
}

#[test]
fn metadata_owns_its_strings_and_outlives_engine_and_results() {
    let mut builder = ScanEngineBuilder::new(false);
    let id = String::from("custom/clé-🔑");
    let value = String::from("DO_NOT_EXPOSE");
    builder.add_literal(&id, &value, FindingSeverity::High);
    drop(id);
    drop(value);
    let engine = builder.build_structured().unwrap();
    let rules = engine.active_rules();
    let second = engine.active_rules();
    let result = engine.scan("DO_NOT_EXPOSE");
    drop(result);
    drop(engine);
    assert_eq!(rules, second);
    assert_eq!(rules[0].rule_id(), "custom/clé-🔑");
    assert!(!format!("{rules:?}").contains("DO_NOT_EXPOSE"));
}

#[test]
fn build_categories_are_derived_before_diagnostic_formatting() {
    for (pattern, code) in [
        ("[", ScannerBuildErrorCode::InvalidPattern),
        ("a*", ScannerBuildErrorCode::PatternMatchesEmpty),
        (r"\b", ScannerBuildErrorCode::PatternMatchesEmpty),
    ] {
        let expected = Rule::pattern("custom/clé-🔑", pattern, Severity::High)
            .unwrap_err()
            .to_string();
        let mut builder = ScanEngineBuilder::new(false);
        let error = builder
            .add_pattern_structured("custom/clé-🔑", pattern, FindingSeverity::High, None)
            .unwrap_err();
        assert_eq!(error.code, code);
        assert_eq!(error.rule_id.as_deref(), Some("custom/clé-🔑"));
        assert_eq!(error.message, expected);
        assert_eq!(builder.custom_rule_count(), 0);
    }
    for (id, value, duplicate, code) in [
        ("", "SECRET", false, ScannerBuildErrorCode::EmptyRuleId),
        ("empty", "", false, ScannerBuildErrorCode::EmptyMatcher),
        ("same", "", true, ScannerBuildErrorCode::DuplicateRuleId),
    ] {
        let mut builder = ScanEngineBuilder::new(false);
        builder.add_literal(id, value, FindingSeverity::High);
        let mut core = Scanner::builder().rule(Rule::literal(id, value, Severity::High));
        if duplicate {
            builder.add_prefix(id, "pre_", FindingSeverity::Low);
            core = core.rule(Rule::prefix(id, "pre_", Severity::Low));
        }
        let expected = core.build().unwrap_err().to_string();
        let error = builder.build_structured().err().unwrap();
        assert_eq!(error.code, code);
        assert_eq!(error.message, expected);
        assert_eq!(
            error.rule_id.as_deref(),
            if code == ScannerBuildErrorCode::EmptyRuleId {
                None
            } else {
                Some(id)
            }
        );
    }
    for (error, code) in [
        (
            cribra::ScannerBuildError::TooManyRules,
            ScannerBuildErrorCode::TooManyRules,
        ),
        (
            cribra::ScannerBuildError::Rule(cribra::RuleError::MissingCaptureGroup {
                name: "absent".into(),
            }),
            ScannerBuildErrorCode::MissingCaptureGroup,
        ),
    ] {
        let expected_message = error.to_string();
        let mapped = BuildFailure::scanner(error);
        assert_eq!(mapped.code, code);
        assert_eq!(mapped.rule_id, None);
        assert_eq!(mapped.message, expected_message);
    }
}
