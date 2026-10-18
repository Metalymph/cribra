#![cfg(feature = "core")]

#[path = "support/configuration.rs"]
mod configuration;
#[path = "support/incremental.rs"]
mod support;

use cribra_uniffi::{
    Finding, IncrementalError, IncrementalLimits, IncrementalState, RuleId, RuleKind, RuleSpec,
    ScanSource, Scanner, ScannerConfig, SourceId,
};
use serde_json::json;

fn source(text: &str) -> ScanSource {
    ScanSource {
        id: SourceId {
            value: "document/文書é🙂".into(),
        },
        text: text.into(),
    }
}
fn literal_config() -> ScannerConfig {
    ScannerConfig::CustomOnly {
        rules: vec![RuleSpec {
            id: RuleId {
                value: "literal".into(),
            },
            kind: RuleKind::Literal,
            value: "x".into(),
            severity: cribra_uniffi::Severity::High,
            remediation: None,
        }],
    }
}
fn error_json(error: IncrementalError) -> serde_json::Value {
    let IncrementalError::Rejected {
        code,
        terminal,
        expected_revision,
        actual_revision,
        limit,
        required,
        detail,
    } = error;
    json!({"code":format!("{code:?}"),"terminal":terminal,"expectedRevision":expected_revision,
        "actualRevision":actual_revision,"limit":limit,"required":required,"detail":detail})
}

#[test]
fn unicode_edits_noops_and_recoverable_errors_match_core_and_whole_source() {
    let fixture = support::fixture();
    let case = configuration::cases()
        .into_iter()
        .find(|c| c.name == "custom")
        .unwrap();
    let config = case.config;
    let core_scanner = configuration::core_build(&config).unwrap();
    let scanner = Scanner::configured(config).unwrap();
    assert_eq!(
        scanner
            .incremental(source(&case.source))
            .unwrap()
            .report()
            .unwrap(),
        scanner.scan(source(&case.source))
    );
    let initial = fixture["source"].as_str().unwrap();
    let mut core = core_scanner.incremental_session(initial.into()).unwrap();
    let session = scanner.incremental(source(initial)).unwrap();
    let mut successes = 0;
    let mut failures = 0;
    for edit in fixture["edits"].as_array().unwrap() {
        let revision = edit["revision"].as_u64().unwrap();
        let start = edit["start"].as_u64().unwrap();
        let end = edit["end"].as_u64().unwrap();
        let replacement = edit["replacement"].as_str().unwrap();
        let before = (session.revision().unwrap(), session.report().unwrap());
        let expected = core.apply_edit(revision, start as usize..end as usize, replacement);
        let actual = session.apply_edit(revision, start, end, replacement.into());
        match (expected, actual) {
            (Ok(expected), Ok(actual)) => {
                assert_eq!(actual, expected);
                successes += 1;
            }
            (Err(expected), Err(actual)) => {
                failures += 1;
                assert_eq!(
                    error_json(actual),
                    support::core_error(expected, !core.is_active())
                );
                assert_eq!(
                    (session.revision().unwrap(), session.report().unwrap()),
                    before
                );
            }
            _ => panic!("Core/adapter outcome mismatch"),
        }
        assert_eq!(session.revision().unwrap(), core.revision());
        let snapshot = session.snapshot().unwrap();
        assert_eq!(snapshot.revision, core.revision());
        assert_eq!(snapshot.report, session.report().unwrap());
        assert_eq!(session.state(), IncrementalState::Active);
        let report = session.report().unwrap();
        assert_eq!(report.source_bytes, core.source().len() as u64);
        assert_eq!(
            report.findings,
            core.report()
                .findings()
                .iter()
                .map(Finding::from)
                .collect::<Vec<_>>()
        );
        assert_eq!(report.candidate_count, core.report().candidate_len() as u64);
        assert_eq!(report, scanner.scan(source(core.source())));
    }
    assert_eq!((successes, failures), (7, 4));
    assert_eq!(session.revision().unwrap(), 7);
    assert_eq!(session.report().unwrap().findings.len(), 4);
}

