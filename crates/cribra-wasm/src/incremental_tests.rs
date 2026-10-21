//! Native Core authority checks; JS types and bigint validation run in generated WASM.
use super::*;
use crate::{FindingSeverity, ScanEngineBuilder};

fn engine() -> ScanEngine {
    let mut builder = ScanEngineBuilder::new(false);
    builder.add_literal("literal", "SECRET", FindingSeverity::High);
    builder.build_core().unwrap()
}

fn parity(session: &IncrementalSession, core: &cribra::IncrementalSession) {
    let snapshot = session.snapshot();
    assert_eq!(snapshot.revision(), core.revision());
    assert_eq!(snapshot.result.source_bytes, core.source().len());
    assert_eq!(&snapshot.result.report, core.report());
    assert_eq!(session.is_active(), core.is_active());
}

#[test]
fn edits_and_recoverable_errors_preserve_exact_core_snapshots() {
    let engine = engine();
    let source = "é🙂 SECRET\nABCD-EFGH-IJKL-MNOP";
    let mut actual = engine.incremental(source).unwrap();
    let mut core = engine.scanner.incremental_session(source.into()).unwrap();
    let initial = actual.snapshot();
    assert_eq!(initial.revision(), 0);
    parity(&actual, &core);
    for (revision, start, end, replacement) in [
        (u64::MAX, 0, 0, "DO_NOT_EXPOSE"),
        (9_007_199_254_740_993, 0, 0, ""),
        (0, 1, 1, ""),
        (0, 3, 2, ""),
        (0, 0, usize::MAX, ""),
        (0, 0, 0, ""), // no-op increments
        (1, 0, 2, "前"),
        (2, 0, 0, "SECRET\r\n"),
        (3, 0, 8, ""),
        (4, 0, 3, "é"),
    ] {
        let expected = core.apply_edit(revision, start..end, replacement);
        let result = actual.edit_core(revision, start, end, replacement);
        assert_eq!(
            result,
            expected.map_err(|e| IncrementalError::from_core(e, !core.is_active()))
        );
        parity(&actual, &core);
        assert!(actual.is_active());
        let whole = engine.scan(core.source());
        assert_eq!(actual.snapshot().result.report, whole.report);
    }
    assert_eq!(initial.revision(), 0);
    assert_eq!(initial.result.source_bytes, source.len());
    assert_eq!(initial.result.report, engine.scan(source).report);
}

#[test]
fn snapshots_and_results_outlive_edits_engine_and_session() {
    let engine = engine();
    let mut session = engine.incremental("SECRET").unwrap();
    let snapshot = session.snapshot();
    let result = snapshot.result();
    let second = snapshot.result();
    drop(engine);
    session.edit_core(0, 0, 6, "").unwrap();
    assert_eq!(session.snapshot().revision(), 1);
    assert_eq!(session.snapshot().result.finding_count(), 0);
    drop(session);
    assert_eq!(snapshot.revision(), 0);
    drop(snapshot);
    assert_eq!(result.finding_count(), 1);
    assert_eq!(result.source_bytes, 6);
    assert_eq!(result.report, second.report);
    assert!(!format!("{:?}", result.report).contains("SECRET"));
    let mut empty = engine_for_empty().incremental("").unwrap();
    assert_eq!(empty.snapshot().result.source_bytes, 0);
    assert_eq!(empty.edit_core(0, 0, 0, ""), Ok(1));
}

fn engine_for_empty() -> ScanEngine {
    ScanEngineBuilder::new(false).build_core().unwrap()
}

