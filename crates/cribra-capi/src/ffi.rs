//! Native ABI entry points.

use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    ptr, slice, str,
};

use cribra::{
    CandidateEvidence, Confidence, DetectionMode, Explanation, Remediation, Rule, Scanner,
    SensitiveCandidateKind, Severity, builtins,
};

use crate::{
    CRIBRA_BUILD_ERROR, CRIBRA_CONFIDENCE_HIGH, CRIBRA_CONFIDENCE_LOW, CRIBRA_CONFIDENCE_MEDIUM,
    CRIBRA_INTERNAL_ERROR, CRIBRA_INVALID_ARGUMENT, CRIBRA_INVALID_UTF8, CRIBRA_OK,
    CRIBRA_OUT_OF_RANGE, CRIBRA_REMEDIATION_NONE, CRIBRA_REMEDIATION_REMOVE_SENSITIVE_VALUE,
    CRIBRA_REMEDIATION_REPLACE_PRIVATE_KEY, CRIBRA_REMEDIATION_REVIEW_SENSITIVE_HASH,
    CRIBRA_REMEDIATION_REVOKE_AND_ROTATE_CREDENTIAL, CRIBRA_REMEDIATION_ROTATE_CREDENTIAL,
    CRIBRA_REMEDIATION_ROTATE_PASSWORD, CRIBRA_REMEDIATION_UNKNOWN, CRIBRA_SEVERITY_CRITICAL,
    CRIBRA_SEVERITY_HIGH, CRIBRA_SEVERITY_INFO, CRIBRA_SEVERITY_LOW, CRIBRA_SEVERITY_MEDIUM,
    CRIBRA_CANDIDATE_EVIDENCE_NONE, CRIBRA_CANDIDATE_EVIDENCE_STRUCTURAL,
    CRIBRA_CANDIDATE_EVIDENCE_UNKNOWN, CRIBRA_CANDIDATE_KIND_RECOVERY_LIKE_CODE,
    CRIBRA_CANDIDATE_KIND_UNKNOWN, CRIBRA_DETECTION_MODE_CONTEXTUAL,
    CRIBRA_DETECTION_MODE_DETERMINISTIC, CRIBRA_DETECTION_MODE_MATCHER_ONLY,
    CRIBRA_DETECTION_MODE_NONE, CRIBRA_DETECTION_MODE_UNKNOWN, CRIBRA_EXPLANATION_AMBIGUOUS,
    CRIBRA_EXPLANATION_CLASSIFIED, CRIBRA_EXPLANATION_UNKNOWN, CRIBRA_RULE_KIND_LITERAL,
    CRIBRA_RULE_KIND_PATTERN, CRIBRA_RULE_KIND_PREFIX, CRIBRA_RULE_KIND_SUFFIX, CribraBuilder,
    CribraCandidateEvidence, CribraCandidateKind, CribraCandidateView, CribraConfidence,
    CribraDetectionMode, CribraExplanationView, CribraFindingView, CribraRemediation,
    CribraReport, CribraRuleConfig, CribraScanner, CribraSeverity, CribraStatus, CribraStringView,
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

unsafe fn utf8_from_raw<'a>(
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

fn candidate_kind_code(value: SensitiveCandidateKind) -> CribraCandidateKind {
    match value {
        SensitiveCandidateKind::RecoveryLikeCode => CRIBRA_CANDIDATE_KIND_RECOVERY_LIKE_CODE,
        _ => CRIBRA_CANDIDATE_KIND_UNKNOWN,
    }
}

fn candidate_evidence_code(value: CandidateEvidence) -> CribraCandidateEvidence {
    match value {
        CandidateEvidence::Structural => CRIBRA_CANDIDATE_EVIDENCE_STRUCTURAL,
        _ => CRIBRA_CANDIDATE_EVIDENCE_UNKNOWN,
    }
}

fn detection_mode_code(value: DetectionMode) -> CribraDetectionMode {
    match value {
        DetectionMode::MatcherOnly => CRIBRA_DETECTION_MODE_MATCHER_ONLY,
        DetectionMode::Deterministic => CRIBRA_DETECTION_MODE_DETERMINISTIC,
        DetectionMode::Contextual => CRIBRA_DETECTION_MODE_CONTEXTUAL,
    }
}

fn explanation_view(value: Explanation) -> CribraExplanationView {
    match value {
        Explanation::Classified(mode) => CribraExplanationView {
            kind: CRIBRA_EXPLANATION_CLASSIFIED,
            detection_mode: detection_mode_code(mode),
            candidate_evidence: CRIBRA_CANDIDATE_EVIDENCE_NONE,
        },
        Explanation::Ambiguous(evidence) => CribraExplanationView {
            kind: CRIBRA_EXPLANATION_AMBIGUOUS,
            detection_mode: CRIBRA_DETECTION_MODE_NONE,
            candidate_evidence: candidate_evidence_code(evidence),
        },
        _ => CribraExplanationView {
            kind: CRIBRA_EXPLANATION_UNKNOWN,
            detection_mode: CRIBRA_DETECTION_MODE_UNKNOWN,
            candidate_evidence: CRIBRA_CANDIDATE_EVIDENCE_UNKNOWN,
        },
    }
}

fn severity_from_code(value: CribraSeverity) -> Result<Severity, CribraStatus> {
    match value {
        CRIBRA_SEVERITY_INFO => Ok(Severity::Info),
        CRIBRA_SEVERITY_LOW => Ok(Severity::Low),
        CRIBRA_SEVERITY_MEDIUM => Ok(Severity::Medium),
        CRIBRA_SEVERITY_HIGH => Ok(Severity::High),
        CRIBRA_SEVERITY_CRITICAL => Ok(Severity::Critical),
        _ => Err(CRIBRA_INVALID_ARGUMENT),
    }
}

fn remediation_from_code(value: CribraRemediation) -> Result<Option<Remediation>, CribraStatus> {
    match value {
        CRIBRA_REMEDIATION_NONE => Ok(None),
        CRIBRA_REMEDIATION_REVOKE_AND_ROTATE_CREDENTIAL => {
            Ok(Some(Remediation::RevokeAndRotateCredential))
        }
        CRIBRA_REMEDIATION_ROTATE_CREDENTIAL => Ok(Some(Remediation::RotateCredential)),
        CRIBRA_REMEDIATION_ROTATE_PASSWORD => Ok(Some(Remediation::RotatePassword)),
        CRIBRA_REMEDIATION_REPLACE_PRIVATE_KEY => Ok(Some(Remediation::ReplacePrivateKey)),
        CRIBRA_REMEDIATION_REMOVE_SENSITIVE_VALUE => Ok(Some(Remediation::RemoveSensitiveValue)),
        CRIBRA_REMEDIATION_REVIEW_SENSITIVE_HASH => Ok(Some(Remediation::ReviewSensitiveHash)),
        _ => Err(CRIBRA_INVALID_ARGUMENT),
    }
}

fn configured_rule(
    kind: u32,
    id: &str,
    value: &str,
    severity: Severity,
    remediation: Option<Remediation>,
) -> Result<Rule, CribraStatus> {
    let rule = match kind {
        CRIBRA_RULE_KIND_LITERAL => Rule::literal(id, value, severity),
        CRIBRA_RULE_KIND_PREFIX => Rule::prefix(id, value, severity),
        CRIBRA_RULE_KIND_SUFFIX => Rule::suffix(id, value, severity),
        CRIBRA_RULE_KIND_PATTERN => {
            Rule::pattern(id, value, severity).map_err(|_| CRIBRA_BUILD_ERROR)?
        }
        _ => return Err(CRIBRA_INVALID_ARGUMENT),
    };

    Ok(match remediation {
        Some(remediation) => rule.with_remediation(remediation),
        None => rule,
    })
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

/// Adds Cribra's current canonical built-in catalog to a builder.
///
/// This allows native consumers to combine the standard catalog with custom
/// rules while preserving scanner-wide rule-ID validation.
///
/// # Safety
///
/// `builder` must be a live builder handle returned by [`cribra_builder_new`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cribra_builder_add_current_builtins(
    builder: *mut CribraBuilder,
) -> CribraStatus {
    contain_status(|| {
        if builder.is_null() {
            return CRIBRA_INVALID_ARGUMENT;
        }

        // SAFETY: caller guarantees unique access to a live builder handle.
        let builder = unsafe { &mut *builder };
        let Some(inner) = builder.inner.take() else {
            return CRIBRA_INVALID_ARGUMENT;
        };
        builder.inner = Some(inner.builtins(builtins::CURRENT));
        CRIBRA_OK
    })
}

/// Adds one public custom rule to a scanner builder.
///
/// `id` and `value` are copied into Rust-owned rule storage before this function
/// returns. Literal, prefix and suffix rules retain the core's existing deferred
/// empty-value validation. Pattern syntax and zero-length-capable patterns are
/// rejected while constructing the rule.
///
/// Internal validators and capture projection are intentionally unavailable
/// through this ABI.
///
/// # Safety
///
/// `builder` must be a live builder handle. `config` must point to a readable
/// [`CribraRuleConfig`]. Each non-empty string view in `config` must reference
/// readable bytes for the duration of this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cribra_builder_add_rule(
    builder: *mut CribraBuilder,
    config: *const CribraRuleConfig,
) -> CribraStatus {
    contain_status(|| {
        if builder.is_null() || config.is_null() {
            return CRIBRA_INVALID_ARGUMENT;
        }

        // SAFETY: caller guarantees a readable configuration value.
        let config = unsafe { &*config };

        let id = match unsafe { utf8_from_raw(config.id.ptr, config.id.len) } {
            Ok(value) => value,
            Err(status) => return status,
        };
        let value = match unsafe { utf8_from_raw(config.value.ptr, config.value.len) } {
            Ok(value) => value,
            Err(status) => return status,
        };
        let severity = match severity_from_code(config.severity) {
            Ok(value) => value,
            Err(status) => return status,
        };
        let remediation = match remediation_from_code(config.remediation) {
            Ok(value) => value,
            Err(status) => return status,
        };
        let rule = match configured_rule(config.kind, id, value, severity, remediation) {
            Ok(rule) => rule,
            Err(status) => return status,
        };

        // SAFETY: caller guarantees unique access to a live builder handle.
        let builder = unsafe { &mut *builder };
        let Some(inner) = builder.inner.take() else {
            return CRIBRA_INVALID_ARGUMENT;
        };
        builder.inner = Some(inner.rule(rule));
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

        let source = match unsafe { utf8_from_raw(source, source_len) } {
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

/// Returns the number of ambiguous sensitive candidates in a report.
///
/// Candidates are distinct from classified findings and do not contribute to
/// [`cribra_report_finding_count`].
///
/// # Safety
///
/// `report` must be a live report handle. `out_count` must point to writable
/// memory for one `size_t`-compatible value.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cribra_report_candidate_count(
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
        unsafe { ptr::write(out_count, report.inner.candidate_len()) };
        CRIBRA_OK
    })
}

/// Returns a projection of one ambiguous sensitive candidate by index.
///
/// The candidate view contains presentation-safe metadata only and never
/// contains the candidate's source value.
///
/// # Safety
///
/// `report` must be a live report handle. `out_candidate` must point to writable
/// memory for one [`CribraCandidateView`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cribra_report_candidate_at(
    report: *const CribraReport,
    index: usize,
    out_candidate: *mut CribraCandidateView,
) -> CribraStatus {
    contain_status(|| {
        if let Err(status) = unsafe { clear_value(out_candidate) } {
            return status;
        }
        if report.is_null() {
            return CRIBRA_INVALID_ARGUMENT;
        }
        // SAFETY: caller guarantees a live immutable report handle.
        let report = unsafe { &*report };
        let Some(candidate) = report.inner.candidates().get(index) else {
            return CRIBRA_OUT_OF_RANGE;
        };
        let location = candidate.location();
        let view = CribraCandidateView {
            kind: candidate_kind_code(candidate.kind()),
            start: location.start(),
            end: location.end(),
            line: location.line(),
            column: location.column(),
            evidence: candidate_evidence_code(candidate.evidence()),
        };
        // SAFETY: `clear_value` validated the out-pointer.
        unsafe { ptr::write(out_candidate, view) };
        CRIBRA_OK
    })
}

