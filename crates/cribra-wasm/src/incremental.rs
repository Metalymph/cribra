//! Core-owned editable documents and independently owned metadata snapshots.
use wasm_bindgen::prelude::*;

use crate::{ScanEngine, ScanResult};

/// Stable incremental failure codes. Existing assignments never move.
#[wasm_bindgen]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IncrementalErrorCode {
    /// Core no longer accepts edits after a terminal failure.
    NotActive = 0,
    /// Expected revision differs from the last committed revision.
    StaleRevision = 1,
    /// The half-open range is reversed or outside the document.
    InvalidRange = 2,
    /// A range endpoint divides a UTF-8 scalar.
    InvalidUtf8Boundary = 3,
    /// Core cannot increment the committed u64 revision.
    RevisionOverflow = 4,
    /// Core cannot represent the edited document length.
    DocumentLengthOverflow = 5,
    /// Raw candidate emissions exceeded the operation budget.
    CandidateLimitExceeded = 6,
    /// Old and staged cache records exceeded the budget.
    CacheLimitExceeded = 7,
    /// Materialized findings and candidate copies exceeded the budget.
    ResultLimitExceeded = 8,
    /// Logical transaction payload exceeded the budget.
    TransactionStorageLimitExceeded = 9,
    /// Core resource accounting overflowed.
    ResourceAccountingOverflow = 10,
    /// Core matching or transaction invariants failed.
    ExecutionInvariantViolated = 11,
    /// A future Core error is not understood by this adapter.
    UnknownCoreError = 12,
    /// A limit is not a nonnegative safe integer representable on this target.
    InvalidLimit = 13,
    /// An offset is not a nonnegative safe integer representable on this target.
    OffsetOutOfRange = 14,
    /// A revision is not a bigint in the u64 range; conversion never wraps.
    InvalidRevision = 15,
}

/// Owned thrown wasm-bindgen object. Inspect it and call generated free() once.
#[wasm_bindgen]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IncrementalError {
    code: IncrementalErrorCode,
    terminal: bool,
}

#[wasm_bindgen]
impl IncrementalError {
    /// Stable reason independent of diagnostic wording.
    #[wasm_bindgen(getter)]
    pub fn code(&self) -> IncrementalErrorCode {
        self.code
    }

    /// Whether the edited session is inactive, read from Core after failure.
    /// Construction failures have no session and report false.
    #[wasm_bindgen(getter)]
    pub fn terminal(&self) -> bool {
        self.terminal
    }

    /// Fixed diagnostics contain neither source nor replacement text.
    #[wasm_bindgen(getter)]
    pub fn message(&self) -> String {
        match self.code {
            IncrementalErrorCode::NotActive => "incremental session is not active",
            IncrementalErrorCode::StaleRevision => "incremental revision does not match",
            IncrementalErrorCode::InvalidRange => "incremental byte range is invalid",
            IncrementalErrorCode::InvalidUtf8Boundary => "incremental range divides a UTF-8 scalar",
            IncrementalErrorCode::RevisionOverflow => "incremental revision overflow",
            IncrementalErrorCode::DocumentLengthOverflow => "incremental document length overflow",
            IncrementalErrorCode::CandidateLimitExceeded => "incremental candidate limit exceeded",
            IncrementalErrorCode::CacheLimitExceeded => "incremental cache limit exceeded",
            IncrementalErrorCode::ResultLimitExceeded => "incremental result limit exceeded",
            IncrementalErrorCode::TransactionStorageLimitExceeded => {
                "incremental transaction storage limit exceeded"
            }
            IncrementalErrorCode::ResourceAccountingOverflow => {
                "incremental resource accounting overflow"
            }
            IncrementalErrorCode::ExecutionInvariantViolated => {
                "incremental execution invariant violated"
            }
            IncrementalErrorCode::UnknownCoreError => "unknown Core incremental failure",
            IncrementalErrorCode::InvalidLimit => {
                "incremental limit must be a nonnegative safe integer representable on this target"
            }
            IncrementalErrorCode::OffsetOutOfRange => {
                "incremental offset must be a nonnegative safe integer representable on this target"
            }
            IncrementalErrorCode::InvalidRevision => {
                "incremental revision must be a bigint in the u64 range"
            }
        }
        .to_owned()
    }
}

