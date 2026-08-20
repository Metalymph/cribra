//! Native ABI entry points.

use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    ptr, slice, str,
};

use cribra::{Confidence, Remediation, Scanner, Severity};

use crate::{
    CRIBRA_BUILD_ERROR, CRIBRA_CONFIDENCE_HIGH, CRIBRA_CONFIDENCE_LOW, CRIBRA_CONFIDENCE_MEDIUM,
    CRIBRA_INTERNAL_ERROR, CRIBRA_INVALID_ARGUMENT, CRIBRA_INVALID_UTF8, CRIBRA_OK,
    CRIBRA_OUT_OF_RANGE, CRIBRA_REMEDIATION_NONE, CRIBRA_REMEDIATION_REMOVE_SENSITIVE_VALUE,
    CRIBRA_REMEDIATION_REPLACE_PRIVATE_KEY, CRIBRA_REMEDIATION_REVIEW_SENSITIVE_HASH,
    CRIBRA_REMEDIATION_REVOKE_AND_ROTATE_CREDENTIAL, CRIBRA_REMEDIATION_ROTATE_CREDENTIAL,
    CRIBRA_REMEDIATION_ROTATE_PASSWORD, CRIBRA_REMEDIATION_UNKNOWN, CRIBRA_SEVERITY_CRITICAL,
    CRIBRA_SEVERITY_HIGH, CRIBRA_SEVERITY_INFO, CRIBRA_SEVERITY_LOW, CRIBRA_SEVERITY_MEDIUM,
    CribraBuilder, CribraConfidence, CribraFindingView, CribraRemediation, CribraReport,
    CribraScanner, CribraSeverity, CribraStatus, CribraStringView,
};

/// Experimental native ABI major version.
pub const ABI_VERSION_MAJOR: u32 = 0;
/// Experimental native ABI minor version.
pub const ABI_VERSION_MINOR: u32 = 1;
/// Experimental native ABI patch version.
pub const ABI_VERSION_PATCH: u32 = 0;

fn contain_status(operation: impl FnOnce() -> CribraStatus) -> CribraStatus {
    catch_unwind(AssertUnwindSafe(operation)).unwrap_or(CRIBRA_INTERNAL_ERROR)
}

fn contain_drop(operation: impl FnOnce()) {
    let _ = catch_unwind(AssertUnwindSafe(operation));
}

unsafe fn clear_out<T>(out: *mut *mut T) -> Result<(), CribraStatus> {
    if out.is_null() {
        return Err(CRIBRA_INVALID_ARGUMENT);
    }
    // SAFETY: the caller contract requires `out` to reference writable memory.
    unsafe { ptr::write(out, ptr::null_mut()) };
    Ok(())
}

unsafe fn clear_value<T: Default>(out: *mut T) -> Result<(), CribraStatus> {
    if out.is_null() {
        return Err(CRIBRA_INVALID_ARGUMENT);
    }
    // SAFETY: the caller contract requires `out` to reference writable memory.
    unsafe { ptr::write(out, T::default()) };
    Ok(())
}

unsafe fn source_from_raw<'a>(
    source: *const u8,
    source_len: usize,
) -> Result<&'a str, CribraStatus> {
    if source_len == 0 {
        return Ok("");
    }
    if source.is_null() {
        return Err(CRIBRA_INVALID_ARGUMENT);
    }

    // SAFETY: caller guarantees `source_len` readable bytes for this call.
    let bytes = unsafe { slice::from_raw_parts(source, source_len) };
    str::from_utf8(bytes).map_err(|_| CRIBRA_INVALID_UTF8)
}

fn string_view(value: &str) -> CribraStringView {
    CribraStringView {
        ptr: value.as_ptr(),
        len: value.len(),
    }
}

fn severity_code(value: Severity) -> CribraSeverity {
    match value {
        Severity::Info => CRIBRA_SEVERITY_INFO,
        Severity::Low => CRIBRA_SEVERITY_LOW,
        Severity::Medium => CRIBRA_SEVERITY_MEDIUM,
        Severity::High => CRIBRA_SEVERITY_HIGH,
        Severity::Critical => CRIBRA_SEVERITY_CRITICAL,
    }
}

