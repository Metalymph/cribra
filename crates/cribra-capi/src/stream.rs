//! Direct Core streaming projection; no source decoding, buffering or replay.

use std::{ptr, slice};

use crate::{
    CribraError, CribraReport, CribraScanner, CribraStatus, CribraStream, CribraStreamLimits,
    error::{ErrorSlot, contain_status_with_error},
    ffi::{clear_out, contain_drop},
    status::*,
};

fn stream_status(error: cribra::StreamError) -> CribraStatus {
    use cribra::StreamError as Core;
    match error {
        Core::UnsupportedRuleSet => CRIBRA_STREAM_UNSUPPORTED_RULE_SET,
        Core::NotActive => CRIBRA_STREAM_NOT_ACTIVE,
        Core::InvalidUtf8 => CRIBRA_STREAM_INVALID_UTF8,
        Core::IncompleteUtf8 => CRIBRA_STREAM_INCOMPLETE_UTF8,
        Core::CandidateLimitExceeded { .. } => CRIBRA_STREAM_CANDIDATE_LIMIT_EXCEEDED,
        Core::ResultLimitExceeded { .. } => CRIBRA_STREAM_RESULT_LIMIT_EXCEEDED,
        Core::StorageLimitExceeded { .. } => CRIBRA_STREAM_STORAGE_LIMIT_EXCEEDED,
        Core::StorageRequirementOverflow => CRIBRA_STREAM_STORAGE_REQUIREMENT_OVERFLOW,
        Core::SourceOffsetOverflow => CRIBRA_STREAM_SOURCE_OFFSET_OVERFLOW,
        Core::CoordinateOverflow => CRIBRA_STREAM_COORDINATE_OVERFLOW,
        Core::ExecutionInvariantViolated => CRIBRA_STREAM_EXECUTION_INVARIANT_VIOLATED,
        _ => CRIBRA_STREAM_UNKNOWN_ERROR,
    }
}

impl CribraStreamLimits {
    fn into_core(self) -> Result<cribra::StreamLimits, CribraStatus> {
        fn limit(enabled: u32, value: usize) -> Result<Option<usize>, CribraStatus> {
            match enabled {
                0 => Ok(None),
                1 => Ok(Some(value)),
                _ => Err(CRIBRA_INVALID_ARGUMENT),
            }
        }
        Ok(cribra::StreamLimits::default()
            .with_max_candidates(limit(self.max_candidates_enabled, self.max_candidates)?)
            .with_max_stream_storage_bytes(limit(
                self.max_stream_storage_bytes_enabled,
                self.max_stream_storage_bytes,
            )?)
            .with_max_retained_results(limit(
                self.max_retained_results_enabled,
                self.max_retained_results,
            )?))
    }
}

/// Opens one byte-oriented Core stream with unlimited optional budgets.
///
/// Unsupported rulesets (including complete built-in portfolios) fail admission
/// with CRIBRA_STREAM_UNSUPPORTED_RULE_SET. No whole-source fallback is used.
/// The stream owns its execution independently of the scanner's lifetime.
/// Only successful finish returns a complete existing CribraReport.
///
/// # Safety
///
/// `scanner` must be a live immutable scanner handle. `out_stream` must reference
/// writable memory for one stream pointer; it is cleared on failure. A non-null
/// `out_error` must reference a writable error slot, freed before reuse.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cribra_scanner_stream_new(
    scanner: *const CribraScanner,
    out_stream: *mut *mut CribraStream,
    out_error: *mut *mut CribraError,
) -> CribraStatus {
    let limits = CribraStreamLimits::default();
    // SAFETY: the same pointer contracts are forwarded; limits lives for the call.
    unsafe { cribra_scanner_stream_new_with_limits(scanner, &limits, out_stream, out_error) }
}

