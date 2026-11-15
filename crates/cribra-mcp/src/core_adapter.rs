//! Adapter from validated MCP arguments to Cribra Core.
//!
//! This module is the only place that calls Core scanning and transformation.
//! It projects Core's public, presentation-safe metadata into the JSON shapes
//! declared by the MCP output schemas. It never reads or returns matched source
//! text, and it never interpolates Core error text into client-visible
//! messages.

use cribra::{
    Confidence, Finding, IncrementalError, IncrementalLimits, Redaction, Remediation, Rule,
    Scanner, SensitiveCandidate, SensitiveCandidateKind, Severity, builtins,
    transform::{self, TemplateOptions, TransformError},
};
use serde_json::{Map, Value, json};

use crate::limits::{MAX_CANDIDATES, MAX_FINDINGS};
use crate::tool_arguments::{
    CustomRule, ErrorCode, ScanArgs, ToolError, TransformArgs, TransformMode,
};
use crate::tooling::ToolSuccess;

/// The largest number of structural candidates Core can observe for a source
/// within the MCP 1 MiB input limit.
///
/// A recovery-like candidate occupies 19 bytes, and two candidates cannot
/// share a token boundary, so consecutive candidates start at least 20 bytes
/// apart. The 16-byte divisor is therefore a conservative upper bound.
const MAX_STRUCTURAL_CANDIDATES: usize = crate::limits::MAX_SOURCE_BYTES / 16;

/// The result-materialization budget shared by scan and transform.
///
/// Core's budget counts accepted findings plus *pre-suppression* structural
/// candidates, while the MCP per-channel limits apply to the finalized report.
/// Structural candidates that overlap a finding are removed after they have
/// been counted. A budget of `MAX_FINDINGS + MAX_CANDIDATES` would therefore
/// reject scans whose finalized candidate count is zero but whose suppressed
/// structural span count is large. Using the true structural bound keeps the
/// budget above every report that satisfies the per-channel limits, while still
/// bounding pathological finding materialization.
const MAX_MATERIALIZED_RESULTS: usize = MAX_FINDINGS + MAX_STRUCTURAL_CANDIDATES;

/// Executes a validated scan request against Core with bounded result
/// materialization.
pub(crate) fn scan(args: ScanArgs) -> Result<ToolSuccess, ToolError> {
    let scanner = build_scanner(&args.rules)?;
    let limits =
        IncrementalLimits::default().with_max_materialized_results(Some(MAX_MATERIALIZED_RESULTS));
    let session = scanner
        .incremental_session_with_limits(args.source, limits)
        .map_err(incremental_error)?;
    let report = session.report();

    if report.len() > MAX_FINDINGS || report.candidate_len() > MAX_CANDIDATES {
        return Err(ToolError::new(ErrorCode::ResultLimitExceeded));
    }

    let mut findings = Vec::with_capacity(report.len());
    for finding in report.findings() {
        findings.push(finding_json(finding)?);
    }

    let mut candidates = Vec::with_capacity(report.candidate_len());
    for candidate in report.candidates() {
        candidates.push(candidate_json(candidate)?);
    }

    let structured = json!({
        "status": "ok",
        "findings": findings,
        "candidates": candidates,
        "summary": summary_json(report, session.source().len()),
    });

    let text = format!(
        "Scan complete: {} {}, {} {}.",
        report.len(),
        if report.len() == 1 {
            "finding"
        } else {
            "findings"
        },
        report.candidate_len(),
        if report.candidate_len() == 1 {
            "candidate"
        } else {
            "candidates"
        }
    );

    Ok(ToolSuccess { structured, text })
}

