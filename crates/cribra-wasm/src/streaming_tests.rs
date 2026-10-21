//! Native-safe adapter checks; generated JS numeric conversion is qualified separately.
use super::*;
use crate::{FindingSeverity, ScanEngineBuilder};

fn literal(value: &str) -> ScanEngine {
    let mut builder = ScanEngineBuilder::new(false);
    builder.add_literal("literal", value, FindingSeverity::High);
    builder.build_core().unwrap()
}

fn empty() -> ScanEngine {
    ScanEngineBuilder::new(false).build_core().unwrap()
}

#[test]
fn numeric_limits_are_checked_before_integer_conversion() {
    for value in [
        -1.0,
        -0.5,
        0.5,
        f64::NAN,
        f64::INFINITY,
        f64::NEG_INFINITY,
        9_007_199_254_740_992.0,
        1e30,
    ] {
        assert_eq!(
            numeric_limit(value).unwrap_err().code,
            StreamErrorCode::InvalidLimit
        );
    }
    for (value, expected) in [
        (0.0, 0),
        (-0.0, 0),
        (1.0, 1),
        (u32::MAX as f64, u32::MAX as usize),
    ] {
        assert_eq!(numeric_limit(value), Ok(expected));
    }
    let max = (usize::MAX as f64).min(9_007_199_254_740_991.0);
    assert_eq!(numeric_limit(max), Ok(max as usize));
    assert!(numeric_limit(max + 1.0).is_err());
}

#[test]
fn byte_partitions_preserve_core_findings_candidates_metadata_and_order() {
    let engine = literal("秘密🔑");
    let source = "前\r\né🙂e\u{301} 秘密🔑\nABCD-EFGH-IJKL-MNOP\nQRST-UVWX-YZ12-3456";
    let whole = engine.scan(source);
    assert_eq!((whole.finding_count(), whole.candidate_count()), (1, 2));
    for split in 0..=source.len() {
        let mut session = engine.stream().unwrap();
        let mut core = engine.scanner.stream().unwrap();
        for chunk in [
            &source.as_bytes()[..split],
            &[],
            &source.as_bytes()[split..],
        ] {
            core.push(chunk).unwrap();
            session.push(chunk).unwrap();
        }
        let actual = session.finish().unwrap();
        assert_eq!(actual.source_bytes, source.len());
        assert_eq!(actual.report, core.finish().unwrap());
        assert_eq!(actual.report, whole.report);
        assert_eq!(
            session.push(&[]).unwrap_err().code,
            StreamErrorCode::NotActive
        );
        assert_eq!(
            session.finish().err().unwrap().code,
            StreamErrorCode::NotActive
        );
    }
    let mut session = engine.stream().unwrap();
    for byte in source.as_bytes() {
        session.push(&[*byte]).unwrap();
    }
    let result = session.finish().unwrap();
    drop(engine);
    drop(session);
    assert_eq!(result.report, whole.report);
    let view = format!("{:?}", result.report);
    assert!(!view.contains("秘密🔑"));
    assert!(!view.contains("ABCD-EFGH-IJKL-MNOP"));
}

#[test]
fn empty_chunks_and_empty_source_and_engine_lifetimes() {
    let engine = empty();
    let mut session = engine.stream().unwrap();
    drop(engine);
    for _ in 0..3 {
        session.push(&[]).unwrap();
    }
    let result = session.finish().unwrap();
    assert_eq!(result.source_bytes, 0);
    assert!(!result.needs_review());
    assert_eq!(
        session.push(b"x").unwrap_err().code,
        StreamErrorCode::NotActive
    );
}

#[test]
fn malformed_and_incomplete_utf8_consume_the_session_without_partial_results() {
    let engine = literal("SECRET");
    for bytes in [
        b"SECRET \xff".as_slice(),
        &[0xc0, 0xaf],
        &[0xe2, 0x28, 0xa1],
        &[0xed, 0xa0, 0x80],
    ] {
        let mut session = engine.stream().unwrap();
        assert_eq!(
            session.push(bytes).unwrap_err().code,
            StreamErrorCode::InvalidUtf8
        );
        assert!(session.inner.is_none());
        assert_eq!(
            session.finish().err().unwrap().code,
            StreamErrorCode::NotActive
        );
    }
    for bytes in [&[0xc3][..], &[0xf0, 0x9f], &[0xe2, 0x82]] {
        let mut session = engine.stream().unwrap();
        session.push(b"SECRET ").unwrap();
        for byte in bytes {
            session.push(&[*byte]).unwrap();
        }
        assert_eq!(
            session.finish().err().unwrap().code,
            StreamErrorCode::IncompleteUtf8
        );
        assert!(session.inner.is_none());
        assert_eq!(
            session.push(&[]).unwrap_err().code,
            StreamErrorCode::NotActive
        );
    }
}

