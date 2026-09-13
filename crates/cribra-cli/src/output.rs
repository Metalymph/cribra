//! Stable human-readable output for the canonical Cribra CLI.
//!
//! Output is derived exclusively from Cribra public result metadata. Matched
//! source values are never rendered.

use std::fmt::Write;

use cribra::{
    CandidateEvidence, Confidence, Remediation, ScanReport, SensitiveCandidateKind, Severity,
};

use crate::command::OutputFormat;

fn push_json_field(output: &mut String, name: &str, value: &str) {
    push_json_string(output, name);
    output.push(':');
    push_json_string(output, value);
}

fn push_json_string(output: &mut String, value: &str) {
    output.push('"');

    for character in value.chars() {
        match character {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            '\u{08}' => output.push_str("\\b"),
            '\u{0c}' => output.push_str("\\f"),
            character if character <= '\u{1f}' => {
                write!(output, "\\u{:04x}", character as u32)
                    .expect("writing to String cannot fail");
            }
            character => output.push(character),
        }
    }

    output.push('"');
}

/// Renders one scan report as deterministic metadata-only JSON.
pub(crate) fn render_json(source_name: &str, report: &ScanReport) -> String {
    let mut output = String::new();

    let status = if !report.findings().is_empty() {
        "findings"
    } else if !report.candidates().is_empty() {
        "review"
    } else {
        "clean"
    };

    output.push('{');

    push_json_field(&mut output, "source", source_name);
    output.push(',');

    push_json_field(&mut output, "status", status);
    output.push(',');

    write!(output, "\"findings_count\":{},", report.len()).expect("writing to String cannot fail");

    write!(output, "\"candidates_count\":{},", report.candidate_len())
        .expect("writing to String cannot fail");

    output.push_str("\"findings\":[");

    for (index, finding) in report.findings().iter().enumerate() {
        if index != 0 {
            output.push(',');
        }

        let location = finding.location();

        output.push('{');

        push_json_field(&mut output, "rule_id", finding.rule_id().as_str());
        output.push(',');

        write!(
            output,
            "\"start\":{},\"end\":{},\"line\":{},\"column\":{},",
            location.start(),
            location.end(),
            location.line(),
            location.column(),
        )
        .expect("writing to String cannot fail");

        push_json_field(&mut output, "severity", severity_name(finding.severity()));
        output.push(',');

        push_json_field(
            &mut output,
            "confidence",
            confidence_name(finding.confidence()),
        );
        output.push(',');

        push_json_field(
            &mut output,
            "remediation",
            remediation_name(finding.remediation()),
        );

        output.push('}');
    }

    output.push_str("],\"candidates\":[");

    for (index, candidate) in report.candidates().iter().enumerate() {
        if index != 0 {
            output.push(',');
        }

        let location = candidate.location();

        output.push('{');

        write!(
            output,
            "\"start\":{},\"end\":{},\"line\":{},\"column\":{},",
            location.start(),
            location.end(),
            location.line(),
            location.column(),
        )
        .expect("writing to String cannot fail");

        push_json_field(&mut output, "kind", candidate_kind_name(candidate.kind()));
        output.push(',');

        push_json_field(
            &mut output,
            "evidence",
            candidate_evidence_name(candidate.evidence()),
        );

        output.push('}');
    }

    output.push_str("]}\n");

    output
}

/// Renders one scan report using the requested stable CLI format.
pub(crate) fn render(format: OutputFormat, source_name: &str, report: &ScanReport) -> String {
    match format {
        OutputFormat::Human => render_human(source_name, report),
        OutputFormat::Json => render_json(source_name, report),
    }
}

/// Renders one scan report in Cribra's stable human-readable CLI format.
///
/// The output contains source identity, aggregate state, finding metadata and
/// review-candidate metadata. It never includes matched source material.
pub(crate) fn render_human(source_name: &str, report: &ScanReport) -> String {
    let mut output = String::new();

    let status = if !report.findings().is_empty() {
        "findings"
    } else if !report.candidates().is_empty() {
        "review"
    } else {
        "clean"
    };

    writeln!(output, "source: {source_name}").expect("writing to String cannot fail");
    writeln!(output, "status: {status}").expect("writing to String cannot fail");
    writeln!(output, "findings: {}", report.len()).expect("writing to String cannot fail");
    writeln!(output, "candidates: {}", report.candidate_len())
        .expect("writing to String cannot fail");

    for finding in report.findings() {
        let location = finding.location();

        writeln!(
            output,
            "finding: {} {}:{} bytes={}..{} severity={} confidence={} remediation={}",
            finding.rule_id().as_str(),
            location.line(),
            location.column(),
            location.start(),
            location.end(),
            severity_name(finding.severity()),
            confidence_name(finding.confidence()),
            remediation_name(finding.remediation()),
        )
        .expect("writing to String cannot fail");
    }

    for candidate in report.candidates() {
        let location = candidate.location();

        writeln!(
            output,
            "candidate: {}:{} bytes={}..{} evidence={}",
            location.line(),
            location.column(),
            location.start(),
            location.end(),
            candidate_evidence_name(candidate.evidence()),
        )
        .expect("writing to String cannot fail");
    }

    output
}

