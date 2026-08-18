//! Custom-rule example for application/domain knowledge.

use silens_scan::{DetectionMode, Explanation, Remediation, Rule, Scanner, Severity, builtins};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let scanner = Scanner::builder()
        .builtins(builtins::CURRENT)
        .rule(
            Rule::prefix("acme.service-token", "acme_live_", Severity::Critical)
                .with_remediation(Remediation::RotateCredential),
        )
        .rule(
            Rule::pattern(
                "acme.recovery-code",
                r"\b[A-Z2-9]{4}(?:-[A-Z2-9]{4}){3}\b",
                Severity::High,
            )?
            .with_remediation(Remediation::RotateCredential),
        )
        .build()?;

    let source = "\
SERVICE_TOKEN=acme_live_1234567890
RECOVERY=ABCD-EFGH-IJKL-MNOP
";

    let results = scanner.scan([("config.env", source)]);
    let report = results.single_report().expect("one source was scanned");

    for finding in report.findings() {
        let explanation = finding
            .explanation(&scanner)
            .expect("finding should resolve against producing scanner");

        assert_eq!(
            explanation,
            Explanation::Classified(DetectionMode::MatcherOnly)
        );

        println!(
            "{}:{} {} {:?}",
            finding.location().line(),
            finding.location().column(),
            finding.rule_id(),
            finding.remediation(),
        );
    }

    assert_eq!(report.candidate_len(), 0);

    Ok(())
}
