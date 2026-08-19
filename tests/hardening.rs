//! Battle-hardening tests over the canonical corpus.
//!
//! These tests exercise cross-feature invariants that are intentionally broader
//! than unit tests: repeatability, serial/parallel equivalence, transformation
//! stability, Unicode/CRLF handling, dense inputs, empty inputs and large lines.

use std::{
    fs,
    path::{Path, PathBuf},
};

use cribra::{
    Remediation, Rule, ScanSort, Scanner, Severity,
    transform::{
        PseudonymizationOptions, SynthesisOptions, pseudonymize, redact, synthesize, template,
    },
};

#[path = "../examples/fixtures/corpus.rs"]
mod corpus;

#[test]
fn canonical_corpus_is_repeatable_across_multiple_scans() {
    let scanner = corpus::scanner().expect("canonical scanner should build");
    let inputs = load_corpus_inputs();

    let first = scanner.scan(
        inputs
            .iter()
            .map(|(key, source)| (key.as_str(), source.as_str())),
    );
    let second = scanner.scan(
        inputs
            .iter()
            .map(|(key, source)| (key.as_str(), source.as_str())),
    );

    assert_eq!(first, second);
    assert_eq!(first.summary(), second.summary());
    assert_eq!(
        first.query().sort(ScanSort::Location).into_vec(),
        second.query().sort(ScanSort::Location).into_vec(),
    );
}

#[cfg(feature = "parallel")]
#[test]
fn canonical_corpus_serial_and_parallel_results_are_identical() {
    let scanner = corpus::scanner().expect("canonical scanner should build");
    let inputs = load_corpus_inputs();

    let serial = scanner.scan(
        inputs
            .iter()
            .map(|(key, source)| (key.as_str(), source.as_str())),
    );
    let parallel = scanner.parallel_scan(
        inputs
            .iter()
            .map(|(key, source)| (key.as_str(), source.as_str())),
    );

    assert_eq!(serial, parallel);
    assert_eq!(serial.summary(), parallel.summary());

    assert_eq!(
        serial
            .query()
            .minimum_severity(Severity::High)
            .sort(ScanSort::RuleId)
            .into_vec(),
        parallel
            .query()
            .minimum_severity(Severity::High)
            .sort(ScanSort::RuleId)
            .into_vec(),
    );
}

#[test]
fn canonical_transformations_are_repeatable_for_every_input() {
    let scanner = corpus::scanner().expect("canonical scanner should build");

    for (key, source) in load_corpus_inputs() {
        let results = scanner.scan([(key.as_str(), source.as_str())]);
        let report = results.single_report().expect("one source");

        assert_eq!(
            redact(&source, report).unwrap(),
            redact(&source, report).unwrap(),
            "redact repeatability failed for {key}",
        );
        assert_eq!(
            template(&source, report).unwrap(),
            template(&source, report).unwrap(),
            "template repeatability failed for {key}",
        );

        let pseudonymization = PseudonymizationOptions::new(corpus::PSEUDONYMIZATION_KEY);
        assert_eq!(
            pseudonymize(&source, report, &pseudonymization).unwrap(),
            pseudonymize(&source, report, &pseudonymization).unwrap(),
            "pseudonymization repeatability failed for {key}",
        );

        let synthesis = SynthesisOptions::new(corpus::SYNTHESIS_KEY);
        assert_eq!(
            synthesize(&source, report, &synthesis).unwrap(),
            synthesize(&source, report, &synthesis).unwrap(),
            "synthesis repeatability failed for {key}",
        );
    }
}

#[test]
fn transformation_output_never_reintroduces_detected_values() {
    let scanner = corpus::scanner().expect("canonical scanner should build");

    for (key, source) in load_corpus_inputs() {
        let results = scanner.scan([(key.as_str(), source.as_str())]);
        let report = results.single_report().expect("one source");

        let detected_values = report
            .findings()
            .iter()
            .map(|finding| {
                let location = finding.location();
                source[location.start()..location.end()].to_owned()
            })
            .collect::<Vec<_>>();

        for output in [
            redact(&source, report).unwrap(),
            template(&source, report).unwrap(),
            pseudonymize(
                &source,
                report,
                &PseudonymizationOptions::new(corpus::PSEUDONYMIZATION_KEY),
            )
            .unwrap(),
            synthesize(
                &source,
                report,
                &SynthesisOptions::new(corpus::SYNTHESIS_KEY),
            )
            .unwrap(),
        ] {
            for detected in &detected_values {
                assert!(
                    !output.contains(detected),
                    "{key} retained detected value {detected:?}",
                );
            }
        }
    }
}

#[test]
fn empty_input_is_clean_and_all_transformations_are_identity() {
    let scanner = corpus::scanner().expect("canonical scanner should build");
    let source = "";
    let results = scanner.scan([("empty", source)]);
    let report = results.single_report().expect("one source");

    assert!(report.is_empty());
    assert_eq!(redact(source, report).unwrap(), source);
    assert_eq!(template(source, report).unwrap(), source);
    assert_eq!(
        pseudonymize(
            source,
            report,
            &PseudonymizationOptions::new(corpus::PSEUDONYMIZATION_KEY),
        )
        .unwrap(),
        source,
    );
    assert_eq!(
        synthesize(
            source,
            report,
            &SynthesisOptions::new(corpus::SYNTHESIS_KEY),
        )
        .unwrap(),
        source,
    );
}