impl IncrementalError {
    fn from_core(error: cribra::IncrementalError, terminal: bool) -> Self {
        use cribra::IncrementalError as Core;
        let code = match error {
            Core::NotActive => IncrementalErrorCode::NotActive,
            Core::StaleRevision { .. } => IncrementalErrorCode::StaleRevision,
            Core::InvalidRange => IncrementalErrorCode::InvalidRange,
            Core::InvalidUtf8Boundary => IncrementalErrorCode::InvalidUtf8Boundary,
            Core::RevisionOverflow => IncrementalErrorCode::RevisionOverflow,
            Core::DocumentLengthOverflow => IncrementalErrorCode::DocumentLengthOverflow,
            Core::CandidateLimitExceeded { .. } => IncrementalErrorCode::CandidateLimitExceeded,
            Core::CacheLimitExceeded { .. } => IncrementalErrorCode::CacheLimitExceeded,
            Core::ResultLimitExceeded { .. } => IncrementalErrorCode::ResultLimitExceeded,
            Core::TransactionStorageLimitExceeded { .. } => {
                IncrementalErrorCode::TransactionStorageLimitExceeded
            }
            Core::ResourceAccountingOverflow => IncrementalErrorCode::ResourceAccountingOverflow,
            Core::ExecutionInvariantViolated => IncrementalErrorCode::ExecutionInvariantViolated,
            _ => IncrementalErrorCode::UnknownCoreError,
        };
        Self { code, terminal }
    }
}

#[wasm_bindgen(typescript_custom_section)]
const INCREMENTAL_LIMITS_TYPES: &str = r#"
/** Missing/undefined budgets are unlimited; explicit zero is meaningful. */
export interface IncrementalLimits {
    readonly maxCandidates?: number;
    readonly maxRetainedCacheRecords?: number;
    readonly maxMaterializedResults?: number;
    readonly maxTransactionStorageBytes?: number;
}
"#;

#[wasm_bindgen]
extern "C" {
    /// Input-only typed limits whose raw property values are validated before Core.
    #[wasm_bindgen(typescript_type = "IncrementalLimits")]
    pub type IncrementalLimits;
    #[wasm_bindgen(method, getter, js_name = maxCandidates)]
    fn max_candidates(this: &IncrementalLimits) -> JsValue;
    #[wasm_bindgen(method, getter, js_name = maxRetainedCacheRecords)]
    fn max_retained_cache_records(this: &IncrementalLimits) -> JsValue;
    #[wasm_bindgen(method, getter, js_name = maxMaterializedResults)]
    fn max_materialized_results(this: &IncrementalLimits) -> JsValue;
    #[wasm_bindgen(method, getter, js_name = maxTransactionStorageBytes)]
    fn max_transaction_storage_bytes(this: &IncrementalLimits) -> JsValue;
}

#[wasm_bindgen]
impl ScanEngine {
    /// Copies the initial document directly into Core, starting at revision zero.
    /// Built-ins and custom rules retain Core's full incremental semantics.
    pub fn incremental(&self, source: &str) -> Result<IncrementalSession, IncrementalError> {
        self.open_incremental(source, cribra::IncrementalLimits::default())
    }

    /// Opens a Core document with four independent optional operation budgets.
    /// Invalid JS limits fail before construction and never create a session.
    #[wasm_bindgen(js_name = incrementalWithLimits)]
    pub fn incremental_with_limits(
        &self,
        source: &str,
        limits: &IncrementalLimits,
    ) -> Result<IncrementalSession, IncrementalError> {
        let limit = |value| {
            crate::numeric::js_limit(value).map_err(|()| IncrementalError {
                code: IncrementalErrorCode::InvalidLimit,
                terminal: false,
            })
        };
        let candidates = limit(limits.max_candidates())?;
        let cache = limit(limits.max_retained_cache_records())?;
        let results = limit(limits.max_materialized_results())?;
        let storage = limit(limits.max_transaction_storage_bytes())?;
        self.open_incremental(
            source,
            cribra::IncrementalLimits::default()
                .with_max_candidates(candidates)
                .with_max_retained_cache_records(cache)
                .with_max_materialized_results(results)
                .with_max_transaction_storage_bytes(storage),
        )
    }
}

