//! Native ABI entry points.

use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    ptr, slice, str,
    time::UNIX_EPOCH,
};

use cribra::{
    CandidateEvidence, Confidence, DetectionMode, Explanation, Redaction, Remediation, Rule,
    RuleError, Scanner, ScannerBuildError, SensitiveCandidateKind, Severity, builtins,
    transform::{
        PseudonymizationOptions, ShareBundle, ShareMode, ShareModeKind, SynthesisOptions,
        TemplateOptions, TransformError, pseudonymize, redact, redact_with, synthesize, template,
        template_with,
    },
};

use crate::error::{ErrorSlot, contain_status_with_error, set_error};
use crate::{
    CRIBRA_BATCH_EXECUTION_AUTO, CRIBRA_BATCH_EXECUTION_SERIAL, CRIBRA_BUILD_ERROR,
    CRIBRA_CANDIDATE_EVIDENCE_NONE, CRIBRA_CANDIDATE_EVIDENCE_STRUCTURAL,
    CRIBRA_CANDIDATE_EVIDENCE_UNKNOWN, CRIBRA_CANDIDATE_KIND_RECOVERY_LIKE_CODE,
    CRIBRA_CANDIDATE_KIND_UNKNOWN, CRIBRA_CONFIDENCE_HIGH, CRIBRA_CONFIDENCE_LOW,
    CRIBRA_CONFIDENCE_MEDIUM, CRIBRA_DETECTION_MODE_CONTEXTUAL,
    CRIBRA_DETECTION_MODE_DETERMINISTIC, CRIBRA_DETECTION_MODE_MATCHER_ONLY,
    CRIBRA_DETECTION_MODE_NONE, CRIBRA_DETECTION_MODE_UNKNOWN, CRIBRA_EXPLANATION_AMBIGUOUS,
    CRIBRA_EXPLANATION_CLASSIFIED, CRIBRA_EXPLANATION_UNKNOWN, CRIBRA_INTERNAL_ERROR,
    CRIBRA_INVALID_ARGUMENT, CRIBRA_INVALID_UTF8, CRIBRA_OK, CRIBRA_OUT_OF_RANGE,
    CRIBRA_REMEDIATION_NONE, CRIBRA_REMEDIATION_REMOVE_SENSITIVE_VALUE,
    CRIBRA_REMEDIATION_REPLACE_PRIVATE_KEY, CRIBRA_REMEDIATION_REVIEW_SENSITIVE_HASH,
    CRIBRA_REMEDIATION_REVOKE_AND_ROTATE_CREDENTIAL, CRIBRA_REMEDIATION_ROTATE_CREDENTIAL,
    CRIBRA_REMEDIATION_ROTATE_PASSWORD, CRIBRA_REMEDIATION_UNKNOWN, CRIBRA_RULE_ERROR,
    CRIBRA_RULE_KIND_LITERAL, CRIBRA_RULE_KIND_PATTERN, CRIBRA_RULE_KIND_PREFIX,
    CRIBRA_RULE_KIND_SUFFIX, CRIBRA_SEVERITY_CRITICAL, CRIBRA_SEVERITY_HIGH, CRIBRA_SEVERITY_INFO,
    CRIBRA_SEVERITY_LOW, CRIBRA_SEVERITY_MEDIUM, CRIBRA_SHARE_MODE_PSEUDONYMIZE,
    CRIBRA_SHARE_MODE_REDACT, CRIBRA_SHARE_MODE_SYNTHESIZE, CRIBRA_SHARE_MODE_TEMPLATE,
    CRIBRA_SHARE_MODE_UNKNOWN, CRIBRA_TRANSFORM_ERROR, CribraBatchEntryView, CribraBatchExecution,
    CribraBatchInput, CribraBatchResults, CribraBatchSummary, CribraBuilder,
    CribraCandidateEvidence, CribraCandidateKind, CribraCandidateView, CribraConfidence,
    CribraDetectionMode, CribraError, CribraExplanationView, CribraFindingView, CribraOutput,
    CribraPseudonymizeConfig, CribraRemediation, CribraReport, CribraRuleConfig,
    CribraScanSummaryView, CribraScanner, CribraSeverity, CribraShareBundle,
    CribraShareBundleConfig, CribraShareEntryView, CribraShareManifestView, CribraShareMode,
    CribraStatus, CribraStringView, CribraSynthesizeConfig, CribraTemplateConfig,
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

unsafe fn utf8_from_raw<'a>(source: *const u8, source_len: usize) -> Result<&'a str, CribraStatus> {
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
            Rule::pattern(id, value, severity).map_err(|_| CRIBRA_RULE_ERROR)?
        }
        _ => return Err(CRIBRA_INVALID_ARGUMENT),
    };

    Ok(match remediation {
        Some(remediation) => rule.with_remediation(remediation),
        None => rule,
    })
}

fn rule_error_message(error: &RuleError) -> &'static str {
    match error {
        RuleError::InvalidPattern(_) => "custom rule pattern is invalid",
        RuleError::PatternMatchesEmpty => "custom rule pattern can match empty input",
        RuleError::MissingCaptureGroup { .. } => "rule capture configuration is invalid",
    }
}

fn scanner_build_error_message(error: &ScannerBuildError) -> &'static str {
    match error {
        ScannerBuildError::Rule(error) => rule_error_message(error),
        ScannerBuildError::EmptyRuleId => "rule identifier cannot be empty",
        ScannerBuildError::DuplicateRuleId { .. } => "duplicate rule identifier",
        ScannerBuildError::EmptyMatcher { .. } => "rule matcher cannot be empty",
        ScannerBuildError::TooManyRules => "configured rule count exceeds the scanner limit",
        ScannerBuildError::AutomatonBuild(_) => "scanner matcher compilation failed",
    }
}

fn transform_error_message(error: &TransformError) -> &'static str {
    match error {
        TransformError::InvalidSpan { .. } => "transform report contains an invalid source span",
        TransformError::InvalidUtf8Boundary { .. } => {
            "transform report span is not aligned to UTF-8 boundaries"
        }
        TransformError::OverlappingSpans { .. } => {
            "transform requires non-overlapping finding spans"
        }
        TransformError::MissingShareMode => "share bundle transformation mode is not configured",
        TransformError::SourceCountMismatch { .. } => {
            "share bundle source count does not match scan results"
        }
        TransformError::SourceLengthMismatch { .. } => {
            "share bundle source length does not match scan results"
        }
        _ => "source transformation could not be applied safely",
    }
}

unsafe fn key32_from_raw(key: *const u8, key_len: usize) -> Result<[u8; 32], CribraStatus> {
    if key.is_null() || key_len != 32 {
        return Err(CRIBRA_INVALID_ARGUMENT);
    }
    // SAFETY: caller guarantees exactly `key_len` readable bytes for this call.
    let bytes = unsafe { slice::from_raw_parts(key, key_len) };
    let mut output = [0_u8; 32];
    output.copy_from_slice(bytes);
    Ok(output)
}

fn source_matches_report(source_len: usize, report: &CribraReport) -> bool {
    source_len == report.source_bytes
}

unsafe fn write_output(
    output: Result<String, TransformError>,
    out_output: *mut *mut CribraOutput,
    out_error: *mut *mut CribraError,
) -> CribraStatus {
    let output = match output {
        Ok(output) => output,
        Err(error) => {
            unsafe {
                set_error(
                    out_error,
                    CribraError::new(CRIBRA_TRANSFORM_ERROR, transform_error_message(&error)),
                );
            }
            return CRIBRA_TRANSFORM_ERROR;
        }
    };
    let output = Box::new(CribraOutput::new(output));
    // SAFETY: caller supplied a validated writable out-pointer.
    unsafe { ptr::write(out_output, Box::into_raw(output)) };
    CRIBRA_OK
}

unsafe fn share_mode_from_config(
    config: &CribraShareBundleConfig,
) -> Result<ShareMode, CribraStatus> {
    match config.mode {
        CRIBRA_SHARE_MODE_REDACT => Ok(ShareMode::Redact),
        CRIBRA_SHARE_MODE_TEMPLATE => Ok(ShareMode::Template),
        CRIBRA_SHARE_MODE_PSEUDONYMIZE => {
            let key = unsafe { key32_from_raw(config.key, config.key_len) }?;
            let text = unsafe { utf8_from_raw(config.text.ptr, config.text.len) }?;
            let mut options = PseudonymizationOptions::new(key);
            if !text.is_empty() {
                options = options.prefix(text);
            }
            if config.digest_bytes != 0 {
                options = options.digest_bytes(config.digest_bytes);
            }
            Ok(ShareMode::Pseudonymize(options))
        }
        CRIBRA_SHARE_MODE_SYNTHESIZE => {
            let key = unsafe { key32_from_raw(config.key, config.key_len) }?;
            let text = unsafe { utf8_from_raw(config.text.ptr, config.text.len) }?;
            let mut options = SynthesisOptions::new(key);
            if !text.is_empty() {
                options = options.marker(text);
            }
            Ok(ShareMode::Synthesize(options))
        }
        _ => Err(CRIBRA_INVALID_ARGUMENT),
    }
}

fn share_mode_code(value: ShareModeKind) -> CribraShareMode {
    match value {
        ShareModeKind::Redact => CRIBRA_SHARE_MODE_REDACT,
        ShareModeKind::Template => CRIBRA_SHARE_MODE_TEMPLATE,
        ShareModeKind::Pseudonymize => CRIBRA_SHARE_MODE_PSEUDONYMIZE,
        ShareModeKind::Synthesize => CRIBRA_SHARE_MODE_SYNTHESIZE,
        _ => CRIBRA_SHARE_MODE_UNKNOWN,
    }
}

