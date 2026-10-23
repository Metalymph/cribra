//! Direct Core-owned incremental documents, transactional edits and snapshots.

use std::{ptr, slice, str};

use crate::{
    CribraError, CribraIncremental, CribraIncrementalLimits, CribraIncrementalSnapshot,
    CribraReport, CribraScanner, CribraStatus,
    error::{ErrorSlot, contain_status_with_error},
    ffi::{clear_out, clear_value, contain_drop},
    status::*,
};

pub(crate) fn incremental_status(error: cribra::IncrementalError) -> CribraStatus {
    use cribra::IncrementalError as Core;
    match error {
        Core::NotActive => CRIBRA_INCREMENTAL_NOT_ACTIVE,
        Core::StaleRevision { .. } => CRIBRA_INCREMENTAL_STALE_REVISION,
        Core::InvalidRange => CRIBRA_INCREMENTAL_INVALID_RANGE,
        Core::InvalidUtf8Boundary => CRIBRA_INCREMENTAL_INVALID_UTF8_BOUNDARY,
        Core::RevisionOverflow => CRIBRA_INCREMENTAL_REVISION_OVERFLOW,
        Core::DocumentLengthOverflow => CRIBRA_INCREMENTAL_DOCUMENT_LENGTH_OVERFLOW,
        Core::CandidateLimitExceeded { .. } => CRIBRA_INCREMENTAL_CANDIDATE_LIMIT_EXCEEDED,
        Core::CacheLimitExceeded { .. } => CRIBRA_INCREMENTAL_CACHE_LIMIT_EXCEEDED,
        Core::ResultLimitExceeded { .. } => CRIBRA_INCREMENTAL_RESULT_LIMIT_EXCEEDED,
        Core::TransactionStorageLimitExceeded { .. } => {
            CRIBRA_INCREMENTAL_TRANSACTION_STORAGE_LIMIT_EXCEEDED
        }
        Core::ResourceAccountingOverflow => CRIBRA_INCREMENTAL_RESOURCE_ACCOUNTING_OVERFLOW,
        Core::ExecutionInvariantViolated => CRIBRA_INCREMENTAL_EXECUTION_INVARIANT_VIOLATED,
        _ => CRIBRA_INCREMENTAL_UNKNOWN_ERROR,
    }
}

impl CribraIncrementalLimits {
    fn into_core(self) -> Result<cribra::IncrementalLimits, CribraStatus> {
        fn limit(enabled: u32, value: usize) -> Result<Option<usize>, CribraStatus> {
            match enabled {
                0 => Ok(None),
                1 => Ok(Some(value)),
                _ => Err(CRIBRA_INVALID_ARGUMENT),
            }
        }
        Ok(cribra::IncrementalLimits::default()
            .with_max_candidates(limit(self.max_candidates_enabled, self.max_candidates)?)
            .with_max_retained_cache_records(limit(
                self.max_retained_cache_records_enabled,
                self.max_retained_cache_records,
            )?)
            .with_max_materialized_results(limit(
                self.max_materialized_results_enabled,
                self.max_materialized_results,
            )?)
            .with_max_transaction_storage_bytes(limit(
                self.max_transaction_storage_bytes_enabled,
                self.max_transaction_storage_bytes,
            )?))
    }
}

// SAFETY: nonempty input must be readable for len bytes throughout the returned
// borrow. No raw borrow escapes an ABI call. Validate slice length before use.
unsafe fn input_utf8<'a>(bytes: *const u8, len: usize) -> Result<&'a str, CribraStatus> {
    if len > isize::MAX as usize || (len != 0 && bytes.is_null()) {
        return Err(CRIBRA_INVALID_ARGUMENT);
    }
    if len == 0 {
        return Ok("");
    }
    // SAFETY: caller guarantees readable memory; null and length checked above.
    str::from_utf8(unsafe { slice::from_raw_parts(bytes, len) }).map_err(|_| CRIBRA_INVALID_UTF8)
}

/// Creates a Core-owned incremental UTF-8 document at revision zero.
///
/// Copies the initial source once into Core-owned storage; no adapter source
/// buffer or edit engine is retained. The session may outlive its scanner.
/// Null source plus zero length is an empty document. Invalid UTF-8 is rejected
/// before invoking Core. This constructor leaves all optional budgets unlimited.
///
/// # Safety
///
/// `scanner` must be live and immutable. Nonempty source must reference source_len
/// readable bytes, with length at most PTRDIFF_MAX. `out_incremental` must be a
/// writable handle-pointer slot. Optional out_error must be writable, freed
/// before reuse. Inputs and output slots must not alias. No input borrow survives.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cribra_scanner_incremental_new(
    scanner: *const CribraScanner,
    source: *const u8,
    source_len: usize,
    out_incremental: *mut *mut CribraIncremental,
    out_error: *mut *mut CribraError,
) -> CribraStatus {
    let limits = CribraIncrementalLimits::default();
    // SAFETY: pointer contracts are forwarded; limits lives throughout the call.
    unsafe {
        cribra_scanner_incremental_new_with_limits(
            scanner,
            source,
            source_len,
            &limits,
            out_incremental,
            out_error,
        )
    }
}

