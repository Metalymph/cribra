//! Test-only direct-Core oracle for the React Native synthetic scan fixture.
//!
//! This bypasses the C ABI and the TurboModule so the React Native result can
//! be compared against Cribra Core itself. It emits only source-free metadata.

use cribra::{
    CandidateEvidence, Confidence, DetectionMode, Explanation, Remediation,
    RuleError, RuleKind, ScannerBuildError, SensitiveCandidateKind, Severity,
};
use serde_json::{Value, json};
use std::fs;

fn remediation_label(value: Remediation) -> &'static str {
    match value {
        Remediation::RevokeAndRotateCredential => "revoke_and_rotate_credential",
        Remediation::RotateCredential => "rotate_credential",
        Remediation::RotatePassword => "rotate_password",
        Remediation::ReplacePrivateKey => "replace_private_key",
        Remediation::RemoveSensitiveValue => "remove_sensitive_value",
        Remediation::ReviewSensitiveHash => "review_sensitive_hash",
        Remediation::ReviewPasswordVerifier => "review_password_verifier",
        _ => "unknown",
    }
}

fn detection_mode_label(value: DetectionMode) -> &'static str {
    match value {
        DetectionMode::MatcherOnly => "matcher_only",
        DetectionMode::Deterministic => "deterministic",
        DetectionMode::Contextual => "contextual",
    }
}

fn rule_kind_label(value: RuleKind) -> &'static str {
    match value {
        RuleKind::Literal => "literal",
        RuleKind::Prefix => "prefix",
        RuleKind::Suffix => "suffix",
        RuleKind::Pattern => "pattern",
    }
}

fn severity_from_label(value: &str) -> Severity {
    match value {
        "info" => Severity::Info,
        "low" => Severity::Low,
        "medium" => Severity::Medium,
        "high" => Severity::High,
        "critical" => Severity::Critical,
        other => panic!("unknown severity label {other}"),
    }
}

fn remediation_from_label(value: Option<&str>) -> Option<Remediation> {
    value.map(|label| match label {
        "revoke_and_rotate_credential" => Remediation::RevokeAndRotateCredential,
        "rotate_credential" => Remediation::RotateCredential,
        "rotate_password" => Remediation::RotatePassword,
        "replace_private_key" => Remediation::ReplacePrivateKey,
        "remove_sensitive_value" => Remediation::RemoveSensitiveValue,
        "review_sensitive_hash" => Remediation::ReviewSensitiveHash,
        "review_password_verifier" => Remediation::ReviewPasswordVerifier,
        other => panic!("unknown remediation label {other}"),
    })
}

fn candidate_kind_label(value: SensitiveCandidateKind) -> &'static str {
    match value {
        SensitiveCandidateKind::RecoveryLikeCode => "recovery_like_code",
        _ => "unknown",
    }
}

fn evidence_label(value: CandidateEvidence) -> &'static str {
    match value {
        CandidateEvidence::Structural => "structural",
        _ => "unknown",
    }
}

fn explanation_json(value: Explanation) -> Value {
    match value {
        Explanation::Classified(mode) => json!({
            "kind": "classified",
            "detectionMode": detection_mode_label(mode),
        }),
        Explanation::Ambiguous(evidence) => json!({
            "kind": "ambiguous",
            "evidence": evidence_label(evidence),
        }),
        _ => json!({"kind": "unknown"}),
    }
}

fn finding_json(scanner: &cribra::Scanner, finding: &cribra::Finding) -> Value {
    let location = finding.location();
    let explanation = finding
        .explanation(scanner)
        .map(explanation_json)
        .unwrap_or_else(|| json!({"kind": "unknown"}));

    json!({
        "ruleId": finding.rule_id().as_str(),
        "span": {"start": location.start(), "end": location.end()},
        "line": location.line(),
        "column": location.column(),
        "severity": finding.severity().to_string(),
        "confidence": confidence_label(finding.confidence()),
        "remediation": finding.remediation().map(remediation_label),
        "explanation": explanation,
    })
}