fn confidence_code(value: Confidence) -> CribraConfidence {
    match value {
        Confidence::Low => CRIBRA_CONFIDENCE_LOW,
        Confidence::Medium => CRIBRA_CONFIDENCE_MEDIUM,
        Confidence::High => CRIBRA_CONFIDENCE_HIGH,
    }
}

fn remediation_code(value: Option<Remediation>) -> CribraRemediation {
    match value {
        None => CRIBRA_REMEDIATION_NONE,
        Some(Remediation::RevokeAndRotateCredential) => {
            CRIBRA_REMEDIATION_REVOKE_AND_ROTATE_CREDENTIAL
        }
        Some(Remediation::RotateCredential) => CRIBRA_REMEDIATION_ROTATE_CREDENTIAL,
        Some(Remediation::RotatePassword) => CRIBRA_REMEDIATION_ROTATE_PASSWORD,
        Some(Remediation::ReplacePrivateKey) => CRIBRA_REMEDIATION_REPLACE_PRIVATE_KEY,
        Some(Remediation::RemoveSensitiveValue) => CRIBRA_REMEDIATION_REMOVE_SENSITIVE_VALUE,
        Some(Remediation::ReviewSensitiveHash) => CRIBRA_REMEDIATION_REVIEW_SENSITIVE_HASH,
        Some(_) => CRIBRA_REMEDIATION_UNKNOWN,
    }
}

/// Returns the native ABI major version.
#[unsafe(no_mangle)]
pub extern "C" fn cribra_abi_version_major() -> u32 {
    ABI_VERSION_MAJOR
}

/// Returns the native ABI minor version.
#[unsafe(no_mangle)]
pub extern "C" fn cribra_abi_version_minor() -> u32 {
    ABI_VERSION_MINOR
}

/// Returns the native ABI patch version.
#[unsafe(no_mangle)]
pub extern "C" fn cribra_abi_version_patch() -> u32 {
    ABI_VERSION_PATCH
}

/// Creates an empty scanner builder.
///
/// # Safety
///
/// `out_builder` must point to writable memory for one builder pointer. The
/// returned handle must eventually be released with [`cribra_builder_free`] or
/// consumed by [`cribra_builder_build`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cribra_builder_new(out_builder: *mut *mut CribraBuilder) -> CribraStatus {
    contain_status(|| {
        if let Err(status) = unsafe { clear_out(out_builder) } {
            return status;
        }

        let builder = Box::new(CribraBuilder::empty());
        // SAFETY: `clear_out` established a non-null writable out-pointer.
        unsafe { ptr::write(out_builder, Box::into_raw(builder)) };
        CRIBRA_OK
    })
}

/// Consumes a builder and compiles an immutable scanner.
///
/// The builder is consumed by every non-null build attempt.
///
/// # Safety
///
/// `builder` must be a live handle returned by [`cribra_builder_new`] and must
/// not be used after this call. `out_scanner` must point to writable memory for
/// one scanner pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cribra_builder_build(
    builder: *mut CribraBuilder,
    out_scanner: *mut *mut CribraScanner,
) -> CribraStatus {
    contain_status(|| {
        if let Err(status) = unsafe { clear_out(out_scanner) } {
            return status;
        }
        if builder.is_null() {
            return CRIBRA_INVALID_ARGUMENT;
        }

        // SAFETY: caller provides unique ownership of a live builder handle.
        let mut builder = unsafe { Box::from_raw(builder) };
        let Some(inner) = builder.inner.take() else {
            return CRIBRA_INVALID_ARGUMENT;
        };

        match inner.build() {
            Ok(scanner) => {
                let scanner = Box::new(CribraScanner::new(scanner));
                // SAFETY: `clear_out` validated the out-pointer.
                unsafe { ptr::write(out_scanner, Box::into_raw(scanner)) };
                CRIBRA_OK
            }
            Err(_) => CRIBRA_BUILD_ERROR,
        }
    })
}