fn scan_summary_view(summary: cribra::ScanSummary) -> CribraScanSummaryView {
    CribraScanSummaryView {
        scanned_sources: summary.scanned_sources(),
        scanned_bytes: summary.scanned_bytes(),
        reports_with_findings: summary.reports_with_findings(),
        reports_without_findings: summary.reports_without_findings(),
        total_findings: summary.total_findings(),
        total_candidates: summary.total_candidates(),
        reports_with_candidates: summary.reports_with_candidates(),
        critical: summary.critical(),
        high: summary.high(),
        medium: summary.medium(),
        low: summary.low(),
        info: summary.info(),
    }
}

fn share_manifest_view(
    manifest: &cribra::transform::ShareManifest,
) -> Result<CribraShareManifestView, CribraStatus> {
    let generated = manifest
        .generated_at()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| CRIBRA_INTERNAL_ERROR)?;

    Ok(CribraShareManifestView {
        mode: share_mode_code(manifest.mode()),
        summary: scan_summary_view(manifest.summary()),
        generated_at_secs: generated.as_secs(),
        generated_at_nanos: generated.subsec_nanos(),
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
    out_error: *mut *mut CribraError,
) -> CribraStatus {
    let error_slot = unsafe { ErrorSlot::from_raw(out_error) };
    contain_status_with_error(error_slot, || {
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
    out_error: *mut *mut CribraError,
) -> CribraStatus {
    let error_slot = unsafe { ErrorSlot::from_raw(out_error) };
    contain_status_with_error(error_slot, || {
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
            Err(error) => {
                unsafe {
                    set_error(
                        out_error,
                        CribraError::new(CRIBRA_BUILD_ERROR, scanner_build_error_message(&error)),
                    );
                }
                CRIBRA_BUILD_ERROR
            }
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
    out_error: *mut *mut CribraError,
) -> CribraStatus {
    let error_slot = unsafe { ErrorSlot::from_raw(out_error) };
    contain_status_with_error(error_slot, || {
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

        let report = Box::new(CribraReport::new(report, source_len));
        // SAFETY: `clear_out` validated the out-pointer.
        unsafe { ptr::write(out_report, Box::into_raw(report)) };
        CRIBRA_OK
    })
}

/// Scans an ordered batch of caller-owned UTF-8 sources.
///
/// The input descriptor array and all pointed-to bytes are borrowed only for
/// this call. Keys are copied into Rust-owned result storage. Source text is
/// never retained. Successful results preserve input order exactly.
///
/// If any descriptor contains invalid pointer/UTF-8 input, the whole call fails
/// before scanning and `out_results` remains null; partial results are never
/// returned.
///
/// `execution` accepts [`CRIBRA_BATCH_EXECUTION_AUTO`] or
/// [`CRIBRA_BATCH_EXECUTION_SERIAL`]. AUTO remains semantically identical to
/// SERIAL and may use Cribra's parallel implementation only when the adapter is
/// built with its optional `parallel` feature.
///
/// `inputs == NULL` with `input_count == 0` is accepted as an empty batch.
///
/// # Safety
///
/// `scanner` must be a live scanner handle. When `input_count > 0`, `inputs`
/// must reference at least `input_count` readable [`CribraBatchInput`] values.
/// Every non-empty string view must reference readable bytes for the duration of
/// this call. `out_results` must point to writable memory for one batch-results
/// pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cribra_scanner_scan_batch(
    scanner: *const CribraScanner,
    inputs: *const CribraBatchInput,
    input_count: usize,
    execution: CribraBatchExecution,
    out_results: *mut *mut CribraBatchResults,
    out_error: *mut *mut CribraError,
) -> CribraStatus {
    let error_slot = unsafe { ErrorSlot::from_raw(out_error) };
    contain_status_with_error(error_slot, || {
        if let Err(status) = unsafe { clear_out(out_results) } {
            return status;
        }
        if scanner.is_null() {
            return CRIBRA_INVALID_ARGUMENT;
        }
        if input_count > 0 && inputs.is_null() {
            return CRIBRA_INVALID_ARGUMENT;
        }

        let descriptors = if input_count == 0 {
            &[][..]
        } else {
            // SAFETY: caller guarantees `input_count` readable descriptors.
            unsafe { slice::from_raw_parts(inputs, input_count) }
        };

        // Validate the complete boundary before scanning so failures are atomic.
        let mut prepared = Vec::with_capacity(descriptors.len());
        for descriptor in descriptors {
            let key = match unsafe { utf8_from_raw(descriptor.key.ptr, descriptor.key.len) } {
                Ok(value) => value.to_owned(),
                Err(status) => return status,
            };
            let source =
                match unsafe { utf8_from_raw(descriptor.source.ptr, descriptor.source.len) } {
                    Ok(value) => value,
                    Err(status) => return status,
                };
            prepared.push((key, source));
        }

        // SAFETY: caller guarantees a live immutable scanner handle.
        let scanner = unsafe { &*scanner };

        let results = match execution {
            CRIBRA_BATCH_EXECUTION_SERIAL => scanner.inner.scan(prepared),
            CRIBRA_BATCH_EXECUTION_AUTO => {
                #[cfg(feature = "parallel")]
                {
                    scanner.inner.parallel_scan(prepared)
                }
                #[cfg(not(feature = "parallel"))]
                {
                    scanner.inner.scan(prepared)
                }
            }
            _ => return CRIBRA_INVALID_ARGUMENT,
        };

        let results = Box::new(CribraBatchResults::new(results));

        // SAFETY: `clear_out` validated the out-pointer.
        unsafe { ptr::write(out_results, Box::into_raw(results)) };
        CRIBRA_OK
    })
}

/// Returns the number of source entries in ordered batch results.
///
/// # Safety
///
/// `results` must be a live batch-results handle. `out_count` must point to
/// writable memory for one `size_t`-compatible value.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cribra_batch_results_count(
    results: *const CribraBatchResults,
    out_count: *mut usize,
) -> CribraStatus {
    contain_status(|| {
        if let Err(status) = unsafe { clear_value(out_count) } {
            return status;
        }
        if results.is_null() {
            return CRIBRA_INVALID_ARGUMENT;
        }

        // SAFETY: caller guarantees a live immutable batch-results handle.
        let results = unsafe { &*results };
        // SAFETY: `clear_value` validated the out-pointer.
        unsafe { ptr::write(out_count, results.inner.len()) };
        CRIBRA_OK
    })
}

/// Returns a borrowed metadata projection for one ordered batch entry.
///
/// The returned key view borrows storage owned by `results` and remains valid
/// only while that parent handle remains alive.
///
/// # Safety
///
/// `results` must be a live batch-results handle. `out_entry` must point to
/// writable memory for one [`CribraBatchEntryView`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cribra_batch_results_entry_at(
    results: *const CribraBatchResults,
    index: usize,
    out_entry: *mut CribraBatchEntryView,
) -> CribraStatus {
    contain_status(|| {
        if let Err(status) = unsafe { clear_value(out_entry) } {
            return status;
        }
        if results.is_null() {
            return CRIBRA_INVALID_ARGUMENT;
        }

        // SAFETY: caller guarantees a live immutable batch-results handle.
        let results = unsafe { &*results };
        let Some(entry) = results.inner.as_slice().get(index) else {
            return CRIBRA_OUT_OF_RANGE;
        };

        let view = CribraBatchEntryView {
            key: string_view(entry.key()),
            source_bytes: entry.source_bytes(),
            finding_count: entry.report().len(),
            candidate_count: entry.report().candidate_len(),
        };

        // SAFETY: `clear_value` validated the out-pointer.
        unsafe { ptr::write(out_entry, view) };
        CRIBRA_OK
    })
}

/// Returns one classified finding from an ordered batch entry.
///
/// No child report handle is created; the returned view remains subordinate to
/// the parent batch-results lifetime.
///
/// # Safety
///
/// `results` must be a live batch-results handle. `out_finding` must point to
/// writable memory for one [`CribraFindingView`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cribra_batch_results_finding_at(
    results: *const CribraBatchResults,
    entry_index: usize,
    finding_index: usize,
    out_finding: *mut CribraFindingView,
) -> CribraStatus {
    contain_status(|| {
        if let Err(status) = unsafe { clear_value(out_finding) } {
            return status;
        }
        if results.is_null() {
            return CRIBRA_INVALID_ARGUMENT;
        }

        // SAFETY: caller guarantees a live immutable batch-results handle.
        let results = unsafe { &*results };
        let Some(entry) = results.inner.as_slice().get(entry_index) else {
            return CRIBRA_OUT_OF_RANGE;
        };
        let Some(finding) = entry.report().findings().get(finding_index) else {
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

/// Returns one ambiguous candidate from an ordered batch entry.
///
/// No child report handle is created.
///
/// # Safety
///
/// `results` must be a live batch-results handle. `out_candidate` must point to
/// writable memory for one [`CribraCandidateView`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cribra_batch_results_candidate_at(
    results: *const CribraBatchResults,
    entry_index: usize,
    candidate_index: usize,
    out_candidate: *mut CribraCandidateView,
) -> CribraStatus {
    contain_status(|| {
        if let Err(status) = unsafe { clear_value(out_candidate) } {
            return status;
        }
        if results.is_null() {
            return CRIBRA_INVALID_ARGUMENT;
        }

        // SAFETY: caller guarantees a live immutable batch-results handle.
        let results = unsafe { &*results };
        let Some(entry) = results.inner.as_slice().get(entry_index) else {
            return CRIBRA_OUT_OF_RANGE;
        };
        let Some(candidate) = entry.report().candidates().get(candidate_index) else {
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

/// Returns aggregate metadata for ordered batch results.
///
/// # Safety
///
/// `results` must be a live batch-results handle. `out_summary` must point to
/// writable memory for one [`CribraBatchSummary`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cribra_batch_results_summary(
    results: *const CribraBatchResults,
    out_summary: *mut CribraBatchSummary,
) -> CribraStatus {
    contain_status(|| {
        if let Err(status) = unsafe { clear_value(out_summary) } {
            return status;
        }
        if results.is_null() {
            return CRIBRA_INVALID_ARGUMENT;
        }

        // SAFETY: caller guarantees a live immutable batch-results handle.
        let results = unsafe { &*results };
        let view = CribraBatchSummary {
            sources: results.inner.len(),
            source_bytes: results.inner.total_bytes(),
            findings: results.inner.total_findings(),
            candidates: results.inner.total_candidates(),
        };

        // SAFETY: `clear_value` validated the out-pointer.
        unsafe { ptr::write(out_summary, view) };
        CRIBRA_OK
    })
}

/// Releases ordered batch results. Passing null is a no-op.
///
/// # Safety
///
/// A non-null `results` must be a live batch-results handle that has not already
/// been freed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cribra_batch_results_free(results: *mut CribraBatchResults) {
    if results.is_null() {
        return;
    }

    contain_drop(|| {
        // SAFETY: caller transfers ownership of one live handle.
        drop(unsafe { Box::from_raw(results) });
    });
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
    out_error: *mut *mut CribraError,
) -> CribraStatus {
    let error_slot = unsafe { ErrorSlot::from_raw(out_error) };
    contain_status_with_error(error_slot, || {
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
            unsafe {
                set_error(
                    out_error,
                    CribraError::new(
                        CRIBRA_INVALID_ARGUMENT,
                        "finding explanation authority does not match scanner",
                    ),
                );
            }
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

/// Redacts all classified finding spans using Cribra's standard marker.
///
/// The returned output is Rust-owned and must be released exactly once with
/// [`cribra_output_free`].
///
/// # Safety
///
/// `report` must be live. `source` follows the same UTF-8 buffer contract as
/// [`cribra_scanner_scan`]. `out_output` must be writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cribra_transform_redact(
    source: *const u8,
    source_len: usize,
    report: *const CribraReport,
    out_output: *mut *mut CribraOutput,
    out_error: *mut *mut CribraError,
) -> CribraStatus {
    let error_slot = unsafe { ErrorSlot::from_raw(out_error) };
    contain_status_with_error(error_slot, || {
        if let Err(status) = unsafe { clear_out(out_output) } {
            return status;
        }
        if report.is_null() {
            return CRIBRA_INVALID_ARGUMENT;
        }
        let source = match unsafe { utf8_from_raw(source, source_len) } {
            Ok(value) => value,
            Err(status) => return status,
        };
        // SAFETY: caller guarantees a live immutable report.
        let report = unsafe { &*report };
        if !source_matches_report(source.len(), report) {
            unsafe {
                set_error(
                    out_error,
                    CribraError::new(
                        CRIBRA_TRANSFORM_ERROR,
                        "transform source length does not match report",
                    ),
                );
            }
            return CRIBRA_TRANSFORM_ERROR;
        }
        // SAFETY: `clear_out` validated the output pointer.
        unsafe { write_output(redact(source, &report.inner), out_output, out_error) }
    })
}

/// Redacts findings using a caller-selected UTF-8 replacement.
///
/// # Safety
///
/// Pointer contracts are identical to [`cribra_transform_redact`]. `replacement`
/// is borrowed only for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cribra_transform_redact_with(
    source: *const u8,
    source_len: usize,
    report: *const CribraReport,
    replacement: *const u8,
    replacement_len: usize,
    out_output: *mut *mut CribraOutput,
    out_error: *mut *mut CribraError,
) -> CribraStatus {
    let error_slot = unsafe { ErrorSlot::from_raw(out_error) };
    contain_status_with_error(error_slot, || {
        if let Err(status) = unsafe { clear_out(out_output) } {
            return status;
        }
        if report.is_null() {
            return CRIBRA_INVALID_ARGUMENT;
        }
        let source = match unsafe { utf8_from_raw(source, source_len) } {
            Ok(value) => value,
            Err(status) => return status,
        };
        let replacement = match unsafe { utf8_from_raw(replacement, replacement_len) } {
            Ok(value) => value,
            Err(status) => return status,
        };
        // SAFETY: caller guarantees a live immutable report.
        let report = unsafe { &*report };
        if !source_matches_report(source.len(), report) {
            unsafe {
                set_error(
                    out_error,
                    CribraError::new(
                        CRIBRA_TRANSFORM_ERROR,
                        "transform source length does not match report",
                    ),
                );
            }
            return CRIBRA_TRANSFORM_ERROR;
        }
        let redaction = Redaction::new(replacement);
        // SAFETY: `clear_out` validated the output pointer.
        unsafe {
            write_output(
                redact_with(source, &report.inner, &redaction),
                out_output,
                out_error,
            )
        }
    })
}

/// Generates semantic placeholders using default template options.
///
/// # Safety
///
/// Pointer contracts are identical to [`cribra_transform_redact`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cribra_transform_template(
    source: *const u8,
    source_len: usize,
    report: *const CribraReport,
    out_output: *mut *mut CribraOutput,
    out_error: *mut *mut CribraError,
) -> CribraStatus {
    let error_slot = unsafe { ErrorSlot::from_raw(out_error) };
    contain_status_with_error(error_slot, || {
        if let Err(status) = unsafe { clear_out(out_output) } {
            return status;
        }
        if report.is_null() {
            return CRIBRA_INVALID_ARGUMENT;
        }
        let source = match unsafe { utf8_from_raw(source, source_len) } {
            Ok(value) => value,
            Err(status) => return status,
        };
        // SAFETY: caller guarantees a live immutable report.
        let report = unsafe { &*report };
        if !source_matches_report(source.len(), report) {
            unsafe {
                set_error(
                    out_error,
                    CribraError::new(
                        CRIBRA_TRANSFORM_ERROR,
                        "transform source length does not match report",
                    ),
                );
            }
            return CRIBRA_TRANSFORM_ERROR;
        }
        // SAFETY: `clear_out` validated the output pointer.
        unsafe { write_output(template(source, &report.inner), out_output, out_error) }
    })
}

/// Generates semantic placeholders using explicit configuration.
///
/// `numbered` accepts only `0` or `1`.
///
/// # Safety
///
/// `config` must be readable for this call. Its namespace view is borrowed only
/// for this call. Other pointer contracts match [`cribra_transform_redact`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cribra_transform_template_with(
    source: *const u8,
    source_len: usize,
    report: *const CribraReport,
    config: *const CribraTemplateConfig,
    out_output: *mut *mut CribraOutput,
    out_error: *mut *mut CribraError,
) -> CribraStatus {
    let error_slot = unsafe { ErrorSlot::from_raw(out_error) };
    contain_status_with_error(error_slot, || {
        if let Err(status) = unsafe { clear_out(out_output) } {
            return status;
        }
        if report.is_null() || config.is_null() {
            return CRIBRA_INVALID_ARGUMENT;
        }
        let source = match unsafe { utf8_from_raw(source, source_len) } {
            Ok(value) => value,
            Err(status) => return status,
        };
        // SAFETY: caller guarantees a readable configuration.
        let config = unsafe { &*config };
        let namespace = match unsafe { utf8_from_raw(config.namespace.ptr, config.namespace.len) } {
            Ok(value) => value,
            Err(status) => return status,
        };
        let numbered = match config.numbered {
            0 => false,
            1 => true,
            _ => return CRIBRA_INVALID_ARGUMENT,
        };
        let options = TemplateOptions::new()
            .namespace(namespace)
            .numbered(numbered);
        // SAFETY: caller guarantees a live immutable report.
        let report = unsafe { &*report };
        if !source_matches_report(source.len(), report) {
            unsafe {
                set_error(
                    out_error,
                    CribraError::new(
                        CRIBRA_TRANSFORM_ERROR,
                        "transform source length does not match report",
                    ),
                );
            }
            return CRIBRA_TRANSFORM_ERROR;
        }
        // SAFETY: `clear_out` validated the output pointer.
        unsafe {
            write_output(
                template_with(source, &report.inner, &options),
                out_output,
                out_error,
            )
        }
    })
}

/// Pseudonymizes findings with a mandatory caller-provided 32-byte key.
///
/// # Safety
///
/// `config` and its referenced key/prefix bytes must remain readable for this
/// call. Other pointer contracts match [`cribra_transform_redact`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cribra_transform_pseudonymize(
    source: *const u8,
    source_len: usize,
    report: *const CribraReport,
    config: *const CribraPseudonymizeConfig,
    out_output: *mut *mut CribraOutput,
    out_error: *mut *mut CribraError,
) -> CribraStatus {
    let error_slot = unsafe { ErrorSlot::from_raw(out_error) };
    contain_status_with_error(error_slot, || {
        if let Err(status) = unsafe { clear_out(out_output) } {
            return status;
        }
        if report.is_null() || config.is_null() {
            return CRIBRA_INVALID_ARGUMENT;
        }
        let source = match unsafe { utf8_from_raw(source, source_len) } {
            Ok(value) => value,
            Err(status) => return status,
        };
        // SAFETY: caller guarantees a readable configuration.
        let config = unsafe { &*config };
        let key = match unsafe { key32_from_raw(config.key, config.key_len) } {
            Ok(value) => value,
            Err(status) => return status,
        };
        let prefix = match unsafe { utf8_from_raw(config.prefix.ptr, config.prefix.len) } {
            Ok(value) => value,
            Err(status) => return status,
        };
        let options = PseudonymizationOptions::new(key)
            .prefix(prefix)
            .digest_bytes(config.digest_bytes);
        // SAFETY: caller guarantees a live immutable report.
        let report = unsafe { &*report };
        if !source_matches_report(source.len(), report) {
            unsafe {
                set_error(
                    out_error,
                    CribraError::new(
                        CRIBRA_TRANSFORM_ERROR,
                        "transform source length does not match report",
                    ),
                );
            }
            return CRIBRA_TRANSFORM_ERROR;
        }
        // SAFETY: `clear_out` validated the output pointer.
        unsafe {
            write_output(
                pseudonymize(source, &report.inner, &options),
                out_output,
                out_error,
            )
        }
    })
}

/// Synthesizes deterministic share-safe values with a caller-provided key.
///
/// # Safety
///
/// `config` and its referenced key/marker bytes must remain readable for this
/// call. Other pointer contracts match [`cribra_transform_redact`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cribra_transform_synthesize(
    source: *const u8,
    source_len: usize,
    report: *const CribraReport,
    config: *const CribraSynthesizeConfig,
    out_output: *mut *mut CribraOutput,
    out_error: *mut *mut CribraError,
) -> CribraStatus {
    let error_slot = unsafe { ErrorSlot::from_raw(out_error) };
    contain_status_with_error(error_slot, || {
        if let Err(status) = unsafe { clear_out(out_output) } {
            return status;
        }
        if report.is_null() || config.is_null() {
            return CRIBRA_INVALID_ARGUMENT;
        }
        let source = match unsafe { utf8_from_raw(source, source_len) } {
            Ok(value) => value,
            Err(status) => return status,
        };
        // SAFETY: caller guarantees a readable configuration.
        let config = unsafe { &*config };
        let key = match unsafe { key32_from_raw(config.key, config.key_len) } {
            Ok(value) => value,
            Err(status) => return status,
        };
        let marker = match unsafe { utf8_from_raw(config.marker.ptr, config.marker.len) } {
            Ok(value) => value,
            Err(status) => return status,
        };
        let options = SynthesisOptions::new(key).marker(marker);
        // SAFETY: caller guarantees a live immutable report.
        let report = unsafe { &*report };
        if !source_matches_report(source.len(), report) {
            unsafe {
                set_error(
                    out_error,
                    CribraError::new(
                        CRIBRA_TRANSFORM_ERROR,
                        "transform source length does not match report",
                    ),
                );
            }
            return CRIBRA_TRANSFORM_ERROR;
        }
        // SAFETY: `clear_out` validated the output pointer.
        unsafe {
            write_output(
                synthesize(source, &report.inner, &options),
                out_output,
                out_error,
            )
        }
    })
}

/// Builds a share-safe transformed batch from ordered scan results.
///
/// `sources` must contain the same logical UTF-8 sources, in the same order,
/// that produced `results`. The core validates source count and byte lengths
/// before transformation. Source text is borrowed only for this call and is
/// never retained.
///
/// On success, the returned bundle owns cloned source keys, transformed content,
/// and share-safe manifest metadata. It must eventually be released with
/// [`cribra_share_bundle_free`].
///
/// # Safety
///
/// `results` must be a live batch-results handle. `config` must point to a
/// readable [`CribraShareBundleConfig`]. When `source_count > 0`, `sources` must
/// reference at least `source_count` readable [`CribraStringView`] values. Every
/// non-empty source/config text view must reference readable bytes for the
/// duration of this call. `out_bundle` must point to writable memory for one
/// share-bundle pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cribra_share_bundle_build(
    results: *const CribraBatchResults,
    sources: *const CribraStringView,
    source_count: usize,
    config: *const CribraShareBundleConfig,
    out_bundle: *mut *mut CribraShareBundle,
    out_error: *mut *mut CribraError,
) -> CribraStatus {
    let error_slot = unsafe { ErrorSlot::from_raw(out_error) };
    contain_status_with_error(error_slot, || {
        if let Err(status) = unsafe { clear_out(out_bundle) } {
            return status;
        }
        if results.is_null() || config.is_null() {
            return CRIBRA_INVALID_ARGUMENT;
        }
        if source_count > 0 && sources.is_null() {
            return CRIBRA_INVALID_ARGUMENT;
        }

        // SAFETY: caller guarantees live/readable values for this call.
        let results = unsafe { &*results };
        // SAFETY: caller guarantees a readable configuration value.
        let config = unsafe { &*config };

        let mode = match unsafe { share_mode_from_config(config) } {
            Ok(mode) => mode,
            Err(status) => return status,
        };

        let source_views = if source_count == 0 {
            &[][..]
        } else {
            // SAFETY: caller guarantees `source_count` readable views.
            unsafe { slice::from_raw_parts(sources, source_count) }
        };

        let mut prepared = Vec::with_capacity(source_views.len());
        for source in source_views {
            let source = match unsafe { utf8_from_raw(source.ptr, source.len) } {
                Ok(source) => source,
                Err(status) => return status,
            };
            prepared.push(source);
        }

        let bundle = match ShareBundle::builder()
            .mode(mode)
            .build(&results.inner, prepared)
        {
            Ok(bundle) => bundle,
            Err(error) => {
                unsafe {
                    set_error(
                        out_error,
                        CribraError::new(CRIBRA_TRANSFORM_ERROR, transform_error_message(&error)),
                    );
                }
                return CRIBRA_TRANSFORM_ERROR;
            }
        };

        let bundle = Box::new(CribraShareBundle::new(bundle));
        // SAFETY: `clear_out` validated the out-pointer.
        unsafe { ptr::write(out_bundle, Box::into_raw(bundle)) };
        CRIBRA_OK
    })
}

/// Returns the number of transformed sources in a share bundle.
///
/// # Safety
///
/// `bundle` must be a live share-bundle handle. `out_count` must point to
/// writable memory for one `size_t`-compatible value.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cribra_share_bundle_count(
    bundle: *const CribraShareBundle,
    out_count: *mut usize,
) -> CribraStatus {
    contain_status(|| {
        if let Err(status) = unsafe { clear_value(out_count) } {
            return status;
        }
        if bundle.is_null() {
            return CRIBRA_INVALID_ARGUMENT;
        }

        // SAFETY: caller guarantees a live immutable share-bundle handle.
        let bundle = unsafe { &*bundle };
        // SAFETY: `clear_value` validated the out-pointer.
        unsafe { ptr::write(out_count, bundle.inner.len()) };
        CRIBRA_OK
    })
}

/// Returns one borrowed transformed-source projection.
///
/// Both returned string views remain valid only while `bundle` remains alive.
///
/// # Safety
///
/// `bundle` must be a live share-bundle handle. `out_entry` must point to
/// writable memory for one [`CribraShareEntryView`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cribra_share_bundle_entry_at(
    bundle: *const CribraShareBundle,
    index: usize,
    out_entry: *mut CribraShareEntryView,
) -> CribraStatus {
    contain_status(|| {
        if let Err(status) = unsafe { clear_value(out_entry) } {
            return status;
        }
        if bundle.is_null() {
            return CRIBRA_INVALID_ARGUMENT;
        }

        // SAFETY: caller guarantees a live immutable share-bundle handle.
        let bundle = unsafe { &*bundle };
        let Some(entry) = bundle.inner.sources().get(index) else {
            return CRIBRA_OUT_OF_RANGE;
        };

        let view = CribraShareEntryView {
            key: string_view(entry.key()),
            content: string_view(entry.content()),
        };

        // SAFETY: `clear_value` validated the out-pointer.
        unsafe { ptr::write(out_entry, view) };
        CRIBRA_OK
    })
}

