//! Cross-format and boundary regression contract for the Cribra v0.2 corpus.
//!
//! Cribra is application-agnostic: callers provide UTF-8 text rather than
//! parsed ENV/JSON/YAML/TOML documents. Equivalent sensitive content should
//! therefore preserve the same observable classification semantics across
//! common textual representations and line-ending styles.

use std::collections::BTreeSet;

use cribra::Scanner;

const ENV: &str = include_str!("fixtures/corpus_v02/formats/sample.env");
const JSON: &str = include_str!("fixtures/corpus_v02/formats/sample.json");
const YAML: &str = include_str!("fixtures/corpus_v02/formats/sample.yaml");
const TOML: &str = include_str!("fixtures/corpus_v02/formats/sample.toml");
const TEXT: &str = include_str!("fixtures/corpus_v02/formats/sample.txt");
const CRLF: &str = include_str!("fixtures/corpus_v02/formats/sample-crlf.env");

const GITHUB_RULE: &str = "github.classic-pat";
const API_RULE: &str = "generic.api-key";
const TOKEN: &str = "ghp_AbCdEf0123456789_AbCdEf0123456789";
const CANDIDATE: &str = "QRST-UVWX-YZ12-3456";

fn ids(report: &cribra::ScanReport) -> BTreeSet<&str> {
    report
        .findings()
        .iter()
        .map(|finding| finding.rule_id().as_str())
        .collect()
}

fn assert_equivalent_semantics(name: &str, source: &str) {
    let scanner = Scanner::default();
    let results = scanner.scan([(name, source)]);
    let report = results.single_report().expect("one source was scanned");
    let found = ids(report);

    assert!(
        found.contains(GITHUB_RULE),
        "{name} missed deterministic GitHub detection; found {found:?}"
    );
    assert!(
        found.contains(API_RULE),
        "{name} missed contextual API-key detection; found {found:?}"
    );
    assert_eq!(
        report.candidate_len(),
        1,
        "{name} must retain exactly one ambiguous recovery-like candidate"
    );
    assert_eq!(
        &source[report.candidates()[0].location().byte_range()],
        CANDIDATE
    );
}

#[test]
fn equivalent_sensitive_content_is_format_agnostic() {
    for (name, source) in [
        ("env", ENV),
        ("json", JSON),
        ("yaml", YAML),
        ("toml", TOML),
        ("text", TEXT),
    ] {
        assert_equivalent_semantics(name, source);
    }
}

#[test]
fn crlf_and_lf_preserve_equivalent_detection_semantics() {
    assert!(CRLF.contains("\r\n"), "fixture must actually contain CRLF");

    assert_equivalent_semantics("lf", ENV);
    assert_equivalent_semantics("crlf", CRLF);

    let scanner = Scanner::default();
    let lf = scanner.scan([("lf", ENV)]);
    let crlf = scanner.scan([("crlf", CRLF)]);

    let lf_report = lf.single_report().expect("one LF source");
    let crlf_report = crlf.single_report().expect("one CRLF source");

    assert_eq!(ids(lf_report), ids(crlf_report));
    assert_eq!(lf_report.candidate_len(), crlf_report.candidate_len());
}

#[test]
fn deterministic_secret_is_detected_at_start_and_end_of_input() {
    let scanner = Scanner::default();

    for source in [
        TOKEN.to_owned(),
        format!("{TOKEN}\nordinary text"),
        format!("ordinary text\n{TOKEN}"),
    ] {
        let results = scanner.scan([("boundary", source.as_str())]);
        let report = results.single_report().expect("one source was scanned");

        let finding = report
            .findings()
            .iter()
            .find(|finding| finding.rule_id().as_str() == GITHUB_RULE)
            .expect("GitHub token should be detected at source boundary");

        assert_eq!(
            &source[finding.location().byte_range()],
            TOKEN,
            "boundary detection must preserve the exact token span"
        );
    }
}

#[test]
fn unicode_surrounding_text_does_not_change_detection_or_candidate_spans() {
    let source = format!("秘密😀::{TOKEN}::αβγ::{CANDIDATE}::данные");
    let scanner = Scanner::default();
    let results = scanner.scan([("unicode", source.as_str())]);
    let report = results.single_report().expect("one source was scanned");

    let finding = report
        .findings()
        .iter()
        .find(|finding| finding.rule_id().as_str() == GITHUB_RULE)
        .expect("GitHub token should be detected");

    assert_eq!(&source[finding.location().byte_range()], TOKEN);
    assert_eq!(report.candidate_len(), 1);
    assert_eq!(
        &source[report.candidates()[0].location().byte_range()],
        CANDIDATE
    );
}

#[test]
fn adjacent_classified_values_remain_independently_observable() {
    let source = format!("{TOKEN},{TOKEN}");
    let scanner = Scanner::default();
    let results = scanner.scan([("adjacent", source.as_str())]);
    let report = results.single_report().expect("one source was scanned");

    let github = report
        .findings()
        .iter()
        .filter(|finding| finding.rule_id().as_str() == GITHUB_RULE)
        .collect::<Vec<_>>();

    assert_eq!(github.len(), 2);

    for finding in github {
        assert_eq!(&source[finding.location().byte_range()], TOKEN);
    }
}

#[test]
fn multiple_sources_keep_input_order_and_per_source_semantics() {
    let scanner = Scanner::default();
    let results = scanner.scan([
        ("env", ENV),
        ("json", JSON),
        ("yaml", YAML),
        ("toml", TOML),
        ("text", TEXT),
        ("crlf", CRLF),
    ]);

    let keys = results.iter().map(|entry| *entry.key()).collect::<Vec<_>>();

    assert_eq!(keys, ["env", "json", "yaml", "toml", "text", "crlf"]);

    for entry in results.iter() {
        let found = ids(entry.report());
        assert!(found.contains(GITHUB_RULE));
        assert!(found.contains(API_RULE));
        assert_eq!(entry.report().candidate_len(), 1);
    }
}