#[test]
fn all_four_limits_reject_construction_and_preserve_terminal_snapshots() {
    let engine = engine();
    for (limits, code, source, replacement) in [
        (
            cribra::IncrementalLimits::default().with_max_candidates(Some(0)),
            IncrementalErrorCode::CandidateLimitExceeded,
            "SECRET",
            "SECRET",
        ),
        (
            cribra::IncrementalLimits::default().with_max_retained_cache_records(Some(0)),
            IncrementalErrorCode::CacheLimitExceeded,
            "SECRET",
            "SECRET",
        ),
        (
            cribra::IncrementalLimits::default().with_max_materialized_results(Some(0)),
            IncrementalErrorCode::ResultLimitExceeded,
            "SECRET",
            "SECRET",
        ),
        (
            cribra::IncrementalLimits::default().with_max_transaction_storage_bytes(Some(1024)),
            IncrementalErrorCode::TransactionStorageLimitExceeded,
            &"x".repeat(4096),
            &"x".repeat(4096),
        ),
    ] {
        let failure = engine.open_incremental(source, limits).err().unwrap();
        assert_eq!(failure.code, code);
        assert!(!failure.terminal);
        let mut actual = engine.open_incremental("", limits).unwrap();
        let mut core = engine
            .scanner
            .incremental_session_with_limits("".into(), limits)
            .unwrap();
        let before = actual.snapshot();
        let expected = core.apply_edit(0, 0..0, replacement).unwrap_err();
        let error = actual.edit_core(0, 0, 0, replacement).unwrap_err();
        assert_eq!(
            error,
            IncrementalError::from_core(expected, !core.is_active())
        );
        assert!(error.terminal);
        assert!(!actual.is_active());
        parity(&actual, &core);
        assert_eq!(actual.snapshot().revision(), before.revision());
        assert_eq!(actual.snapshot().result.report, before.result.report);
        let error = actual.edit_core(0, 0, 0, "").unwrap_err();
        assert_eq!(error.code, IncrementalErrorCode::NotActive);
        assert!(error.terminal);
    }
    assert_eq!(
        engine
            .open_incremental(
                "",
                cribra::IncrementalLimits::default().with_max_transaction_storage_bytes(Some(0))
            )
            .err()
            .unwrap()
            .code,
        IncrementalErrorCode::TransactionStorageLimitExceeded
    );
    let zero = cribra::IncrementalLimits::default()
        .with_max_candidates(Some(0))
        .with_max_retained_cache_records(Some(0))
        .with_max_materialized_results(Some(0));
    assert_eq!(
        engine
            .open_incremental("", zero)
            .unwrap()
            .snapshot()
            .revision(),
        0
    );
    let mut cache = engine
        .open_incremental(
            "SECRET",
            cribra::IncrementalLimits::default().with_max_retained_cache_records(Some(1)),
        )
        .unwrap();
    assert_eq!(
        cache.edit_core(0, 0, 6, "SECRET").unwrap_err().code,
        IncrementalErrorCode::CacheLimitExceeded
    );
    assert_eq!(cache.snapshot().result.finding_count(), 1);
}

#[test]
fn builtins_are_incrementally_supported_and_metadata_explanations_keep_authority() {
    let engine = ScanEngine::new();
    let source = "password=CorrectHorseBatteryStaple\nABCD-EFGH-IJKL-MNOP";
    let mut session = engine.incremental(source).unwrap();
    let before = session.snapshot();
    assert_eq!(before.result.report, engine.scan(source).report);
    for finding in before.result.report.findings() {
        assert!(finding.explanation(&engine.scanner).is_some());
        assert!(finding.explanation(&engine_for_empty().scanner).is_none());
    }
    session.edit_core(0, 0, source.len(), "é SECRET").unwrap();
    assert_eq!(
        session.snapshot().result.report,
        engine.scan("é SECRET").report
    );
    assert_eq!(before.result.report, engine.scan(source).report);
}

#[test]
fn every_core_error_has_a_distinct_code_without_source_diagnostics() {
    use cribra::IncrementalError as Core;
    for (index, error) in [
        Core::NotActive,
        Core::StaleRevision {
            expected: u64::MAX,
            actual: 0,
        },
        Core::InvalidRange,
        Core::InvalidUtf8Boundary,
        Core::RevisionOverflow,
        Core::DocumentLengthOverflow,
        Core::CandidateLimitExceeded { limit: 0 },
        Core::CacheLimitExceeded { limit: 0 },
        Core::ResultLimitExceeded { limit: 0 },
        Core::TransactionStorageLimitExceeded {
            required: 1,
            limit: 0,
        },
        Core::ResourceAccountingOverflow,
        Core::ExecutionInvariantViolated,
    ]
    .into_iter()
    .enumerate()
    {
        for terminal in [false, true] {
            let mapped = IncrementalError::from_core(error, terminal);
            assert_eq!(mapped.code as usize, index);
            assert_eq!(mapped.terminal(), terminal);
            assert!(!mapped.message().contains("SECRET"));
            assert!(!mapped.message().contains("DO_NOT_EXPOSE"));
        }
    }
    assert_eq!(IncrementalErrorCode::UnknownCoreError as u32, 12);
    assert_eq!(IncrementalErrorCode::InvalidLimit as u32, 13);
    assert_eq!(IncrementalErrorCode::OffsetOutOfRange as u32, 14);
    assert_eq!(IncrementalErrorCode::InvalidRevision as u32, 15);
}
