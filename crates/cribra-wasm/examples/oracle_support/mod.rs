//! Direct-Core fixture configuration and metadata projection, without adapter DTOs.
use cribra::{Explanation, Remediation, Rule, Scanner, Severity};
use serde_json::{Value, json};

fn explanation(value: Explanation) -> Value {
    match value {
        Explanation::Classified(mode) => {
            json!({"kind":"Classified", "detectionMode":format!("{mode:?}")})
        }
        Explanation::Ambiguous(evidence) => {
            json!({"kind":"Ambiguous", "candidateEvidence":format!("{evidence:?}")})
        }
        _ => panic!("unhandled Core explanation"),
    }
}

pub fn report(scanner: &Scanner, report: &cribra::ScanReport, source_bytes: usize) -> Value {
    json!({
        "sourceBytes":source_bytes, "findingCount":report.len(), "candidateCount":report.candidate_len(),
        "needsReview":report.needs_review(), "hasCritical":report.has_critical(),
        "findings":report.findings().iter().map(|f| json!({
            "ruleId":f.rule_id().as_str(), "start":f.location().start(), "end":f.location().end(),
            "line":f.location().line(), "column":f.location().column(), "severity":format!("{:?}",f.severity()),
            "confidence":format!("{:?}", f.confidence()), "remediation":f.remediation().map(|r|format!("{r:?}")),
            "explanation":explanation(f.explanation(scanner).unwrap())
        })).collect::<Vec<_>>(),
        "candidates":report.candidates().iter().map(|c| json!({
            "kind":format!("{:?}",c.kind()), "evidence":format!("{:?}",c.evidence()),
            "start":c.location().start(), "end":c.location().end(), "line":c.location().line(), "column":c.location().column(),
            "explanation":explanation(c.explanation())
        })).collect::<Vec<_>>()
    })
}

pub fn scanner(case: &Value) -> Scanner {
    scanner_result(case).unwrap()
}

pub fn scanner_result(case: &Value) -> Result<Scanner, Value> {
    let mut builder = Scanner::builder();
    if case["builtins"].as_bool().unwrap_or(false) {
        builder = builder.builtins(cribra::builtins::CURRENT);
    }
    if case["financial"].as_bool().unwrap_or(false) {
        builder = builder.builtins(cribra::builtins::financial::CURRENT);
    }
    if case["personal"].as_bool().unwrap_or(false) {
        builder = builder.builtins(cribra::builtins::personal::CURRENT);
    }
    for spec in case["rules"].as_array().unwrap() {
        let id = spec["id"].as_str().unwrap();
        let value = spec["value"].as_str().unwrap();
        let severity = match spec["severity"].as_str().unwrap() {
            "High" => Severity::High,
            "Critical" => Severity::Critical,
            "Medium" => Severity::Medium,
            "Low" => Severity::Low,
            "Info" => Severity::Info,
            _ => panic!("unknown severity"),
        };
        let rule = match spec["kind"].as_str().unwrap() {
            "literal" => Rule::literal(id, value, severity),
            "prefix" => Rule::prefix(id, value, severity),
            "suffix" => Rule::suffix(id, value, severity),
            "pattern" => Rule::pattern(id, value, severity).map_err(|e| rule_error(e, Some(id)))?,
            _ => panic!("unknown rule kind"),
        };
        let remediation = match spec["remediation"].as_str() {
            None | Some("None") => None,
            Some("RevokeAndRotateCredential") => Some(Remediation::RevokeAndRotateCredential),
            Some("RotateCredential") => Some(Remediation::RotateCredential),
            Some("RotatePassword") => Some(Remediation::RotatePassword),
            Some("ReplacePrivateKey") => Some(Remediation::ReplacePrivateKey),
            Some("RemoveSensitiveValue") => Some(Remediation::RemoveSensitiveValue),
            Some("ReviewSensitiveHash") => Some(Remediation::ReviewSensitiveHash),
            Some("ReviewPasswordVerifier") => Some(Remediation::ReviewPasswordVerifier),
            _ => panic!("unknown fixture remediation"),
        };
        builder = builder.rule(match remediation {
            Some(r) => rule.with_remediation(r),
            None => rule,
        });
    }
    builder.build().map_err(|error| {
        use cribra::ScannerBuildError as Core;
        let (code, id) = match &error {
            Core::Rule(e) => {
                return json!({"code":rule_code(e),"ruleId":null,"message":error.to_string()});
            }
            Core::EmptyRuleId => ("EmptyRuleId", None),
            Core::DuplicateRuleId { rule_id } => ("DuplicateRuleId", Some(rule_id.as_str())),
            Core::EmptyMatcher { rule_id } => ("EmptyMatcher", Some(rule_id.as_str())),
            Core::TooManyRules => ("TooManyRules", None),
            Core::AutomatonBuild(_) => ("AutomatonBuild", None),
        };
        json!({"code":code,"ruleId":id,"message":error.to_string()})
    })
}

fn rule_code(error: &cribra::RuleError) -> &'static str {
    match error {
        cribra::RuleError::InvalidPattern(_) => "InvalidPattern",
        cribra::RuleError::PatternMatchesEmpty => "PatternMatchesEmpty",
        cribra::RuleError::MissingCaptureGroup { .. } => "MissingCaptureGroup",
    }
}
fn rule_error(error: cribra::RuleError, id: Option<&str>) -> Value {
    json!({"code":rule_code(&error),"ruleId":id,"message":error.to_string()})
}