/// Opens one Core stream with explicit optional budgets.
///
/// Flags must be 0 or 1. An enabled zero is a zero budget; disabled values are
/// ignored. Limits are copied for admission and execution. Core defines error
/// precedence, admission, logical resource accounting and terminal failures.
/// Null output or invalid arguments do not create a stream. Errors never return
/// a partially constructed handle. Diagnostics contain no source or rule text.
///
/// # Safety
///
/// `scanner` must be live and immutable, `limits` must reference a readable
/// CribraStreamLimits, and `out_stream` must reference writable memory for one
/// stream pointer. Optional `out_error` must be a writable error slot, freed
/// before reuse. Inputs and output slots must not alias.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cribra_scanner_stream_new_with_limits(
    scanner: *const CribraScanner,
    limits: *const CribraStreamLimits,
    out_stream: *mut *mut CribraStream,
    out_error: *mut *mut CribraError,
) -> CribraStatus {
    // SAFETY: caller provides a writable optional diagnostic slot.
    let error_slot = unsafe { ErrorSlot::from_raw(out_error) };
    contain_status_with_error(error_slot, || {
        // SAFETY: caller provides a writable output slot when non-null.
        if let Err(status) = unsafe { clear_out(out_stream) } {
            return status;
        }
        if scanner.is_null() || limits.is_null() {
            return CRIBRA_INVALID_ARGUMENT;
        }
        // SAFETY: caller guarantees readable configuration and a live scanner.
        let limits = match unsafe { *limits }.into_core() {
            Ok(limits) => limits,
            Err(status) => return status,
        };
        let scanner = unsafe { &*scanner };
        match scanner.inner.stream_with_limits(limits) {
            Ok(session) => {
                let stream = Box::new(CribraStream::new(session));
                // SAFETY: clear_out validated the output slot.
                unsafe { ptr::write(out_stream, Box::into_raw(stream)) };
                CRIBRA_OK
            }
            Err(error) => stream_status(error),
        }
    })
}

/// Feeds arbitrary byte chunks in source order without retaining caller memory.
///
/// UTF-8 scalars may span calls. Null plus zero length is an empty chunk; empty
/// chunks on active streams are no-ops. Malformed UTF-8 and all Core execution
/// failures discard the session and return distinct CRIBRA_STREAM_* statuses.
/// A contained execution panic also discards the session and returns
/// CRIBRA_INTERNAL_ERROR. Subsequent push/finish returns CRIBRA_STREAM_NOT_ACTIVE,
/// including an empty push. No partial report is ever returned.
/// Invalid buffer arguments return CRIBRA_INVALID_ARGUMENT without consuming an
/// active session. Source-byte counts increase only after successful Core pushes
/// and checked adapter accounting. No adapter UTF-8 decoding or source buffer.
///
/// # Safety
///
/// `stream` must be live, with exclusive access for this call (no concurrent
/// push, finish or free). For non-empty input, `chunk` must reference `chunk_len`
/// readable bytes, with length no greater than PTRDIFF_MAX. Input and output
/// slots must not alias the handle. Optional `out_error` must reference a writable
/// error slot, freed before reuse. No caller buffer borrow survives this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cribra_stream_push(
    stream: *mut CribraStream,
    chunk: *const u8,
    chunk_len: usize,
    out_error: *mut *mut CribraError,
) -> CribraStatus {
    // SAFETY: caller provides a writable optional diagnostic slot.
    let error_slot = unsafe { ErrorSlot::from_raw(out_error) };
    contain_status_with_error(error_slot, || {
        if stream.is_null() {
            return CRIBRA_INVALID_ARGUMENT;
        }
        // SAFETY: caller guarantees exclusive access to a live stream handle.
        let stream = unsafe { &mut *stream };
        if stream.inner.is_none() {
            return CRIBRA_STREAM_NOT_ACTIVE;
        }
        if chunk_len > isize::MAX as usize || (chunk_len != 0 && chunk.is_null()) {
            return CRIBRA_INVALID_ARGUMENT;
        }
        let bytes = if chunk_len == 0 {
            &[]
        } else {
            // SAFETY: caller guarantees readable bytes; null/length checked above.
            unsafe { slice::from_raw_parts(chunk, chunk_len) }
        };
        match stream.push(bytes) {
            Ok(()) => CRIBRA_OK,
            Err(error) => stream_status(error),
        }
    })
}

