//! Core-owned synchronous byte streams with validated JavaScript limits.

use wasm_bindgen::prelude::*;

use crate::{ScanEngine, ScanResult};

/// Stable programmatic streaming failure codes. Existing assignments never move.
#[wasm_bindgen]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamErrorCode {
    /// The scanner cannot stream this ruleset exactly.
    UnsupportedRuleSet = 0,
    /// Finish or a terminal failure consumed the session.
    NotActive = 1,
    /// Source bytes contain malformed UTF-8.
    InvalidUtf8 = 2,
    /// EOF divides an incomplete UTF-8 scalar.
    IncompleteUtf8 = 3,
    /// Core's cumulative raw rule candidate limit was exceeded.
    CandidateLimitExceeded = 4,
    /// Core's retained finding/review result limit was exceeded.
    ResultLimitExceeded = 5,
    /// Core's logical executor storage admission limit was exceeded.
    StorageLimitExceeded = 6,
    /// Core's static storage arithmetic overflowed.
    StorageRequirementOverflow = 7,
    /// Core or adapter source-byte accounting overflowed.
    SourceOffsetOverflow = 8,
    /// Core's line or scalar-column arithmetic overflowed.
    CoordinateOverflow = 9,
    /// Core's streaming execution invariant failed.
    ExecutionInvariantViolated = 10,
    /// A future Core streaming failure is not understood by this adapter.
    UnknownCoreError = 11,
    /// A supplied limit is not a safe, nonnegative integer representable as usize.
    InvalidLimit = 12,
}

/// Thrown streaming failure with fixed source-free diagnostics.
///
/// This is an exported wasm-bindgen object, not a native JavaScript Error.
/// Catch it, inspect code/message, then call its generated free() exactly once.
#[wasm_bindgen]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StreamError {
    code: StreamErrorCode,
}

#[wasm_bindgen]
impl StreamError {
    /// Stable symbolic reason, independent of diagnostic wording.
    #[wasm_bindgen(getter)]
    pub fn code(&self) -> StreamErrorCode {
        self.code
    }

    /// Privacy-safe diagnostic without source, matched values or supplied limits.
    #[wasm_bindgen(getter)]
    pub fn message(&self) -> String {
        match self.code {
            StreamErrorCode::UnsupportedRuleSet => "ruleset does not support exact streaming",
            StreamErrorCode::NotActive => "stream session is not active",
            StreamErrorCode::InvalidUtf8 => "stream contains malformed UTF-8",
            StreamErrorCode::IncompleteUtf8 => "stream ends with incomplete UTF-8",
            StreamErrorCode::CandidateLimitExceeded => "stream candidate limit exceeded",
            StreamErrorCode::ResultLimitExceeded => "stream retained result limit exceeded",
            StreamErrorCode::StorageLimitExceeded => "stream logical storage limit exceeded",
            StreamErrorCode::StorageRequirementOverflow => "stream storage requirement overflow",
            StreamErrorCode::SourceOffsetOverflow => "stream source byte offset overflow",
            StreamErrorCode::CoordinateOverflow => "stream source coordinate overflow",
            StreamErrorCode::ExecutionInvariantViolated => "stream execution invariant violated",
            StreamErrorCode::UnknownCoreError => "unknown Core streaming failure",
            StreamErrorCode::InvalidLimit => {
                "stream limit must be a nonnegative safe integer representable on this target"
            }
        }
        .to_owned()
    }
}

impl From<cribra::StreamError> for StreamError {
    fn from(error: cribra::StreamError) -> Self {
        use cribra::StreamError as Core;
        let code = match error {
            Core::UnsupportedRuleSet => StreamErrorCode::UnsupportedRuleSet,
            Core::NotActive => StreamErrorCode::NotActive,
            Core::InvalidUtf8 => StreamErrorCode::InvalidUtf8,
            Core::IncompleteUtf8 => StreamErrorCode::IncompleteUtf8,
            Core::CandidateLimitExceeded { .. } => StreamErrorCode::CandidateLimitExceeded,
            Core::ResultLimitExceeded { .. } => StreamErrorCode::ResultLimitExceeded,
            Core::StorageLimitExceeded { .. } => StreamErrorCode::StorageLimitExceeded,
            Core::StorageRequirementOverflow => StreamErrorCode::StorageRequirementOverflow,
            Core::SourceOffsetOverflow => StreamErrorCode::SourceOffsetOverflow,
            Core::CoordinateOverflow => StreamErrorCode::CoordinateOverflow,
            Core::ExecutionInvariantViolated => StreamErrorCode::ExecutionInvariantViolated,
            _ => StreamErrorCode::UnknownCoreError,
        };
        Self { code }
    }
}

#[wasm_bindgen(typescript_custom_section)]
const STREAM_LIMITS_TYPES: &str = r#"
/** Optional Core budgets. Missing/undefined means unlimited; zero is a real limit.
 * Numbers must be nonnegative safe integers representable on the WASM target.
 */