#[test]
fn zero_and_nonzero_core_limits_preserve_admission_and_terminal_semantics() {
    let engine = literal("SECRET");
    let required = match engine
        .scanner
        .stream_with_limits(cribra::StreamLimits::default().with_max_stream_storage_bytes(Some(0)))
        .unwrap_err()
    {
        cribra::StreamError::StorageLimitExceeded { required, .. } => required,
        other => panic!("unexpected Core storage failure: {other:?}"),
    };
    for value in [0, required - 1] {
        assert_eq!(
            engine
                .open_stream(
                    cribra::StreamLimits::default().with_max_stream_storage_bytes(Some(value))
                )
                .err()
                .unwrap()
                .code,
            StreamErrorCode::StorageLimitExceeded
        );
    }
    let mut exact = engine
        .open_stream(cribra::StreamLimits::default().with_max_stream_storage_bytes(Some(required)))
        .unwrap();
    exact.push(b"SECRET").unwrap();
    assert_eq!(exact.finish().unwrap().finding_count(), 1);
    for (limits, code) in [
        (
            cribra::StreamLimits::default().with_max_candidates(Some(0)),
            StreamErrorCode::CandidateLimitExceeded,
        ),
        (
            cribra::StreamLimits::default().with_max_retained_results(Some(0)),
            StreamErrorCode::ResultLimitExceeded,
        ),
        (
            cribra::StreamLimits::default().with_max_candidates(Some(1)),
            StreamErrorCode::CandidateLimitExceeded,
        ),
        (
            cribra::StreamLimits::default().with_max_retained_results(Some(1)),
            StreamErrorCode::ResultLimitExceeded,
        ),
    ] {
        let mut core = engine.scanner.stream_with_limits(limits).unwrap();
        let mut session = engine.open_stream(limits).unwrap();
        let expected = core.push(b"SECRET SECRET ").unwrap_err();
        assert_eq!(StreamError::from(expected).code, code);
        assert_eq!(session.push(b"SECRET SECRET ").unwrap_err().code, code);
        assert_eq!(
            session.finish().err().unwrap().code,
            StreamErrorCode::NotActive
        );
    }
    let mut review = empty()
        .open_stream(cribra::StreamLimits::default().with_max_retained_results(Some(0)))
        .unwrap();
    review.push(b"ABCD-EFGH-IJKL-MNOP").unwrap();
    assert_eq!(
        review.finish().err().unwrap().code,
        StreamErrorCode::ResultLimitExceeded
    );
    assert_eq!(
        review.finish().err().unwrap().code,
        StreamErrorCode::NotActive
    );
    let mut builder = ScanEngineBuilder::new(false);
    builder
        .add_pattern_core("eof", r"abc\z", FindingSeverity::High)
        .unwrap();
    let mut eof = builder
        .build_core()
        .unwrap()
        .open_stream(cribra::StreamLimits::default().with_max_candidates(Some(0)))
        .unwrap();
    eof.push(b"abc").unwrap();
    assert_eq!(
        eof.finish().err().unwrap().code,
        StreamErrorCode::CandidateLimitExceeded
    );
    let mut suppressed = literal("ABCD-EFGH-IJKL-MNOP")
        .open_stream(cribra::StreamLimits::default().with_max_retained_results(Some(1)))
        .unwrap();
    suppressed.push(b"ABCD-EFGH-IJKL-MNOP").unwrap();
    assert_eq!(
        suppressed.finish().err().unwrap().code,
        StreamErrorCode::ResultLimitExceeded
    );
}

#[test]
fn unsupported_admission_and_checked_byte_accounting_fail_closed() {
    let engine = ScanEngine::new();
    assert_eq!(
        engine.stream().err().unwrap().code,
        StreamErrorCode::UnsupportedRuleSet
    );
    let mut builder = ScanEngineBuilder::new(false);
    builder
        .add_pattern_core("DO_NOT_EXPOSE", "a+", FindingSeverity::High)
        .unwrap();
    assert_eq!(
        builder.build_core().unwrap().stream().err().unwrap().code,
        StreamErrorCode::UnsupportedRuleSet
    );
    let mut session = empty().stream().unwrap();
    // Adapter-only counter injection, not a claim to have streamed usize::MAX bytes.
    session.source_bytes = usize::MAX;
    assert_eq!(
        session.push(b"x").unwrap_err().code,
        StreamErrorCode::SourceOffsetOverflow
    );
    assert_eq!(session.source_bytes, usize::MAX);
    assert_eq!(
        session.finish().err().unwrap().code,
        StreamErrorCode::NotActive
    );
}

#[test]
fn all_core_error_codes_are_distinct_and_diagnostics_are_source_free() {
    use cribra::StreamError as Core;
    for (index, error) in [
        Core::UnsupportedRuleSet,
        Core::NotActive,
        Core::InvalidUtf8,
        Core::IncompleteUtf8,
        Core::CandidateLimitExceeded { limit: 0 },
        Core::ResultLimitExceeded { limit: 0 },
        Core::StorageLimitExceeded {
            required: 1,
            limit: 0,
        },
        Core::StorageRequirementOverflow,
        Core::SourceOffsetOverflow,
        Core::CoordinateOverflow,
        Core::ExecutionInvariantViolated,
    ]
    .into_iter()
    .enumerate()
    {
        let projected = StreamError::from(error);
        assert_eq!(projected.code as usize, index);
        assert!(!projected.message().contains("SECRET"));
        assert!(!projected.message().contains("DO_NOT_EXPOSE"));
    }
    assert_eq!(StreamErrorCode::UnknownCoreError as u32, 11);
    assert_eq!(StreamErrorCode::InvalidLimit as u32, 12);
}