/// Resolves typed explanation facts for one classified finding.
///
/// Explanation authority remains scanner-owned. If `scanner` cannot resolve
/// metadata compatible with the selected finding, this function fails closed
/// with [`CRIBRA_INVALID_ARGUMENT`] and leaves `out_explanation` empty.
///
/// # Safety
///
/// `scanner` and `report` must be live immutable handles. `out_explanation`
/// must point to writable memory for one [`CribraExplanationView`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cribra_scanner_explain_finding(
    scanner: *const CribraScanner,
    report: *const CribraReport,
    index: usize,
    out_explanation: *mut CribraExplanationView,
) -> CribraStatus {
    contain_status(|| {
        if let Err(status) = unsafe { clear_value(out_explanation) } {
            return status;
        }
        if scanner.is_null() || report.is_null() {
            return CRIBRA_INVALID_ARGUMENT;
        }
        // SAFETY: caller guarantees a live immutable scanner handle.
        let scanner = unsafe { &*scanner };
        // SAFETY: caller guarantees a live immutable report handle.
        let report = unsafe { &*report };
        let Some(finding) = report.inner.findings().get(index) else {
            return CRIBRA_OUT_OF_RANGE;
        };
        let Some(explanation) = Explanation::for_finding(&scanner.inner, finding) else {
            return CRIBRA_INVALID_ARGUMENT;
        };
        // SAFETY: `clear_value` validated the out-pointer.
        unsafe { ptr::write(out_explanation, explanation_view(explanation)) };
        CRIBRA_OK
    })
}