fn confidence_label(value: Confidence) -> &'static str {
    match value {
        Confidence::Low => "low",
        Confidence::Medium => "medium",
        Confidence::High => "high",
    }
}

fn candidate_json(candidate: &cribra::SensitiveCandidate) -> Value {
    let location = candidate.location();
    json!({
        "kind": candidate_kind_label(candidate.kind()),
        "evidence": evidence_label(candidate.evidence()),
        "span": {"start": location.start(), "end": location.end()},
        "line": location.line(),
        "column": location.column(),
        "explanation": explanation_json(candidate.explanation()),
    })
}

fn result_json(scanner: &cribra::Scanner, entry: &cribra::ScanEntry<String>) -> Value {
    let report = entry.report();
    let findings: Vec<Value> = report
        .findings()
        .iter()
        .map(|finding| finding_json(scanner, finding))
        .collect();
    let candidates: Vec<Value> = report
        .candidates()
        .iter()
        .map(candidate_json)
        .collect();
    let has_critical = report.has_critical();

    json!({
        "sourceBytes": entry.source_bytes(),
        "findings": findings,
        "candidates": candidates,
        "findingCount": findings.len(),
        "candidateCount": report.candidate_len(),
        "needsReview": report.needs_review(),
        "hasCritical": has_critical,
    })
}

fn summary_json(results: &cribra::ScanResults<String>) -> Value {
    let summary = results.summary();
    json!({
        "scannedSources": summary.scanned_sources(),
        "scannedBytes": summary.scanned_bytes(),
        "reportsWithFindings": summary.reports_with_findings(),
        "reportsWithoutFindings": summary.reports_without_findings(),
        "totalFindings": summary.total_findings(),
        "totalCandidates": summary.total_candidates(),
        "reportsWithCandidates": summary.reports_with_candidates(),
        "hasCandidates": summary.has_candidates(),
        "critical": summary.critical(),
        "high": summary.high(),
        "medium": summary.medium(),
        "low": summary.low(),
        "info": summary.info(),
        "isClean": summary.is_clean(),
        "hasCritical": summary.has_critical(),
    })
}

fn rule_error_code(error: &RuleError, rule_id: Option<&str>) -> (u32, Option<String>) {
    let code = match error {
        RuleError::InvalidPattern(_) => 1,
        RuleError::PatternMatchesEmpty => 2,
        RuleError::MissingCaptureGroup { .. } => 3,
    };
    (code, rule_id.map(str::to_owned))
}

fn build_error_code(error: ScannerBuildError) -> (u32, Option<String>) {
    use ScannerBuildError as Build;
    match error {
        Build::Rule(error) => rule_error_code(&error, None),
        Build::EmptyRuleId => (4, None),
        Build::DuplicateRuleId { rule_id } => (5, Some(rule_id.as_str().to_owned())),
        Build::EmptyMatcher { rule_id } => (6, Some(rule_id.as_str().to_owned())),
        Build::TooManyRules => (7, None),
        Build::AutomatonBuild(_) => (8, None),
    }
}

fn build_configured_scanner(
    case: &Value,
) -> Result<cribra::Scanner, (u32, Option<String>)> {
    let current = case["current"].as_bool().unwrap_or(true);
    let financial = case["financial"].as_bool().unwrap_or(false);
    let personal = case["personal"].as_bool().unwrap_or(false);

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

    if let Some(rules) = case["rules"].as_array() {
        for rule in rules {
            let id = rule["id"].as_str().expect("rule id");
            let value = rule["value"].as_str().expect("rule value");
            let severity =
                severity_from_label(rule["severity"].as_str().expect("rule severity"));
            let remediation = remediation_from_label(rule["remediation"].as_str());
            let configured = match rule["kind"].as_str().expect("rule kind") {
                "literal" => cribra::Rule::literal(id, value, severity),
                "prefix" => cribra::Rule::prefix(id, value, severity),
                "suffix" => cribra::Rule::suffix(id, value, severity),
                "pattern" => cribra::Rule::pattern(id, value, severity)
                    .map_err(|error| rule_error_code(&error, Some(id)))?,
                other => panic!("unknown rule kind {other}"),
            };
            builder = builder.rule(match remediation {
                Some(remediation) => configured.with_remediation(remediation),
                None => configured,
            });
        }
    }

    builder.build().map_err(build_error_code)
}