fn severity_name(severity: Severity) -> &'static str {
    match severity {
        Severity::Info => "info",
        Severity::Low => "low",
        Severity::Medium => "medium",
        Severity::High => "high",
        Severity::Critical => "critical",
    }
}

fn confidence_name(confidence: Confidence) -> &'static str {
    match confidence {
        Confidence::Low => "low",
        Confidence::Medium => "medium",
        Confidence::High => "high",
    }
}

fn remediation_name(remediation: Option<Remediation>) -> &'static str {
    match remediation {
        None => "none",
        Some(Remediation::RevokeAndRotateCredential) => "revoke-and-rotate-credential",
        Some(Remediation::RotateCredential) => "rotate-credential",
        Some(Remediation::RotatePassword) => "rotate-password",
        Some(Remediation::ReplacePrivateKey) => "replace-private-key",
        Some(Remediation::RemoveSensitiveValue) => "remove-sensitive-value",
        Some(Remediation::ReviewSensitiveHash) => "review-sensitive-hash",
        Some(Remediation::ReviewPasswordVerifier) => "review-password-verifier",
        Some(_) => "unknown",
    }
}

fn candidate_kind_name(kind: SensitiveCandidateKind) -> &'static str {
    match kind {
        SensitiveCandidateKind::RecoveryLikeCode => "recovery-like-code",
        _ => "unknown",
    }
}

fn candidate_evidence_name(evidence: CandidateEvidence) -> &'static str {
    match evidence {
        CandidateEvidence::Structural => "structural",
        _ => "unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_report_has_stable_human_shape() {
        let scanner = cribra::Scanner::default();
        let results = scanner.scan([("clean.txt", "ordinary text")]);
        let report = results.single_report().expect("one report");

        assert_eq!(
            render_human("clean.txt", report),
            "\
source: clean.txt
status: clean
findings: 0
candidates: 0
"
        );
    }

    #[test]
    fn finding_output_contains_metadata_but_not_secret_value() {
        let scanner = cribra::Scanner::default();
        let secret = "ghp_AbCdEf0123456789_AbCdEf0123456789";
        let source = format!("GITHUB_TOKEN={secret}");
        let results = scanner.scan([("config.env", source.as_str())]);
        let report = results.single_report().expect("one report");

        let output = render_human("config.env", report);

        assert!(output.contains("status: findings"));
        assert!(output.contains("github"));
        assert!(output.contains("severity="));
        assert!(output.contains("confidence="));
        assert!(!output.contains(secret));
    }

    #[test]
    fn candidate_output_does_not_include_source_value() {
        let scanner = cribra::Scanner::default();
        let candidate = "ABCD-EFGH-IJKL-MNOP";
        let results = scanner.scan([("candidate.txt", candidate)]);
        let report = results.single_report().expect("one report");

        let output = render_human("candidate.txt", report);

        assert!(output.contains("status: review"));
        assert!(output.contains("candidate:"));
        assert!(!output.contains(candidate));
    }

    #[test]
    fn json_output_is_metadata_only() {
        let scanner = cribra::Scanner::default();
        let secret = "ghp_AbCdEf0123456789_AbCdEf0123456789";
        let source = format!("GITHUB_TOKEN={secret}");
        let results = scanner.scan([("config.env", source.as_str())]);
        let report = results.single_report().expect("one report");

        let output = render_json("config.env", report);

        assert!(output.starts_with('{'));
        assert!(output.ends_with("}\n"));
        assert!(output.contains("\"source\":\"config.env\""));
        assert!(output.contains("\"status\":\"findings\""));
        assert!(output.contains("\"rule_id\":"));
        assert!(output.contains("\"severity\":"));
        assert!(output.contains("\"confidence\":"));
        assert!(!output.contains(secret));
    }

    #[test]
    fn json_output_escapes_source_name() {
        let scanner = cribra::Scanner::default();
        let results = scanner.scan([("clean", "ordinary text")]);
        let report = results.single_report().expect("one report");

        let output = render_json("dir/\"quoted\"\\file", report);

        assert!(output.contains("\"source\":\"dir/\\\"quoted\\\"\\\\file\""));
    }

    #[test]
    fn output_format_is_explicit() {
        assert_eq!(OutputFormat::parse("human"), Some(OutputFormat::Human));
        assert_eq!(OutputFormat::parse("json"), Some(OutputFormat::Json));
        assert_eq!(OutputFormat::parse("yaml"), None);
    }
}