/// Executes a validated scan-and-transform request against Core with bounded
/// result materialization.
pub(crate) fn transform(args: TransformArgs) -> Result<ToolSuccess, ToolError> {
    let scanner = build_scanner(&args.rules)?;
    let limits =
        IncrementalLimits::default().with_max_materialized_results(Some(MAX_MATERIALIZED_RESULTS));
    let session = scanner
        .incremental_session_with_limits(args.source, limits)
        .map_err(incremental_error)?;
    let report = session.report();

    if report.len() > MAX_FINDINGS {
        return Err(ToolError::new(ErrorCode::ResultLimitExceeded));
    }

    let source = session.source();
    let output = match &args.mode {
        TransformMode::Redact { replacement } => match replacement {
            Some(replacement) => {
                transform::redact_with(source, report, &Redaction::new(replacement.as_str()))
            }
            None => transform::redact(source, report),
        },
        TransformMode::Template {
            namespace,
            numbered,
        } => {
            let options = TemplateOptions::new()
                .namespace(namespace.as_str())
                .numbered(*numbered);
            transform::template_with(source, report, &options)
        }
    }
    .map_err(transform_error)?;

    let structured = json!({
        "status": "ok",
        "mode": args.mode.as_str(),
        "output": output.as_str(),
        "bytesIn": source.len(),
        "bytesOut": output.len(),
        "findingCount": report.len(),
    });

    Ok(ToolSuccess {
        structured,
        text: output,
    })
}

/// Projects the counters that Core's single-source summary would report.
///
/// This is used because bounded execution goes through an incremental session,
/// which exposes its authoritative report but not `ScanResults::summary()`.
/// The MCP tests compare this projection with Core's summary for representative
/// scans.
fn summary_json(report: &cribra::ScanReport, source_len: usize) -> Value {
    let mut critical = 0;
    let mut high = 0;
    let mut medium = 0;
    let mut low = 0;
    let mut info = 0;
    for finding in report.findings() {
        match finding.severity() {
            Severity::Critical => critical += 1,
            Severity::High => high += 1,
            Severity::Medium => medium += 1,
            Severity::Low => low += 1,
            Severity::Info => info += 1,
        }
    }

    let has_findings = !report.is_empty();
    let has_candidates = report.has_candidates();
    json!({
        "scannedSources": 1,
        "scannedBytes": source_len,
        "reportsWithFindings": usize::from(has_findings),
        "reportsWithoutFindings": usize::from(!has_findings),
        "totalFindings": report.len(),
        "totalCandidates": report.candidate_len(),
        "reportsWithCandidates": usize::from(has_candidates),
        "critical": critical,
        "high": high,
        "medium": medium,
        "low": low,
        "info": info,
    })
}

fn build_scanner(rules: &[CustomRule]) -> Result<Scanner, ToolError> {
    if rules.is_empty() {
        return Ok(Scanner::default());
    }

    let mut compiled = Vec::with_capacity(rules.len());
    for rule in rules {
        let pattern = Rule::pattern(rule.id.as_str(), &rule.pattern, rule.severity)
            .map_err(|_| ToolError::new(ErrorCode::InvalidRulePattern))?;
        compiled.push(pattern);
    }

    Scanner::builder()
        .builtins(builtins::CURRENT)
        .rules(compiled)
        .build()
        .map_err(|_| ToolError::new(ErrorCode::InvalidRuleConfiguration))
}

fn finding_json(finding: &Finding) -> Result<Value, ToolError> {
    let location = finding.location();
    let mut object = Map::new();
    object.insert("ruleId".to_owned(), json!(finding.rule_id().as_str()));
    object.insert("start".to_owned(), json!(location.start()));
    object.insert("end".to_owned(), json!(location.end()));
    object.insert("line".to_owned(), json!(location.line()));
    object.insert("column".to_owned(), json!(location.column()));
    object.insert(
        "severity".to_owned(),
        json!(severity_name(finding.severity())),
    );
    object.insert(
        "confidence".to_owned(),
        json!(confidence_name(finding.confidence())),
    );
    if let Some(remediation) = remediation_name(finding.remediation()) {
        object.insert("remediation".to_owned(), json!(remediation));
    }
    Ok(Value::Object(object))
}

fn candidate_json(candidate: &SensitiveCandidate) -> Result<Value, ToolError> {
    let location = candidate.location();
    let kind =
        candidate_kind_name(candidate.kind()).ok_or(ToolError::new(ErrorCode::InternalError))?;
    let evidence = candidate_evidence_name(candidate.evidence())
        .ok_or(ToolError::new(ErrorCode::InternalError))?;

    Ok(json!({
        "kind": kind,
        "evidence": evidence,
        "start": location.start(),
        "end": location.end(),
        "line": location.line(),
        "column": location.column(),
    }))
}

