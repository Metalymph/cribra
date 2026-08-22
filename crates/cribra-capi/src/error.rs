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
}

impl CribraError {
    /// Creates an owned privacy-safe ABI diagnostic.
    pub(crate) fn new(status: CribraStatus, message: impl Into<String>) -> Self {
        debug_assert_ne!(status, CRIBRA_OK);
        Self {
            status,
            message: message.into(),
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
    /// A non-null `raw` pointer must reference writable memory for one
    /// [`CribraError`] pointer for the duration of the enclosing ABI call.
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
