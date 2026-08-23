use std::{
    fs,
    path::{Path, PathBuf},
};

use cribra::{
    CandidateEvidence, Confidence, DetectionMode, Explanation, Remediation, Rule, Scanner,
    SensitiveCandidateKind, Severity,
    transform::{
        PseudonymizationOptions, SynthesisOptions, pseudonymize, redact, synthesize, template,
    },
};
use serde::Serialize;

const PSEUDONYMIZATION_KEY: [u8; 32] = [0x31; 32];
const SYNTHESIS_KEY: [u8; 32] = [0x53; 32];

#[derive(Serialize)]
struct Oracle {
    schema: u32,
    cases: Vec<CaseOracle>,
}

#[derive(Serialize)]
struct CaseOracle {
    name: String,
    scanner: ScannerKind,
    source: String,
    source_bytes: usize,
    finding_count: usize,
    candidate_count: usize,
    needs_review: bool,
    has_critical: bool,
    findings: Vec<FindingOracle>,
    candidates: Vec<CandidateOracle>,
    transforms: TransformOracle,
}

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
enum ScannerKind {
    CanonicalCustom,
    DefaultBuiltins,
}

#[derive(Serialize)]
struct FindingOracle {
    rule_id: String,
    start: usize,
    end: usize,
    line: usize,
    column: usize,
    severity: String,
    confidence: String,
    remediation: Option<String>,
    explanation: ExplanationOracle,
}

#[derive(Serialize)]
struct CandidateOracle {
    start: usize,
    end: usize,
    line: usize,
    column: usize,
    kind: String,
    evidence: String,
    explanation: ExplanationOracle,
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum ExplanationOracle {
    Classified { detection_mode: String },
    Ambiguous { evidence: String },
    Unknown,
}

#[derive(Serialize)]
struct TransformOracle {
    redacted: String,
    template: String,
    pseudonymized: String,
    synthesized: String,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let repo_root = repo_root();
    let fixture_root = repo_root.join("examples/fixtures/inputs");

    let canonical_scanner = canonical_equivalent_scanner()?;
    let default_scanner = Scanner::default();
    let mut cases = Vec::new();

    for path in sorted_files(&fixture_root)? {
        let relative = path
            .strip_prefix(&fixture_root)?
            .to_string_lossy()
            .replace('\\', "/");
        let source = fs::read_to_string(&path)?;

        cases.push(case_oracle(
            format!("fixture:{relative}"),
            ScannerKind::CanonicalCustom,
            &canonical_scanner,
            source,
        )?);
    }

    cases.push(case_oracle(
        "unicode-custom".to_owned(),
        ScannerKind::CanonicalCustom,
        &canonical_scanner,
        "π😀 prefix\nvalue=DEMO_SECRET_ALPHA suffix".to_owned(),
    )?);

    cases.push(case_oracle(
        "candidate-only".to_owned(),
        ScannerKind::DefaultBuiltins,
        &default_scanner,
        "ABCD-EFGH-IJKL-MNOP".to_owned(),
    )?);

    cases.push(case_oracle(
        "builtin-remediation".to_owned(),
        ScannerKind::DefaultBuiltins,
        &default_scanner,
        "GITHUB_TOKEN=ghp_AbCdEf0123456789_AbCdEf0123456789".to_owned(),
    )?);

    let oracle = Oracle { schema: 1, cases };
    let output_dir = repo_root.join("target/wasm-parity");
    fs::create_dir_all(&output_dir)?;
    let output = output_dir.join("oracle.json");
    fs::write(&output, serde_json::to_string_pretty(&oracle)? + "\n")?;