/// Releases a scanner builder. Passing null is a no-op.
///
/// # Safety
///
/// A non-null `builder` must be a live builder handle that has not already been
/// consumed or freed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cribra_builder_free(builder: *mut CribraBuilder) {
    if builder.is_null() {
        return;
    }

    contain_drop(|| {
        // SAFETY: caller transfers ownership of one live handle.
        drop(unsafe { Box::from_raw(builder) });
    });
}

/// Creates a scanner using Cribra's current canonical built-in catalog.
///
/// # Safety
///
/// `out_scanner` must point to writable memory for one scanner pointer. The
/// returned handle must eventually be released with [`cribra_scanner_free`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cribra_scanner_new_current(
    out_scanner: *mut *mut CribraScanner,
) -> CribraStatus {
    contain_status(|| {
        if let Err(status) = unsafe { clear_out(out_scanner) } {
            return status;
        }

        let scanner = Box::new(CribraScanner::new(Scanner::default()));
        // SAFETY: `clear_out` validated the out-pointer.
        unsafe { ptr::write(out_scanner, Box::into_raw(scanner)) };
        CRIBRA_OK
    })
}

/// Scans one caller-owned UTF-8 source and returns an owned report.
///
/// The source is borrowed only for this call and is never retained. Null plus a
/// zero length is accepted as an empty source.
///
/// # Safety
///
/// `scanner` must be a live scanner handle. For non-empty input, `source` must
/// reference at least `source_len` readable bytes for the duration of this call.
/// `out_report` must point to writable memory for one report pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cribra_scanner_scan(
    scanner: *const CribraScanner,
    source: *const u8,
    source_len: usize,
    out_report: *mut *mut CribraReport,
) -> CribraStatus {
    contain_status(|| {
        if let Err(status) = unsafe { clear_out(out_report) } {
            return status;
        }
        if scanner.is_null() {
            return CRIBRA_INVALID_ARGUMENT;
        }

        let source = match unsafe { source_from_raw(source, source_len) } {
            Ok(source) => source,
            Err(status) => return status,
        };

        // SAFETY: caller guarantees a live immutable scanner handle.
        let scanner = unsafe { &*scanner };

        let mut entries = scanner.inner.scan([((), source)]).into_inner();
        let (_, _, report) = entries
            .pop()
            .expect("single-source adapter must produce exactly one result")
            .into_parts();
        debug_assert!(entries.is_empty());

        let report = Box::new(CribraReport::new(report));
        // SAFETY: `clear_out` validated the out-pointer.
        unsafe { ptr::write(out_report, Box::into_raw(report)) };
        CRIBRA_OK
    })
}

/// Returns the number of classified findings in a report.
///
/// Candidates are intentionally excluded from this count and receive their own
/// API in v0.3.5.
///
/// # Safety
///
/// `report` must be a live report handle. `out_count` must point to writable
/// memory for one `size_t`-compatible value.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cribra_report_finding_count(
    report: *const CribraReport,
    out_count: *mut usize,
) -> CribraStatus {
    contain_status(|| {
        if let Err(status) = unsafe { clear_value(out_count) } {
            return status;
        }
        if report.is_null() {
            return CRIBRA_INVALID_ARGUMENT;
        }

        // SAFETY: caller guarantees a live immutable report handle.
        let report = unsafe { &*report };

        // SAFETY: `clear_value` validated the out-pointer.
        unsafe { ptr::write(out_count, report.inner.len()) };
        CRIBRA_OK
    })
}