export interface StreamLimits {
    readonly maxCandidates?: number;
    readonly maxStreamStorageBytes?: number;
    readonly maxRetainedResults?: number;
}
"#;

#[wasm_bindgen]
extern "C" {
    /// Typed input-only options object; property values are validated before Core.
    #[wasm_bindgen(typescript_type = "StreamLimits")]
    pub type StreamLimits;

    #[wasm_bindgen(method, getter, js_name = maxCandidates)]
    fn max_candidates(this: &StreamLimits) -> JsValue;
    #[wasm_bindgen(method, getter, js_name = maxStreamStorageBytes)]
    fn max_stream_storage_bytes(this: &StreamLimits) -> JsValue;
    #[wasm_bindgen(method, getter, js_name = maxRetainedResults)]
    fn max_retained_results(this: &StreamLimits) -> JsValue;
}

fn numeric_limit(value: f64) -> Result<usize, StreamError> {
    const MAX_SAFE_INTEGER: f64 = 9_007_199_254_740_991.0;
    if !value.is_finite()
        || value < 0.0
        || value.fract() != 0.0
        || value > MAX_SAFE_INTEGER
        || value > usize::MAX as f64
    {
        return Err(StreamError {
            code: StreamErrorCode::InvalidLimit,
        });
    }
    // Conversion occurs only after both JS-safe-integer and target-width checks.
    Ok(value as usize)
}

fn js_limit(value: JsValue) -> Result<Option<usize>, StreamError> {
    if value.is_undefined() {
        return Ok(None);
    }
    let number = value.as_f64().ok_or(StreamError {
        code: StreamErrorCode::InvalidLimit,
    })?;
    numeric_limit(number).map(Some)
}

#[wasm_bindgen]
impl ScanEngine {
    /// Opens an unlimited Core stream. Unsupported rulesets fail admission;
    /// there is no whole-source fallback. The stream may outlive this engine.
    pub fn stream(&self) -> Result<StreamSession, StreamError> {
        self.open_stream(cribra::StreamLimits::default())
    }

    /// Opens a stream with independently optional named Core budgets.
    /// Missing/undefined is unlimited; zero is meaningful. Invalid JS numbers
    /// and non-number property values throw InvalidLimit before Core admission.
    #[wasm_bindgen(js_name = streamWithLimits)]
    pub fn stream_with_limits(&self, limits: &StreamLimits) -> Result<StreamSession, StreamError> {
        let max_candidates = js_limit(limits.max_candidates())?;
        let max_stream_storage_bytes = js_limit(limits.max_stream_storage_bytes())?;
        let max_retained_results = js_limit(limits.max_retained_results())?;
        let limits = cribra::StreamLimits::default()
            .with_max_candidates(max_candidates)
            .with_max_stream_storage_bytes(max_stream_storage_bytes)
            .with_max_retained_results(max_retained_results);
        self.open_stream(limits)
    }
}

impl ScanEngine {
    fn open_stream(&self, limits: cribra::StreamLimits) -> Result<StreamSession, StreamError> {
        self.scanner
            .stream_with_limits(limits)
            .map(StreamSession::from_core)
            .map_err(Into::into)
    }
}

/// One Core byte stream, independent of its engine's lifetime.
///
/// No adapter source buffer, independent decoder, scanner or result cache exists.
/// Finish consumes Core state while leaving this object valid until generated free().
#[wasm_bindgen]
pub struct StreamSession {
    inner: Option<cribra::StreamSession>,
    source_bytes: usize,
}

impl StreamSession {
    fn from_core(inner: cribra::StreamSession) -> Self {
        Self {
            inner: Some(inner),
            source_bytes: 0,
        }
    }
}

#[wasm_bindgen]
impl StreamSession {
    /// Feeds bytes in order; UTF-8 scalars may span chunks. Empty active pushes
    /// are no-ops. No caller buffer is retained. Any Core push failure or byte
    /// accounting overflow is terminal and discards the session and partial output.
    pub fn push(&mut self, chunk: &[u8]) -> Result<(), StreamError> {
        let mut inner = self.inner.take().ok_or(cribra::StreamError::NotActive)?;
        inner.push(chunk)?;
        self.source_bytes = self
            .source_bytes
            .checked_add(chunk.len())
            .ok_or(cribra::StreamError::SourceOffsetOverflow)?;
        self.inner = Some(inner);
        Ok(())
    }

    /// Signals genuine EOF, transferring a complete owned existing ScanResult.
    /// Success or failure consumes Core state. Later push/finish throws NotActive.
    /// No partial ScanResult is returned, including incomplete UTF-8 at EOF.
    pub fn finish(&mut self) -> Result<ScanResult, StreamError> {
        let inner = self.inner.take().ok_or(cribra::StreamError::NotActive)?;
        Ok(ScanResult {
            source_bytes: self.source_bytes,
            report: inner.finish()?,
        })
    }
}

#[cfg(test)]
#[path = "streaming_tests.rs"]
mod tests;