/// Returns typed explanation facts for one ambiguous candidate.
///
/// Candidate explanation is derived directly from the candidate's existing
/// evidence. It never acquires finding severity, confidence, remediation, or
/// source content.
///
/// # Safety
///
/// `report` must be a live immutable report handle. `out_explanation` must point
/// to writable memory for one [`CribraExplanationView`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cribra_report_explain_candidate(
    report: *const CribraReport,
    index: usize,
    out_explanation: *mut CribraExplanationView,
) -> CribraStatus {
    contain_status(|| {
        if let Err(status) = unsafe { clear_value(out_explanation) } {
            return status;
        }
        if report.is_null() {
            return CRIBRA_INVALID_ARGUMENT;
        }
        // SAFETY: caller guarantees a live immutable report handle.
        let report = unsafe { &*report };
        let Some(candidate) = report.inner.candidates().get(index) else {
            return CRIBRA_OUT_OF_RANGE;
        };
        // SAFETY: `clear_value` validated the out-pointer.
        unsafe { ptr::write(out_explanation, explanation_view(candidate.explanation())) };
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

    fn string_input(value: &str) -> CribraStringView {
        CribraStringView {
            ptr: value.as_ptr(),
            len: value.len(),
        }
    }

    fn rule_config<'a>(
        kind: u32,
        id: &'a str,
        value: &'a str,
        severity: CribraSeverity,
    ) -> CribraRuleConfig {
        CribraRuleConfig {
            kind,
            id: string_input(id),
            value: string_input(value),
            severity,
            remediation: CRIBRA_REMEDIATION_NONE,
        }
    }

    #[test]
    fn native_builder_supports_all_public_custom_rule_families() {
        let cases = [
            (
                CRIBRA_RULE_KIND_LITERAL,
                "native.literal",
                "EXACT_SECRET",
                "before EXACT_SECRET after",
            ),
            (
                CRIBRA_RULE_KIND_PREFIX,
                "native.prefix",
                "native_live_",
                "token=native_live_ABC123",
            ),
            (
                CRIBRA_RULE_KIND_SUFFIX,
                "native.suffix",
                "_native_secret",
                "token=ABC123_native_secret",
            ),
            (
                CRIBRA_RULE_KIND_PATTERN,
                "native.pattern",
                r"NATIVE-[A-Z0-9]{8}",
                "token=NATIVE-AB12CD34",
            ),
        ];

        for (kind, id, value, source) in cases {
            let mut builder = ptr::null_mut();
            let mut scanner = ptr::null_mut();
            let mut report = ptr::null_mut();
            let config = rule_config(kind, id, value, CRIBRA_SEVERITY_HIGH);

            unsafe {
                assert_eq!(cribra_builder_new(&mut builder), CRIBRA_OK);
                assert_eq!(cribra_builder_add_rule(builder, &config), CRIBRA_OK);
                assert_eq!(cribra_builder_build(builder, &mut scanner), CRIBRA_OK);
                assert_eq!(
                    cribra_scanner_scan(
                        scanner,
                        source.as_ptr(),
                        source.len(),
                        &mut report
                    ),
                    CRIBRA_OK
                );

                let mut count = 0;
                assert_eq!(cribra_report_finding_count(report, &mut count), CRIBRA_OK);
                assert_eq!(count, 1, "{id}");

                let mut finding = CribraFindingView::default();
                assert_eq!(
                    cribra_report_finding_at(report, 0, &mut finding),
                    CRIBRA_OK
                );
                let rule_id =
                    slice::from_raw_parts(finding.rule_id.ptr, finding.rule_id.len);
                assert_eq!(str::from_utf8(rule_id).unwrap(), id);

                cribra_report_free(report);
                cribra_scanner_free(scanner);
            }
        }
    }

    #[test]
    fn custom_rule_input_is_copied_before_return() {
        let mut builder = ptr::null_mut();
        let mut scanner = ptr::null_mut();
        let id = String::from("native.owned");
        let value = String::from("OWNED_SECRET");
        let config = rule_config(
            CRIBRA_RULE_KIND_LITERAL,
            &id,
            &value,
            CRIBRA_SEVERITY_HIGH,
        );

        unsafe {
            assert_eq!(cribra_builder_new(&mut builder), CRIBRA_OK);
            assert_eq!(cribra_builder_add_rule(builder, &config), CRIBRA_OK);
        }

        drop(id);
        drop(value);

        unsafe {
            assert_eq!(cribra_builder_build(builder, &mut scanner), CRIBRA_OK);
            cribra_scanner_free(scanner);
        }
    }

    #[test]
    fn invalid_pattern_is_rejected_without_consuming_builder() {
        let mut builder = ptr::null_mut();
        let mut scanner = ptr::null_mut();
        let invalid = rule_config(
            CRIBRA_RULE_KIND_PATTERN,
            "native.invalid-pattern",
            "(",
            CRIBRA_SEVERITY_HIGH,
        );
        let valid = rule_config(
            CRIBRA_RULE_KIND_LITERAL,
            "native.valid",
            "VALID_SECRET",
            CRIBRA_SEVERITY_HIGH,
        );

        unsafe {
            assert_eq!(cribra_builder_new(&mut builder), CRIBRA_OK);
            assert_eq!(
                cribra_builder_add_rule(builder, &invalid),
                CRIBRA_BUILD_ERROR
            );
            assert_eq!(cribra_builder_add_rule(builder, &valid), CRIBRA_OK);
            assert_eq!(cribra_builder_build(builder, &mut scanner), CRIBRA_OK);
            cribra_scanner_free(scanner);
        }
    }

    #[test]
    fn duplicate_rule_ids_remain_scanner_build_errors() {
        let mut builder = ptr::null_mut();
        let mut scanner = ptr::dangling_mut::<CribraScanner>();
        let first = rule_config(
            CRIBRA_RULE_KIND_LITERAL,
            "native.duplicate",
            "FIRST_SECRET",
            CRIBRA_SEVERITY_HIGH,
        );
        let second = rule_config(
            CRIBRA_RULE_KIND_LITERAL,
            "native.duplicate",
            "SECOND_SECRET",
            CRIBRA_SEVERITY_CRITICAL,
        );

        unsafe {
            assert_eq!(cribra_builder_new(&mut builder), CRIBRA_OK);
            assert_eq!(cribra_builder_add_rule(builder, &first), CRIBRA_OK);
            assert_eq!(cribra_builder_add_rule(builder, &second), CRIBRA_OK);
            assert_eq!(
                cribra_builder_build(builder, &mut scanner),
                CRIBRA_BUILD_ERROR
            );
            assert!(scanner.is_null());
        }
    }

    #[test]
    fn current_builtins_can_be_combined_with_custom_rules() {
        let mut builder = ptr::null_mut();
        let mut scanner = ptr::null_mut();
        let mut report = ptr::null_mut();
        let custom = rule_config(
            CRIBRA_RULE_KIND_LITERAL,
            "native.extra",
            "NATIVE_EXTRA_SECRET",
            CRIBRA_SEVERITY_HIGH,
        );
        let source = b"GITHUB_TOKEN=ghp_AbCdEf0123456789_AbCdEf0123456789 NATIVE_EXTRA_SECRET";

        unsafe {
            assert_eq!(cribra_builder_new(&mut builder), CRIBRA_OK);
            assert_eq!(
                cribra_builder_add_current_builtins(builder),
                CRIBRA_OK
            );
            assert_eq!(cribra_builder_add_rule(builder, &custom), CRIBRA_OK);
            assert_eq!(cribra_builder_build(builder, &mut scanner), CRIBRA_OK);
            assert_eq!(
                cribra_scanner_scan(
                    scanner,
                    source.as_ptr(),
                    source.len(),
                    &mut report
                ),
                CRIBRA_OK
            );

            let mut count = 0;
            assert_eq!(cribra_report_finding_count(report, &mut count), CRIBRA_OK);
            assert!(count >= 2);

            cribra_report_free(report);
            cribra_scanner_free(scanner);
        }
    }

    #[test]
    fn report_exposes_ambiguous_candidate_without_finding_semantics() {
        let mut scanner = ptr::null_mut();
        let mut report = ptr::null_mut();
        let source = b"backup=ABCD-EFGH-IJKL-MNOP";

        unsafe {
            assert_eq!(cribra_scanner_new_current(&mut scanner), CRIBRA_OK);
            assert_eq!(
                cribra_scanner_scan(scanner, source.as_ptr(), source.len(), &mut report),
                CRIBRA_OK
            );
            let mut finding_count = usize::MAX;
            let mut candidate_count = usize::MAX;
            assert_eq!(cribra_report_finding_count(report, &mut finding_count), CRIBRA_OK);
            assert_eq!(cribra_report_candidate_count(report, &mut candidate_count), CRIBRA_OK);
            assert_eq!(finding_count, 0);
            assert_eq!(candidate_count, 1);
            let mut candidate = CribraCandidateView::default();
            assert_eq!(cribra_report_candidate_at(report, 0, &mut candidate), CRIBRA_OK);
            assert_eq!(candidate.kind, CRIBRA_CANDIDATE_KIND_RECOVERY_LIKE_CODE);
            assert_eq!(candidate.evidence, CRIBRA_CANDIDATE_EVIDENCE_STRUCTURAL);
            assert!(candidate.start < candidate.end);
            let mut explanation = CribraExplanationView::default();
            assert_eq!(cribra_report_explain_candidate(report, 0, &mut explanation), CRIBRA_OK);
            assert_eq!(explanation.kind, CRIBRA_EXPLANATION_AMBIGUOUS);
            assert_eq!(explanation.detection_mode, CRIBRA_DETECTION_MODE_NONE);
            assert_eq!(explanation.candidate_evidence, CRIBRA_CANDIDATE_EVIDENCE_STRUCTURAL);
            cribra_report_free(report);
            cribra_scanner_free(scanner);
        }
    }

    #[test]
    fn finding_explanation_uses_scanner_authority() {
        let mut scanner = ptr::null_mut();
        let mut report = ptr::null_mut();
        let source = b"GITHUB_TOKEN=ghp_AbCdEf0123456789_AbCdEf0123456789";
        unsafe {
            assert_eq!(cribra_scanner_new_current(&mut scanner), CRIBRA_OK);
            assert_eq!(cribra_scanner_scan(scanner, source.as_ptr(), source.len(), &mut report), CRIBRA_OK);
            let mut explanation = CribraExplanationView::default();
            assert_eq!(cribra_scanner_explain_finding(scanner, report, 0, &mut explanation), CRIBRA_OK);
            assert_eq!(explanation.kind, CRIBRA_EXPLANATION_CLASSIFIED);
            assert_eq!(explanation.detection_mode, CRIBRA_DETECTION_MODE_DETERMINISTIC);
            assert_eq!(explanation.candidate_evidence, CRIBRA_CANDIDATE_EVIDENCE_NONE);
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
