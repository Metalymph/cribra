//! Initial native ABI entry points.

use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    ptr, slice, str,
};

use cribra::Scanner;

use crate::{
    CRIBRA_BUILD_ERROR, CRIBRA_INTERNAL_ERROR, CRIBRA_INVALID_ARGUMENT, CRIBRA_INVALID_UTF8,
    CRIBRA_OK, CribraBuilder, CribraReport, CribraScanner, CribraStatus,
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
        let mut report = std::ptr::dangling_mut::<CribraReport>();
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
        let mut report = std::ptr::dangling_mut::<CribraReport>();

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
}
