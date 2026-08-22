//! Ownership and lifetime regression tests for the native ABI.
//!
//! These tests deliberately exercise only documented-valid lifetimes. They do
//! not attempt use-after-free or double-free, which remain caller-contract
//! violations and therefore cannot be tested safely from Rust.

use std::{ptr, slice, str};

use crate::*;

fn string_view(value: &str) -> CribraStringView {
    CribraStringView {
        ptr: value.as_ptr(),
        len: value.len(),
    }
}

unsafe fn view_text(view: CribraStringView) -> String {
    if view.len == 0 {
        return String::new();
    }

    // SAFETY: callers use this helper only while the owning ABI handle remains
    // alive, so the borrowed view is valid for `len` bytes.
    let bytes = unsafe { slice::from_raw_parts(view.ptr, view.len) };
    str::from_utf8(bytes).unwrap().to_owned()
}

#[test]
fn every_owned_handle_accepts_null_free() {
    unsafe {
        cribra_builder_free(ptr::null_mut());
        cribra_scanner_free(ptr::null_mut());
        cribra_report_free(ptr::null_mut());
        cribra_batch_results_free(ptr::null_mut());
        cribra_output_free(ptr::null_mut());
        cribra_share_bundle_free(ptr::null_mut());
        cribra_error_free(ptr::null_mut());
    }
}

#[test]
fn failed_scan_clears_owned_output_before_returning_error() {
    let mut scanner = ptr::null_mut();
    let mut report = ptr::dangling_mut::<CribraReport>();
    let mut error = ptr::null_mut();
    let invalid_utf8 = [0xff_u8];

    unsafe {
        assert_eq!(cribra_scanner_new_current(&mut scanner), CRIBRA_OK);

        assert_eq!(
            cribra_scanner_scan(
                scanner,
                invalid_utf8.as_ptr(),
                invalid_utf8.len(),
                &mut report,
                &mut error,
            ),
            CRIBRA_INVALID_UTF8
        );

        assert!(report.is_null());
        assert!(!error.is_null());

        let mut status = u32::MAX;
        assert_eq!(cribra_error_status(error, &mut status), CRIBRA_OK);
        assert_eq!(status, CRIBRA_INVALID_UTF8);

        cribra_error_free(error);
        cribra_scanner_free(scanner);
    }
}

#[test]
fn transformed_output_owns_content_independently_of_report_and_source() {
    let mut builder = ptr::null_mut();
    let mut scanner = ptr::null_mut();
    let mut report = ptr::null_mut();
    let mut output = ptr::null_mut();
    let mut error = ptr::null_mut();

    let id = String::from("hardening.secret");
    let secret = String::from("OWNERSHIP_SECRET");
    let source = String::from("token=OWNERSHIP_SECRET");

    let config = CribraRuleConfig {
        kind: CRIBRA_RULE_KIND_LITERAL,
        id: string_view(&id),
        value: string_view(&secret),
        severity: CRIBRA_SEVERITY_HIGH,
        remediation: CRIBRA_REMEDIATION_NONE,
    };

    unsafe {
        assert_eq!(cribra_builder_new(&mut builder), CRIBRA_OK);
        assert_eq!(
            cribra_builder_add_rule(builder, &config, &mut error),
            CRIBRA_OK
        );
        assert!(error.is_null());

        assert_eq!(
            cribra_builder_build(builder, &mut scanner, &mut error),
            CRIBRA_OK
        );
        assert!(error.is_null());

        assert_eq!(
            cribra_scanner_scan(
                scanner,
                source.as_ptr(),
                source.len(),
                &mut report,
                &mut error,
            ),
            CRIBRA_OK
        );
        assert!(error.is_null());

        assert_eq!(
            cribra_transform_redact(
                source.as_ptr(),
                source.len(),
                report,
                &mut output,
                &mut error,
            ),
            CRIBRA_OK
        );
        assert!(error.is_null());
        assert!(!output.is_null());

        // The output must own its transformed content independently of these
        // inputs/parents once construction succeeds.
        cribra_report_free(report);
        cribra_scanner_free(scanner);
    }

    drop(source);
    drop(secret);
    drop(id);

    unsafe {
        let mut view = CribraStringView::default();
        assert_eq!(cribra_output_view(output, &mut view), CRIBRA_OK);
        assert_eq!(view_text(view), "token=[REDACTED]");
        cribra_output_free(output);
    }
}

