//! Native streaming conformance through the public ABI and direct Core oracles.

use std::{ptr, slice, str};

use crate::*;

fn view(text: &str) -> CribraStringView {
    CribraStringView {
        ptr: text.as_ptr(),
        len: text.len(),
    }
}

fn rule(id: &str, kind: CribraRuleKind, value: &str) -> CribraRuleConfig {
    CribraRuleConfig {
        id: view(id),
        kind,
        value: view(value),
        severity: CRIBRA_SEVERITY_HIGH,
        remediation: CRIBRA_REMEDIATION_REMOVE_SENSITIVE_VALUE,
    }
}

fn scanner(rules: &[CribraRuleConfig]) -> *mut CribraScanner {
    let mut builder = ptr::null_mut();
    let mut scanner = ptr::null_mut();
    unsafe {
        assert_eq!(cribra_builder_new(&mut builder), CRIBRA_OK);
        for config in rules {
            assert_eq!(
                cribra_builder_add_rule(builder, config, ptr::null_mut()),
                CRIBRA_OK
            );
        }
        assert_eq!(
            cribra_builder_build(builder, &mut scanner, ptr::null_mut()),
            CRIBRA_OK
        );
    }
    scanner
}

unsafe fn stream(scanner: *const CribraScanner, limits: CribraStreamLimits) -> *mut CribraStream {
    let mut stream = ptr::null_mut();
    let mut error = ptr::null_mut();
    assert_eq!(
        unsafe { cribra_scanner_stream_new_with_limits(scanner, &limits, &mut stream, &mut error) },
        CRIBRA_OK
    );
    assert!(!stream.is_null());
    assert!(error.is_null());
    stream
}

unsafe fn diagnostic(error: *mut CribraError, expected: CribraStatus) {
    assert!(!error.is_null());
    let mut status = CRIBRA_OK;
    let mut message = CribraStringView::default();
    unsafe {
        assert_eq!(cribra_error_status(error, &mut status), CRIBRA_OK);
        assert_eq!(status, expected);
        assert_eq!(cribra_error_message(error, &mut message), CRIBRA_OK);
        let text = str::from_utf8(slice::from_raw_parts(message.ptr, message.len)).unwrap();
        assert!(!text.is_empty());
        for private in ["SECRET", "DO_NOT_EXPOSE", "ABCD-EFGH-IJKL-MNOP"] {
            assert!(!text.contains(private), "diagnostic leaked caller input");
        }
        cribra_error_free(error);
    }
}

unsafe fn inactive(stream: *mut CribraStream) {
    let mut report = ptr::dangling_mut::<CribraReport>();
    let mut error = ptr::null_mut();
    unsafe {
        assert_eq!(
            cribra_stream_push(stream, ptr::null(), 0, &mut error),
            CRIBRA_STREAM_NOT_ACTIVE
        );
        diagnostic(error, CRIBRA_STREAM_NOT_ACTIVE);
        assert_eq!(
            cribra_stream_push(stream, b"x".as_ptr(), 1, ptr::null_mut()),
            CRIBRA_STREAM_NOT_ACTIVE
        );
        assert_eq!(
            cribra_stream_finish(stream, &mut report, &mut error),
            CRIBRA_STREAM_NOT_ACTIVE
        );
        assert!(report.is_null());
        diagnostic(error, CRIBRA_STREAM_NOT_ACTIVE);
    }
}

#[test]
fn status_assignments_are_stable_and_stream_failures_are_separate() {
    assert_eq!(
        [
            CRIBRA_OK,
            CRIBRA_INVALID_ARGUMENT,
            CRIBRA_INVALID_UTF8,
            CRIBRA_RULE_ERROR,
            CRIBRA_BUILD_ERROR,
            CRIBRA_TRANSFORM_ERROR,
            CRIBRA_OUT_OF_RANGE,
            CRIBRA_INTERNAL_ERROR
        ],
        [0, 1, 2, 3, 4, 5, 6, 255]
    );
    assert_eq!(
        [
            CRIBRA_STREAM_UNSUPPORTED_RULE_SET,
            CRIBRA_STREAM_NOT_ACTIVE,
            CRIBRA_STREAM_INVALID_UTF8,
            CRIBRA_STREAM_INCOMPLETE_UTF8,
            CRIBRA_STREAM_CANDIDATE_LIMIT_EXCEEDED,
            CRIBRA_STREAM_RESULT_LIMIT_EXCEEDED,
            CRIBRA_STREAM_STORAGE_LIMIT_EXCEEDED,
            CRIBRA_STREAM_STORAGE_REQUIREMENT_OVERFLOW,
            CRIBRA_STREAM_SOURCE_OFFSET_OVERFLOW,
            CRIBRA_STREAM_COORDINATE_OVERFLOW,
            CRIBRA_STREAM_EXECUTION_INVARIANT_VIOLATED,
            CRIBRA_STREAM_UNKNOWN_ERROR
        ],
        [7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18]
    );
}