/// Creates an incremental document with all four optional Core budgets.
///
/// Enabled flags must be 0 or 1; enabled zero is a real zero limit and disabled
/// values are ignored. Configuration is copied. Core defines construction,
/// resource accounting and error precedence after input/configuration validation.
/// Failure clears out_incremental and creates no handle or partial report.
/// Diagnostics contain no source, matcher text or panic payload.
///
/// # Safety
///
/// Same contracts as cribra_scanner_incremental_new. In addition, limits must
/// reference one readable CribraIncrementalLimits for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cribra_scanner_incremental_new_with_limits(
    scanner: *const CribraScanner,
    source: *const u8,
    source_len: usize,
    limits: *const CribraIncrementalLimits,
    out_incremental: *mut *mut CribraIncremental,
    out_error: *mut *mut CribraError,
) -> CribraStatus {
    // SAFETY: caller supplies an optional writable diagnostic slot.
    let error_slot = unsafe { ErrorSlot::from_raw(out_error) };
    contain_status_with_error(error_slot, || {
        // SAFETY: caller supplies a writable output slot when non-null.
        if let Err(status) = unsafe { clear_out(out_incremental) } {
            return status;
        }
        if scanner.is_null() || limits.is_null() {
            return CRIBRA_INVALID_ARGUMENT;
        }
        // SAFETY: caller guarantees readable configuration and source memory.
        let limits = match unsafe { *limits }.into_core() {
            Ok(limits) => limits,
            Err(status) => return status,
        };
        let source = match unsafe { input_utf8(source, source_len) } {
            Ok(source) => source,
            Err(status) => return status,
        };
        // SAFETY: caller guarantees a live immutable scanner.
        let scanner = unsafe { &*scanner };
        match scanner
            .inner
            .incremental_session_with_limits(source.to_owned(), limits)
        {
            Ok(core) => {
                let handle = Box::new(CribraIncremental::new(core));
                // SAFETY: clear_out validated the output slot.
                unsafe { ptr::write(out_incremental, Box::into_raw(handle)) };
                CRIBRA_OK
            }
            Err(error) => incremental_status(error),
        }
    })
}

/// Transactionally replaces the half-open UTF-8 byte range [start, end).
///
/// Core checks expected_revision and range/boundaries. Every successful edit,
/// including a no-op, increments revision once and writes out_revision. On failure
/// out_revision is zero; status distinguishes failure from a valid revision zero.
/// StaleRevision, InvalidRange, InvalidUtf8Boundary, RevisionOverflow and
/// DocumentLengthOverflow preserve activity. Resource/invariant errors preserve
/// the last snapshot but permanently block edits (CRIBRA_INCREMENTAL_NOT_ACTIVE).
/// Replacement UTF-8 is validated before invoking Core; invalid UTF-8 returns
/// CRIBRA_INVALID_UTF8 without changing the session, even if Core is terminal.
/// A contained execution panic returns CRIBRA_INTERNAL_ERROR and permanently
/// invalidates the Core session: later edits/snapshots return
/// CRIBRA_INCREMENTAL_SESSION_INVALIDATED. is_active then succeeds with zero.
///
/// # Safety
///
/// incremental must be live with exclusive access (no concurrent edit, snapshot,
/// is_active or free). Nonempty replacement must reference replacement_len readable
/// bytes, length at most PTRDIFF_MAX; null plus zero means empty replacement.
/// out_revision must be writable. Optional out_error must be writable, freed
/// before reuse. Inputs/output slots must not alias the handle or each other.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cribra_incremental_apply_edit(
    incremental: *mut CribraIncremental,
    expected_revision: u64,
    start: usize,
    end: usize,
    replacement: *const u8,
    replacement_len: usize,
    out_revision: *mut u64,
    out_error: *mut *mut CribraError,
) -> CribraStatus {
    // SAFETY: caller supplies an optional writable diagnostic slot.
    let error_slot = unsafe { ErrorSlot::from_raw(out_error) };
    contain_status_with_error(error_slot, || {
        // SAFETY: caller supplies a writable output slot when non-null.
        if let Err(status) = unsafe { clear_value(out_revision) } {
            return status;
        }
        if incremental.is_null() {
            return CRIBRA_INVALID_ARGUMENT;
        }
        // SAFETY: caller guarantees readable replacement bytes.
        let replacement = match unsafe { input_utf8(replacement, replacement_len) } {
            Ok(replacement) => replacement,
            Err(status) => return status,
        };
        // SAFETY: caller guarantees exclusive access to a live handle.
        let incremental = unsafe { &mut *incremental };
        incremental
            .with_core(|core| {
                match core.apply_edit(expected_revision, start..end, replacement) {
                    Ok(revision) => {
                        // SAFETY: clear_value validated the output slot.
                        unsafe { ptr::write(out_revision, revision) };
                        CRIBRA_OK
                    }
                    Err(error) => incremental_status(error),
                }
            })
            .unwrap_or_else(|status| status)
    })
}