#[test]
fn batch_results_own_keys_and_reports_but_not_input_sources() {
    let mut scanner = ptr::null_mut();
    let mut results = ptr::null_mut();
    let mut error = ptr::null_mut();

    let key_a = String::from("alpha");
    let key_b = String::from("beta");
    let source_a = String::from("ordinary input");
    let source_b = String::from("backup=ABCD-EFGH-IJKL-MNOP");

    let inputs = [
        CribraBatchInput {
            key: string_view(&key_a),
            source: string_view(&source_a),
        },
        CribraBatchInput {
            key: string_view(&key_b),
            source: string_view(&source_b),
        },
    ];

    unsafe {
        assert_eq!(cribra_scanner_new_current(&mut scanner), CRIBRA_OK);
        assert_eq!(
            cribra_scanner_scan_batch(
                scanner,
                inputs.as_ptr(),
                inputs.len(),
                CRIBRA_BATCH_EXECUTION_SERIAL,
                &mut results,
                &mut error,
            ),
            CRIBRA_OK
        );
        assert!(error.is_null());

        cribra_scanner_free(scanner);
    }

    drop(key_a);
    drop(key_b);
    drop(source_a);
    drop(source_b);

    unsafe {
        let mut count = usize::MAX;
        assert_eq!(cribra_batch_results_count(results, &mut count), CRIBRA_OK);
        assert_eq!(count, 2);

        let mut first = CribraBatchEntryView::default();
        let mut second = CribraBatchEntryView::default();

        assert_eq!(
            cribra_batch_results_entry_at(results, 0, &mut first),
            CRIBRA_OK
        );
        assert_eq!(
            cribra_batch_results_entry_at(results, 1, &mut second),
            CRIBRA_OK
        );

        assert_eq!(view_text(first.key), "alpha");
        assert_eq!(view_text(second.key), "beta");
        assert_eq!(second.candidate_count, 1);

        cribra_batch_results_free(results);
    }
}

#[test]
fn share_bundle_owns_transformed_sources_after_batch_and_inputs_are_released() {
    let mut scanner = ptr::null_mut();
    let mut results = ptr::null_mut();
    let mut bundle = ptr::null_mut();
    let mut error = ptr::null_mut();

    let key = String::from("clean.txt");
    let source = String::from("ordinary content");
    let input = CribraBatchInput {
        key: string_view(&key),
        source: string_view(&source),
    };

    unsafe {
        assert_eq!(cribra_scanner_new_current(&mut scanner), CRIBRA_OK);
        assert_eq!(
            cribra_scanner_scan_batch(
                scanner,
                &input,
                1,
                CRIBRA_BATCH_EXECUTION_SERIAL,
                &mut results,
                &mut error,
            ),
            CRIBRA_OK
        );
        assert!(error.is_null());

        let sources = [string_view(&source)];
        let config = CribraShareBundleConfig {
            mode: CRIBRA_SHARE_MODE_REDACT,
            ..CribraShareBundleConfig::default()
        };

        assert_eq!(
            cribra_share_bundle_build(
                results,
                sources.as_ptr(),
                sources.len(),
                &config,
                &mut bundle,
                &mut error,
            ),
            CRIBRA_OK
        );
        assert!(error.is_null());

        cribra_batch_results_free(results);
        cribra_scanner_free(scanner);
    }

    drop(key);
    drop(source);

    unsafe {
        let mut count = usize::MAX;
        assert_eq!(cribra_share_bundle_count(bundle, &mut count), CRIBRA_OK);
        assert_eq!(count, 1);

        let mut entry = CribraShareEntryView::default();
        assert_eq!(
            cribra_share_bundle_entry_at(bundle, 0, &mut entry),
            CRIBRA_OK
        );
        assert_eq!(view_text(entry.key), "clean.txt");
        assert_eq!(view_text(entry.content), "ordinary content");

        cribra_share_bundle_free(bundle);
    }
}

#[test]
fn error_message_is_borrowed_from_owned_error_until_error_free() {
    let mut scanner = ptr::null_mut();
    let mut report = ptr::null_mut();
    let mut error = ptr::null_mut();
    let invalid_utf8 = [0xff_u8];

    unsafe {
        assert_eq!(cribra_scanner_new_current(&mut scanner), CRIBRA_OK);

        assert_eq!(
            cribra_scanner_scan(
                scanner,
                invalid_utf8.as_ptr(),
                invalid_utf8.len(),
                &mut report,
                &mut error,
            ),
            CRIBRA_INVALID_UTF8
        );
        assert!(report.is_null());

        let mut message = CribraStringView::default();
        assert_eq!(cribra_error_message(error, &mut message), CRIBRA_OK);
        assert_eq!(view_text(message), "input is not valid UTF-8");

        cribra_error_free(error);
        cribra_scanner_free(scanner);
    }
}

fn assert_send_sync<T: Send + Sync>() {}

#[test]
fn immutable_owned_handle_types_are_send_and_sync() {
    assert_send_sync::<CribraScanner>();
    assert_send_sync::<CribraReport>();
    assert_send_sync::<CribraBatchResults>();
    assert_send_sync::<CribraOutput>();
    assert_send_sync::<CribraShareBundle>();
    assert_send_sync::<CribraError>();
}