/// Returns a borrowed projection of one classified finding by index.
///
/// The returned `rule_id` view borrows report-owned storage and remains valid
/// only while `report` remains alive and unmodified. Reports are immutable.
///
/// # Safety
///
/// `report` must be a live report handle. `out_finding` must point to writable
/// memory for one [`CribraFindingView`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cribra_report_finding_at(
    report: *const CribraReport,
    index: usize,
    out_finding: *mut CribraFindingView,
) -> CribraStatus {
    contain_status(|| {
        if let Err(status) = unsafe { clear_value(out_finding) } {
            return status;
        }
        if report.is_null() {
            return CRIBRA_INVALID_ARGUMENT;
        }

        // SAFETY: caller guarantees a live immutable report handle.
        let report = unsafe { &*report };
        let Some(finding) = report.inner.findings().get(index) else {
            return CRIBRA_OUT_OF_RANGE;
        };

        let location = finding.location();
        let view = CribraFindingView {
            rule_id: string_view(finding.rule_id().as_str()),
            start: location.start(),
            end: location.end(),
            line: location.line(),
            column: location.column(),
            severity: severity_code(finding.severity()),
            confidence: confidence_code(finding.confidence()),
            remediation: remediation_code(finding.remediation()),
        };

        // SAFETY: `clear_value` validated the out-pointer.
        unsafe { ptr::write(out_finding, view) };
        CRIBRA_OK
    })
}

/// Releases an immutable scanner. Passing null is a no-op.
///
/// # Safety
///
/// A non-null `scanner` must be a live scanner handle that has not already been
/// freed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cribra_scanner_free(scanner: *mut CribraScanner) {
    if scanner.is_null() {
        return;
    }

    contain_drop(|| {
        // SAFETY: caller transfers ownership of one live handle.
        drop(unsafe { Box::from_raw(scanner) });
    });
}