#[test]
fn unsupported_builtin_portfolios_and_unbounded_patterns_fail_admission() {
    unsafe {
        let mut current = ptr::null_mut();
        assert_eq!(cribra_scanner_new_current(&mut current), CRIBRA_OK);
        let unbounded = scanner(&[rule("DO_NOT_EXPOSE", CRIBRA_RULE_KIND_PATTERN, "a+")]);
        for scanner in [current, unbounded] {
            assert!(matches!(
                (*scanner).inner.stream(),
                Err(cribra::StreamError::UnsupportedRuleSet)
            ));
            let mut session = ptr::dangling_mut::<CribraStream>();
            let mut error = ptr::null_mut();
            assert_eq!(
                cribra_scanner_stream_new(scanner, &mut session, &mut error),
                CRIBRA_STREAM_UNSUPPORTED_RULE_SET
            );
            assert!(session.is_null());
            diagnostic(error, CRIBRA_STREAM_UNSUPPORTED_RULE_SET);
            assert_eq!(
                cribra_scanner_stream_new_with_limits(
                    scanner,
                    &CribraStreamLimits::default(),
                    &mut session,
                    &mut error
                ),
                CRIBRA_STREAM_UNSUPPORTED_RULE_SET
            );
            assert!(session.is_null());
            diagnostic(error, CRIBRA_STREAM_UNSUPPORTED_RULE_SET);
            // Failed stream admission does not affect the reusable scanner.
            let mut report = ptr::null_mut();
            assert_eq!(
                cribra_scanner_scan(scanner, b"aaa".as_ptr(), 3, &mut report, ptr::null_mut()),
                CRIBRA_OK
            );
            cribra_report_free(report);
            cribra_scanner_free(scanner);
        }
    }
}

