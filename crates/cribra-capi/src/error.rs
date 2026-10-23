//! Explicit Rust-owned diagnostics for the native ABI.

use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    ptr,
};

use crate::{
    status::{CRIBRA_INTERNAL_ERROR, CRIBRA_INVALID_ARGUMENT, CRIBRA_OK, CribraStatus},
    types::CribraStringView,
};

/// Opaque Rust-owned native error object.
///
/// The diagnostic message is privacy-safe metadata. It must never contain
/// original source text, matched sensitive values, transformation keys, panic
/// payloads, or other caller secrets.
pub struct CribraError {
    status: CribraStatus,
    message: String,
    build_code: crate::CribraBuildErrorCode,
    rule_id: Option<String>,
}

impl CribraError {
    /// Creates an owned privacy-safe ABI diagnostic.
    pub(crate) fn new(status: CribraStatus, message: impl Into<String>) -> Self {
        debug_assert_ne!(status, CRIBRA_OK);
        Self {
            status,
            message: message.into(),
            build_code: crate::CRIBRA_BUILD_CODE_NONE,
            rule_id: None,
        }
    }

    /// Generic diagnostic used when an unwinding panic is contained.
    pub(crate) fn internal() -> Self {
        Self::new(
            CRIBRA_INTERNAL_ERROR,
            "internal Cribra native adapter failure",
        )
    }

    pub(crate) fn status(&self) -> CribraStatus {
        self.status
    }

    pub(crate) fn message(&self) -> &str {
        &self.message
    }

    pub(crate) fn with_rule_error(mut self, error: &cribra::RuleError, id: Option<&str>) -> Self {
        self.build_code = match error {
            cribra::RuleError::InvalidPattern(_) => crate::CRIBRA_BUILD_CODE_INVALID_PATTERN,
            cribra::RuleError::PatternMatchesEmpty => {
                crate::CRIBRA_BUILD_CODE_PATTERN_MATCHES_EMPTY
            }
            cribra::RuleError::MissingCaptureGroup { .. } => {
                crate::CRIBRA_BUILD_CODE_MISSING_CAPTURE_GROUP
            }
        };
        self.rule_id = id.map(str::to_owned);
        self
    }

    pub(crate) fn with_build_error(mut self, error: &cribra::ScannerBuildError) -> Self {
        use cribra::ScannerBuildError as Core;
        let (code, id) = match error {
            Core::Rule(error) => return self.with_rule_error(error, None),
            Core::EmptyRuleId => (crate::CRIBRA_BUILD_CODE_EMPTY_RULE_ID, None),
            Core::DuplicateRuleId { rule_id } => (
                crate::CRIBRA_BUILD_CODE_DUPLICATE_RULE_ID,
                Some(rule_id.as_str()),
            ),
            Core::EmptyMatcher { rule_id } => (
                crate::CRIBRA_BUILD_CODE_EMPTY_MATCHER,
                Some(rule_id.as_str()),
            ),
            Core::TooManyRules => (crate::CRIBRA_BUILD_CODE_TOO_MANY_RULES, None),
            Core::AutomatonBuild(_) => (crate::CRIBRA_BUILD_CODE_AUTOMATON_BUILD, None),
        };
        self.build_code = code;
        self.rule_id = id.map(str::to_owned);
        self
    }
}

/// Reads Core-derived construction information without parsing the message.
///
/// The view is cleared on invalid arguments. Successful non-construction errors
/// return code NONE and no rule ID. Rule-ID bytes remain valid until error free.
///
/// # Safety
/// `error` must be a live immutable error handle. `out_info` must point to writable
/// memory for one CribraBuildErrorView, without aliasing the error handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cribra_error_build_info(
    error: *const CribraError,
    out_info: *mut crate::CribraBuildErrorView,
) -> CribraStatus {
    contain_status(|| {
        // SAFETY: the caller guarantees a writable output slot when non-null.
        if let Err(status) = unsafe { crate::ffi::clear_value(out_info) } {
            return status;
        }
        if error.is_null() {
            return CRIBRA_INVALID_ARGUMENT;
        }
        // SAFETY: the caller guarantees a live immutable error handle.
        let error = unsafe { &*error };
        let rule_id = error
            .rule_id
            .as_ref()
            .map(|id| CribraStringView {
                ptr: id.as_ptr(),
                len: id.len(),
            })
            .unwrap_or_default();
        // SAFETY: clear_value validated the writable slot.
        unsafe {
            ptr::write(
                out_info,
                crate::CribraBuildErrorView {
                    code: error.build_code,
                    rule_id,
                },
            )
        };
        CRIBRA_OK
    })
}

