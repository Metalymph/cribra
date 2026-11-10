//! Test-only direct-Core oracle for the React Native synthetic scan fixture.
//!
//! This bypasses the C ABI and the TurboModule so the React Native result can
//! be compared against Cribra Core itself. It emits only source-free metadata.

use cribra::{
    CandidateEvidence, Confidence, DetectionMode, Explanation, Remediation,
    SensitiveCandidateKind,
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

fn main() {
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
