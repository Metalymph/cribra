//! Incremental ABI conformance using original Core sessions and owned reports.

use std::{ptr, slice, str};

use crate::*;

fn view(text: &str) -> CribraStringView {
    CribraStringView {
        ptr: text.as_ptr(),
        len: text.len(),
    }
}

fn scanner(rules: &[(&str, CribraRuleKind, &str)]) -> *mut CribraScanner {
    let mut builder = ptr::null_mut();
    let mut scanner = ptr::null_mut();
    unsafe {
        assert_eq!(cribra_builder_new(&mut builder), CRIBRA_OK);
        for (id, kind, value) in rules {
            let config = CribraRuleConfig {
                id: view(id),
                kind: *kind,
                value: view(value),
                severity: CRIBRA_SEVERITY_HIGH,
                remediation: CRIBRA_REMEDIATION_REMOVE_SENSITIVE_VALUE,
            };
            assert_eq!(
                cribra_builder_add_rule(builder, &config, ptr::null_mut()),
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

unsafe fn session(
    scanner: *const CribraScanner,
    source: &str,
    limits: CribraIncrementalLimits,
) -> *mut CribraIncremental {
    let mut handle = ptr::null_mut();
    let mut error = ptr::null_mut();
    assert_eq!(
        unsafe {
            cribra_scanner_incremental_new_with_limits(
                scanner,
                source.as_ptr(),
                source.len(),
                &limits,
                &mut handle,
                &mut error,
            )
        },
        CRIBRA_OK
    );
    assert!(!handle.is_null());
    assert!(error.is_null());
    handle
}

unsafe fn snapshot(handle: *mut CribraIncremental) -> CribraIncrementalSnapshot {
    let mut snapshot = CribraIncrementalSnapshot::default();
    assert_eq!(
        unsafe { cribra_incremental_snapshot(handle, &mut snapshot, ptr::null_mut()) },
        CRIBRA_OK
    );
    assert!(!snapshot.report.is_null());
    snapshot
}

unsafe fn active(handle: *mut CribraIncremental) -> bool {
    let mut active = u32::MAX;
    assert_eq!(
        unsafe { cribra_incremental_is_active(handle, &mut active, ptr::null_mut()) },
        CRIBRA_OK
    );
    assert!(active <= 1);
    active == 1
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
        for private in [
            "SECRET",
            "DO_NOT_EXPOSE",
            "ABCD-EFGH-IJKL-MNOP",
            "CorrectHorseBatteryStaple",
        ] {
            assert!(!text.contains(private));
        }
        cribra_error_free(error);
    }
}

unsafe fn assert_same_snapshot(a: &CribraIncrementalSnapshot, b: &CribraIncrementalSnapshot) {
    assert_eq!(a.revision, b.revision);
    assert_eq!(a.source_bytes, b.source_bytes);
    unsafe {
        assert_eq!((*a.report).inner, (*b.report).inner);
    }
}

#[test]
fn initial_revision_empty_document_and_enabled_zero_budgets() {
    unsafe {
        let scanner = scanner(&[]);
        let mut handle = ptr::null_mut();
        assert_eq!(
            cribra_scanner_incremental_new(scanner, ptr::null(), 0, &mut handle, ptr::null_mut()),
            CRIBRA_OK
        );
        assert!(active(handle));
        let initial = snapshot(handle);
        assert_eq!((initial.revision, initial.source_bytes), (0, 0));
        assert!(!(*initial.report).inner.needs_review());
        cribra_report_free(initial.report);
        cribra_incremental_free(handle);
        let limits = CribraIncrementalLimits {
            max_candidates_enabled: 1,
            max_retained_cache_records_enabled: 1,
            max_materialized_results_enabled: 1,
            max_transaction_storage_bytes_enabled: 1,
            ..Default::default()
        };
        let handle = session(scanner, "", limits);
        let mut revision = u64::MAX;
        assert_eq!(
            cribra_incremental_apply_edit(
                handle,
                0,
                0,
                0,
                ptr::null(),
                0,
                &mut revision,
                ptr::null_mut()
            ),
            CRIBRA_OK
        );
        assert_eq!(revision, 1);
        let after = snapshot(handle);
        assert_eq!((after.revision, after.source_bytes), (1, 0));
        cribra_report_free(after.report);
        cribra_incremental_free(handle);
        cribra_incremental_free(ptr::null_mut());
        cribra_scanner_free(scanner);
    }
}

#[test]
fn edits_snapshots_and_existing_explanations_match_core_and_whole_source() {
    let custom = scanner(&[
        ("literal", CRIBRA_RULE_KIND_LITERAL, "秘密🔑"),
        ("prefix", CRIBRA_RULE_KIND_PREFIX, "pre_"),
        ("suffix", CRIBRA_RULE_KIND_SUFFIX, "_END"),
        ("pattern", CRIBRA_RULE_KIND_PATTERN, "code[0-9]{2}"),
    ]);
    let mut builtins = ptr::null_mut();
    unsafe {
        assert_eq!(cribra_scanner_new_current(&mut builtins), CRIBRA_OK);
    }
    for (scanner, initial) in [
        (
            custom,
            "前\r\né🙂e\u{301} 秘密🔑 pre_ABC token_END code42\nABCD-EFGH-IJKL-MNOP",
        ),
        (
            scanner(&[("unbounded", CRIBRA_RULE_KIND_PATTERN, "a+")]),
            "aaa ABCD-EFGH-IJKL-MNOP",
        ),
        (
            builtins,
            "password=CorrectHorseBatteryStaple\nGITHUB_TOKEN=ghp_AbCdEf0123456789_AbCdEf0123456789\nABCD-EFGH-IJKL-MNOP",
        ),
    ] {
        unsafe {
            let mut core = (*scanner)
                .inner
                .incremental_session(initial.into())
                .unwrap();
            let handle = session(scanner, initial, CribraIncrementalLimits::default());
            for step in 0..=6 {
                let current = snapshot(handle);
                assert_eq!(current.revision, core.revision());
                assert_eq!(current.source_bytes, core.source().len());
                assert_eq!((*current.report).source_bytes, current.source_bytes);
                assert_eq!(&(*current.report).inner, core.report());
                let mut whole = ptr::null_mut();
                assert_eq!(
                    cribra_scanner_scan(
                        scanner,
                        core.source().as_ptr(),
                        core.source().len(),
                        &mut whole,
                        ptr::null_mut()
                    ),
                    CRIBRA_OK
                );
                assert_eq!((*current.report).inner, (*whole).inner);
                let mut count = usize::MAX;
                assert_eq!(
                    cribra_report_finding_count(current.report, &mut count),
                    CRIBRA_OK
                );
                assert_eq!(count, core.report().len());
                for index in 0..count {
                    let mut explanation = CribraExplanationView::default();
                    assert_eq!(
                        cribra_scanner_explain_finding(
                            scanner,
                            current.report,
                            index,
                            &mut explanation,
                            ptr::null_mut()
                        ),
                        CRIBRA_OK
                    );
                    assert_eq!(explanation.kind, CRIBRA_EXPLANATION_CLASSIFIED);
                    let expected = core.report().findings()[index]
                        .explanation(&(*scanner).inner)
                        .unwrap()
                        .detection_mode()
                        .unwrap();
                    assert_eq!(
                        explanation.detection_mode,
                        match expected {
                            cribra::DetectionMode::MatcherOnly =>
                                CRIBRA_DETECTION_MODE_MATCHER_ONLY,
                            cribra::DetectionMode::Deterministic =>
                                CRIBRA_DETECTION_MODE_DETERMINISTIC,
                            cribra::DetectionMode::Contextual => CRIBRA_DETECTION_MODE_CONTEXTUAL,
                        }
                    );
                }
                assert_eq!(
                    cribra_report_candidate_count(current.report, &mut count),
                    CRIBRA_OK
                );
                assert_eq!(count, core.report().candidate_len());
                for index in 0..count {
                    let mut candidate = CribraCandidateView::default();
                    let mut explanation = CribraExplanationView::default();
                    assert_eq!(
                        cribra_report_candidate_at(current.report, index, &mut candidate),
                        CRIBRA_OK
                    );
                    let expected = core.report().candidates()[index].location();
                    assert_eq!(
                        (
                            candidate.start,
                            candidate.end,
                            candidate.line,
                            candidate.column
                        ),
                        (
                            expected.start(),
                            expected.end(),
                            expected.line(),
                            expected.column()
                        )
                    );
                    assert_eq!(
                        cribra_report_explain_candidate(current.report, index, &mut explanation),
                        CRIBRA_OK
                    );
                    assert_eq!(explanation.kind, CRIBRA_EXPLANATION_AMBIGUOUS);
                    assert_eq!(
                        explanation.candidate_evidence,
                        CRIBRA_CANDIDATE_EVIDENCE_STRUCTURAL
                    );
                }
                cribra_report_free(whole);
                cribra_report_free(current.report);
                if step == 6 {
                    break;
                }
                let (start, end, replacement) = match step {
                    0 => (0, 0, "🙂"), // insertion: four UTF-8 bytes
                    1 => (0, 4, "é"),  // replacement: two UTF-8 bytes
                    2 => (0, 2, ""),   // deletion restores original prefix
                    3 => (
                        core.source().len(),
                        core.source().len(),
                        "\0\nQRST-UVWX-YZ12-3456",
                    ),
                    4 => (0, 0, ""), // no-op still advances revision
                    _ => (0, core.source().len(), initial),
                };
                let expected = core
                    .apply_edit(core.revision(), start..end, replacement)
                    .unwrap();
                let mut revision = u64::MAX;
                assert_eq!(
                    cribra_incremental_apply_edit(
                        handle,
                        expected - 1,
                        start,
                        end,
                        replacement.as_ptr(),
                        replacement.len(),
                        &mut revision,
                        ptr::null_mut()
                    ),
                    CRIBRA_OK
                );
                assert_eq!(revision, expected);
                assert!(active(handle));
            }
            cribra_incremental_free(handle);
            cribra_scanner_free(scanner);
        }
    }
}

#[test]
fn stale_revision_ranges_and_scalar_boundaries_are_recoverable_with_core_precedence() {
    unsafe {
        let scanner = scanner(&[]);
        let handle = session(scanner, "é🙂SECRET", CribraIncrementalLimits::default());
        let before = snapshot(handle);
        let mut core = (*scanner)
            .inner
            .incremental_session("é🙂SECRET".into())
            .unwrap();
        for (expected_revision, start, end, status) in [
            (1, 999, 0, CRIBRA_INCREMENTAL_STALE_REVISION),
            (0, 5, 2, CRIBRA_INCREMENTAL_INVALID_RANGE),
            (0, 0, 99, CRIBRA_INCREMENTAL_INVALID_RANGE),
            (0, 1, 1, CRIBRA_INCREMENTAL_INVALID_UTF8_BOUNDARY),
            (0, 2, 3, CRIBRA_INCREMENTAL_INVALID_UTF8_BOUNDARY),
        ] {
            let expected = core
                .apply_edit(expected_revision, start..end, "x")
                .unwrap_err();
            assert_eq!(crate::incremental::incremental_status(expected), status);
            let mut revision = u64::MAX;
            let mut error = ptr::null_mut();
            assert_eq!(
                cribra_incremental_apply_edit(
                    handle,
                    expected_revision,
                    start,
                    end,
                    b"x".as_ptr(),
                    1,
                    &mut revision,
                    &mut error
                ),
                status
            );
            assert_eq!(revision, 0);
            diagnostic(error, status);
            assert!(active(handle));
            let after = snapshot(handle);
            assert_same_snapshot(&before, &after);
            cribra_report_free(after.report);
        }
        let mut revision = 0;
        assert_eq!(
            cribra_incremental_apply_edit(
                handle,
                0,
                0,
                2,
                b"E".as_ptr(),
                1,
                &mut revision,
                ptr::null_mut()
            ),
            CRIBRA_OK
        );
        assert_eq!(revision, 1);
        cribra_report_free(before.report);
        cribra_incremental_free(handle);
        cribra_scanner_free(scanner);
    }
}

#[test]
fn every_resource_limit_is_enforced_during_construction_without_partial_handles() {
    unsafe {
        let scanner = scanner(&[("literal", CRIBRA_RULE_KIND_LITERAL, "x")]);
        for (limits, core_limits, status) in [
            (
                CribraIncrementalLimits {
                    max_candidates_enabled: 1,
                    ..Default::default()
                },
                cribra::IncrementalLimits::default().with_max_candidates(Some(0)),
                CRIBRA_INCREMENTAL_CANDIDATE_LIMIT_EXCEEDED,
            ),
            (
                CribraIncrementalLimits {
                    max_retained_cache_records_enabled: 1,
                    ..Default::default()
                },
                cribra::IncrementalLimits::default().with_max_retained_cache_records(Some(0)),
                CRIBRA_INCREMENTAL_CACHE_LIMIT_EXCEEDED,
            ),
            (
                CribraIncrementalLimits {
                    max_materialized_results_enabled: 1,
                    ..Default::default()
                },
                cribra::IncrementalLimits::default().with_max_materialized_results(Some(0)),
                CRIBRA_INCREMENTAL_RESULT_LIMIT_EXCEEDED,
            ),
            (
                CribraIncrementalLimits {
                    max_transaction_storage_bytes_enabled: 1,
                    ..Default::default()
                },
                cribra::IncrementalLimits::default().with_max_transaction_storage_bytes(Some(0)),
                CRIBRA_INCREMENTAL_TRANSACTION_STORAGE_LIMIT_EXCEEDED,
            ),
        ] {
            let expected = (*scanner)
                .inner
                .incremental_session_with_limits("x".into(), core_limits)
                .unwrap_err();
            assert_eq!(crate::incremental::incremental_status(expected), status);
            let mut handle = ptr::dangling_mut::<CribraIncremental>();
            let mut error = ptr::null_mut();
            assert_eq!(
                cribra_scanner_incremental_new_with_limits(
                    scanner,
                    b"x".as_ptr(),
                    1,
                    &limits,
                    &mut handle,
                    &mut error
                ),
                status
            );
            assert!(handle.is_null());
            diagnostic(error, status);
        }
        cribra_scanner_free(scanner);
    }
}

#[test]
fn terminal_edit_failures_preserve_last_committed_snapshot_for_every_resource_limit() {
    for (limits, core_limits, initial, replacement, status) in [
        (
            CribraIncrementalLimits {
                max_candidates_enabled: 1,
                max_candidates: 1,
                ..Default::default()
            },
            cribra::IncrementalLimits::default().with_max_candidates(Some(1)),
            "x",
            "xx",
            CRIBRA_INCREMENTAL_CANDIDATE_LIMIT_EXCEEDED,
        ),
        (
            CribraIncrementalLimits {
                max_retained_cache_records_enabled: 1,
                max_retained_cache_records: 1,
                ..Default::default()
            },
            cribra::IncrementalLimits::default().with_max_retained_cache_records(Some(1)),
            "x",
            "",
            CRIBRA_INCREMENTAL_CACHE_LIMIT_EXCEEDED,
        ),
        (
            CribraIncrementalLimits {
                max_materialized_results_enabled: 1,
                max_materialized_results: 1,
                ..Default::default()
            },
            cribra::IncrementalLimits::default().with_max_materialized_results(Some(1)),
            "x",
            "x",
            CRIBRA_INCREMENTAL_RESULT_LIMIT_EXCEEDED,
        ),
        (
            CribraIncrementalLimits {
                max_transaction_storage_bytes_enabled: 1,
                ..Default::default()
            },
            cribra::IncrementalLimits::default().with_max_transaction_storage_bytes(Some(0)),
            "",
            "a",
            CRIBRA_INCREMENTAL_TRANSACTION_STORAGE_LIMIT_EXCEEDED,
        ),
    ] {
        unsafe {
            let rules = if initial.is_empty() {
                vec![]
            } else {
                vec![("literal", CRIBRA_RULE_KIND_LITERAL, "x")]
            };
            let scanner = scanner(&rules);
            let mut core = (*scanner)
                .inner
                .incremental_session_with_limits(initial.into(), core_limits)
                .unwrap();
            let handle = session(scanner, initial, limits);
            // Publish a nonzero revision for the zero-storage empty document.
            if initial.is_empty() {
                core.apply_edit(0, 0..0, "").unwrap();
                let mut revision = 0;
                assert_eq!(
                    cribra_incremental_apply_edit(
                        handle,
                        0,
                        0,
                        0,
                        ptr::null(),
                        0,
                        &mut revision,
                        ptr::null_mut()
                    ),
                    CRIBRA_OK
                );
                assert_eq!(revision, 1);
            }
            let before = snapshot(handle);
            let expected = core
                .apply_edit(core.revision(), 0..0, replacement)
                .unwrap_err();
            assert_eq!(crate::incremental::incremental_status(expected), status);
            assert!(!core.is_active());
            let mut revision = u64::MAX;
            let mut error = ptr::null_mut();
            assert_eq!(
                cribra_incremental_apply_edit(
                    handle,
                    before.revision,
                    0,
                    0,
                    replacement.as_ptr(),
                    replacement.len(),
                    &mut revision,
                    &mut error
                ),
                status
            );
            assert_eq!(revision, 0);
            diagnostic(error, status);
            assert!(!active(handle));
            assert!(
                (*handle).inner.is_some(),
                "ordinary terminal failure must retain Core"
            );
            let after = snapshot(handle);
            assert_same_snapshot(&before, &after);
            assert_eq!(&(*after.report).inner, core.report());
            // NotActive precedes stale revision and bad ranges for valid UTF-8.
            assert_eq!(
                cribra_incremental_apply_edit(
                    handle,
                    u64::MAX,
                    999,
                    0,
                    ptr::null(),
                    0,
                    &mut revision,
                    &mut error
                ),
                CRIBRA_INCREMENTAL_NOT_ACTIVE
            );
            diagnostic(error, CRIBRA_INCREMENTAL_NOT_ACTIVE);
            let again = snapshot(handle);
            assert_same_snapshot(&before, &again);
            cribra_scanner_free(scanner);
            cribra_incremental_free(handle);
            assert_eq!((*after.report).source_bytes, after.source_bytes);
            cribra_report_free(before.report);
            cribra_report_free(after.report);
            cribra_report_free(again.report);
        }
    }
}

#[test]
fn session_owns_document_and_snapshots_outlive_scanner_edits_and_session() {
    unsafe {
        let scanner = scanner(&[("literal", CRIBRA_RULE_KIND_LITERAL, "SECRET")]);
        let mut input = b"SECRET\nABCD-EFGH-IJKL-MNOP".to_vec();
        let mut handle = ptr::null_mut();
        assert_eq!(
            cribra_scanner_incremental_new(
                scanner,
                input.as_ptr(),
                input.len(),
                &mut handle,
                ptr::null_mut()
            ),
            CRIBRA_OK
        );
        input.fill(b'x');
        drop(input);
        cribra_scanner_free(scanner);
        let before = snapshot(handle);
        let second = snapshot(handle);
        assert_ne!(before.report, second.report);
        assert_same_snapshot(&before, &second);
        assert_eq!((*before.report).inner.len(), 1);
        assert_eq!((*before.report).inner.candidate_len(), 1);
        let mut revision = 0;
        assert_eq!(
            cribra_incremental_apply_edit(
                handle,
                0,
                0,
                6,
                ptr::null(),
                0,
                &mut revision,
                ptr::null_mut()
            ),
            CRIBRA_OK
        );
        let after = snapshot(handle);
        assert_eq!(
            (after.revision, after.source_bytes),
            (1, before.source_bytes - 6)
        );
        assert!((*after.report).inner.is_empty());
        assert_eq!((*after.report).inner.candidate_len(), 1);
        cribra_incremental_free(handle);
        assert_eq!((*before.report).inner.len(), 1);
        let mut output = ptr::null_mut();
        let original = b"SECRET\nABCD-EFGH-IJKL-MNOP";
        assert_eq!(
            cribra_transform_redact(
                original.as_ptr(),
                original.len(),
                before.report,
                &mut output,
                ptr::null_mut()
            ),
            CRIBRA_OK
        );
        cribra_output_free(output);
        assert_eq!(
            cribra_transform_redact(
                original.as_ptr(),
                original.len() - 1,
                before.report,
                &mut output,
                ptr::null_mut()
            ),
            CRIBRA_TRANSFORM_ERROR
        );
        assert!(output.is_null());
        cribra_report_free(before.report);
        cribra_report_free(second.report);
        cribra_report_free(after.report);
    }
}

#[test]
fn pointer_and_utf8_preflight_do_not_change_the_session_or_expose_reports() {
    unsafe {
        let scanner = scanner(&[]);
        let mut handle = ptr::dangling_mut::<CribraIncremental>();
        let mut error = ptr::null_mut();
        let malformed = [0xff];
        for (bytes, len, status) in [
            (ptr::null(), 1, CRIBRA_INVALID_ARGUMENT),
            (malformed.as_ptr(), 1, CRIBRA_INVALID_UTF8),
            (b"x".as_ptr(), usize::MAX, CRIBRA_INVALID_ARGUMENT),
        ] {
            assert_eq!(
                cribra_scanner_incremental_new(scanner, bytes, len, &mut handle, &mut error),
                status
            );
            assert!(handle.is_null());
            diagnostic(error, status);
        }
        assert_eq!(
            cribra_scanner_incremental_new(
                ptr::null(),
                ptr::null(),
                0,
                &mut handle,
                ptr::null_mut()
            ),
            CRIBRA_INVALID_ARGUMENT
        );
        assert!(handle.is_null());
        assert_eq!(
            cribra_scanner_incremental_new(
                scanner,
                ptr::null(),
                0,
                ptr::null_mut(),
                ptr::null_mut()
            ),
            CRIBRA_INVALID_ARGUMENT
        );
        assert_eq!(
            cribra_scanner_incremental_new_with_limits(
                scanner,
                ptr::null(),
                0,
                ptr::null(),
                &mut handle,
                ptr::null_mut()
            ),
            CRIBRA_INVALID_ARGUMENT
        );
        assert!(handle.is_null());
        let handle = session(scanner, "éSECRET", CribraIncrementalLimits::default());
        let before = snapshot(handle);
        let mut revision = u64::MAX;
        assert_eq!(
            cribra_incremental_apply_edit(
                ptr::null_mut(),
                0,
                0,
                0,
                ptr::null(),
                0,
                &mut revision,
                ptr::null_mut()
            ),
            CRIBRA_INVALID_ARGUMENT
        );
        assert_eq!(revision, 0);
        assert_eq!(
            cribra_incremental_apply_edit(
                handle,
                0,
                0,
                0,
                ptr::null(),
                0,
                ptr::null_mut(),
                ptr::null_mut()
            ),
            CRIBRA_INVALID_ARGUMENT
        );
        for (bytes, len, status) in [
            (ptr::null(), 1, CRIBRA_INVALID_ARGUMENT),
            (malformed.as_ptr(), 1, CRIBRA_INVALID_UTF8),
            (b"x".as_ptr(), usize::MAX, CRIBRA_INVALID_ARGUMENT),
        ] {
            revision = u64::MAX;
            assert_eq!(
                cribra_incremental_apply_edit(
                    handle,
                    u64::MAX,
                    999,
                    0,
                    bytes,
                    len,
                    &mut revision,
                    &mut error
                ),
                status
            );
            assert_eq!(revision, 0);
            diagnostic(error, status);
        }
        let mut out = CribraIncrementalSnapshot {
            revision: 99,
            source_bytes: 99,
            report: ptr::dangling_mut(),
        };
        assert_eq!(
            cribra_incremental_snapshot(ptr::null_mut(), &mut out, &mut error),
            CRIBRA_INVALID_ARGUMENT
        );
        assert_eq!((out.revision, out.source_bytes), (0, 0));
        assert!(out.report.is_null());
        diagnostic(error, CRIBRA_INVALID_ARGUMENT);
        assert_eq!(
            cribra_incremental_snapshot(handle, ptr::null_mut(), ptr::null_mut()),
            CRIBRA_INVALID_ARGUMENT
        );
        let mut out_active = u32::MAX;
        assert_eq!(
            cribra_incremental_is_active(ptr::null_mut(), &mut out_active, ptr::null_mut()),
            CRIBRA_INVALID_ARGUMENT
        );
        assert_eq!(out_active, 0);
        assert_eq!(
            cribra_incremental_is_active(handle, ptr::null_mut(), ptr::null_mut()),
            CRIBRA_INVALID_ARGUMENT
        );
        assert!(active(handle));
        let after = snapshot(handle);
        assert_same_snapshot(&before, &after);
        assert_eq!(
            cribra_incremental_apply_edit(
                handle,
                0,
                0,
                2,
                b"E".as_ptr(),
                1,
                &mut revision,
                ptr::null_mut()
            ),
            CRIBRA_OK
        );
        assert_eq!(revision, 1);
        cribra_report_free(before.report);
        cribra_report_free(after.report);
        cribra_incremental_free(handle);
        cribra_scanner_free(scanner);
    }
}

#[test]
fn enabled_flags_are_validated_and_disabled_values_are_ignored() {
    unsafe {
        let scanner = scanner(&[]);
        for invalid in [2, u32::MAX] {
            for limits in [
                CribraIncrementalLimits {
                    max_candidates_enabled: invalid,
                    ..Default::default()
                },
                CribraIncrementalLimits {
                    max_retained_cache_records_enabled: invalid,
                    ..Default::default()
                },
                CribraIncrementalLimits {
                    max_materialized_results_enabled: invalid,
                    ..Default::default()
                },
                CribraIncrementalLimits {
                    max_transaction_storage_bytes_enabled: invalid,
                    ..Default::default()
                },
            ] {
                let mut handle = ptr::dangling_mut::<CribraIncremental>();
                assert_eq!(
                    cribra_scanner_incremental_new_with_limits(
                        scanner,
                        ptr::null(),
                        0,
                        &limits,
                        &mut handle,
                        ptr::null_mut()
                    ),
                    CRIBRA_INVALID_ARGUMENT
                );
                assert!(handle.is_null());
            }
        }
        for value in [0, usize::MAX] {
            let limits = CribraIncrementalLimits {
                max_candidates: value,
                max_retained_cache_records: value,
                max_materialized_results: value,
                max_transaction_storage_bytes: value,
                ..Default::default()
            };
            let handle = session(scanner, "ABCD-EFGH-IJKL-MNOP", limits);
            let mut revision = 0;
            assert_eq!(
                cribra_incremental_apply_edit(
                    handle,
                    0,
                    0,
                    0,
                    b"x".as_ptr(),
                    1,
                    &mut revision,
                    ptr::null_mut()
                ),
                CRIBRA_OK
            );
            assert_eq!(revision, 1);
            cribra_incremental_free(handle);
        }
        cribra_scanner_free(scanner);
    }
}

#[test]
fn contained_guard_panic_invalidates_core_and_never_exposes_a_possibly_mutated_snapshot() {
    use crate::error::{ErrorSlot, contain_status_with_error};
    unsafe {
        let scanner = scanner(&[]);
        let handle = session(scanner, "SECRET", CribraIncrementalLimits::default());
        let before = snapshot(handle);
        let mut error = ptr::null_mut();
        let slot = ErrorSlot::from_raw(&mut error);
        // Inject at the same guard used by edit, snapshot and activity entry points.
        // Core has committed an edit before the panic; exposing it afterward would
        // incorrectly promise panic recovery. No Core internals or test hooks change.
        let status = contain_status_with_error(slot, || {
            (*handle)
                .with_core(|core| -> CribraStatus {
                    core.apply_edit(0, 0..0, "DO_NOT_EXPOSE").unwrap();
                    panic!("SECRET DO_NOT_EXPOSE");
                })
                .unwrap_or_else(|status| status)
        });
        assert_eq!(status, CRIBRA_INTERNAL_ERROR);
        diagnostic(error, CRIBRA_INTERNAL_ERROR);
        assert!((*handle).inner.is_none());
        assert!(!active(handle));
        let mut revision = u64::MAX;
        assert_eq!(
            cribra_incremental_apply_edit(
                handle,
                1,
                0,
                0,
                ptr::null(),
                0,
                &mut revision,
                &mut error
            ),
            CRIBRA_INCREMENTAL_SESSION_INVALIDATED
        );
        assert_eq!(revision, 0);
        diagnostic(error, CRIBRA_INCREMENTAL_SESSION_INVALIDATED);
        for _ in 0..2 {
            let mut out = CribraIncrementalSnapshot {
                revision: 99,
                source_bytes: 99,
                report: ptr::dangling_mut(),
            };
            assert_eq!(
                cribra_incremental_snapshot(handle, &mut out, &mut error),
                CRIBRA_INCREMENTAL_SESSION_INVALIDATED
            );
            assert_eq!((out.revision, out.source_bytes), (0, 0));
            assert!(out.report.is_null());
            diagnostic(error, CRIBRA_INCREMENTAL_SESSION_INVALIDATED);
        }
        assert_eq!((before.revision, before.source_bytes), (0, 6));
        cribra_incremental_free(handle);
        cribra_report_free(before.report);
        cribra_scanner_free(scanner);
    }
}

#[test]
fn every_core_error_including_rare_overflows_preserves_distinct_status_assignments() {
    use cribra::IncrementalError as Core;
    let errors = [
        Core::NotActive,
        Core::StaleRevision {
            expected: 0,
            actual: 1,
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
    ];
    // Rare overflow/invariant cases are conversion checks, not manufactured Core
    // execution campaigns. The guard restores all normally returned errors alike.
    for (index, error) in errors.into_iter().enumerate() {
        let expected = 19 + index as u32;
        assert_eq!(crate::incremental::incremental_status(error), expected);
        unsafe {
            let scanner = scanner(&[]);
            let handle = session(scanner, "", CribraIncrementalLimits::default());
            let result = (*handle).with_core(|_| Err::<(), _>(error)).unwrap();
            assert_eq!(result, Err(error));
            assert!(active(handle));
            let current = snapshot(handle);
            assert_eq!((current.revision, current.source_bytes), (0, 0));
            cribra_report_free(current.report);
            cribra_incremental_free(handle);
            cribra_scanner_free(scanner);
        }
    }
    assert_eq!(CRIBRA_INCREMENTAL_UNKNOWN_ERROR, 31);
    assert_eq!(CRIBRA_INCREMENTAL_SESSION_INVALIDATED, 32);
    assert_eq!(
        [
            CRIBRA_OK,
            CRIBRA_INVALID_ARGUMENT,
            CRIBRA_INVALID_UTF8,
            CRIBRA_RULE_ERROR,
            CRIBRA_BUILD_ERROR,
            CRIBRA_TRANSFORM_ERROR,
            CRIBRA_OUT_OF_RANGE,
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
        [
            0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18
        ]
    );
    assert_eq!(CRIBRA_INTERNAL_ERROR, 255);
}