fn contain_status(operation: impl FnOnce() -> CribraStatus) -> CribraStatus {
    catch_unwind(AssertUnwindSafe(operation)).unwrap_or(CRIBRA_INTERNAL_ERROR)
}

fn contain_drop(operation: impl FnOnce()) {
    let _ = catch_unwind(AssertUnwindSafe(operation));
}

impl CribraError {
    pub(crate) fn from_status(status: CribraStatus) -> Self {
        let message = match status {
            crate::status::CRIBRA_INVALID_ARGUMENT => "invalid argument",
            crate::status::CRIBRA_INVALID_UTF8 => "input is not valid UTF-8",
            crate::status::CRIBRA_RULE_ERROR => "custom rule configuration is invalid",
            crate::status::CRIBRA_BUILD_ERROR => "scanner configuration could not be built",
            crate::status::CRIBRA_TRANSFORM_ERROR => {
                "source transformation could not be applied safely"
            }
            crate::status::CRIBRA_OUT_OF_RANGE => "index is out of range",
            crate::status::CRIBRA_STREAM_UNSUPPORTED_RULE_SET => {
                "ruleset does not support exact streaming"
            }
            crate::status::CRIBRA_STREAM_NOT_ACTIVE => "stream session is not active",
            crate::status::CRIBRA_STREAM_INVALID_UTF8 => "stream contains malformed UTF-8",
            crate::status::CRIBRA_STREAM_INCOMPLETE_UTF8 => "stream ends with incomplete UTF-8",
            crate::status::CRIBRA_STREAM_CANDIDATE_LIMIT_EXCEEDED => {
                "stream candidate limit exceeded"
            }
            crate::status::CRIBRA_STREAM_RESULT_LIMIT_EXCEEDED => {
                "stream retained result limit exceeded"
            }
            crate::status::CRIBRA_STREAM_STORAGE_LIMIT_EXCEEDED => {
                "stream logical storage limit exceeded"
            }
            crate::status::CRIBRA_STREAM_STORAGE_REQUIREMENT_OVERFLOW => {
                "stream storage requirement overflow"
            }
            crate::status::CRIBRA_STREAM_SOURCE_OFFSET_OVERFLOW => {
                "stream source byte offset overflow"
            }
            crate::status::CRIBRA_STREAM_COORDINATE_OVERFLOW => "stream source coordinate overflow",
            crate::status::CRIBRA_STREAM_EXECUTION_INVARIANT_VIOLATED => {
                "stream execution invariant violated"
            }
            crate::status::CRIBRA_STREAM_UNKNOWN_ERROR => "unknown Core streaming failure",
            crate::status::CRIBRA_INCREMENTAL_NOT_ACTIVE => "incremental session is not active",
            crate::status::CRIBRA_INCREMENTAL_STALE_REVISION => {
                "incremental edit revision is stale"
            }
            crate::status::CRIBRA_INCREMENTAL_INVALID_RANGE => "incremental edit range is invalid",
            crate::status::CRIBRA_INCREMENTAL_INVALID_UTF8_BOUNDARY => {
                "incremental edit divides a UTF-8 scalar"
            }
            crate::status::CRIBRA_INCREMENTAL_REVISION_OVERFLOW => "incremental revision overflow",
            crate::status::CRIBRA_INCREMENTAL_DOCUMENT_LENGTH_OVERFLOW => {
                "incremental document length overflow"
            }
            crate::status::CRIBRA_INCREMENTAL_CANDIDATE_LIMIT_EXCEEDED => {
                "incremental candidate limit exceeded"
            }
            crate::status::CRIBRA_INCREMENTAL_CACHE_LIMIT_EXCEEDED => {
                "incremental cache record limit exceeded"
            }
            crate::status::CRIBRA_INCREMENTAL_RESULT_LIMIT_EXCEEDED => {
                "incremental result materialization limit exceeded"
            }
            crate::status::CRIBRA_INCREMENTAL_TRANSACTION_STORAGE_LIMIT_EXCEEDED => {
                "incremental logical transaction storage limit exceeded"
            }
            crate::status::CRIBRA_INCREMENTAL_RESOURCE_ACCOUNTING_OVERFLOW => {
                "incremental resource accounting overflow"
            }
            crate::status::CRIBRA_INCREMENTAL_EXECUTION_INVARIANT_VIOLATED => {
                "incremental execution invariant violated"
            }
            crate::status::CRIBRA_INCREMENTAL_UNKNOWN_ERROR => "unknown Core incremental failure",
            crate::status::CRIBRA_INCREMENTAL_SESSION_INVALIDATED => {
                "incremental session invalidated by a contained panic"
            }
            crate::status::CRIBRA_INTERNAL_ERROR => "internal Cribra native adapter failure",
            _ => "Cribra native operation failed",
        };
        Self::new(status, message)
    }
}