fn rule_metadata_json(metadata: &cribra::RuleMetadata<'_>) -> Value {
    json!({
        "ruleId": metadata.id(),
        "kind": rule_kind_label(metadata.kind()),
        "detectionMode": detection_mode_label(metadata.detection_mode()),
        "severity": metadata.severity().to_string(),
        "remediation": metadata.remediation().map(remediation_label),
    })
}

fn run_config_oracle(path: &str) {
    let raw = fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("read config cases {path}: {error}"));
    let cases: Vec<Value> =
        serde_json::from_str(&raw).expect("config cases must be a JSON array");

    let mut output = Vec::new();
    for case in cases {
        let name = case["name"].as_str().expect("case name").to_owned();
        match build_configured_scanner(&case) {
            Ok(scanner) => {
                let rules: Vec<Value> =
                    scanner.rule_metadata().map(|rule| rule_metadata_json(&rule)).collect();
                let mut entry = json!({"name": name, "rules": rules});

                if let Some(sources) = case["scan"].as_array() {
                    let inputs: Vec<(String, String)> = sources
                        .iter()
                        .map(|source| {
                            (
                                source["id"].as_str().expect("source id").to_owned(),
                                source["text"].as_str().expect("source text").to_owned(),
                            )
                        })
                        .collect();
                    let results = scanner.scan(
                        inputs
                            .iter()
                            .map(|(id, text)| (id.clone(), text.as_str())),
                    );
                    let entries: Vec<Value> = results
                        .iter()
                        .map(|entry| {
                            json!({
                                "id": entry.key(),
                                "sourceBytes": entry.source_bytes(),
                                "result": result_json(&scanner, entry),
                            })
                        })
                        .collect();
                    entry["entries"] = json!(entries);
                    entry["summary"] = summary_json(&results);
                }

                output.push(entry);
            }
            Err((code, rule_id)) => {
                output.push(json!({
                    "name": name,
                    "error": {"code": code, "ruleId": rule_id},
                }));
            }
        }
    }

    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "abiVersion": {"major": 0, "minor": 3, "patch": 0},
            "cases": output,
        }))
        .expect("serialize config oracle")
    );
}

fn main() {
    let mut args = std::env::args().skip(1);
    if args.next().as_deref() == Some("--config-cases") {
        let path = args
            .next()
            .unwrap_or_else(|| "fixtures/rn13-config-cases.json".to_owned());
        run_config_oracle(&path);
        return;
    }

    let fixture_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "fixtures/synthetic-source.json".to_owned());
    let raw = fs::read_to_string(&fixture_path)
        .unwrap_or_else(|error| panic!("read fixture {fixture_path}: {error}"));
    let sources: Vec<Value> =
        serde_json::from_str(&raw).expect("fixture must be a JSON array");

    let inputs: Vec<(String, String)> = sources
        .iter()
        .map(|source| {
            (
                source["id"].as_str().expect("source id").to_owned(),
                source["text"].as_str().expect("source text").to_owned(),
            )
        })
        .collect();

    let scanner = cribra::Scanner::default();
    let results = scanner.scan(
        inputs
            .iter()
            .map(|(id, text)| (id.clone(), text.as_str())),
    );

    let entries: Vec<Value> = results
        .iter()
        .map(|entry| {
            json!({
                "id": entry.key(),
                "sourceBytes": entry.source_bytes(),
                "result": result_json(&scanner, entry),
            })
        })
        .collect();

    let oracle = json!({
        "abiVersion": {"major": 0, "minor": 3, "patch": 0},
        "entries": entries,
        "summary": summary_json(&results),
    });

    println!(
        "{}",
        serde_json::to_string_pretty(&oracle).expect("serialize oracle")
    );
}