#[test]
fn every_byte_split_and_chunk_size_matches_core_and_whole_source_reports() {
    let scanner = scanner(&[
        rule("unicode", CRIBRA_RULE_KIND_LITERAL, "秘密🔑"),
        rule("prefix", CRIBRA_RULE_KIND_PREFIX, "pre_"),
        rule("suffix", CRIBRA_RULE_KIND_SUFFIX, "_END"),
        rule("pattern", CRIBRA_RULE_KIND_PATTERN, "code[0-9]{2}"),
    ]);
    let source = "前\r\né🙂e\u{301} 秘密🔑 pre_ABC-12 token_END code42\nABCD-EFGH-IJKL-MNOP\nQRST-UVWX-YZ12-3456";
    unsafe {
        let mut whole = ptr::null_mut();
        assert_eq!(
            cribra_scanner_scan(
                scanner,
                source.as_ptr(),
                source.len(),
                &mut whole,
                ptr::null_mut()
            ),
            CRIBRA_OK
        );
        assert_eq!((*whole).inner.len(), 4);
        assert_eq!((*whole).inner.candidate_len(), 2);
        for split in 0..=source.len() {
            let session = stream(scanner, CribraStreamLimits::default());
            let mut direct = (*scanner).inner.stream().unwrap();
            for chunk in [
                &source.as_bytes()[..split],
                &[],
                &source.as_bytes()[split..],
            ] {
                direct.push(chunk).unwrap();
                assert_eq!(
                    cribra_stream_push(session, chunk.as_ptr(), chunk.len(), ptr::null_mut()),
                    CRIBRA_OK
                );
            }
            let mut report = ptr::null_mut();
            assert_eq!(
                cribra_stream_finish(session, &mut report, ptr::null_mut()),
                CRIBRA_OK
            );
            assert_eq!((*report).source_bytes, source.len());
            assert_eq!((*report).inner, direct.finish().unwrap(), "split {split}");
            assert_eq!((*report).inner, (*whole).inner, "split {split}");
            let mut finding = CribraFindingView::default();
            assert_eq!(cribra_report_finding_at(report, 0, &mut finding), CRIBRA_OK);
            assert_eq!(
                (finding.start, finding.end, finding.line, finding.column),
                (15, 25, 2, 6)
            );
            let mut explanation = CribraExplanationView::default();
            assert_eq!(
                cribra_scanner_explain_finding(
                    scanner,
                    report,
                    0,
                    &mut explanation,
                    ptr::null_mut()
                ),
                CRIBRA_OK
            );
            assert_eq!(explanation.kind, CRIBRA_EXPLANATION_CLASSIFIED);
            assert_eq!(
                explanation.detection_mode,
                CRIBRA_DETECTION_MODE_MATCHER_ONLY
            );
            assert_eq!(
                cribra_report_explain_candidate(report, 0, &mut explanation),
                CRIBRA_OK
            );
            assert_eq!(explanation.kind, CRIBRA_EXPLANATION_AMBIGUOUS);
            inactive(session);
            cribra_stream_free(session);
            // Reports are independently owned after stream_free.
            assert_eq!((*report).inner, (*whole).inner);
            cribra_report_free(report);
        }
        for size in [1, 2, 3, 7, 31, source.len()] {
            let session = stream(scanner, CribraStreamLimits::default());
            for chunk in source.as_bytes().chunks(size) {
                let temporary = chunk.to_vec();
                assert_eq!(
                    cribra_stream_push(
                        session,
                        temporary.as_ptr(),
                        temporary.len(),
                        ptr::null_mut()
                    ),
                    CRIBRA_OK
                );
                drop(temporary);
            }
            let mut report = ptr::null_mut();
            assert_eq!(
                cribra_stream_finish(session, &mut report, ptr::null_mut()),
                CRIBRA_OK
            );
            assert_eq!((*report).inner, (*whole).inner);
            assert_eq!((*report).source_bytes, source.len());
            cribra_report_free(report);
            cribra_stream_free(session);
        }
        cribra_report_free(whole);
        cribra_scanner_free(scanner);
    }
}

#[test]
fn empty_chunks_empty_source_and_scanner_independent_lifetimes() {
    unsafe {
        let scanner = self::scanner(&[]);
        let mut session = ptr::null_mut();
        assert_eq!(
            cribra_scanner_stream_new(scanner, &mut session, ptr::null_mut()),
            CRIBRA_OK
        );
        cribra_scanner_free(scanner);
        for _ in 0..3 {
            assert_eq!(
                cribra_stream_push(session, ptr::null(), 0, ptr::null_mut()),
                CRIBRA_OK
            );
        }
        let mut report = ptr::null_mut();
        assert_eq!(
            cribra_stream_finish(session, &mut report, ptr::null_mut()),
            CRIBRA_OK
        );
        inactive(session);
        cribra_stream_free(session);
        assert_eq!((*report).source_bytes, 0);
        assert!(!(*report).inner.needs_review());
        cribra_report_free(report);
        // Active cancellation and null destruction are also valid.
        let scanner = self::scanner(&[]);
        let session = stream(scanner, CribraStreamLimits::default());
        cribra_stream_free(session);
        cribra_scanner_free(scanner);
        cribra_stream_free(ptr::null_mut());
    }
}

