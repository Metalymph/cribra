//! Test fixture decoding and independent calls to the public Core transform APIs.

use cribra_uniffi::{Confidence, Finding, Location, RuleId, Severity};
use serde_json::{Value, json};

pub fn cases() -> Vec<Value> {
    serde_json::from_str(include_str!("../fixtures/flow.json")).unwrap()
}

pub fn findings(case: &Value) -> Vec<Finding> {
    case["findings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| Finding {
            remediation: None,
            explanation: None,
            rule_id: RuleId {
                value: f["id"].as_str().unwrap().into(),
            },
            location: Location {
                start: f["start"].as_u64().unwrap(),
                end: f["end"].as_u64().unwrap(),
                line: f["line"].as_u64().unwrap_or(1),
                column: f["column"].as_u64().unwrap_or(1),
            },
            severity: match f["severity"].as_str().unwrap_or("high") {
                "info" => Severity::Info,
                "low" => Severity::Low,
                "medium" => Severity::Medium,
                "high" => Severity::High,
                "critical" => Severity::Critical,
                _ => panic!("unknown severity"),
            },
            confidence: match f["confidence"].as_str().unwrap_or("high") {
                "low" => Confidence::Low,
                "medium" => Confidence::Medium,
                "high" => Confidence::High,
                _ => panic!("unknown confidence"),
            },
        })
        .collect()
}

pub fn core_report(case: &Value) -> cribra::ScanReport {
    let findings: Vec<Value> = case["findings"].as_array().unwrap().iter().map(|f| json!({
        "rule_id":f["id"], "location":{"start":f["start"],"end":f["end"],
            "line":f["line"].as_u64().unwrap_or(1),"column":f["column"].as_u64().unwrap_or(1)},
        "severity":f["severity"].as_str().unwrap_or("high"),
        "confidence":f["confidence"].as_str().unwrap_or("high"),
    })).collect();
    serde_json::from_value(json!({"findings":findings,"candidates":[]})).unwrap()
}

pub fn core_error(error: cribra::transform::TransformError) -> Value {
    use cribra::transform::TransformError as E;
    let span = |start, end| Some(json!({"start":start,"end":end}));
    let (code, first, second, length) = match &error {
        E::InvalidSpan {
            start,
            end,
            source_len,
        } => ("InvalidSpan", span(start, end), None, Some(*source_len)),
        E::InvalidUtf8Boundary { start, end } => {
            ("InvalidUtf8Boundary", span(start, end), None, None)
        }
        E::OverlappingSpans { first, second } => (
            "OverlappingSpans",
            span(&first.start, &first.end),
            span(&second.start, &second.end),
            None,
        ),
        _ => panic!("unexpected error for a Core Redact/Template call: {error}"),
    };
    json!({"code":code,"span":first,"otherSpan":second,"sourceLen":length,"detail":error.to_string()})
}

pub fn outcome(result: Result<String, cribra::transform::TransformError>) -> Value {
    match result {
        Ok(content) => json!({"content":content}),
        Err(error) => json!({"error":core_error(error)}),
    }
}

pub fn core_outcomes(case: &Value) -> Value {
    let source = case["source"].as_str().unwrap();
    let report = core_report(case);
    let redact = match case["replacement"].as_str() {
        Some(replacement) => {
            cribra::transform::redact_with(source, &report, &cribra::Redaction::new(replacement))
        }
        None => cribra::transform::redact(source, &report),
    };
    let mut options = cribra::transform::TemplateOptions::new()
        .numbered(case["numbered"].as_bool().unwrap_or(false));
    if let Some(namespace) = case["namespace"].as_str() {
        options = options.namespace(namespace);
    }
    let template = cribra::transform::template_with(source, &report, &options);
    json!({"name":case["name"],"redact":outcome(redact),"template":outcome(template)})
}