#[test]
fn all_resource_failures_preserve_last_snapshot_and_block_later_edits() {
    for (limits, core_limits, initial, replacement) in [
        (
            IncrementalLimits {
                max_candidates: Some(1),
                ..Default::default()
            },
            cribra::IncrementalLimits::default().with_max_candidates(Some(1)),
            "x",
            "xx",
        ),
        (
            IncrementalLimits {
                max_retained_cache_records: Some(1),
                ..Default::default()
            },
            cribra::IncrementalLimits::default().with_max_retained_cache_records(Some(1)),
            "x",
            "",
        ),
        (
            IncrementalLimits {
                max_materialized_results: Some(1),
                ..Default::default()
            },
            cribra::IncrementalLimits::default().with_max_materialized_results(Some(1)),
            "x",
            "x",
        ),
        (
            IncrementalLimits {
                max_transaction_storage_bytes: Some(0),
                ..Default::default()
            },
            cribra::IncrementalLimits::default().with_max_transaction_storage_bytes(Some(0)),
            "",
            "a",
        ),
    ] {
        let config = if initial.is_empty() {
            ScannerConfig::CustomOnly { rules: vec![] }
        } else {
            literal_config()
        };
        let core_scanner = configuration::core_build(&config).unwrap();
        let scanner = Scanner::configured(config).unwrap();
        let mut core = core_scanner
            .incremental_session_with_limits(initial.into(), core_limits)
            .unwrap();
        let session = scanner
            .incremental_with_limits(source(initial), limits)
            .unwrap();
        if initial.is_empty() {
            assert_eq!(core.apply_edit(0, 0..0, ""), Ok(1));
            assert_eq!(session.apply_edit(0, 0, 0, "".into()), Ok(1));
        }
        let revision = core.revision();
        let before = session.report().unwrap();
        let before_snapshot = session.snapshot().unwrap();
        let error = core.apply_edit(revision, 0..0, replacement).unwrap_err();
        let actual = session
            .apply_edit(revision, 0, 0, replacement.into())
            .unwrap_err();
        assert_eq!(
            error_json(actual),
            support::core_error(error, !core.is_active())
        );
        assert!(!core.is_active());
        assert_eq!(session.state(), IncrementalState::Failed);
        assert_eq!(session.revision().unwrap(), revision);
        assert_eq!(session.report().unwrap(), before);
        assert_eq!(session.snapshot().unwrap(), before_snapshot);
        // NotActive takes precedence over stale revision and an invalid range.
        let expected = core.apply_edit(u64::MAX, 999..0, "").unwrap_err();
        let actual = session.apply_edit(u64::MAX, 999, 0, "".into()).unwrap_err();
        assert_eq!(error_json(actual), support::core_error(expected, true));
        assert_eq!(session.report().unwrap(), before);
        assert_eq!(session.snapshot().unwrap(), before_snapshot);
    }
}

#[test]
fn construction_limits_match_core_without_creating_partial_objects() {
    for (limits, core_limits) in [
        (
            IncrementalLimits {
                max_candidates: Some(0),
                ..Default::default()
            },
            cribra::IncrementalLimits::default().with_max_candidates(Some(0)),
        ),
        (
            IncrementalLimits {
                max_retained_cache_records: Some(0),
                ..Default::default()
            },
            cribra::IncrementalLimits::default().with_max_retained_cache_records(Some(0)),
        ),
        (
            IncrementalLimits {
                max_materialized_results: Some(0),
                ..Default::default()
            },
            cribra::IncrementalLimits::default().with_max_materialized_results(Some(0)),
        ),
        (
            IncrementalLimits {
                max_transaction_storage_bytes: Some(0),
                ..Default::default()
            },
            cribra::IncrementalLimits::default().with_max_transaction_storage_bytes(Some(0)),
        ),
    ] {
        let config = literal_config();
        let core = configuration::core_build(&config).unwrap();
        let scanner = Scanner::configured(config).unwrap();
        let expected = core
            .incremental_session_with_limits("x".into(), core_limits)
            .unwrap_err();
        let actual = scanner
            .incremental_with_limits(source("x"), limits)
            .err()
            .unwrap();
        assert_eq!(error_json(actual), support::core_error(expected, true));
    }
}