#[test]
fn malformed_and_incomplete_utf8_are_distinct_terminal_errors_without_reports() {
    unsafe {
        let scanner = scanner(&[rule("literal", CRIBRA_RULE_KIND_LITERAL, "SECRET")]);
        for bytes in [
            b"SECRET \xff".as_slice(),
            &[0xc0, 0xaf],
            &[0xe2, 0x28, 0xa1],
            &[0xed, 0xa0, 0x80],
        ] {
            let session = stream(scanner, CribraStreamLimits::default());
            let mut error = ptr::null_mut();
            let mut direct = (*scanner).inner.stream().unwrap();
            assert_eq!(direct.push(bytes), Err(cribra::StreamError::InvalidUtf8));
            assert_eq!(
                cribra_stream_push(session, bytes.as_ptr(), bytes.len(), &mut error),
                CRIBRA_STREAM_INVALID_UTF8
            );
            diagnostic(error, CRIBRA_STREAM_INVALID_UTF8);
            inactive(session);
            cribra_stream_free(session);
        }
        for bytes in [&[0xc3][..], &[0xf0, 0x9f], &[0xe2, 0x82]] {
            let session = stream(scanner, CribraStreamLimits::default());
            assert_eq!(
                cribra_stream_push(session, b"SECRET ".as_ptr(), 7, ptr::null_mut()),
                CRIBRA_OK
            );
            for byte in bytes {
                assert_eq!(
                    cribra_stream_push(session, byte, 1, ptr::null_mut()),
                    CRIBRA_OK
                );
            }
            let mut report = ptr::dangling_mut::<CribraReport>();
            let mut error = ptr::null_mut();
            assert_eq!(
                cribra_stream_finish(session, &mut report, &mut error),
                CRIBRA_STREAM_INCOMPLETE_UTF8
            );
            assert!(report.is_null());
            diagnostic(error, CRIBRA_STREAM_INCOMPLETE_UTF8);
            inactive(session);
            cribra_stream_free(session);
        }
        // A valid scalar prefix becomes definitively malformed in a later call.
        let session = stream(scanner, CribraStreamLimits::default());
        assert_eq!(
            cribra_stream_push(session, [0xf0, 0x9f].as_ptr(), 2, ptr::null_mut()),
            CRIBRA_OK
        );
        assert_eq!(
            cribra_stream_push(session, b"x".as_ptr(), 1, ptr::null_mut()),
            CRIBRA_STREAM_INVALID_UTF8
        );
        inactive(session);
        cribra_stream_free(session);
        cribra_scanner_free(scanner);
    }
}

#[test]
fn explicit_limits_match_core_at_push_and_storage_admission_boundaries() {
    unsafe {
        let scanner = scanner(&[rule("literal", CRIBRA_RULE_KIND_LITERAL, "SECRET")]);
        let required = match (*scanner)
            .inner
            .stream_with_limits(
                cribra::StreamLimits::default().with_max_stream_storage_bytes(Some(0)),
            )
            .unwrap_err()
        {
            cribra::StreamError::StorageLimitExceeded { required, .. } => required,
            other => panic!("unexpected Core admission result: {other:?}"),
        };
        for allowance in [0, required - 1] {
            let limits = CribraStreamLimits {
                max_stream_storage_bytes_enabled: 1,
                max_stream_storage_bytes: allowance,
                ..CribraStreamLimits::default()
            };
            let mut session = ptr::dangling_mut::<CribraStream>();
            let mut error = ptr::null_mut();
            assert_eq!(
                cribra_scanner_stream_new_with_limits(scanner, &limits, &mut session, &mut error),
                CRIBRA_STREAM_STORAGE_LIMIT_EXCEEDED
            );
            assert!(session.is_null());
            diagnostic(error, CRIBRA_STREAM_STORAGE_LIMIT_EXCEEDED);
        }
        let limits = CribraStreamLimits {
            max_stream_storage_bytes_enabled: 1,
            max_stream_storage_bytes: required,
            ..CribraStreamLimits::default()
        };
        let exact = stream(scanner, limits);
        assert_eq!(
            cribra_stream_push(exact, b"SECRET".as_ptr(), 6, ptr::null_mut()),
            CRIBRA_OK
        );
        let mut report = ptr::null_mut();
        assert_eq!(
            cribra_stream_finish(exact, &mut report, ptr::null_mut()),
            CRIBRA_OK
        );
        assert_eq!((*report).inner.len(), 1);
        cribra_report_free(report);
        cribra_stream_free(exact);

        for (limits, core_limits, expected) in [
            (
                CribraStreamLimits {
                    max_candidates_enabled: 1,
                    max_candidates: 1,
                    ..CribraStreamLimits::default()
                },
                cribra::StreamLimits::default().with_max_candidates(Some(1)),
                CRIBRA_STREAM_CANDIDATE_LIMIT_EXCEEDED,
            ),
            (
                CribraStreamLimits {
                    max_retained_results_enabled: 1,
                    max_retained_results: 1,
                    ..CribraStreamLimits::default()
                },
                cribra::StreamLimits::default().with_max_retained_results(Some(1)),
                CRIBRA_STREAM_RESULT_LIMIT_EXCEEDED,
            ),
        ] {
            let session = stream(scanner, limits);
            let mut direct = (*scanner).inner.stream_with_limits(core_limits).unwrap();
            direct.push(b"SECRET ").unwrap();
            assert_eq!(
                cribra_stream_push(session, b"SECRET ".as_ptr(), 7, ptr::null_mut()),
                CRIBRA_OK
            );
            assert!(direct.push(b"SECRET ").is_err());
            let mut error = ptr::null_mut();
            assert_eq!(
                cribra_stream_push(session, b"SECRET ".as_ptr(), 7, &mut error),
                expected
            );
            diagnostic(error, expected);
            assert_eq!((*session).source_bytes, 7);
            inactive(session);
            cribra_stream_free(session);
        }
        cribra_scanner_free(scanner);
    }
}

