use cribra::{IncrementalError, IncrementalLimits, Rule, Scanner, Severity, builtins};

#[test]
fn public_document_ownership_and_revisioned_utf8_edits() {
    let scanner = Scanner::builder()
        .rule(Rule::literal("secret", "SECRET", Severity::High))
        .build()
        .unwrap();
    let oracle = scanner.clone();
    let mut session = scanner.incremental_session("é\r\nSECRET".into()).unwrap();
    drop(scanner);
    for (range, replacement) in [(0..0, "🦀"), (4..6, "€"), (0..4, ""), (0..0, "")] {
        let revision = session.revision();
        session.apply_edit(revision, range, replacement).unwrap();
        assert_eq!(session.revision(), revision + 1);
        let results = oracle.scan([((), session.source())]);
        assert_eq!(session.report(), results.single_report().unwrap());
    }
    let report = session.report().clone(); // caller-owned, independent of the session
    assert_eq!(
        session.apply_edit(0, 0..0, ""),
        Err(IncrementalError::StaleRevision {
            expected: 0,
            actual: 4
        })
    );
    assert!(session.is_active());
    assert_eq!(session.report(), &report);
    assert_eq!(
        session.apply_edit(4, 1..1, ""),
        Err(IncrementalError::InvalidUtf8Boundary)
    );
    assert!(session.is_active());
    assert!(!format!("{session:?}").contains("SECRET"));
}

#[test]
fn public_resource_failure_is_terminal_and_preserves_complete_snapshot() {
    let scanner = Scanner::builder()
        .rule(Rule::literal("x", "x", Severity::High))
        .build()
        .unwrap();
    let limits = IncrementalLimits::default()
        .with_max_candidates(Some(1))
        .with_max_retained_cache_records(Some(3))
        .with_max_materialized_results(Some(1));
    let mut session = scanner
        .incremental_session_with_limits("x".into(), limits)
        .unwrap();
    let old = session.report().clone();
    assert_eq!(
        session.apply_edit(0, 1..1, "x"),
        Err(IncrementalError::ResultLimitExceeded { limit: 1 })
    );
    assert!(!session.is_active());
    assert_eq!(session.source(), "x");
    assert_eq!(session.revision(), 0);
    assert_eq!(session.report(), &old);
    assert_eq!(
        session.apply_edit(0, 0..1, ""),
        Err(IncrementalError::NotActive)
    );
}

#[test]
fn public_builtin_portfolios_are_incremental_independently_of_stream_admission() {
    for scanner in [
        Scanner::builder()
            .builtins(builtins::CURRENT)
            .build()
            .unwrap(),
        Scanner::builder()
            .builtins(builtins::personal::CURRENT)
            .build()
            .unwrap(),
        Scanner::builder()
            .builtins(builtins::financial::CURRENT)
            .build()
            .unwrap(),
        Scanner::builder()
            .builtins(builtins::CURRENT)
            .builtins(builtins::personal::CURRENT)
            .builtins(builtins::financial::CURRENT)
            .build()
            .unwrap(),
    ] {
        let source = "password=CorrectHorseBatteryStaple\r\n44051401458\nQRST-UVWX-YZ12-3456";
        let limits = IncrementalLimits::default()
            .with_max_candidates(Some(1000))
            .with_max_retained_cache_records(Some(2000))
            .with_max_materialized_results(Some(1000))
            .with_max_transaction_storage_bytes(Some(1024 * 1024));
        let mut session = scanner
            .incremental_session_with_limits(source.into(), limits)
            .unwrap();
        for replacement in ["🦀\n", "é\r", "", "x"] {
            session
                .apply_edit(session.revision(), 0..0, replacement)
                .unwrap();
            let results = scanner.scan([((), session.source())]);
            assert_eq!(session.report(), results.single_report().unwrap());
        }
    }
}

#[test]
fn public_zero_raw_limit_accepts_nonmatching_input_and_fails_before_emission() {
    let scanner = Scanner::builder()
        .rule(Rule::pattern("regex", "x", Severity::High).unwrap())
        .build()
        .unwrap();
    let mut session = scanner
        .incremental_session_with_limits(
            "clean".into(),
            IncrementalLimits::default().with_max_candidates(Some(0)),
        )
        .unwrap();
    assert_eq!(
        session.apply_edit(0, 0..0, "x"),
        Err(IncrementalError::CandidateLimitExceeded { limit: 0 })
    );
    assert_eq!(session.source(), "clean");
    assert!(!session.is_active());
}