/// Stores one owned error when diagnostics were requested.
///
/// A null `out_error` means that the caller requested status-only behavior.
///
/// # Safety
///
/// A non-null `out_error` must point to writable memory for one
/// [`CribraError`] pointer.
pub(crate) unsafe fn set_error(out_error: *mut *mut CribraError, error: CribraError) {
    if out_error.is_null() {
        return;
    }

    // SAFETY: caller contract requires a writable out-pointer when non-null.
    unsafe { ptr::write(out_error, Box::into_raw(Box::new(error))) };
}

/// Validated optional error-output slot.
///
/// The only way to construct this type is through [`ErrorSlot::from_raw`],
/// whose safety contract establishes that a non-null pointer is writable for
/// one [`CribraError`] pointer. Safe methods can then maintain that invariant
/// without widening `unsafe` scope across ABI operation closures.
pub(crate) struct ErrorSlot {
    raw: *mut *mut CribraError,
}

impl ErrorSlot {
    /// Creates an optional error-output slot from an ABI raw pointer.
    ///
    /// A null pointer means that diagnostics were not requested.
    ///
    /// # Safety
    ///
    /// A non-null `raw` pointer must reference readable and writable memory for one
    /// [`CribraError`] pointer for the entire lifetime of the returned `ErrorSlot`.
    /// The pointed-to slot must not be concurrently accessed or invalidated while
    /// the `ErrorSlot` is in use.
    pub(crate) unsafe fn from_raw(raw: *mut *mut CribraError) -> Self {
        Self { raw }
    }

    fn clear(&self) {
        if self.raw.is_null() {
            return;
        }

        // SAFETY: the constructor's invariant guarantees a writable slot.
        unsafe { ptr::write(self.raw, ptr::null_mut()) };
    }

    fn current(&self) -> *mut CribraError {
        if self.raw.is_null() {
            return ptr::null_mut();
        }

        // SAFETY: the constructor's invariant guarantees a readable slot.
        unsafe { *self.raw }
    }

    fn set(&self, error: CribraError) {
        if self.raw.is_null() {
            return;
        }

        // SAFETY: the constructor's invariant guarantees a writable slot.
        unsafe { ptr::write(self.raw, Box::into_raw(Box::new(error))) };
    }
}

/// Executes one status-returning ABI operation with optional diagnostics.
///
/// The error slot is cleared before execution. On ordinary failure, an error
/// supplied by the operation is preserved; otherwise a privacy-safe generic
/// diagnostic is synthesized from the returned coarse status. Unwinding panics
/// are contained and converted to [`CRIBRA_INTERNAL_ERROR`] with a generic
/// message that never includes the panic payload.
pub(crate) fn contain_status_with_error(
    out_error: ErrorSlot,
    operation: impl FnOnce() -> CribraStatus,
) -> CribraStatus {
    out_error.clear();

    match catch_unwind(AssertUnwindSafe(operation)) {
        Ok(status) => {
            if status != CRIBRA_OK && out_error.current().is_null() {
                out_error.set(CribraError::from_status(status));
            }
            status
        }
        Err(_) => {
            out_error.set(CribraError::internal());
            CRIBRA_INTERNAL_ERROR
        }
    }
}