impl ScanEngine {
    fn open_incremental(
        &self,
        source: &str,
        limits: cribra::IncrementalLimits,
    ) -> Result<IncrementalSession, IncrementalError> {
        self.scanner
            .incremental_session_with_limits(source.to_owned(), limits)
            .map(|inner| IncrementalSession { inner })
            .map_err(|error| IncrementalError::from_core(error, false))
    }
}

/// Core exclusively owns this document, revision and transaction state.
/// Terminal Core errors preserve snapshots while preventing future edits.
#[wasm_bindgen]
pub struct IncrementalSession {
    inner: cribra::IncrementalSession,
}

#[wasm_bindgen]
impl IncrementalSession {
    /// Replaces a half-open UTF-8 byte range. Every successful edit, including
    /// a no-op, returns the next exact bigint revision. Inputs never coerce or wrap.
    #[wasm_bindgen(js_name = applyEdit)]
    #[allow(
        non_snake_case,
        reason = "preserve the public TypeScript parameter spelling"
    )]
    pub fn apply_edit(
        &mut self,
        #[wasm_bindgen(unchecked_param_type = "bigint")] expectedRevision: JsValue,
        #[wasm_bindgen(unchecked_param_type = "number")] start: JsValue,
        #[wasm_bindgen(unchecked_param_type = "number")] end: JsValue,
        replacement: &str,
    ) -> Result<u64, IncrementalError> {
        let invalid = |code| IncrementalError {
            code,
            terminal: !self.inner.is_active(),
        };
        let revision = u64::try_from(expectedRevision)
            .map_err(|_| invalid(IncrementalErrorCode::InvalidRevision))?;
        let start = crate::numeric::js_usize(&start)
            .ok_or_else(|| invalid(IncrementalErrorCode::OffsetOutOfRange))?;
        let end = crate::numeric::js_usize(&end)
            .ok_or_else(|| invalid(IncrementalErrorCode::OffsetOutOfRange))?;
        self.edit_core(revision, start, end, replacement)
    }

    /// Clones one committed revision and complete report. Available after terminal
    /// Core failures. Snapshots retain no source and outlive edits/session free().
    pub fn snapshot(&self) -> IncrementalSnapshot {
        IncrementalSnapshot {
            revision: self.inner.revision(),
            result: ScanResult {
                source_bytes: self.inner.source().len(),
                report: self.inner.report().clone(),
            },
        }
    }

    /// Whether Core currently accepts edits.
    #[wasm_bindgen(js_name = isActive)]
    pub fn is_active(&self) -> bool {
        self.inner.is_active()
    }
}

impl IncrementalSession {
    fn edit_core(
        &mut self,
        revision: u64,
        start: usize,
        end: usize,
        replacement: &str,
    ) -> Result<u64, IncrementalError> {
        self.inner
            .apply_edit(revision, start..end, replacement)
            .map_err(|error| IncrementalError::from_core(error, !self.inner.is_active()))
    }
}

/// Independently owned metadata for one committed revision; no document accessor.
#[wasm_bindgen]
pub struct IncrementalSnapshot {
    revision: u64,
    result: ScanResult,
}

#[wasm_bindgen]
impl IncrementalSnapshot {
    /// Exact committed revision as a JavaScript bigint.
    #[wasm_bindgen(getter)]
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Returns a fresh, independently owned ScanResult on each access. Free each
    /// returned result separately, even after freeing this snapshot or its session.
    #[wasm_bindgen(getter)]
    pub fn result(&self) -> ScanResult {
        ScanResult {
            source_bytes: self.result.source_bytes,
            report: self.result.report.clone(),
        }
    }
}

#[cfg(test)]
#[path = "incremental_tests.rs"]
mod tests;