/// Copies one coherent committed revision, byte count and complete owned report.
///
/// The snapshot is readable after ordinary Core terminal errors. It is independent
/// of future edits and the session/scanner lifetime. Free its report exactly once
/// with cribra_report_free, including before reusing the same output slot.
/// Failure clears the entire snapshot and never transfers a partial report.
/// No source getter is exported. A contained execution/report-cloning panic
/// permanently invalidates the Core session; later snapshots/edits are rejected.
///
/// # Safety
///
/// incremental must be live with exclusive access. out_snapshot must reference a
/// writable CribraIncrementalSnapshot. Optional out_error must be writable, freed
/// before reuse. Output slots must not alias the handle or each other.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cribra_incremental_snapshot(
    incremental: *mut CribraIncremental,
    out_snapshot: *mut CribraIncrementalSnapshot,
    out_error: *mut *mut CribraError,
) -> CribraStatus {
    // SAFETY: caller supplies an optional writable diagnostic slot.
    let error_slot = unsafe { ErrorSlot::from_raw(out_error) };
    contain_status_with_error(error_slot, || {
        // SAFETY: caller supplies a writable output slot when non-null.
        if let Err(status) = unsafe { clear_value(out_snapshot) } {
            return status;
        }
        if incremental.is_null() {
            return CRIBRA_INVALID_ARGUMENT;
        }
        // SAFETY: caller guarantees exclusive access to a live handle.
        let incremental = unsafe { &mut *incremental };
        incremental
            .with_core(|core| {
                let revision = core.revision();
                let source_bytes = core.source().len();
                let report = Box::new(CribraReport::new(core.report().clone(), source_bytes));
                let snapshot = CribraIncrementalSnapshot {
                    revision,
                    source_bytes,
                    report: Box::into_raw(report),
                };
                // SAFETY: clear_value validated the output slot.
                unsafe { ptr::write(out_snapshot, snapshot) };
                CRIBRA_OK
            })
            .unwrap_or_else(|status| status)
    })
}

/// Writes 1 if Core permits edits, or 0 after a terminal error or contained panic.
///
/// Returns CRIBRA_OK for a live handle even after panic invalidation. This is an
/// activity query, not an edit or snapshot. Never exports a Rust bool layout.
/// Failure clears out_active to zero. A panic during this query also invalidates
/// the Core session and returns CRIBRA_INTERNAL_ERROR.
///
/// # Safety
///
/// incremental must be live with exclusive access. out_active must reference one
/// writable uint32_t. Optional out_error must be writable, freed before reuse.
/// Output slots must not alias the handle or each other.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cribra_incremental_is_active(
    incremental: *mut CribraIncremental,
    out_active: *mut u32,
    out_error: *mut *mut CribraError,
) -> CribraStatus {
    // SAFETY: caller supplies an optional writable diagnostic slot.
    let error_slot = unsafe { ErrorSlot::from_raw(out_error) };
    contain_status_with_error(error_slot, || {
        // SAFETY: caller supplies a writable output slot when non-null.
        if let Err(status) = unsafe { clear_value(out_active) } {
            return status;
        }
        if incremental.is_null() {
            return CRIBRA_INVALID_ARGUMENT;
        }
        // SAFETY: caller guarantees exclusive access to a live handle.
        let incremental = unsafe { &mut *incremental };
        if incremental.inner.is_none() {
            return CRIBRA_OK;
        }
        incremental
            .with_core(|core| {
                // SAFETY: clear_value validated the output slot.
                unsafe { ptr::write(out_active, u32::from(core.is_active())) };
                CRIBRA_OK
            })
            .unwrap_or_else(|status| status)
    })
}

/// Frees an incremental handle, active, terminal or panic-invalidated.
/// Null is a no-op. Independently owned snapshot reports remain valid.
///
/// # Safety
///
/// A non-null handle must be live and exclusively owned, returned by a constructor
/// and not previously freed. No concurrent handle operation is permitted.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cribra_incremental_free(incremental: *mut CribraIncremental) {
    if !incremental.is_null() {
        contain_drop(|| {
            // SAFETY: caller transfers exclusive ownership of one live handle.
            drop(unsafe { Box::from_raw(incremental) });
        });
    }
}