const fn severity_name(severity: Severity) -> &'static str {
    match severity {
        Severity::Info => "info",
        Severity::Low => "low",
        Severity::Medium => "medium",
        Severity::High => "high",
        Severity::Critical => "critical",
    }
}

const fn confidence_name(confidence: Confidence) -> &'static str {
    match confidence {
        Confidence::Low => "low",
        Confidence::Medium => "medium",
        Confidence::High => "high",
    }
}

const fn remediation_name(remediation: Option<Remediation>) -> Option<&'static str> {
    match remediation {
        Some(Remediation::RevokeAndRotateCredential) => Some("revoke_and_rotate_credential"),
        Some(Remediation::RotateCredential) => Some("rotate_credential"),
        Some(Remediation::RotatePassword) => Some("rotate_password"),
        Some(Remediation::ReplacePrivateKey) => Some("replace_private_key"),
        Some(Remediation::RemoveSensitiveValue) => Some("remove_sensitive_value"),
        Some(Remediation::ReviewSensitiveHash) => Some("review_sensitive_hash"),
        Some(Remediation::ReviewPasswordVerifier) => Some("review_password_verifier"),
        // `Remediation` is non-exhaustive. Unknown future variants are optional
        // metadata, so omitting them cannot corrupt counters or required fields.
        Some(_) | None => None,
    }
}

const fn candidate_kind_name(kind: SensitiveCandidateKind) -> Option<&'static str> {
    match kind {
        SensitiveCandidateKind::RecoveryLikeCode => Some("recovery_like_code"),
        // `SensitiveCandidateKind` is non-exhaustive and `kind` is required by
        // the output schema. An unknown variant is an explicit internal error
        // rather than a silently dropped candidate.
        _ => None,
    }
}

const fn candidate_evidence_name(evidence: cribra::CandidateEvidence) -> Option<&'static str> {
    match evidence {
        cribra::CandidateEvidence::Structural => Some("structural"),
        _ => None,
    }
}

fn transform_error(error: TransformError) -> ToolError {
    match error {
        TransformError::OverlappingSpans { .. } => ToolError::new(ErrorCode::OverlappingSpans),
        TransformError::InvalidSpan { .. } | TransformError::InvalidUtf8Boundary { .. } => {
            ToolError::new(ErrorCode::InternalError)
        }
        _ => ToolError::new(ErrorCode::InternalError),
    }
}

