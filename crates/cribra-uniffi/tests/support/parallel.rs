//! Test-only fixture and independent direct-Core parallel/sequential oracle.
use cribra_uniffi::{RuleId, RuleKind, RuleSpec, ScanSource, ScannerConfig, Severity, SourceId};
use serde_json::{Value, json};

pub const MODES: [&str; 4] = [
    "builtInOnly",
    "customOnly",
    "builtInAndCustom",
    "emptyRules",
];
pub fn fixture() -> Value {
    serde_json::from_str(include_str!("../fixtures/parallel.json")).unwrap()
}
pub fn config(mode: &str) -> ScannerConfig {
    let rules: Vec<_> = fixture()["rules"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| RuleSpec {
            id: RuleId {
                value: r["id"].as_str().unwrap().into(),
            },
            kind: RuleKind::Literal,
            value: r["value"].as_str().unwrap().into(),
            severity: match r["severity"].as_str().unwrap() {
                "info" => Severity::Info,
                "low" => Severity::Low,
                "medium" => Severity::Medium,
                "high" => Severity::High,
                "critical" => Severity::Critical,
                _ => unreachable!(),
            },
            remediation: None,
        })
        .collect();
    match mode {
        "builtInOnly" => ScannerConfig::BuiltInOnly,
        "customOnly" => ScannerConfig::CustomOnly { rules },
        "builtInAndCustom" => ScannerConfig::BuiltInAndCustom { rules },
        "emptyRules" => ScannerConfig::CustomOnly { rules: vec![] },
        _ => unreachable!(),
    }
}
pub fn core(mode: &str) -> cribra::Scanner {
    let mut builder = cribra::Scanner::builder();
    if matches!(mode, "builtInOnly" | "builtInAndCustom") {
        builder = builder.builtins(cribra::builtins::CURRENT);
    }
    if matches!(mode, "customOnly" | "builtInAndCustom") {
        for r in fixture()["rules"].as_array().unwrap() {
            let severity = match r["severity"].as_str().unwrap() {
                "info" => cribra::Severity::Info,
                "low" => cribra::Severity::Low,
                "medium" => cribra::Severity::Medium,
                "high" => cribra::Severity::High,
                "critical" => cribra::Severity::Critical,
                _ => unreachable!(),
            };
            builder = builder.rule(cribra::Rule::literal(
                r["id"].as_str().unwrap(),
                r["value"].as_str().unwrap(),
                severity,
            ));
        }
    }
    builder.build().unwrap()
}
pub fn sources() -> Vec<ScanSource> {
    fixture()["sources"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| ScanSource {
            id: SourceId {
                value: s["id"].as_str().unwrap().into(),
            },
            text: s["text"].as_str().unwrap().into(),
        })
        .collect()
}
pub fn batches() -> Vec<(String, Vec<ScanSource>)> {
    let sources = sources();
    let mut batches = vec![("empty".into(), vec![])];
    batches.extend(
        sources
            .iter()
            .enumerate()
            .map(|(i, s)| (format!("single-{i}"), vec![s.clone()])),
    );
    batches.push(("multiple".into(), sources.clone()));
    batches.push(("reversed".into(), sources.iter().rev().cloned().collect()));
    batches.push((
        "large".into(),
        sources.iter().cycle().take(64).cloned().collect(),
    ));
    batches
}
#[path = "metadata.rs"]
mod metadata;
pub fn snapshot(scanner: &cribra::Scanner, results: &cribra::ScanResults<&SourceId>) -> Value {
    let reports: Vec<_> = results
        .iter()
        .map(|entry| {
            metadata::report(
                scanner,
                &entry.key().value,
                entry.source_bytes(),
                entry.report(),
            )
        })
        .collect();
    json!({"reports":reports,"summary":results.summary()})
}