#[test]
fn eof_limits_include_reviews_and_presuppression_records() {
    unsafe {
        for (rules, text, limits, expected) in [
            (
                vec![],
                "ABCD-EFGH-IJKL-MNOP",
                CribraStreamLimits {
                    max_retained_results_enabled: 1,
                    max_retained_results: 0,
                    ..CribraStreamLimits::default()
                },
                CRIBRA_STREAM_RESULT_LIMIT_EXCEEDED,
            ),
            (
                vec![rule("eof", CRIBRA_RULE_KIND_PATTERN, r"abc\z")],
                "abc",
                CribraStreamLimits {
                    max_candidates_enabled: 1,
                    max_candidates: 0,
                    ..CribraStreamLimits::default()
                },
                CRIBRA_STREAM_CANDIDATE_LIMIT_EXCEEDED,
            ),
            (
                vec![rule(
                    "suppress",
                    CRIBRA_RULE_KIND_LITERAL,
                    "ABCD-EFGH-IJKL-MNOP",
                )],
                "ABCD-EFGH-IJKL-MNOP",
                CribraStreamLimits {
                    max_retained_results_enabled: 1,
                    max_retained_results: 1,
                    ..CribraStreamLimits::default()
                },
                CRIBRA_STREAM_RESULT_LIMIT_EXCEEDED,
            ),
        ] {
            let scanner = scanner(&rules);
            let session = stream(scanner, limits);
            let mut direct = (*scanner)
                .inner
                .stream_with_limits(
                    cribra::StreamLimits::default()
                        .with_max_candidates(
                            (limits.max_candidates_enabled == 1).then_some(limits.max_candidates),
                        )
                        .with_max_retained_results(
                            (limits.max_retained_results_enabled == 1)
                                .then_some(limits.max_retained_results),
                        ),
                )
                .unwrap();
            direct.push(text.as_bytes()).unwrap();
            assert!(direct.finish().is_err());
            assert_eq!(
                cribra_stream_push(session, text.as_ptr(), text.len(), ptr::null_mut()),
                CRIBRA_OK
            );
            let mut report = ptr::dangling_mut::<CribraReport>();
            let mut error = ptr::null_mut();
            assert_eq!(
                cribra_stream_finish(session, &mut report, &mut error),
                expected
            );
            assert!(report.is_null());
            diagnostic(error, expected);
            inactive(session);
            cribra_stream_free(session);
            cribra_scanner_free(scanner);
        }
    }
}