fn incremental_error(error: IncrementalError) -> ToolError {
    match error {
        // The combined budget is equivalent to the two MCP channel limits.
        IncrementalError::ResultLimitExceeded { .. }
        | IncrementalError::CandidateLimitExceeded { .. }
        | IncrementalError::CacheLimitExceeded { .. }
        | IncrementalError::TransactionStorageLimitExceeded { .. } => {
            ToolError::new(ErrorCode::ResultLimitExceeded)
        }
        _ => ToolError::new(ErrorCode::InternalError),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::limits::MAX_FINDINGS;

    const GITHUB_PAT: &str = "ghp_abcdefghijklmnopqrstuvwxyzABCDEFGHIJ";

    fn custom(id: &str, pattern: &str) -> CustomRule {
        CustomRule {
            id: id.to_owned(),
            pattern: pattern.to_owned(),
            severity: Severity::High,
        }
    }

    fn scan_args(source: &str, rules: Vec<CustomRule>) -> ScanArgs {
        ScanArgs {
            source: source.to_owned(),
            rules,
        }
    }

    fn transform_args(source: &str, rules: Vec<CustomRule>, mode: TransformMode) -> TransformArgs {
        TransformArgs {
            source: source.to_owned(),
            rules,
            mode,
        }
    }

    fn structured(result: &ToolSuccess) -> &Value {
        &result.structured
    }

    #[test]
    fn builtin_scan_returns_source_free_metadata() {
        let source = format!("before {GITHUB_PAT} after");
        let result = scan(scan_args(&source, Vec::new())).expect("scan");
        let structured = structured(&result);

        assert_eq!(structured["status"], "ok");
        let findings = structured["findings"].as_array().expect("findings");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0]["ruleId"], "github.classic-pat");
        assert_eq!(findings[0]["severity"], "critical");
        assert_eq!(findings[0]["confidence"], "high");
        assert_eq!(findings[0]["remediation"], "revoke_and_rotate_credential");
        assert_eq!(findings[0]["start"], 7);
        assert_eq!(findings[0]["end"], 7 + GITHUB_PAT.len());
        assert_eq!(findings[0]["line"], 1);
        assert_eq!(findings[0]["column"], 8);

        let text = &result.text;
        assert_eq!(text, "Scan complete: 1 finding, 0 candidates.");
        let encoded = structured.to_string();
        assert!(!encoded.contains(GITHUB_PAT));
        assert!(!encoded.contains("before"));
        assert!(!text.contains(GITHUB_PAT));
    }

    #[test]
    fn custom_rules_extend_builtins_and_preserve_rule_identity() {
        let source = "builtin ghp_abcdefghijklmnopqrstuvwxyzABCDEFGHIJ custom SECRET42";
        let result = scan(scan_args(
            source,
            vec![custom("custom.token", "SECRET[0-9]+")],
        ))
        .expect("scan");
        let findings = result.structured["findings"].as_array().expect("findings");
        let ids: Vec<&str> = findings
            .iter()
            .map(|finding| finding["ruleId"].as_str().expect("rule id"))
            .collect();
        assert_eq!(ids, ["github.classic-pat", "custom.token"]);
        assert_eq!(findings[1]["severity"], "high");
        assert_eq!(findings[1]["start"], 56);
        assert!(!result.structured.to_string().contains("SECRET42"));
    }

    #[test]
    fn invalid_and_empty_match_patterns_are_rejected() {
        for pattern in ["[", "a*", "(?i)*"] {
            let error = scan(scan_args("aaa", vec![custom("custom.bad", pattern)]))
                .expect_err("invalid custom rule");
            assert_eq!(error.code(), ErrorCode::InvalidRulePattern);
        }
    }

    #[test]
    fn duplicate_and_colliding_rule_identifiers_are_configuration_errors() {
        let duplicate = scan(scan_args(
            "abc",
            vec![custom("custom.same", "a"), custom("custom.same", "b")],
        ))
        .expect_err("duplicate identifiers");
        assert_eq!(duplicate.code(), ErrorCode::InvalidRuleConfiguration);

        let colliding = scan(scan_args("a", vec![custom("github.classic-pat", "a")]))
            .expect_err("builtin collision");
        assert_eq!(colliding.code(), ErrorCode::InvalidRuleConfiguration);
    }

    #[test]
    fn unicode_and_multiline_coordinates_follow_core() {
        let source = "α\nβ secret\nγ";
        let result =
            scan(scan_args(source, vec![custom("custom.secret", "secret")])).expect("scan");
        let finding = &result.structured["findings"][0];
        assert_eq!(finding["start"], "α\nβ ".len());
        assert_eq!(finding["end"], "α\nβ secret".len());
        assert_eq!(finding["line"], 2);
        assert_eq!(finding["column"], 3);
    }

    #[test]
    fn candidates_are_reported_without_the_candidate_value() {
        let candidate_value = "ABCD-EFGH-IJKL-MNOP";
        let result = scan(scan_args(candidate_value, Vec::new())).expect("scan");
        assert_eq!(result.structured["findings"], serde_json::json!([]));
        let candidates = result.structured["candidates"]
            .as_array()
            .expect("candidates");
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0]["kind"], "recovery_like_code");
        assert_eq!(candidates[0]["evidence"], "structural");
        assert_eq!(candidates[0]["start"], 0);
        assert_eq!(candidates[0]["end"], candidate_value.len());
        assert_eq!(candidates[0]["line"], 1);
        assert_eq!(candidates[0]["column"], 1);
        assert_eq!(result.text, "Scan complete: 0 findings, 1 candidate.");
        assert!(!result.structured.to_string().contains(candidate_value));
    }

    #[test]
    fn suppressed_structural_candidates_do_not_reject_a_valid_scan() {
        // One broad finding covers every recovery-like token. Core counts all
        // structural candidates before suppression, so a budget of exactly
        // 4096 findings + 4096 candidates would wrongly reject this scan even
        // though the finalized report has one finding and no candidates.
        let token = "ABCD-EFGH-IJKL-MNOP";
        let source = vec![token; 9000].join(" ");
        let result = scan(scan_args(&source, vec![custom("custom.all", ".+")])).expect("scan");
        let findings = result.structured["findings"].as_array().expect("findings");
        assert_eq!(findings.len(), 1);
        assert_eq!(result.structured["candidates"], serde_json::json!([]));
        assert_eq!(result.structured["summary"]["totalFindings"], 1);
        assert_eq!(result.structured["summary"]["totalCandidates"], 0);
    }

    #[test]
    fn projected_summary_matches_core_single_source_summary() {
        let cases = [
            ("clean source".to_owned(), Vec::new()),
            (format!("token {GITHUB_PAT}"), Vec::new()),
            ("ABCD-EFGH-IJKL-MNOP".to_owned(), Vec::new()),
            (
                "a SECRET1 b SECRET2".to_owned(),
                vec![
                    CustomRule {
                        id: "custom.secret".to_owned(),
                        pattern: "SECRET[0-9]".to_owned(),
                        severity: Severity::Medium,
                    },
                    CustomRule {
                        id: "custom.a".to_owned(),
                        pattern: "a".to_owned(),
                        severity: Severity::Low,
                    },
                ],
            ),
        ];

        for (source, rules) in cases {
            let result = scan(scan_args(&source, rules.clone())).expect("scan");
            let scanner = build_scanner(&rules).expect("scanner");
            let results = scanner.scan([("mcp", source.as_str())]);
            let summary = results.summary();
            let projected = &result.structured["summary"];

            assert_eq!(projected["scannedSources"], summary.scanned_sources());
            assert_eq!(projected["scannedBytes"], summary.scanned_bytes());
            assert_eq!(
                projected["reportsWithFindings"],
                summary.reports_with_findings()
            );
            assert_eq!(
                projected["reportsWithoutFindings"],
                summary.reports_without_findings()
            );
            assert_eq!(projected["totalFindings"], summary.total_findings());
            assert_eq!(projected["totalCandidates"], summary.total_candidates());
            assert_eq!(
                projected["reportsWithCandidates"],
                summary.reports_with_candidates()
            );
            assert_eq!(projected["critical"], summary.critical());
            assert_eq!(projected["high"], summary.high());
            assert_eq!(projected["medium"], summary.medium());
            assert_eq!(projected["low"], summary.low());
            assert_eq!(projected["info"], summary.info());
        }
    }

    #[test]
    fn too_many_findings_are_rejected_without_truncation() {
        let source = "a".repeat(MAX_FINDINGS + 1);
        let error =
            scan(scan_args(&source, vec![custom("custom.any", "a")])).expect_err("finding limit");
        assert_eq!(error.code(), ErrorCode::ResultLimitExceeded);
    }

    #[test]
    fn transform_redact_uses_core_defaults_and_custom_replacements() {
        let source = format!("token {GITHUB_PAT} done");
        let result = transform(transform_args(
            &source,
            Vec::new(),
            TransformMode::Redact { replacement: None },
        ))
        .expect("redact");
        let structured = structured(&result);
        assert_eq!(structured["mode"], "redact");
        assert_eq!(structured["output"], "token [REDACTED] done");
        assert_eq!(structured["bytesIn"], source.len());
        assert_eq!(
            structured["bytesOut"].as_u64(),
            Some(structured["output"].as_str().unwrap().len() as u64)
        );
        assert_eq!(structured["findingCount"], 1);
        assert_eq!(result.text, structured["output"]);

        let custom = transform(transform_args(
            &source,
            Vec::new(),
            TransformMode::Redact {
                replacement: Some("***".to_owned()),
            },
        ))
        .expect("custom redact");
        assert_eq!(custom.structured["output"], "token *** done");
    }

    #[test]
    fn transform_template_uses_core_namespace_and_numbering() {
        let source = "SECRET SECRET";
        let default = transform(transform_args(
            source,
            vec![custom("custom.secret", "SECRET")],
            TransformMode::Template {
                namespace: "CRIBRA".to_owned(),
                numbered: false,
            },
        ))
        .expect("template");
        assert_eq!(
            default.structured["output"],
            "<CRIBRA:custom.secret> <CRIBRA:custom.secret>"
        );
        assert_eq!(default.structured["findingCount"], 2);

        let numbered = transform(transform_args(
            source,
            vec![custom("custom.secret", "SECRET")],
            TransformMode::Template {
                namespace: "MY_NS".to_owned(),
                numbered: true,
            },
        ))
        .expect("numbered template");
        assert_eq!(
            numbered.structured["output"],
            "<MY_NS:custom.secret:1> <MY_NS:custom.secret:2>"
        );
    }

    #[test]
    fn overlapping_findings_merge_for_redact_and_fail_for_template() {
        let rules = vec![
            custom("custom.first", "abc"),
            custom("custom.second", "bcd"),
        ];
        let redacted = transform(transform_args(
            "abcd",
            rules.clone(),
            TransformMode::Redact { replacement: None },
        ))
        .expect("overlap merges");
        assert_eq!(redacted.structured["output"], "[REDACTED]");

        let template_error = transform(transform_args(
            "abcd",
            rules,
            TransformMode::Template {
                namespace: "CRIBRA".to_owned(),
                numbered: false,
            },
        ))
        .expect_err("template rejects overlap");
        assert_eq!(template_error.code(), ErrorCode::OverlappingSpans);
    }

    #[test]
    fn transform_without_findings_returns_the_source_unchanged() {
        let source = "nothing sensitive 😀";
        let result = transform(transform_args(
            source,
            Vec::new(),
            TransformMode::Redact { replacement: None },
        ))
        .expect("no-op transform");
        assert_eq!(result.structured["output"], source);
        assert_eq!(result.structured["findingCount"], 0);
        assert_eq!(result.structured["bytesIn"], source.len());
        assert_eq!(result.structured["bytesOut"], source.len());
    }

    #[test]
    fn transform_preserves_surrounding_unicode() {
        let source = "😀 SECRET é";
        let result = transform(transform_args(
            source,
            vec![custom("custom.secret", "SECRET")],
            TransformMode::Redact {
                replacement: Some("…".to_owned()),
            },
        ))
        .expect("unicode redact");
        assert_eq!(result.structured["output"], "😀 … é");
        assert_eq!(
            result.structured["bytesOut"].as_u64(),
            Some("😀 … é".len() as u64)
        );
    }

    #[test]
    fn transform_is_not_rejected_by_candidate_count() {
        let candidate = "ABCD-EFGH-IJKL-MNOP";
        let source = vec![candidate; 5000].join(" ");

        let scan_error = scan(scan_args(&source, Vec::new())).expect_err("candidate limit");
        assert_eq!(scan_error.code(), ErrorCode::ResultLimitExceeded);

        let result = transform(transform_args(
            &source,
            Vec::new(),
            TransformMode::Redact { replacement: None },
        ))
        .expect("candidates do not affect transformation");
        assert_eq!(result.structured["output"], source);
        assert_eq!(result.structured["findingCount"], 0);
    }

    #[test]
    fn transform_rejects_too_many_findings() {
        let source = "a".repeat(MAX_FINDINGS + 1);
        let error = transform(transform_args(
            &source,
            vec![custom("custom.any", "a")],
            TransformMode::Redact { replacement: None },
        ))
        .expect_err("finding limit");
        assert_eq!(error.code(), ErrorCode::ResultLimitExceeded);
    }

    #[test]
    fn transform_errors_never_contain_source_or_pattern_text() {
        let source = "abcd SECRET_PATTERN_CANARY";
        let error = transform(transform_args(
            source,
            vec![
                custom("custom.first", "abc"),
                custom("custom.second", "bcd"),
            ],
            TransformMode::Template {
                namespace: "CRIBRA".to_owned(),
                numbered: false,
            },
        ))
        .expect_err("overlap");
        assert_eq!(error.code(), ErrorCode::OverlappingSpans);
        let message = error.code().message();
        assert!(!message.contains("SECRET_PATTERN_CANARY"));
        assert!(!message.contains("abc"));
        assert!(!message.contains("bcd"));
    }
}