/// Returns the share-safe bundle manifest.
///
/// The projection contains only transformation kind, aggregate counters, and
/// generation time. It never contains source text or keyed-transform secrets.
///
/// # Safety
///
/// `bundle` must be a live share-bundle handle. `out_manifest` must point to
/// writable memory for one [`CribraShareManifestView`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cribra_share_bundle_manifest(
    bundle: *const CribraShareBundle,
    out_manifest: *mut CribraShareManifestView,
) -> CribraStatus {
    contain_status(|| {
        if let Err(status) = unsafe { clear_value(out_manifest) } {
            return status;
        }
        if bundle.is_null() {
            return CRIBRA_INVALID_ARGUMENT;
        }

        // SAFETY: caller guarantees a live immutable share-bundle handle.
        let bundle = unsafe { &*bundle };
        let manifest = match share_manifest_view(bundle.inner.manifest()) {
            Ok(manifest) => manifest,
            Err(status) => return status,
        };

        // SAFETY: `clear_value` validated the out-pointer.
        unsafe { ptr::write(out_manifest, manifest) };
        CRIBRA_OK
    })
}

/// Releases a share bundle. Passing null is a no-op.
///
/// # Safety
///
/// A non-null `bundle` must be a live share-bundle handle that has not already
/// been freed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cribra_share_bundle_free(bundle: *mut CribraShareBundle) {
    if bundle.is_null() {
        return;
    }

    contain_drop(|| {
        // SAFETY: caller transfers ownership of one live handle.
        drop(unsafe { Box::from_raw(bundle) });
    });
}