/// Returns the coarse status represented by an error object.
///
/// # Safety
///
/// `error` must be a live [`CribraError`] handle. `out_status` must point to
/// writable memory for one [`CribraStatus`] value.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cribra_error_status(
    error: *const CribraError,
    out_status: *mut CribraStatus,
) -> CribraStatus {
    contain_status(|| {
        if out_status.is_null() {
            return CRIBRA_INVALID_ARGUMENT;
        }

        // Write a deterministic default before validating the handle.
        // SAFETY: caller contract requires a writable `out_status`.
        unsafe { ptr::write(out_status, CRIBRA_OK) };

        if error.is_null() {
            return CRIBRA_INVALID_ARGUMENT;
        }

        // SAFETY: caller contract requires a live immutable error handle.
        let error = unsafe { &*error };

        // SAFETY: `out_status` was validated above.
        unsafe { ptr::write(out_status, error.status()) };
        CRIBRA_OK
    })
}

/// Returns a borrowed UTF-8 diagnostic message.
///
/// The returned bytes are not NUL-terminated and remain valid only while
/// `error` remains alive. The caller must not free the returned view.
///
/// # Safety
///
/// `error` must be a live [`CribraError`] handle. `out_message` must point to
/// writable memory for one [`CribraStringView`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cribra_error_message(
    error: *const CribraError,
    out_message: *mut CribraStringView,
) -> CribraStatus {
    contain_status(|| {
        if out_message.is_null() {
            return CRIBRA_INVALID_ARGUMENT;
        }

        // SAFETY: caller contract requires a writable `out_message`.
        unsafe { ptr::write(out_message, CribraStringView::default()) };

        if error.is_null() {
            return CRIBRA_INVALID_ARGUMENT;
        }

        // SAFETY: caller contract requires a live immutable error handle.
        let error = unsafe { &*error };
        let message = error.message();

        let view = CribraStringView {
            ptr: message.as_ptr(),
            len: message.len(),
        };

        // SAFETY: `out_message` was validated above.
        unsafe { ptr::write(out_message, view) };
        CRIBRA_OK
    })
}

/// Releases an owned native error object.
///
/// Passing null is a no-op.
///
/// # Safety
///
/// A non-null `error` must be a live handle returned by the Cribra native ABI
/// and must not already have been freed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cribra_error_free(error: *mut CribraError) {
    if error.is_null() {
        return;
    }

    contain_drop(|| {
        // SAFETY: caller transfers ownership of one live error handle.
        drop(unsafe { Box::from_raw(error) });
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    // use std::{slice, str};

    #[test]
    fn status_wrapper_creates_generic_error_when_requested() {
        let mut error = ptr::null_mut();

        let slot = unsafe { ErrorSlot::from_raw(&mut error) };
        let status = contain_status_with_error(slot, || crate::status::CRIBRA_INVALID_UTF8);

        assert_eq!(status, crate::status::CRIBRA_INVALID_UTF8);
        assert!(!error.is_null());

        unsafe {
            assert_eq!((*error).status(), crate::status::CRIBRA_INVALID_UTF8);
            assert_eq!((*error).message(), "input is not valid UTF-8");
            cribra_error_free(error);
        }
    }

    #[test]
    fn status_wrapper_contains_panic_without_exposing_payload() {
        let mut error = ptr::null_mut();

        let slot = unsafe { ErrorSlot::from_raw(&mut error) };
        let status = contain_status_with_error(slot, || -> CribraStatus {
            panic!("SECRET_SHOULD_NOT_ESCAPE");
        });

        assert_eq!(status, CRIBRA_INTERNAL_ERROR);
        assert!(!error.is_null());

        unsafe {
            assert_eq!((*error).message(), "internal Cribra native adapter failure");
            assert!(!(*error).message().contains("SECRET_SHOULD_NOT_ESCAPE"));
            cribra_error_free(error);
        }
    }

    #[test]
    fn successful_status_wrapper_clears_requested_error_slot() {
        let mut error = ptr::dangling_mut::<CribraError>();

        let slot = unsafe { ErrorSlot::from_raw(&mut error) };
        let status = contain_status_with_error(slot, || CRIBRA_OK);

        assert_eq!(status, CRIBRA_OK);
        assert!(error.is_null());
    }
}