/// Releases an immutable report. Passing null is a no-op.
///
/// # Safety
///
/// A non-null `report` must be a live report handle that has not already been
/// freed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cribra_report_free(report: *mut CribraReport) {
    if report.is_null() {
        return;
    }

    contain_drop(|| {
        // SAFETY: caller transfers ownership of one live handle.
        drop(unsafe { Box::from_raw(report) });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn abi_version_is_independent_from_crate_version() {
        assert_eq!(
            (
                cribra_abi_version_major(),
                cribra_abi_version_minor(),
                cribra_abi_version_patch(),
            ),
            (0, 1, 0)
        );
    }

    #[test]
    fn current_scanner_scans_one_utf8_source() {
        let mut scanner = ptr::null_mut();
        let mut report = ptr::null_mut();
        let source = b"GITHUB_TOKEN=ghp_AbCdEf0123456789_AbCdEf0123456789";

        unsafe {
            assert_eq!(cribra_scanner_new_current(&mut scanner), CRIBRA_OK);
            assert_eq!(
                cribra_scanner_scan(scanner, source.as_ptr(), source.len(), &mut report),
                CRIBRA_OK
            );
            assert!(!report.is_null());
            cribra_report_free(report);
            cribra_scanner_free(scanner);
        }
    }

    #[test]
    fn empty_builder_builds_an_empty_scanner() {
        let mut builder = ptr::null_mut();
        let mut scanner = ptr::null_mut();

        unsafe {
            assert_eq!(cribra_builder_new(&mut builder), CRIBRA_OK);
            assert_eq!(cribra_builder_build(builder, &mut scanner), CRIBRA_OK);
            assert!(!scanner.is_null());
            cribra_scanner_free(scanner);
        }
    }

    #[test]
    fn invalid_utf8_is_rejected_without_a_report() {
        let mut scanner = ptr::null_mut();
        let mut report = ptr::dangling_mut::<CribraReport>();
        let source = [0xff_u8];

        unsafe {
            assert_eq!(cribra_scanner_new_current(&mut scanner), CRIBRA_OK);
            assert_eq!(
                cribra_scanner_scan(scanner, source.as_ptr(), source.len(), &mut report),
                CRIBRA_INVALID_UTF8
            );
            assert!(report.is_null());
            cribra_scanner_free(scanner);
        }
    }

    #[test]
    fn null_zero_length_source_is_empty_input() {
        let mut scanner = ptr::null_mut();
        let mut report = ptr::null_mut();

        unsafe {
            assert_eq!(cribra_scanner_new_current(&mut scanner), CRIBRA_OK);
            assert_eq!(
                cribra_scanner_scan(scanner, ptr::null(), 0, &mut report),
                CRIBRA_OK
            );
            assert!(!report.is_null());
            cribra_report_free(report);
            cribra_scanner_free(scanner);
        }
    }

    #[test]
    fn null_nonempty_source_is_rejected() {
        let mut scanner = ptr::null_mut();
        let mut report = ptr::dangling_mut::<CribraReport>();

        unsafe {
            assert_eq!(cribra_scanner_new_current(&mut scanner), CRIBRA_OK);
            assert_eq!(
                cribra_scanner_scan(scanner, ptr::null(), 1, &mut report),
                CRIBRA_INVALID_ARGUMENT
            );
            assert!(report.is_null());
            cribra_scanner_free(scanner);
        }
    }

    #[test]
    fn report_exposes_count_and_borrowed_finding_view() {
        let mut scanner = ptr::null_mut();
        let mut report = ptr::null_mut();
        let source = concat!(
            "alpha before\n",
            "STRIPE_SECRET_KEY=sk_live_AbCdEf0123456789_AbCdEf0123456789\n",
            "GITHUB_TOKEN=ghp_AbCdEf0123456789_AbCdEf0123456789\n",
        )
        .as_bytes();

        unsafe {
            assert_eq!(cribra_scanner_new_current(&mut scanner), CRIBRA_OK);
            assert_eq!(
                cribra_scanner_scan(scanner, source.as_ptr(), source.len(), &mut report),
                CRIBRA_OK
            );

            let mut count = usize::MAX;
            assert_eq!(cribra_report_finding_count(report, &mut count), CRIBRA_OK);
            assert!(count >= 2);

            let mut finding = CribraFindingView::default();
            assert_eq!(cribra_report_finding_at(report, 0, &mut finding), CRIBRA_OK);
            assert!(!finding.rule_id.ptr.is_null());
            assert!(finding.rule_id.len > 0);
            assert!(finding.start < finding.end);
            assert!(finding.line >= 1);
            assert!(finding.column >= 1);

            let id_bytes = slice::from_raw_parts(finding.rule_id.ptr, finding.rule_id.len);
            assert_eq!(str::from_utf8(id_bytes).unwrap(), "stripe.live-secret-key");

            cribra_report_free(report);
            cribra_scanner_free(scanner);
        }
    }

    #[test]
    fn report_out_of_range_fails_closed() {
        let mut scanner = ptr::null_mut();
        let mut report = ptr::null_mut();

        unsafe {
            assert_eq!(cribra_scanner_new_current(&mut scanner), CRIBRA_OK);
            assert_eq!(
                cribra_scanner_scan(scanner, ptr::null(), 0, &mut report),
                CRIBRA_OK
            );

            let mut finding = CribraFindingView {
                rule_id: CribraStringView {
                    ptr: ptr::dangling::<u8>(),
                    len: usize::MAX,
                },
                start: usize::MAX,
                end: usize::MAX,
                line: usize::MAX,
                column: usize::MAX,
                severity: u32::MAX,
                confidence: u32::MAX,
                remediation: u32::MAX,
            };

            assert_eq!(
                cribra_report_finding_at(report, 0, &mut finding),
                CRIBRA_OUT_OF_RANGE
            );
            assert!(finding.rule_id.ptr.is_null());
            assert_eq!(finding.rule_id.len, 0);
            assert_eq!(finding.start, 0);
            assert_eq!(finding.end, 0);

            cribra_report_free(report);
            cribra_scanner_free(scanner);
        }
    }

    #[test]
    fn stable_value_mappings_cover_current_core_variants() {
        assert_eq!(severity_code(Severity::Info), CRIBRA_SEVERITY_INFO);
        assert_eq!(severity_code(Severity::Critical), CRIBRA_SEVERITY_CRITICAL);
        assert_eq!(confidence_code(Confidence::Low), CRIBRA_CONFIDENCE_LOW);
        assert_eq!(confidence_code(Confidence::High), CRIBRA_CONFIDENCE_HIGH);
        assert_eq!(remediation_code(None), CRIBRA_REMEDIATION_NONE);
        assert_eq!(
            remediation_code(Some(Remediation::RemoveSensitiveValue)),
            CRIBRA_REMEDIATION_REMOVE_SENSITIVE_VALUE
        );
    }
}
