#![cfg(feature = "core")]

#[path = "support/configuration.rs"]
mod support;

use cribra_uniffi::{
    BuildErrorCode, Finding, RuleId, RuleKind, RuleSpec, ScanSource, Scanner, ScannerBuildError,
    ScannerConfig, Severity, SourceId,
};
use serde_json::json;

#[test]
fn configuration_outcomes_match_core_including_error_precedence() {
    let mut successes = 0;
    let mut failures = 0;
    for case in support::cases() {
        let expected = support::core_build(&case.config);
        let actual = Scanner::configured(case.config.clone());
        match (expected, actual) {
            (Ok(core), Ok(scanner)) => {
                successes += 1;
                let expected_metadata: Vec<_> = core
                    .rule_metadata()
                    .map(cribra_uniffi::RuleMetadata::from)
                    .collect();
                assert_eq!(scanner.active_rules(), expected_metadata, "{}", case.name);
                let source = ScanSource {
                    id: SourceId {
                        value: case.name.clone(),
                    },
                    text: case.source,
                };
                let expected = core.scan([(&source.id, source.text.as_str())]);
                let actual = scanner.scan_many(vec![source.clone()]);
                assert_eq!(actual.summary, expected.summary().into(), "{}", case.name);
                assert_eq!(
                    actual.reports[0].findings,
                    expected
                        .single_report()
                        .unwrap()
                        .findings()
                        .iter()
                        .map(Finding::from)
                        .collect::<Vec<_>>(),
                    "{}",
                    case.name
                );
                assert_eq!(scanner.scan(source), actual.reports[0]);
                assert_eq!(scanner.active_rules(), expected_metadata);
                assert_eq!(
                    Scanner::configured(case.config).unwrap().active_rules(),
                    expected_metadata
                );
            }
            (
                Err(expected),
                Err(ScannerBuildError::Rejected {
                    code,
                    rule_id,
                    detail,
                }),
            ) => {
                failures += 1;
                assert_eq!(
                    json!({"code": format!("{code:?}"), "ruleId": rule_id.map(|id| id.value), "detail": detail}),
                    expected,
                    "{}",
                    case.name
                );
            }
            _ => panic!("adapter/Core success mismatch for {}", case.name),
        }
    }
    assert_eq!((successes, failures), (5, 14));
}

#[test]
fn default_and_builtin_only_are_equal_and_custom_only_never_adds_builtins() {
    let default = Scanner::new();
    let builtins = Scanner::configured(ScannerConfig::BuiltInOnly).unwrap();
    assert_eq!(default.active_rules(), builtins.active_rules());
    let empty = Scanner::configured(ScannerConfig::CustomOnly { rules: vec![] }).unwrap();
    assert!(empty.active_rules().is_empty());
    let source = ScanSource {
        id: SourceId {
            value: "config".into(),
        },
        text: "GITHUB_TOKEN=ghp_AbCdEf0123456789_AbCdEf0123456789".into(),
    };
    assert_eq!(builtins.scan(source.clone()), default.scan(source.clone()));
    assert!(!default.scan(source.clone()).findings.is_empty());
    assert!(empty.scan(source).findings.is_empty());
    let custom = support::cases()
        .into_iter()
        .find(|c| c.name == "custom")
        .unwrap();
    let scanner = Scanner::configured(custom.config).unwrap();
    assert_eq!(scanner.active_rules().len(), 4);
    let report = scanner.scan(ScanSource {
        id: SourceId {
            value: "custom".into(),
        },
        text: custom.source,
    });
    assert_eq!(
        report
            .findings
            .iter()
            .map(|f| f.rule_id.value.as_str())
            .collect::<Vec<_>>(),
        vec![
            "custom/literal-é",
            "custom/prefix",
            "custom/suffix",
            "custom/pattern"
        ]
    );
}

#[test]
fn token_boundaries_suffix_limit_and_full_regex_spans_remain_core_owned() {
    let rules = vec![
        spec("prefix", RuleKind::Prefix, "pre_"),
        spec("suffix", RuleKind::Suffix, "_END"),
        spec("pattern", RuleKind::Pattern, "(?P<secret>code)[0-9]{2}"),
    ];
    let config = ScannerConfig::CustomOnly { rules };
    let core = support::core_build(&config).unwrap();
    let scanner = Scanner::configured(config).unwrap();
    for text in [
        "pre_OK xpre_SKIP token_END token_ENDx code42".to_owned(),
        format!("{}_END pre_OK code42", "a".repeat(4097)),
    ] {
        let results = core.scan([("source", text.as_str())]);
        let expected = results.single_report().unwrap();
        let report = scanner.scan(ScanSource {
            id: SourceId {
                value: "source".into(),
            },
            text,
        });
        assert_eq!(
            report.findings,
            expected
                .findings()
                .iter()
                .map(Finding::from)
                .collect::<Vec<_>>()
        );
        assert_eq!(
            report
                .findings
                .iter()
                .find(|f| f.rule_id.value == "pattern")
                .unwrap()
                .location
                .end
                - report
                    .findings
                    .iter()
                    .find(|f| f.rule_id.value == "pattern")
                    .unwrap()
                    .location
                    .start,
            6
        );
    }
}

#[test]
fn rare_core_error_variants_are_mapped_without_manufacturing_invalid_configs() {
    // Conversion tests only: do not allocate billions of rules or break built-ins.
    let capacity = ScannerBuildError::from(cribra::ScannerBuildError::TooManyRules);
    assert!(matches!(
        capacity,
        ScannerBuildError::Rejected {
            code: BuildErrorCode::TooManyRules,
            rule_id: None,
            ..
        }
    ));
    let capture = ScannerBuildError::from(cribra::ScannerBuildError::Rule(
        cribra::RuleError::MissingCaptureGroup {
            name: "absent".into(),
        },
    ));
    assert!(
        matches!(capture, ScannerBuildError::Rejected { code: BuildErrorCode::MissingCaptureGroup, rule_id: None, ref detail } if detail.contains("absent"))
    );
}

fn spec(id: &str, kind: RuleKind, value: &str) -> RuleSpec {
    RuleSpec {
        id: RuleId { value: id.into() },
        kind,
        value: value.into(),
        severity: Severity::High,
        remediation: None,
    }
}