    println!("{}", output.display());
    Ok(())
}

fn canonical_equivalent_scanner() -> Result<Scanner, cribra::ScannerBuildError> {
    Scanner::builder()
        .rule(Rule::prefix(
            "demo.api-key",
            "demo_api_",
            Severity::Critical,
        ))
        .rule(
            Rule::pattern(
                "demo.password",
                r"demo-pass-[A-Za-z0-9_\-\p{L}]+",
                Severity::High,
            )
            .expect("canonical parity password pattern must compile"),
        )
        .rule(Rule::literal(
            "demo.private-key",
            "DEMO_PRIVATE_KEY_MATERIAL",
            Severity::Critical,
        ))
        .rule(Rule::literal(
            "demo.secret",
            "DEMO_SECRET_ALPHA",
            Severity::High,
        ))
        .build()
}

fn case_oracle(
    name: String,
    scanner_kind: ScannerKind,
    scanner: &Scanner,
    source: String,
) -> Result<CaseOracle, Box<dyn std::error::Error>> {
    let results = scanner.scan([("source", source.as_str())]);
    let report = results.single_report().expect("one parity source");

    let findings = report
        .findings()
        .iter()
        .map(|finding| {
            let location = finding.location();
            FindingOracle {
                rule_id: finding.rule_id().as_str().to_owned(),
                start: location.start(),
                end: location.end(),
                line: location.line(),
                column: location.column(),
                severity: severity_name(finding.severity()),
                confidence: confidence_name(finding.confidence()),
                remediation: finding.remediation().map(remediation_name),
                explanation: finding
                    .explanation(scanner)
                    .map(explanation_oracle)
                    .unwrap_or(ExplanationOracle::Unknown),
            }
        })
        .collect();

    let candidates = report
        .candidates()
        .iter()
        .map(|candidate| {
            let location = candidate.location();
            CandidateOracle {
                start: location.start(),
                end: location.end(),
                line: location.line(),
                column: location.column(),
                kind: candidate_kind_name(candidate.kind()),
                evidence: candidate_evidence_name(candidate.evidence()),
                explanation: explanation_oracle(candidate.explanation()),
            }
        })
        .collect();

    let transforms = TransformOracle {
        redacted: redact(&source, report)?,
        template: template(&source, report)?,
        pseudonymized: pseudonymize(
            &source,
            report,
            &PseudonymizationOptions::new(PSEUDONYMIZATION_KEY),
        )?,
        synthesized: synthesize(&source, report, &SynthesisOptions::new(SYNTHESIS_KEY))?,
    };

    Ok(CaseOracle {
        name,
        scanner: scanner_kind,
        source_bytes: source.len(),
        finding_count: report.len(),
        candidate_count: report.candidate_len(),
        needs_review: report.needs_review(),
        has_critical: report.has_critical(),
        findings,
        candidates,
        transforms,
        source,
    })
}

fn explanation_oracle(explanation: Explanation) -> ExplanationOracle {
    match explanation {
        Explanation::Classified(mode) => ExplanationOracle::Classified {
            detection_mode: detection_mode_name(mode),
        },
        Explanation::Ambiguous(evidence) => ExplanationOracle::Ambiguous {
            evidence: candidate_evidence_name(evidence),
        },
        _ => ExplanationOracle::Unknown,
    }
}

fn severity_name(value: Severity) -> String {
    match value {
        Severity::Info => "Info",
        Severity::Low => "Low",
        Severity::Medium => "Medium",
        Severity::High => "High",
        Severity::Critical => "Critical",
    }
    .to_owned()
}

fn confidence_name(value: Confidence) -> String {
    match value {
        Confidence::Low => "Low",
        Confidence::Medium => "Medium",
        Confidence::High => "High",
    }
    .to_owned()
}

fn remediation_name(value: Remediation) -> String {
    match value {
        Remediation::RevokeAndRotateCredential => "RevokeAndRotateCredential",
        Remediation::RotateCredential => "RotateCredential",
        Remediation::RotatePassword => "RotatePassword",
        Remediation::ReplacePrivateKey => "ReplacePrivateKey",
        Remediation::RemoveSensitiveValue => "RemoveSensitiveValue",
        Remediation::ReviewSensitiveHash => "ReviewSensitiveHash",
        _ => "Unknown",
    }
    .to_owned()
}

fn candidate_kind_name(value: SensitiveCandidateKind) -> String {
    match value {
        SensitiveCandidateKind::RecoveryLikeCode => "RecoveryLikeCode",
        _ => "Unknown",
    }
    .to_owned()
}

fn candidate_evidence_name(value: CandidateEvidence) -> String {
    match value {
        CandidateEvidence::Structural => "Structural",
        _ => "Unknown",
    }
    .to_owned()
}

fn detection_mode_name(value: DetectionMode) -> String {
    match value {
        DetectionMode::MatcherOnly => "MatcherOnly",
        DetectionMode::Deterministic => "Deterministic",
        DetectionMode::Contextual => "Contextual",
    }
    .to_owned()
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("cribra-wasm must live under crates/")
        .to_path_buf()
}

fn sorted_files(root: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut files = recursive_files(root)?;
    files.sort();
    Ok(files)
}

fn recursive_files(root: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();

    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();

        if entry.file_type()?.is_dir() {
            files.extend(recursive_files(&path)?);
        } else if entry.file_type()?.is_file() {
            files.push(path);
        }
    }

    Ok(files)
}