/// Signals genuine EOF and transfers a complete existing CribraReport once.
///
/// Consumes the Core session while the C handle stays valid until stream_free.
/// Success, execution failure and a contained execution panic all leave it
/// inactive; subsequent push/finish returns CRIBRA_STREAM_NOT_ACTIVE. A null
/// out_report returns CRIBRA_INVALID_ARGUMENT without consuming an active session.
/// Any failure with a valid output slot clears it to null, never a partial report.
/// The returned report is independently owned and uses existing report accessors,
/// explanation and transformation APIs, and cribra_report_free.
///
/// # Safety
///
/// `stream` must be live with exclusive access. `out_report` must reference a
/// writable report-pointer slot. Optional `out_error` must reference a writable
/// error slot, freed before reuse. Output slots and the handle must not alias.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cribra_stream_finish(
    stream: *mut CribraStream,
    out_report: *mut *mut CribraReport,
    out_error: *mut *mut CribraError,
) -> CribraStatus {
    // SAFETY: caller provides a writable optional diagnostic slot.
    let error_slot = unsafe { ErrorSlot::from_raw(out_error) };
    contain_status_with_error(error_slot, || {
        // SAFETY: caller provides a writable output slot when non-null.
        if let Err(status) = unsafe { clear_out(out_report) } {
            return status;
        }
        if stream.is_null() {
            return CRIBRA_INVALID_ARGUMENT;
        }
        // SAFETY: caller guarantees exclusive access to a live stream handle.
        let stream = unsafe { &mut *stream };
        match stream.finish() {
            Ok(report) => {
                let report = Box::new(report);
                // SAFETY: clear_out validated the output slot.
                unsafe { ptr::write(out_report, Box::into_raw(report)) };
                CRIBRA_OK
            }
            Err(error) => stream_status(error),
        }
    })
}

/// Frees one streaming handle, active, failed or finished. Null is a no-op.
///
/// # Safety
///
/// A non-null stream must be live and exclusively owned, created by a stream
/// constructor and not previously freed. No concurrent operation is permitted.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cribra_stream_free(stream: *mut CribraStream) {
    if !stream.is_null() {
        contain_drop(|| {
            // SAFETY: caller transfers exclusive ownership of one live handle.
            drop(unsafe { Box::from_raw(stream) });
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_core_stream_errors_have_distinct_typed_statuses() {
        use cribra::StreamError as Core;
        let cases = [
            (Core::UnsupportedRuleSet, CRIBRA_STREAM_UNSUPPORTED_RULE_SET),
            (Core::NotActive, CRIBRA_STREAM_NOT_ACTIVE),
            (Core::InvalidUtf8, CRIBRA_STREAM_INVALID_UTF8),
            (Core::IncompleteUtf8, CRIBRA_STREAM_INCOMPLETE_UTF8),
            (
                Core::CandidateLimitExceeded { limit: 0 },
                CRIBRA_STREAM_CANDIDATE_LIMIT_EXCEEDED,
            ),
            (
                Core::ResultLimitExceeded { limit: 0 },
                CRIBRA_STREAM_RESULT_LIMIT_EXCEEDED,
            ),
            (
                Core::StorageLimitExceeded {
                    required: 1,
                    limit: 0,
                },
                CRIBRA_STREAM_STORAGE_LIMIT_EXCEEDED,
            ),
            (
                Core::StorageRequirementOverflow,
                CRIBRA_STREAM_STORAGE_REQUIREMENT_OVERFLOW,
            ),
            (
                Core::SourceOffsetOverflow,
                CRIBRA_STREAM_SOURCE_OFFSET_OVERFLOW,
            ),
            (Core::CoordinateOverflow, CRIBRA_STREAM_COORDINATE_OVERFLOW),
            (
                Core::ExecutionInvariantViolated,
                CRIBRA_STREAM_EXECUTION_INVARIANT_VIOLATED,
            ),
        ];
        let mut statuses = std::collections::BTreeSet::new();
        for (error, expected) in cases {
            assert_eq!(stream_status(error), expected);
            assert!(statuses.insert(expected));
            assert_eq!(CribraError::from_status(expected).status(), expected);
        }
    }
}