#[test]
fn huge_single_line_preserves_locations_and_query_semantics() {
    let scanner = Scanner::builder()
        .rule(
            Rule::literal("huge.secret", "DEMO_SECRET_ALPHA", Severity::Critical)
                .with_remediation(Remediation::RemoveSensitiveValue),
        )
        .build()
        .unwrap();

    let mut source = "x".repeat(1_000_000);
    source.push_str("DEMO_SECRET_ALPHA");
    source.push_str(&"y".repeat(1_000_000));

    let results = scanner.scan([("huge", source.as_str())]);
    let report = results.single_report().unwrap();

    assert_eq!(report.len(), 1);

    let finding = &report.findings()[0];
    assert_eq!(finding.location().line(), 1);
    assert_eq!(finding.location().column(), 1_000_001);
    assert_eq!(
        &source[finding.location().start()..finding.location().end()],
        "DEMO_SECRET_ALPHA",
    );

    let query = results
        .query()
        .minimum_severity(Severity::High)
        .rule_id("huge.secret");

    assert_eq!(query.count(), 1);
    assert!(query.has_matches());
}

#[test]
fn many_repeated_findings_remain_complete_and_deterministic() {
    let scanner = Scanner::builder()
        .rule(
            Rule::literal("repeat.secret", "DEMO_SECRET_ALPHA", Severity::High)
                .with_remediation(Remediation::RemoveSensitiveValue),
        )
        .build()
        .unwrap();

    let source = (0..5_000)
        .map(|_| "DEMO_SECRET_ALPHA")
        .collect::<Vec<_>>()
        .join(" ");

    let first = scanner.scan([("dense", source.as_str())]);
    let second = scanner.scan([("dense", source.as_str())]);

    assert_eq!(first.total_findings(), 5_000);
    assert_eq!(first, second);

    let sorted = first.query().sort(ScanSort::Location);
    assert_eq!(sorted.len(), 5_000);
    assert_eq!(sorted.first().unwrap().1.location().start(), 0);
    assert!(sorted.last().unwrap().1.location().start() > 0);
}

#[test]
fn utf8_prefix_before_secret_preserves_scalar_column_contract() {
    let scanner = Scanner::builder()
        .rule(
            Rule::literal("unicode.secret", "DEMO_SECRET_ALPHA", Severity::High)
                .with_remediation(Remediation::RemoveSensitiveValue),
        )
        .build()
        .unwrap();

    let source = "😀é漢 DEMO_SECRET_ALPHA";
    let results = scanner.scan([("unicode", source)]);
    let finding = &results.single_report().unwrap().findings()[0];

    assert_eq!(finding.location().line(), 1);
    assert_eq!(finding.location().column(), 5);
    assert_eq!(
        &source[finding.location().start()..finding.location().end()],
        "DEMO_SECRET_ALPHA",
    );
}

#[test]
fn crlf_positions_and_transforms_remain_stable() {
    let scanner = Scanner::builder()
        .rule(
            Rule::literal("crlf.secret", "DEMO_SECRET_ALPHA", Severity::High)
                .with_remediation(Remediation::RemoveSensitiveValue),
        )
        .build()
        .unwrap();

    let source = "FIRST=ok\r\nSECOND=DEMO_SECRET_ALPHA\r\nTHIRD=ok\r\n";
    let results = scanner.scan([("crlf", source)]);
    let report = results.single_report().unwrap();
    let finding = &report.findings()[0];

    assert_eq!(finding.location().line(), 2);
    assert_eq!(finding.location().column(), 8);

    for output in [
        redact(source, report).unwrap(),
        template(source, report).unwrap(),
    ] {
        assert!(output.contains("\r\n"));
        assert!(!output.replace("\r\n", "").contains('\n'));
    }
}

#[cfg(feature = "serde")]
#[test]
fn canonical_batch_round_trips_through_json_without_behavioral_loss() {
    let scanner = corpus::scanner().expect("canonical scanner should build");
    let inputs = load_corpus_inputs();

    let original = scanner.scan(
        inputs
            .iter()
            .map(|(key, source)| (key.clone(), source.as_str())),
    );

    let json = serde_json::to_string(&original).unwrap();
    let decoded: cribra::ScanResults<String> = serde_json::from_str(&json).unwrap();

    assert_eq!(decoded, original);
    assert_eq!(decoded.summary(), original.summary());
}

fn load_corpus_inputs() -> Vec<(String, String)> {
    let inputs = fixture_root().join("inputs");
    let mut paths = recursive_files(&inputs).expect("canonical inputs must be readable");
    paths.sort();

    paths
        .into_iter()
        .map(|path| {
            let relative = path
                .strip_prefix(&inputs)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            let source = fs::read_to_string(&path).unwrap();
            (relative, source)
        })
        .collect()
}

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/fixtures")
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