#[test]
fn argument_preflight_preserves_active_sessions_and_disabled_limits_are_not_sentinels() {
    unsafe {
        let scanner = scanner(&[rule("literal", CRIBRA_RULE_KIND_LITERAL, "SECRET")]);
        for limits in [
            CribraStreamLimits {
                max_candidates_enabled: 2,
                ..CribraStreamLimits::default()
            },
            CribraStreamLimits {
                max_stream_storage_bytes_enabled: u32::MAX,
                ..CribraStreamLimits::default()
            },
            CribraStreamLimits {
                max_retained_results_enabled: 2,
                ..CribraStreamLimits::default()
            },
        ] {
            let mut session = ptr::dangling_mut::<CribraStream>();
            let mut error = ptr::null_mut();
            assert_eq!(
                cribra_scanner_stream_new_with_limits(scanner, &limits, &mut session, &mut error),
                CRIBRA_INVALID_ARGUMENT
            );
            assert!(session.is_null());
            diagnostic(error, CRIBRA_INVALID_ARGUMENT);
        }
        let mut session = ptr::dangling_mut::<CribraStream>();
        assert_eq!(
            cribra_scanner_stream_new_with_limits(
                scanner,
                ptr::null(),
                &mut session,
                ptr::null_mut()
            ),
            CRIBRA_INVALID_ARGUMENT
        );
        assert!(session.is_null());
        assert_eq!(
            cribra_scanner_stream_new(ptr::null(), &mut session, ptr::null_mut()),
            CRIBRA_INVALID_ARGUMENT
        );
        assert!(session.is_null());
        assert_eq!(
            cribra_scanner_stream_new(scanner, ptr::null_mut(), ptr::null_mut()),
            CRIBRA_INVALID_ARGUMENT
        );
        let mut report = ptr::dangling_mut::<CribraReport>();
        assert_eq!(
            cribra_stream_finish(ptr::null_mut(), &mut report, ptr::null_mut()),
            CRIBRA_INVALID_ARGUMENT
        );
        assert!(report.is_null());
        assert_eq!(
            cribra_stream_push(ptr::null_mut(), ptr::null(), 0, ptr::null_mut()),
            CRIBRA_INVALID_ARGUMENT
        );
        // All disabled limits ignore both zero and SIZE_MAX, with no sentinel.
        for value in [0, usize::MAX] {
            let session = stream(
                scanner,
                CribraStreamLimits {
                    max_candidates: value,
                    max_stream_storage_bytes: value,
                    max_retained_results: value,
                    ..CribraStreamLimits::default()
                },
            );
            assert_eq!(
                cribra_stream_push(session, ptr::null(), 1, ptr::null_mut()),
                CRIBRA_INVALID_ARGUMENT
            );
            assert_eq!(
                cribra_stream_push(session, b"x".as_ptr(), usize::MAX, ptr::null_mut()),
                CRIBRA_INVALID_ARGUMENT
            );
            assert_eq!(
                cribra_stream_finish(session, ptr::null_mut(), ptr::null_mut()),
                CRIBRA_INVALID_ARGUMENT
            );
            assert_eq!((*session).source_bytes, 0);
            assert_eq!(
                cribra_stream_push(session, b"SECRET".as_ptr(), 6, ptr::null_mut()),
                CRIBRA_OK
            );
            assert_eq!(
                cribra_stream_finish(session, &mut report, ptr::null_mut()),
                CRIBRA_OK
            );
            assert_eq!((*report).inner.len(), 1);
            cribra_report_free(report);
            cribra_stream_free(session);
        }
        cribra_scanner_free(scanner);
    }
}

#[test]
fn source_accounting_is_checked_and_reports_reuse_existing_transform_validation() {
    unsafe {
        let scanner = scanner(&[rule("literal", CRIBRA_RULE_KIND_LITERAL, "SECRET")]);
        let session = stream(scanner, CribraStreamLimits::default());
        // The rare counter boundary is an adapter-only injected condition, not
        // a claim to have scanned SIZE_MAX actual bytes.
        (*session).source_bytes = usize::MAX;
        let mut error = ptr::null_mut();
        assert_eq!(
            cribra_stream_push(session, b"x".as_ptr(), 1, &mut error),
            CRIBRA_STREAM_SOURCE_OFFSET_OVERFLOW
        );
        diagnostic(error, CRIBRA_STREAM_SOURCE_OFFSET_OVERFLOW);
        assert_eq!((*session).source_bytes, usize::MAX);
        inactive(session);
        cribra_stream_free(session);

        let session = stream(scanner, CribraStreamLimits::default());
        cribra_scanner_free(scanner);
        let mut bytes = b"SECRET".to_vec();
        assert_eq!(
            cribra_stream_push(session, bytes.as_ptr(), bytes.len(), ptr::null_mut()),
            CRIBRA_OK
        );
        bytes.fill(b'x');
        drop(bytes);
        let mut report = ptr::null_mut();
        assert_eq!(
            cribra_stream_finish(session, &mut report, ptr::null_mut()),
            CRIBRA_OK
        );
        cribra_stream_free(session);
        assert_eq!((*report).source_bytes, 6);
        let mut output = ptr::null_mut();
        assert_eq!(
            cribra_transform_redact(b"SECRET".as_ptr(), 6, report, &mut output, ptr::null_mut()),
            CRIBRA_OK
        );
        cribra_output_free(output);
        assert_eq!(
            cribra_transform_redact(b"SHORT".as_ptr(), 5, report, &mut output, ptr::null_mut()),
            CRIBRA_TRANSFORM_ERROR
        );
        assert!(output.is_null());
        cribra_report_free(report);
    }
}
