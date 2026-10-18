#![cfg(feature = "fuse")]
#[path = "support/parallel.rs"]
#[allow(dead_code)]
mod support;
use cribra_uniffi::{Finding, ScanResults, Scanner};

#[test]
fn repeated_parallel_scans_match_sequential_and_core_for_every_observable() {
    for mode in support::MODES {
        let scanner = Scanner::configured(support::config(mode)).unwrap();
        let core = support::core(mode);
        for (name, sources) in support::batches() {
            let inputs = || sources.iter().map(|s| (&s.id, s.text.as_str()));
            let sequential = core.scan(inputs());
            let expected = core.parallel_scan(inputs());
            assert_eq!(expected, sequential);
            let expected_dto = ScanResults {
                reports: expected
                    .iter()
                    .map(|entry| cribra_uniffi::ScanReport {
                        source_id: (*entry.key()).clone(),
                        source_bytes: entry.source_bytes() as u64,
                        findings: entry
                            .report()
                            .findings()
                            .iter()
                            .map(Finding::from)
                            .collect(),
                        candidate_count: entry.report().candidate_len() as u64,
                        needs_review: entry.report().needs_review(),
                    })
                    .collect(),
                summary: expected.summary().into(),
            };
            for _ in 0..8 {
                assert_eq!(
                    scanner.parallel_scan(sources.clone()),
                    expected_dto,
                    "{mode}/{name}"
                );
                assert_eq!(
                    scanner.scan_many(sources.clone()),
                    expected_dto,
                    "{mode}/{name}"
                );
            }
            for (source, report) in sources.iter().zip(&expected_dto.reports) {
                assert_eq!(scanner.scan(source.clone()), *report);
            }
        }
    }
}
#[test]
fn mixed_batch_exercises_all_summary_counters_candidates_coordinates_and_ownership() {
    let scanner = Scanner::configured(support::config("builtInAndCustom")).unwrap();
    let sources = support::sources();
    let results = scanner.parallel_scan(sources.clone());
    let s = &results.summary;
    assert_eq!(
        (
            s.scanned_sources,
            s.reports_with_findings,
            s.reports_without_findings
        ),
        (7, 4, 3)
    );
    assert_eq!(
        s.scanned_bytes,
        sources.iter().map(|s| s.text.len() as u64).sum::<u64>()
    );
    assert_eq!(
        (
            s.total_findings,
            s.total_candidates,
            s.reports_with_candidates
        ),
        (11, 1, 1)
    );
    assert_eq!(
        (s.critical, s.high, s.medium, s.low, s.info),
        (4, 3, 1, 1, 2)
    );
    let candidate = &results.reports[3];
    assert!(
        candidate.findings.is_empty() && candidate.candidate_count == 1 && candidate.needs_review
    );
    assert_eq!(results.reports[0].source_id, results.reports[4].source_id);
    assert_ne!(
        results.reports[5].source_id.value.as_bytes(),
        results.reports[6].source_id.value.as_bytes()
    );
    let l = results.reports[0].findings[0].location;
    assert_eq!((l.start, l.end, l.line, l.column), (28, 65, 2, 19));
    let reversed = scanner.parallel_scan(sources.iter().rev().cloned().collect());
    assert_eq!(reversed.summary, results.summary);
    assert_eq!(
        reversed.reports,
        results.reports.iter().rev().cloned().collect::<Vec<_>>()
    );
    let empty = scanner.parallel_scan(vec![]);
    assert!(empty.reports.is_empty());
    assert_eq!(empty.summary, cribra::ScanSummary::default().into());
    drop(sources);
    drop(scanner);
    assert_eq!(
        results.reports[0].findings[0].rule_id.value,
        "github.classic-pat"
    );
}