#[test]
fn concurrency_revision_conflict_and_session_outliving_scanner() {
    let scanner = Scanner::configured(literal_config()).unwrap();
    let session = scanner.incremental(source("")).unwrap();
    drop(scanner);
    let other = session.clone();
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
    let other_barrier = barrier.clone();
    let thread = std::thread::spawn(move || {
        other_barrier.wait();
        other.apply_edit(0, 0, 0, "x".into())
    });
    barrier.wait();
    let first = session.apply_edit(0, 0, 0, "xx".into());
    let second = thread.join().unwrap();
    assert_eq!(usize::from(first.is_ok()) + usize::from(second.is_ok()), 1);
    let error = first.err().or_else(|| second.err()).unwrap();
    assert_eq!(
        error_json(error),
        support::core_error(
            cribra::IncrementalError::StaleRevision {
                expected: 0,
                actual: 1
            },
            false
        )
    );
    assert_eq!(session.state(), IncrementalState::Active);
    assert_eq!(session.revision().unwrap(), 1);
    let report = session.report().unwrap();
    assert!(report.findings.len() == 1 || report.findings.len() == 2);
    drop(session);
    assert!(!report.findings.is_empty());
}

#[test]
fn default_rules_incremental_remains_core_owned() {
    let scanner = Scanner::new();
    let session = scanner.incremental(source("ordinary text")).unwrap();
    let token = "GITHUB_TOKEN=ghp_AbCdEf0123456789_AbCdEf0123456789";
    assert_eq!(session.apply_edit(0, 0, 13, token.into()), Ok(1));
    assert_eq!(session.report().unwrap(), scanner.scan(source(token)));
}

#[test]
fn atomic_snapshots_stay_coherent_while_edits_and_noops_race_with_readers() {
    use std::sync::{Arc, Barrier};
    const PREFIX: &str = "é🙂e\u{301}\r\n";
    const CHUNK: &str = "x😀e\u{301}\r\n";
    const STEPS: u64 = 128;
    let scanner = Scanner::configured(literal_config()).unwrap();
    let session = scanner.incremental(source(PREFIX)).unwrap();
    drop(scanner);
    let gate = Arc::new(Barrier::new(5));
    std::thread::scope(|threads| {
        for _ in 0..4 {
            let session = session.clone();
            let gate = gate.clone();
            threads.spawn(move || {
                for step in 0..STEPS {
                    gate.wait();
                    for _ in 0..4 {
                        let snapshot = session.snapshot().unwrap();
                        assert!(snapshot.revision == step || snapshot.revision == step + 1);
                        let count = snapshot.revision.div_ceil(2) as usize;
                        let report = snapshot.report;
                        assert_eq!(report.source_id, source("").id);
                        assert_eq!(
                            report.source_bytes,
                            (PREFIX.len() + count * CHUNK.len()) as u64
                        );
                        assert_eq!(report.findings.len(), count);
                        assert_eq!(report.candidate_count, 0);
                        assert_eq!(report.needs_review, count != 0);
                        for (i, finding) in report.findings.iter().enumerate() {
                            let l = finding.location;
                            let start = (PREFIX.len() + i * CHUNK.len()) as u64;
                            assert_eq!(
                                (l.start, l.end, l.line, l.column),
                                (start, start + 1, i as u64 + 2, 1)
                            );
                            assert_eq!(finding.rule_id.value, "literal");
                        }
                    }
                    gate.wait();
                }
            });
        }
        for revision in 0..STEPS {
            let end = (PREFIX.len() + revision.div_ceil(2) as usize * CHUNK.len()) as u64;
            gate.wait();
            let replacement = if revision % 2 == 0 { CHUNK } else { "" };
            assert_eq!(
                session.apply_edit(revision, end, end, replacement.into()),
                Ok(revision + 1)
            );
            gate.wait();
        }
    });
    let retained = session.snapshot().unwrap();
    assert_eq!(retained.revision, STEPS);
    assert_eq!(retained.report, session.report().unwrap());
    assert_eq!(retained.revision, session.revision().unwrap());
    drop(session);
    assert_eq!(retained.report.findings.len(), (STEPS / 2) as usize);
}