/// Borrows the UTF-8 bytes owned by a transformed output handle.
///
/// The returned view remains valid only until [`cribra_output_free`] is called.
///
/// # Safety
///
/// `output` must be live and `out_view` must be writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cribra_output_view(
    output: *const CribraOutput,
    out_view: *mut CribraStringView,
) -> CribraStatus {
    contain_status(|| {
        if let Err(status) = unsafe { clear_value(out_view) } {
            return status;
        }
        if output.is_null() {
            return CRIBRA_INVALID_ARGUMENT;
        }
        // SAFETY: caller guarantees a live immutable output handle.
        let output = unsafe { &*output };
        // SAFETY: `clear_value` validated the output pointer.
        unsafe { ptr::write(out_view, string_view(&output.inner)) };
        CRIBRA_OK
    })
}

/// Releases one Rust-owned transformed output. Passing null is a no-op.
///
/// # Safety
///
/// A non-null `output` must be a live handle returned by a transform function
/// and must not already have been freed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cribra_output_free(output: *mut CribraOutput) {
    if output.is_null() {
        return;
    }
    contain_drop(|| {
        // SAFETY: caller transfers ownership of one live output handle.
        drop(unsafe { Box::from_raw(output) });
    });
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
                cribra_scanner_scan(
                    scanner,
                    source.as_ptr(),
                    source.len(),
                    &mut report,
                    ptr::null_mut()
                ),
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
            assert_eq!(
                cribra_builder_build(builder, &mut scanner, ptr::null_mut()),
                CRIBRA_OK
            );
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
                cribra_scanner_scan(
                    scanner,
                    source.as_ptr(),
                    source.len(),
                    &mut report,
                    ptr::null_mut()
                ),
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
                cribra_scanner_scan(scanner, ptr::null(), 0, &mut report, ptr::null_mut()),
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
                cribra_scanner_scan(scanner, ptr::null(), 1, &mut report, ptr::null_mut()),
                CRIBRA_INVALID_ARGUMENT
            );
            assert!(report.is_null());
            cribra_scanner_free(scanner);
        }
    }

    #[test]
    fn batch_scan_preserves_input_order_and_owns_keys() {
        let mut scanner = ptr::null_mut();
        let mut results = ptr::null_mut();

        let first_key = String::from("first");
        let second_key = String::from("second");
        let first_source = String::from("clean source");
        let second_source = String::from("GITHUB_TOKEN=ghp_AbCdEf0123456789_AbCdEf0123456789");
        let inputs = [
            CribraBatchInput {
                key: string_input(&first_key),
                source: string_input(&first_source),
            },
            CribraBatchInput {
                key: string_input(&second_key),
                source: string_input(&second_source),
            },
        ];

        unsafe {
            assert_eq!(cribra_scanner_new_current(&mut scanner), CRIBRA_OK);
            assert_eq!(
                cribra_scanner_scan_batch(
                    scanner,
                    inputs.as_ptr(),
                    inputs.len(),
                    CRIBRA_BATCH_EXECUTION_AUTO,
                    &mut results,
                    ptr::null_mut(),
                ),
                CRIBRA_OK
            );
        }

        drop(first_key);
        drop(second_key);
        drop(first_source);
        drop(second_source);

        unsafe {
            let mut count = usize::MAX;
            assert_eq!(cribra_batch_results_count(results, &mut count), CRIBRA_OK);
            assert_eq!(count, 2);

            let entries = (*results).inner.as_slice();
            assert_eq!(entries[0].key(), "first");
            assert_eq!(entries[1].key(), "second");
            assert!(entries[0].report().is_empty());
            assert!(!entries[1].report().is_empty());

            cribra_batch_results_free(results);
            cribra_scanner_free(scanner);
        }
    }

    #[test]
    fn invalid_batch_execution_is_rejected_without_results() {
        let mut scanner = ptr::null_mut();
        let mut results = ptr::dangling_mut::<CribraBatchResults>();

        unsafe {
            assert_eq!(cribra_scanner_new_current(&mut scanner), CRIBRA_OK);
            assert_eq!(
                cribra_scanner_scan_batch(
                    scanner,
                    ptr::null(),
                    0,
                    u32::MAX,
                    &mut results,
                    ptr::null_mut(),
                ),
                CRIBRA_INVALID_ARGUMENT
            );
            assert!(results.is_null());
            cribra_scanner_free(scanner);
        }
    }

    #[test]
    fn auto_and_serial_batch_results_are_semantically_equivalent() {
        let mut scanner = ptr::null_mut();
        let mut auto_results = ptr::null_mut();
        let mut serial_results = ptr::null_mut();

        let inputs = [
            CribraBatchInput {
                key: string_input("classified"),
                source: string_input("GITHUB_TOKEN=ghp_AbCdEf0123456789_AbCdEf0123456789"),
            },
            CribraBatchInput {
                key: string_input("ambiguous"),
                source: string_input("backup=ABCD-EFGH-IJKL-MNOP"),
            },
            CribraBatchInput {
                key: string_input("clean"),
                source: string_input("ordinary text"),
            },
        ];

        unsafe {
            assert_eq!(cribra_scanner_new_current(&mut scanner), CRIBRA_OK);

            assert_eq!(
                cribra_scanner_scan_batch(
                    scanner,
                    inputs.as_ptr(),
                    inputs.len(),
                    CRIBRA_BATCH_EXECUTION_AUTO,
                    &mut auto_results,
                    ptr::null_mut(),
                ),
                CRIBRA_OK
            );
            assert_eq!(
                cribra_scanner_scan_batch(
                    scanner,
                    inputs.as_ptr(),
                    inputs.len(),
                    CRIBRA_BATCH_EXECUTION_SERIAL,
                    &mut serial_results,
                    ptr::null_mut(),
                ),
                CRIBRA_OK
            );

            let auto_entries = (*auto_results).inner.as_slice();
            let serial_entries = (*serial_results).inner.as_slice();

            assert_eq!(auto_entries.len(), serial_entries.len());
            for (auto, serial) in auto_entries.iter().zip(serial_entries) {
                assert_eq!(auto.key(), serial.key());
                assert_eq!(auto.source_bytes(), serial.source_bytes());
                assert_eq!(auto.report().findings(), serial.report().findings());
                assert_eq!(auto.report().candidates(), serial.report().candidates());
            }

            cribra_batch_results_free(auto_results);
            cribra_batch_results_free(serial_results);
            cribra_scanner_free(scanner);
        }
    }

    #[test]
    fn batch_traversal_exposes_entries_findings_candidates_and_summary() {
        let mut scanner = ptr::null_mut();
        let mut results = ptr::null_mut();

        let first_key = "classified";
        let second_key = "ambiguous";
        let first_source = "GITHUB_TOKEN=ghp_AbCdEf0123456789_AbCdEf0123456789";
        let second_source = "backup=ABCD-EFGH-IJKL-MNOP";
        let inputs = [
            CribraBatchInput {
                key: string_input(first_key),
                source: string_input(first_source),
            },
            CribraBatchInput {
                key: string_input(second_key),
                source: string_input(second_source),
            },
        ];

        unsafe {
            assert_eq!(cribra_scanner_new_current(&mut scanner), CRIBRA_OK);
            assert_eq!(
                cribra_scanner_scan_batch(
                    scanner,
                    inputs.as_ptr(),
                    inputs.len(),
                    CRIBRA_BATCH_EXECUTION_AUTO,
                    &mut results,
                    ptr::null_mut(),
                ),
                CRIBRA_OK
            );

            let mut first = CribraBatchEntryView::default();
            assert_eq!(
                cribra_batch_results_entry_at(results, 0, &mut first),
                CRIBRA_OK
            );
            let first_key_bytes = slice::from_raw_parts(first.key.ptr, first.key.len);
            assert_eq!(str::from_utf8(first_key_bytes).unwrap(), "classified");
            assert_eq!(first.source_bytes, first_source.len());
            assert!(first.finding_count >= 1);
            assert_eq!(first.candidate_count, 0);

            let mut finding = CribraFindingView::default();
            assert_eq!(
                cribra_batch_results_finding_at(results, 0, 0, &mut finding),
                CRIBRA_OK
            );
            assert!(finding.start < finding.end);

            let mut second = CribraBatchEntryView::default();
            assert_eq!(
                cribra_batch_results_entry_at(results, 1, &mut second),
                CRIBRA_OK
            );
            assert_eq!(second.source_bytes, second_source.len());
            assert_eq!(second.finding_count, 0);
            assert_eq!(second.candidate_count, 1);

            let mut candidate = CribraCandidateView::default();
            assert_eq!(
                cribra_batch_results_candidate_at(results, 1, 0, &mut candidate),
                CRIBRA_OK
            );
            assert_eq!(candidate.kind, CRIBRA_CANDIDATE_KIND_RECOVERY_LIKE_CODE);

            let mut summary = CribraBatchSummary::default();
            assert_eq!(
                cribra_batch_results_summary(results, &mut summary),
                CRIBRA_OK
            );
            assert_eq!(summary.sources, 2);
            assert_eq!(
                summary.source_bytes,
                first_source.len() + second_source.len()
            );
            assert!(summary.findings >= 1);
            assert_eq!(summary.candidates, 1);

            cribra_batch_results_free(results);
            cribra_scanner_free(scanner);
        }
    }

    #[test]
    fn batch_nested_out_of_range_fails_closed() {
        let mut scanner = ptr::null_mut();
        let mut results = ptr::null_mut();
        let input = CribraBatchInput {
            key: string_input("clean"),
            source: string_input("clean source"),
        };

        unsafe {
            assert_eq!(cribra_scanner_new_current(&mut scanner), CRIBRA_OK);
            assert_eq!(
                cribra_scanner_scan_batch(
                    scanner,
                    &input,
                    1,
                    CRIBRA_BATCH_EXECUTION_AUTO,
                    &mut results,
                    ptr::null_mut(),
                ),
                CRIBRA_OK
            );

            let mut entry = CribraBatchEntryView {
                key: CribraStringView {
                    ptr: ptr::dangling(),
                    len: usize::MAX,
                },
                source_bytes: usize::MAX,
                finding_count: usize::MAX,
                candidate_count: usize::MAX,
            };
            assert_eq!(
                cribra_batch_results_entry_at(results, 1, &mut entry),
                CRIBRA_OUT_OF_RANGE
            );
            assert!(entry.key.ptr.is_null());
            assert_eq!(entry.source_bytes, 0);

            let mut finding = CribraFindingView::default();
            assert_eq!(
                cribra_batch_results_finding_at(results, 0, 0, &mut finding),
                CRIBRA_OUT_OF_RANGE
            );

            let mut candidate = CribraCandidateView::default();
            assert_eq!(
                cribra_batch_results_candidate_at(results, 0, 0, &mut candidate),
                CRIBRA_OUT_OF_RANGE
            );

            cribra_batch_results_free(results);
            cribra_scanner_free(scanner);
        }
    }

    #[test]
    fn empty_batch_is_valid() {
        let mut scanner = ptr::null_mut();
        let mut results = ptr::null_mut();

        unsafe {
            assert_eq!(cribra_scanner_new_current(&mut scanner), CRIBRA_OK);
            assert_eq!(
                cribra_scanner_scan_batch(
                    scanner,
                    ptr::null(),
                    0,
                    CRIBRA_BATCH_EXECUTION_AUTO,
                    &mut results,
                    ptr::null_mut(),
                ),
                CRIBRA_OK
            );

            let mut count = usize::MAX;
            assert_eq!(cribra_batch_results_count(results, &mut count), CRIBRA_OK);
            assert_eq!(count, 0);

            cribra_batch_results_free(results);
            cribra_scanner_free(scanner);
        }
    }

    #[test]
    fn invalid_batch_input_fails_atomically() {
        let mut scanner = ptr::null_mut();
        let mut results = ptr::dangling_mut::<CribraBatchResults>();
        let valid_key = "valid";
        let valid_source = "clean";
        let invalid_utf8 = [0xff_u8];

        let inputs = [
            CribraBatchInput {
                key: string_input(valid_key),
                source: string_input(valid_source),
            },
            CribraBatchInput {
                key: string_input("invalid"),
                source: CribraStringView {
                    ptr: invalid_utf8.as_ptr(),
                    len: invalid_utf8.len(),
                },
            },
        ];

        unsafe {
            assert_eq!(cribra_scanner_new_current(&mut scanner), CRIBRA_OK);
            assert_eq!(
                cribra_scanner_scan_batch(
                    scanner,
                    inputs.as_ptr(),
                    inputs.len(),
                    CRIBRA_BATCH_EXECUTION_AUTO,
                    &mut results,
                    ptr::null_mut(),
                ),
                CRIBRA_INVALID_UTF8
            );
            assert!(results.is_null());
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
                cribra_scanner_scan(
                    scanner,
                    source.as_ptr(),
                    source.len(),
                    &mut report,
                    ptr::null_mut()
                ),
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
                cribra_scanner_scan(scanner, ptr::null(), 0, &mut report, ptr::null_mut()),
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
                assert_eq!(
                    cribra_builder_add_rule(builder, &config, ptr::null_mut()),
                    CRIBRA_OK
                );
                assert_eq!(
                    cribra_builder_build(builder, &mut scanner, ptr::null_mut()),
                    CRIBRA_OK
                );
                assert_eq!(
                    cribra_scanner_scan(
                        scanner,
                        source.as_ptr(),
                        source.len(),
                        &mut report,
                        ptr::null_mut()
                    ),
                    CRIBRA_OK
                );

                let mut count = 0;
                assert_eq!(cribra_report_finding_count(report, &mut count), CRIBRA_OK);
                assert_eq!(count, 1, "{id}");

                let mut finding = CribraFindingView::default();
                assert_eq!(cribra_report_finding_at(report, 0, &mut finding), CRIBRA_OK);
                let rule_id = slice::from_raw_parts(finding.rule_id.ptr, finding.rule_id.len);
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
        let config = rule_config(CRIBRA_RULE_KIND_LITERAL, &id, &value, CRIBRA_SEVERITY_HIGH);

        unsafe {
            assert_eq!(cribra_builder_new(&mut builder), CRIBRA_OK);
            assert_eq!(
                cribra_builder_add_rule(builder, &config, ptr::null_mut()),
                CRIBRA_OK
            );
        }

        drop(id);
        drop(value);

        unsafe {
            assert_eq!(
                cribra_builder_build(builder, &mut scanner, ptr::null_mut()),
                CRIBRA_OK
            );
            cribra_scanner_free(scanner);
        }
    }

    unsafe fn ffi_error_text(error: *const CribraError) -> String {
        let mut view = CribraStringView::default();
        assert_eq!(
            unsafe { crate::cribra_error_message(error, &mut view) },
            CRIBRA_OK
        );
        let bytes = unsafe { slice::from_raw_parts(view.ptr, view.len) };
        str::from_utf8(bytes).unwrap().to_owned()
    }

    #[test]
    fn custom_rule_failure_returns_owned_diagnostic() {
        let mut builder = ptr::null_mut();
        let mut error = ptr::null_mut();
        let invalid = rule_config(
            CRIBRA_RULE_KIND_PATTERN,
            "native.invalid-pattern",
            "(",
            CRIBRA_SEVERITY_HIGH,
        );

        unsafe {
            assert_eq!(cribra_builder_new(&mut builder), CRIBRA_OK);
            assert_eq!(
                cribra_builder_add_rule(builder, &invalid, &mut error),
                CRIBRA_RULE_ERROR
            );
            assert!(!error.is_null());

            let mut status = u32::MAX;
            assert_eq!(crate::cribra_error_status(error, &mut status), CRIBRA_OK);
            assert_eq!(status, CRIBRA_RULE_ERROR);
            assert_eq!(
                ffi_error_text(error),
                "custom rule configuration is invalid"
            );

            crate::cribra_error_free(error);
            cribra_builder_free(builder);
        }
    }

    #[test]
    fn scanner_build_failure_returns_privacy_safe_diagnostic() {
        let mut builder = ptr::null_mut();
        let mut scanner = ptr::dangling_mut::<CribraScanner>();
        let mut error = ptr::null_mut();
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
            CRIBRA_SEVERITY_HIGH,
        );

        unsafe {
            assert_eq!(cribra_builder_new(&mut builder), CRIBRA_OK);
            assert_eq!(
                cribra_builder_add_rule(builder, &first, ptr::null_mut()),
                CRIBRA_OK
            );
            assert_eq!(
                cribra_builder_add_rule(builder, &second, ptr::null_mut()),
                CRIBRA_OK
            );
            assert_eq!(
                cribra_builder_build(builder, &mut scanner, &mut error),
                CRIBRA_BUILD_ERROR
            );
            assert!(scanner.is_null());
            assert!(!error.is_null());

            let message = ffi_error_text(error);
            assert_eq!(message, "duplicate rule identifier");
            assert!(!message.contains("FIRST_SECRET"));
            assert!(!message.contains("SECOND_SECRET"));

            crate::cribra_error_free(error);
        }
    }

    #[test]
    fn transform_mismatch_returns_owned_diagnostic() {
        let mut builder = ptr::null_mut();
        let mut scanner = ptr::null_mut();
        let mut report = ptr::null_mut();
        let mut output = ptr::dangling_mut::<CribraOutput>();
        let mut error = ptr::null_mut();
        let rule = rule_config(
            CRIBRA_RULE_KIND_LITERAL,
            "native.secret",
            "SECRET",
            CRIBRA_SEVERITY_HIGH,
        );
        let source = "A=SECRET";
        let changed = "A=SECRET-extra";

        unsafe {
            assert_eq!(cribra_builder_new(&mut builder), CRIBRA_OK);
            assert_eq!(
                cribra_builder_add_rule(builder, &rule, ptr::null_mut()),
                CRIBRA_OK
            );
            assert_eq!(
                cribra_builder_build(builder, &mut scanner, ptr::null_mut()),
                CRIBRA_OK
            );
            assert_eq!(
                cribra_scanner_scan(
                    scanner,
                    source.as_ptr(),
                    source.len(),
                    &mut report,
                    ptr::null_mut(),
                ),
                CRIBRA_OK
            );
            assert_eq!(
                cribra_transform_redact(
                    changed.as_ptr(),
                    changed.len(),
                    report,
                    &mut output,
                    &mut error,
                ),
                CRIBRA_TRANSFORM_ERROR
            );
            assert!(output.is_null());
            assert!(!error.is_null());
            assert_eq!(
                ffi_error_text(error),
                "transform source length does not match report"
            );

            crate::cribra_error_free(error);
            cribra_report_free(report);
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
                cribra_builder_add_rule(builder, &invalid, ptr::null_mut()),
                CRIBRA_RULE_ERROR
            );
            assert_eq!(
                cribra_builder_add_rule(builder, &valid, ptr::null_mut()),
                CRIBRA_OK
            );
            assert_eq!(
                cribra_builder_build(builder, &mut scanner, ptr::null_mut()),
                CRIBRA_OK
            );
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
            assert_eq!(
                cribra_builder_add_rule(builder, &first, ptr::null_mut()),
                CRIBRA_OK
            );
            assert_eq!(
                cribra_builder_add_rule(builder, &second, ptr::null_mut()),
                CRIBRA_OK
            );
            assert_eq!(
                cribra_builder_build(builder, &mut scanner, ptr::null_mut()),
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
            assert_eq!(cribra_builder_add_current_builtins(builder), CRIBRA_OK);
            assert_eq!(
                cribra_builder_add_rule(builder, &custom, ptr::null_mut()),
                CRIBRA_OK
            );
            assert_eq!(
                cribra_builder_build(builder, &mut scanner, ptr::null_mut()),
                CRIBRA_OK
            );
            assert_eq!(
                cribra_scanner_scan(
                    scanner,
                    source.as_ptr(),
                    source.len(),
                    &mut report,
                    ptr::null_mut()
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
                cribra_scanner_scan(
                    scanner,
                    source.as_ptr(),
                    source.len(),
                    &mut report,
                    ptr::null_mut()
                ),
                CRIBRA_OK
            );
            let mut finding_count = usize::MAX;
            let mut candidate_count = usize::MAX;
            assert_eq!(
                cribra_report_finding_count(report, &mut finding_count),
                CRIBRA_OK
            );
            assert_eq!(
                cribra_report_candidate_count(report, &mut candidate_count),
                CRIBRA_OK
            );
            assert_eq!(finding_count, 0);
            assert_eq!(candidate_count, 1);
            let mut candidate = CribraCandidateView::default();
            assert_eq!(
                cribra_report_candidate_at(report, 0, &mut candidate),
                CRIBRA_OK
            );
            assert_eq!(candidate.kind, CRIBRA_CANDIDATE_KIND_RECOVERY_LIKE_CODE);
            assert_eq!(candidate.evidence, CRIBRA_CANDIDATE_EVIDENCE_STRUCTURAL);
            assert!(candidate.start < candidate.end);
            let mut explanation = CribraExplanationView::default();
            assert_eq!(
                cribra_report_explain_candidate(report, 0, &mut explanation),
                CRIBRA_OK
            );
            assert_eq!(explanation.kind, CRIBRA_EXPLANATION_AMBIGUOUS);
            assert_eq!(explanation.detection_mode, CRIBRA_DETECTION_MODE_NONE);
            assert_eq!(
                explanation.candidate_evidence,
                CRIBRA_CANDIDATE_EVIDENCE_STRUCTURAL
            );
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
            assert_eq!(
                cribra_scanner_scan(
                    scanner,
                    source.as_ptr(),
                    source.len(),
                    &mut report,
                    ptr::null_mut()
                ),
                CRIBRA_OK
            );
            let mut explanation = CribraExplanationView::default();
            assert_eq!(
                cribra_scanner_explain_finding(
                    scanner,
                    report,
                    0,
                    &mut explanation,
                    ptr::null_mut()
                ),
                CRIBRA_OK
            );
            assert_eq!(explanation.kind, CRIBRA_EXPLANATION_CLASSIFIED);
            assert_eq!(
                explanation.detection_mode,
                CRIBRA_DETECTION_MODE_DETERMINISTIC
            );
            assert_eq!(
                explanation.candidate_evidence,
                CRIBRA_CANDIDATE_EVIDENCE_NONE
            );
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
    fn transform_fixture() -> (*mut CribraScanner, *mut CribraReport, &'static [u8]) {
        let source = b"TOKEN=SECRET";
        let mut builder = ptr::null_mut();
        let mut scanner = ptr::null_mut();
        let mut report = ptr::null_mut();
        let config = rule_config(
            CRIBRA_RULE_KIND_LITERAL,
            "secret",
            "SECRET",
            CRIBRA_SEVERITY_HIGH,
        );
        unsafe {
            assert_eq!(cribra_builder_new(&mut builder), CRIBRA_OK);
            assert_eq!(
                cribra_builder_add_rule(builder, &config, ptr::null_mut()),
                CRIBRA_OK
            );
            assert_eq!(
                cribra_builder_build(builder, &mut scanner, ptr::null_mut()),
                CRIBRA_OK
            );
            assert_eq!(
                cribra_scanner_scan(
                    scanner,
                    source.as_ptr(),
                    source.len(),
                    &mut report,
                    ptr::null_mut()
                ),
                CRIBRA_OK
            );
        }
        (scanner, report, source)
    }

    fn share_entry_text(entry: CribraShareEntryView) -> (String, String) {
        unsafe {
            let key = slice::from_raw_parts(entry.key.ptr, entry.key.len);
            let content = slice::from_raw_parts(entry.content.ptr, entry.content.len);
            (
                str::from_utf8(key).unwrap().to_owned(),
                str::from_utf8(content).unwrap().to_owned(),
            )
        }
    }

    #[test]
    fn share_bundle_redaction_preserves_order_keys_and_manifest_summary() {
        let mut builder = ptr::null_mut();
        let mut scanner = ptr::null_mut();
        let mut results = ptr::null_mut();
        let mut bundle = ptr::null_mut();

        let rule = rule_config(
            CRIBRA_RULE_KIND_LITERAL,
            "secret",
            "SECRET",
            CRIBRA_SEVERITY_CRITICAL,
        );
        let first_source = "A=SECRET";
        let second_source = "clean";
        let inputs = [
            CribraBatchInput {
                key: string_input("a.env"),
                source: string_input(first_source),
            },
            CribraBatchInput {
                key: string_input("b.env"),
                source: string_input(second_source),
            },
        ];
        let sources = [string_input(first_source), string_input(second_source)];
        let config = CribraShareBundleConfig {
            mode: CRIBRA_SHARE_MODE_REDACT,
            ..CribraShareBundleConfig::default()
        };

        unsafe {
            assert_eq!(cribra_builder_new(&mut builder), CRIBRA_OK);
            assert_eq!(
                cribra_builder_add_rule(builder, &rule, ptr::null_mut()),
                CRIBRA_OK
            );
            assert_eq!(
                cribra_builder_build(builder, &mut scanner, ptr::null_mut()),
                CRIBRA_OK
            );
            assert_eq!(
                cribra_scanner_scan_batch(
                    scanner,
                    inputs.as_ptr(),
                    inputs.len(),
                    CRIBRA_BATCH_EXECUTION_SERIAL,
                    &mut results,
                    ptr::null_mut(),
                ),
                CRIBRA_OK
            );

            assert_eq!(
                cribra_share_bundle_build(
                    results,
                    sources.as_ptr(),
                    sources.len(),
                    &config,
                    &mut bundle,
                    ptr::null_mut(),
                ),
                CRIBRA_OK
            );

            let mut count = usize::MAX;
            assert_eq!(cribra_share_bundle_count(bundle, &mut count), CRIBRA_OK);
            assert_eq!(count, 2);

            let mut first = CribraShareEntryView::default();
            assert_eq!(
                cribra_share_bundle_entry_at(bundle, 0, &mut first),
                CRIBRA_OK
            );
            assert_eq!(
                share_entry_text(first),
                ("a.env".to_owned(), "A=[REDACTED]".to_owned())
            );

            let mut second = CribraShareEntryView::default();
            assert_eq!(
                cribra_share_bundle_entry_at(bundle, 1, &mut second),
                CRIBRA_OK
            );
            assert_eq!(
                share_entry_text(second),
                ("b.env".to_owned(), "clean".to_owned())
            );

            let mut manifest = CribraShareManifestView::default();
            assert_eq!(
                cribra_share_bundle_manifest(bundle, &mut manifest),
                CRIBRA_OK
            );
            assert_eq!(manifest.mode, CRIBRA_SHARE_MODE_REDACT);
            assert_eq!(manifest.summary.scanned_sources, 2);
            assert_eq!(
                manifest.summary.scanned_bytes,
                first_source.len() + second_source.len()
            );
            assert_eq!(manifest.summary.total_findings, 1);
            assert_eq!(manifest.summary.critical, 1);
            assert!(manifest.generated_at_secs > 0);
            assert!(manifest.generated_at_nanos < 1_000_000_000);

            cribra_share_bundle_free(bundle);
            cribra_batch_results_free(results);
            cribra_scanner_free(scanner);
        }
    }

    #[test]
    fn share_bundle_source_count_and_length_mismatch_fail_closed() {
        let mut builder = ptr::null_mut();
        let mut scanner = ptr::null_mut();
        let mut results = ptr::null_mut();
        let mut bundle = ptr::dangling_mut::<CribraShareBundle>();

        let rule = rule_config(
            CRIBRA_RULE_KIND_LITERAL,
            "secret",
            "SECRET",
            CRIBRA_SEVERITY_HIGH,
        );
        let input = CribraBatchInput {
            key: string_input("memory"),
            source: string_input("A=SECRET"),
        };
        let config = CribraShareBundleConfig {
            mode: CRIBRA_SHARE_MODE_REDACT,
            ..CribraShareBundleConfig::default()
        };

        unsafe {
            assert_eq!(cribra_builder_new(&mut builder), CRIBRA_OK);
            assert_eq!(
                cribra_builder_add_rule(builder, &rule, ptr::null_mut()),
                CRIBRA_OK
            );
            assert_eq!(
                cribra_builder_build(builder, &mut scanner, ptr::null_mut()),
                CRIBRA_OK
            );
            assert_eq!(
                cribra_scanner_scan_batch(
                    scanner,
                    &input,
                    1,
                    CRIBRA_BATCH_EXECUTION_SERIAL,
                    &mut results,
                    ptr::null_mut(),
                ),
                CRIBRA_OK
            );

            assert_eq!(
                cribra_share_bundle_build(
                    results,
                    ptr::null(),
                    0,
                    &config,
                    &mut bundle,
                    ptr::null_mut(),
                ),
                CRIBRA_TRANSFORM_ERROR
            );
            assert!(bundle.is_null());

            let changed = [string_input("A=CHANGED_SECRET")];
            bundle = ptr::dangling_mut();
            assert_eq!(
                cribra_share_bundle_build(
                    results,
                    changed.as_ptr(),
                    changed.len(),
                    &config,
                    &mut bundle,
                    ptr::null_mut(),
                ),
                CRIBRA_TRANSFORM_ERROR
            );
            assert!(bundle.is_null());

            cribra_batch_results_free(results);
            cribra_scanner_free(scanner);
        }
    }

    #[test]
    fn keyed_share_modes_require_32_byte_key_and_do_not_expose_it() {
        let mut builder = ptr::null_mut();
        let mut scanner = ptr::null_mut();
        let mut results = ptr::null_mut();
        let mut bundle = ptr::null_mut();

        let rule = rule_config(
            CRIBRA_RULE_KIND_LITERAL,
            "secret",
            "SECRET",
            CRIBRA_SEVERITY_HIGH,
        );
        let source = "TOKEN=SECRET";
        let input = CribraBatchInput {
            key: string_input("memory"),
            source: string_input(source),
        };
        let sources = [string_input(source)];

        unsafe {
            assert_eq!(cribra_builder_new(&mut builder), CRIBRA_OK);
            assert_eq!(
                cribra_builder_add_rule(builder, &rule, ptr::null_mut()),
                CRIBRA_OK
            );
            assert_eq!(
                cribra_builder_build(builder, &mut scanner, ptr::null_mut()),
                CRIBRA_OK
            );
            assert_eq!(
                cribra_scanner_scan_batch(
                    scanner,
                    &input,
                    1,
                    CRIBRA_BATCH_EXECUTION_SERIAL,
                    &mut results,
                    ptr::null_mut(),
                ),
                CRIBRA_OK
            );

            let bad_key = [7_u8; 31];
            let bad = CribraShareBundleConfig {
                mode: CRIBRA_SHARE_MODE_PSEUDONYMIZE,
                key: bad_key.as_ptr(),
                key_len: bad_key.len(),
                ..CribraShareBundleConfig::default()
            };
            assert_eq!(
                cribra_share_bundle_build(
                    results,
                    sources.as_ptr(),
                    sources.len(),
                    &bad,
                    &mut bundle,
                    ptr::null_mut(),
                ),
                CRIBRA_INVALID_ARGUMENT
            );
            assert!(bundle.is_null());

            let key = [7_u8; 32];
            let good = CribraShareBundleConfig {
                mode: CRIBRA_SHARE_MODE_PSEUDONYMIZE,
                key: key.as_ptr(),
                key_len: key.len(),
                text: string_input("native_pseudo_"),
                digest_bytes: 8,
            };
            assert_eq!(
                cribra_share_bundle_build(
                    results,
                    sources.as_ptr(),
                    sources.len(),
                    &good,
                    &mut bundle,
                    ptr::null_mut(),
                ),
                CRIBRA_OK
            );

            let mut entry = CribraShareEntryView::default();
            assert_eq!(
                cribra_share_bundle_entry_at(bundle, 0, &mut entry),
                CRIBRA_OK
            );
            let (_, content) = share_entry_text(entry);
            assert!(content.starts_with("TOKEN=native_pseudo_"));
            assert!(!content.contains("SECRET"));

            let mut manifest = CribraShareManifestView::default();
            assert_eq!(
                cribra_share_bundle_manifest(bundle, &mut manifest),
                CRIBRA_OK
            );
            assert_eq!(manifest.mode, CRIBRA_SHARE_MODE_PSEUDONYMIZE);

            cribra_share_bundle_free(bundle);
            cribra_batch_results_free(results);
            cribra_scanner_free(scanner);
        }
    }

    unsafe fn output_text(output: *const CribraOutput) -> String {
        let mut view = CribraStringView::default();
        assert_eq!(unsafe { cribra_output_view(output, &mut view) }, CRIBRA_OK);
        // SAFETY: the output handle remains alive while the returned view is read.
        let bytes = unsafe { slice::from_raw_parts(view.ptr, view.len) };
        str::from_utf8(bytes).unwrap().to_owned()
    }

    #[test]
    fn transform_outputs_are_owned_and_explicitly_freed() {
        let (scanner, report, source) = transform_fixture();
        let mut output = ptr::null_mut();

        unsafe {
            assert_eq!(
                cribra_transform_redact(
                    source.as_ptr(),
                    source.len(),
                    report,
                    &mut output,
                    ptr::null_mut()
                ),
                CRIBRA_OK
            );
            assert_eq!(output_text(output), "TOKEN=[REDACTED]");
            cribra_output_free(output);
            cribra_report_free(report);
            cribra_scanner_free(scanner);
        }
    }

    #[test]
    fn template_and_custom_redaction_preserve_core_semantics() {
        let (scanner, report, source) = transform_fixture();
        let mut output = ptr::null_mut();
        let replacement = b"***";

        unsafe {
            assert_eq!(
                cribra_transform_redact_with(
                    source.as_ptr(),
                    source.len(),
                    report,
                    replacement.as_ptr(),
                    replacement.len(),
                    &mut output,
                    ptr::null_mut()
                ),
                CRIBRA_OK
            );
            assert_eq!(output_text(output), "TOKEN=***");
            cribra_output_free(output);

            let namespace = b"EXAMPLE";
            let config = CribraTemplateConfig {
                namespace: CribraStringView {
                    ptr: namespace.as_ptr(),
                    len: namespace.len(),
                },
                numbered: 1,
            };
            output = ptr::null_mut();
            assert_eq!(
                cribra_transform_template_with(
                    source.as_ptr(),
                    source.len(),
                    report,
                    &config,
                    &mut output,
                    ptr::null_mut()
                ),
                CRIBRA_OK
            );
            assert_eq!(output_text(output), "TOKEN=<EXAMPLE:secret:1>");
            cribra_output_free(output);
            cribra_report_free(report);
            cribra_scanner_free(scanner);
        }
    }

    #[test]
    fn keyed_transforms_require_exactly_32_key_bytes() {
        let (scanner, report, source) = transform_fixture();
        let key = [7_u8; 31];
        let prefix = b"p_";
        let config = CribraPseudonymizeConfig {
            key: key.as_ptr(),
            key_len: key.len(),
            prefix: CribraStringView {
                ptr: prefix.as_ptr(),
                len: prefix.len(),
            },
            digest_bytes: 16,
        };
        let mut output = ptr::dangling_mut::<CribraOutput>();

        unsafe {
            assert_eq!(
                cribra_transform_pseudonymize(
                    source.as_ptr(),
                    source.len(),
                    report,
                    &config,
                    &mut output,
                    ptr::null_mut()
                ),
                CRIBRA_INVALID_ARGUMENT
            );
            assert!(output.is_null());
            cribra_report_free(report);
            cribra_scanner_free(scanner);
        }
    }

    #[test]
    fn pseudonymize_and_synthesize_return_rust_owned_outputs() {
        let (scanner, report, source) = transform_fixture();
        let key = [9_u8; 32];
        let prefix = b"native_";
        let pseudo = CribraPseudonymizeConfig {
            key: key.as_ptr(),
            key_len: key.len(),
            prefix: CribraStringView {
                ptr: prefix.as_ptr(),
                len: prefix.len(),
            },
            digest_bytes: 16,
        };
        let marker = b"ffi_synthetic";
        let synth = CribraSynthesizeConfig {
            key: key.as_ptr(),
            key_len: key.len(),
            marker: CribraStringView {
                ptr: marker.as_ptr(),
                len: marker.len(),
            },
        };

        unsafe {
            let mut output = ptr::null_mut();
            assert_eq!(
                cribra_transform_pseudonymize(
                    source.as_ptr(),
                    source.len(),
                    report,
                    &pseudo,
                    &mut output,
                    ptr::null_mut()
                ),
                CRIBRA_OK
            );
            let text = output_text(output);
            assert!(text.starts_with("TOKEN=native_"));
            assert!(!text.contains("SECRET"));
            cribra_output_free(output);

            output = ptr::null_mut();
            assert_eq!(
                cribra_transform_synthesize(
                    source.as_ptr(),
                    source.len(),
                    report,
                    &synth,
                    &mut output,
                    ptr::null_mut()
                ),
                CRIBRA_OK
            );
            assert!(!output_text(output).contains("SECRET"));
            cribra_output_free(output);
            cribra_report_free(report);
            cribra_scanner_free(scanner);
        }
    }

    #[test]
    fn transform_rejects_obvious_source_report_length_mismatch() {
        let (scanner, report, _) = transform_fixture();
        let wrong_source = b"TOKEN=SECRET-extra";
        let mut output = ptr::dangling_mut::<CribraOutput>();

        unsafe {
            assert_eq!(
                cribra_transform_redact(
                    wrong_source.as_ptr(),
                    wrong_source.len(),
                    report,
                    &mut output,
                    ptr::null_mut()
                ),
                CRIBRA_TRANSFORM_ERROR
            );
            assert!(output.is_null());
            cribra_report_free(report);
            cribra_scanner_free(scanner);
        }
    }
}
