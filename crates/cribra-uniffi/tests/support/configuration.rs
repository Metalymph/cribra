//! Shared test inputs and an independent construction oracle using Core only.

use cribra_uniffi::{Remediation, RuleId, RuleKind, RuleSpec, ScannerConfig, Severity};
use serde_json::{Value, json};

pub struct Case {
    pub name: String,
    pub config: ScannerConfig,
    pub source: String,
}

pub fn cases() -> Vec<Case> {
    let values: Value =
        serde_json::from_str(include_str!("../fixtures/configurations.json")).unwrap();
    values
        .as_array()
        .unwrap()
        .iter()
        .map(|value| {
            let rules = value["rules"]
                .as_array()
                .unwrap()
                .iter()
                .map(|rule| RuleSpec {
                    id: RuleId {
                        value: rule["id"].as_str().unwrap().to_owned(),
                    },
                    kind: match rule["kind"].as_str().unwrap() {
                        "literal" => RuleKind::Literal,
                        "prefix" => RuleKind::Prefix,
                        "suffix" => RuleKind::Suffix,
                        "pattern" => RuleKind::Pattern,
                        _ => panic!("unknown fixture rule kind"),
                    },
                    value: rule["value"].as_str().unwrap().to_owned(),
                    severity: match rule["severity"].as_str().unwrap() {
                        "info" => Severity::Info,
                        "low" => Severity::Low,
                        "medium" => Severity::Medium,
                        "high" => Severity::High,
                        "critical" => Severity::Critical,
                        _ => panic!("unknown fixture severity"),
                    },
                    remediation: rule["remediation"].as_str().map(|value| match value {
                        "removeSensitiveValue" => Remediation::RemoveSensitiveValue,
                        _ => panic!("unknown fixture remediation"),
                    }),
                })
                .collect();
            Case {
                name: value["name"].as_str().unwrap().to_owned(),
                config: match value["mode"].as_str().unwrap() {
                    "builtInOnly" => ScannerConfig::BuiltInOnly,
                    "customOnly" => ScannerConfig::CustomOnly { rules },
                    "builtInAndCustom" => ScannerConfig::BuiltInAndCustom { rules },
                    "catalogs" => ScannerConfig::Catalogs {
                        current: value["current"].as_bool().unwrap(),
                        financial: value["financial"].as_bool().unwrap(),
                        personal: value["personal"].as_bool().unwrap(),
                        rules,
                    },
                    _ => panic!("unknown fixture mode"),
                },
                source: value["source"].as_str().unwrap_or("").to_owned(),
            }
        })
        .collect()
}

fn rule_error(error: &cribra::RuleError, id: Option<&str>) -> Value {
    let code = match &error {
        cribra::RuleError::InvalidPattern(_) => "InvalidPattern",
        cribra::RuleError::PatternMatchesEmpty => "PatternMatchesEmpty",
        cribra::RuleError::MissingCaptureGroup { .. } => "MissingCaptureGroup",
    };
    json!({"code": code, "ruleId": id, "detail": error.to_string()})
}

pub fn core_build(config: &ScannerConfig) -> Result<cribra::Scanner, Value> {
    let (current, financial, personal, specs) = match config {
        ScannerConfig::BuiltInOnly => (true, false, false, &[][..]),
        ScannerConfig::CustomOnly { rules } => (false, false, false, rules.as_slice()),
        ScannerConfig::BuiltInAndCustom { rules } => (true, false, false, rules.as_slice()),
        ScannerConfig::Catalogs {
            current,
            financial,
            personal,
            rules,
        } => (*current, *financial, *personal, rules.as_slice()),
    };
    let mut builder = cribra::Scanner::builder();
    if current {
        builder = builder.builtins(cribra::builtins::CURRENT);
    }
    if financial {
        builder = builder.builtins(cribra::builtins::financial::CURRENT);
    }
    if personal {
        builder = builder.builtins(cribra::builtins::personal::CURRENT);
    }
    for spec in specs {
        let severity = match spec.severity {
            Severity::Info => cribra::Severity::Info,
            Severity::Low => cribra::Severity::Low,
            Severity::Medium => cribra::Severity::Medium,
            Severity::High => cribra::Severity::High,
            Severity::Critical => cribra::Severity::Critical,
        };
        let rule = match spec.kind {
            RuleKind::Literal => {
                cribra::Rule::literal(spec.id.value.clone(), spec.value.clone(), severity)
            }
            RuleKind::Prefix => {
                cribra::Rule::prefix(spec.id.value.clone(), spec.value.clone(), severity)
            }
            RuleKind::Suffix => {
                cribra::Rule::suffix(spec.id.value.clone(), spec.value.clone(), severity)
            }
            RuleKind::Pattern => {
                cribra::Rule::pattern(spec.id.value.clone(), &spec.value, severity)
                    .map_err(|error| rule_error(&error, Some(&spec.id.value)))?
            }
        };
        let rule = match spec.remediation {
            Some(Remediation::RemoveSensitiveValue) => {
                rule.with_remediation(cribra::Remediation::RemoveSensitiveValue)
            }
            None => rule,
            _ => panic!("unsupported fixture remediation"),
        };
        builder = builder.rule(rule);
    }
    builder.build().map_err(|error| {
        use cribra::ScannerBuildError as E;
        let (code, id) = match &error {
            E::Rule(error) => return rule_error(error, None),
            E::EmptyRuleId => ("EmptyRuleId", None),
            E::DuplicateRuleId { rule_id } => ("DuplicateRuleId", Some(rule_id.as_str())),
            E::EmptyMatcher { rule_id } => ("EmptyMatcher", Some(rule_id.as_str())),
            E::TooManyRules => ("TooManyRules", None),
            E::AutomatonBuild(_) => ("AutomatonBuild", None),
        };
        json!({"code": code, "ruleId": id, "detail": error.to_string()})
    })
}
