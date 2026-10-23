//! Borrowed scanner metadata projected directly from Core's immutable rule table.
use crate::*;
use std::ptr;

/// Returns the number of active Core rules in scanner metadata order.
///
/// # Safety
/// `scanner` must be a live immutable scanner; `out_count` must be writable for
/// one size_t and must not alias the handle. Null arguments fail and clear output.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cribra_scanner_rule_count(
    scanner: *const CribraScanner,
    out_count: *mut usize,
) -> CribraStatus {
    crate::ffi::contain_status(|| {
        // SAFETY: the caller guarantees a writable output slot when non-null.
        if let Err(status) = unsafe { crate::ffi::clear_value(out_count) } {
            return status;
        }
        if scanner.is_null() {
            return CRIBRA_INVALID_ARGUMENT;
        }
        // SAFETY: the caller guarantees a live immutable scanner handle.
        let scanner = unsafe { &*scanner };
        // SAFETY: clear_value validated this output.
        unsafe { ptr::write(out_count, scanner.inner.rule_metadata().len()) };
        CRIBRA_OK
    })
}

/// Returns one scanner-owned metadata view by zero-based Core rule index.
///
/// The ID is borrowed until cribra_scanner_free, survives scans and report frees,
/// and must not be freed separately. Failure clears the view. Metadata contains
/// no matcher payloads, regexes, validators or scanned source.
///
/// # Safety
/// `scanner` must be a live immutable scanner; `out_rule` must be writable for
/// one CribraRuleMetadataView and must not alias the handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cribra_scanner_rule_at(
    scanner: *const CribraScanner,
    index: usize,
    out_rule: *mut CribraRuleMetadataView,
) -> CribraStatus {
    crate::ffi::contain_status(|| {
        // SAFETY: the caller guarantees a writable output slot when non-null.
        if let Err(status) = unsafe { crate::ffi::clear_value(out_rule) } {
            return status;
        }
        if scanner.is_null() {
            return CRIBRA_INVALID_ARGUMENT;
        }
        // SAFETY: the caller guarantees a live immutable scanner handle.
        let scanner = unsafe { &*scanner };
        let Some(rule) = scanner.inner.rule_metadata().nth(index) else {
            return CRIBRA_OUT_OF_RANGE;
        };
        let kind = match rule.kind() {
            cribra::RuleKind::Literal => CRIBRA_RULE_KIND_LITERAL,
            cribra::RuleKind::Prefix => CRIBRA_RULE_KIND_PREFIX,
            cribra::RuleKind::Suffix => CRIBRA_RULE_KIND_SUFFIX,
            cribra::RuleKind::Pattern => CRIBRA_RULE_KIND_PATTERN,
        };
        let view = CribraRuleMetadataView {
            rule_id: CribraStringView {
                ptr: rule.id().as_ptr(),
                len: rule.id().len(),
            },
            kind,
            detection_mode: crate::ffi::detection_mode_code(rule.detection_mode()),
            severity: crate::ffi::severity_code(rule.severity()),
            remediation: crate::ffi::remediation_code(rule.remediation()),
        };
        // SAFETY: clear_value validated this output.
        unsafe { ptr::write(out_rule, view) };
        CRIBRA_OK
    })
}
