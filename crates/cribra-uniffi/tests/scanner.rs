#![cfg(feature = "core")]

mod support;

use cribra_uniffi::{Finding, ScanSource, Scanner, SourceId};

#[test]
fn reusable_scanner_preserves_core_reports_and_summary() {
    let inputs = support::sources();
    let scanner = Scanner::new();
    let core = cribra::Scanner::default();
    let expected = core.scan(
        inputs
            .iter()
            .map(|source| (&source.id, source.text.as_str())),
    );
    let batch = scanner.scan_many(inputs.clone());
    assert_eq!(batch.reports.len(), inputs.len());
    assert_eq!(batch.summary, expected.summary().into());
    for ((actual, expected), source) in batch.reports.iter().zip(expected.iter()).zip(&inputs) {
        assert_eq!(&actual.source_id, *expected.key());
        assert_eq!(actual.source_bytes, expected.source_bytes() as u64);
        assert_eq!(
            actual.findings,
            expected
                .report()
                .findings()
                .iter()
                .map(Finding::from)
                .collect::<Vec<_>>()
        );
        assert_eq!(
            actual.candidate_count,
            expected.report().candidate_len() as u64
        );
        assert_eq!(actual.needs_review, expected.report().needs_review());
        assert_eq!(*actual, scanner.scan(source.clone()));
    }
    assert_eq!(batch, scanner.scan_many(inputs.clone()));
    let reversed = scanner.scan_many(inputs.into_iter().rev().collect());
    assert_eq!(
        batch.reports.iter().rev().collect::<Vec<_>>(),
        reversed.reports.iter().collect::<Vec<_>>()
    );
    assert_eq!(batch.summary, reversed.summary);

    let summary = &batch.summary;
    assert_eq!(summary.scanned_sources, 5);
    assert_eq!(
        summary.scanned_bytes,
        support::sources()
            .iter()
            .map(|s| s.text.len() as u64)
            .sum::<u64>()
    );
    assert_eq!(summary.reports_with_findings, 2);
    assert_eq!(summary.reports_without_findings, 3);
    assert_eq!(summary.total_findings, 3);
    assert_eq!(summary.total_candidates, 1);
    assert_eq!(summary.reports_with_candidates, 1);
    assert_eq!(summary.critical, 2);
    assert_eq!(
        (summary.high, summary.medium, summary.low, summary.info),
        (1, 0, 0, 0)
    );
    assert!(batch.reports[3].findings.is_empty());
    assert_eq!(batch.reports[3].candidate_count, 1);
    assert!(batch.reports[3].needs_review);
    assert_eq!(batch.reports[0].source_id, batch.reports[4].source_id);
    let location = batch.reports[0].findings[0].location;
    assert_eq!(
        (location.start, location.end, location.line, location.column),
        (28, 65, 2, 19)
    );
}

#[test]
fn empty_batches_and_sources_do_not_retain_previous_results() {
    let scanner = Scanner::new();
    let previous = scanner.scan_many(support::sources());
    let empty = scanner.scan_many(Vec::new());
    assert!(empty.reports.is_empty());
    assert_eq!(empty.summary, cribra::ScanSummary::default().into());
    for text in ["", "ordinary text: é🙂"] {
        let report = scanner.scan(ScanSource {
            id: SourceId {
                value: String::new(),
            },
            text: text.to_owned(),
        });
        assert_eq!(report.source_id.value, "");
        assert_eq!(report.source_bytes, text.len() as u64);
        assert!(report.findings.is_empty());
        assert_eq!(report.candidate_count, 0);
        assert!(!report.needs_review);
    }
    // DTO results remain usable after the UniFFI-owned scanner is released.
    drop(scanner);
    assert_eq!(previous.summary.total_findings, 3);
    assert_eq!(
        previous.reports[0].findings[0].rule_id.value,
        "github.classic-pat"
    );
}
