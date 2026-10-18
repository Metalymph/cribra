//! Synchronized projection of Core's transactional incremental documents.

use std::sync::{Arc, Mutex, MutexGuard};

use crate::{ScanReport, SourceId};

/// Core-derived lifecycle; there is no EOF or Finished state for editable documents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum IncrementalState {
    /// Core permits further edit attempts.
    Active,
    /// Core is terminal, or an earlier panic poisoned the adapter mutex.
    Failed,
}

/// One owned, source-free view of the last committed incremental version.
/// Revision, source byte length and report are captured under the same mutex.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct IncrementalSnapshot {
    /// Core revision of this exact report, including successful no-op edits.
    pub revision: u64,
    /// Complete existing report projection at that revision; no source text.
    pub report: ScanReport,
}

/// Optional Core per-operation budgets. None is unlimited; zero is preserved.
///
/// Cache/storage include simultaneously owned old and staged data. Logical
/// storage excludes allocator overhead, spare capacity and FFI/report clones.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct IncrementalLimits {
    /// Newly emitted raw rule candidates per construction or edit.
    pub max_candidates: Option<u64>,
    /// Simultaneously owned rule/structural cache records, including losers.
    pub max_retained_cache_records: Option<u64>,
    /// Materialized findings and structural result copies per operation.
    pub max_materialized_results: Option<u64>,
    /// Conservative logical transaction payload, including the old snapshot.
    pub max_transaction_storage_bytes: Option<u64>,
}

/// Stable symbolic error codes; diagnostic strings and numeric values are not stable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum IncrementalErrorCode {
    /// Core no longer permits edits.
    NotActive,
    /// The expected revision does not match the current revision.
    StaleRevision,
    /// Reversed or out-of-document half-open byte range.
    InvalidRange,
    /// A byte endpoint divides a UTF-8 scalar.
    InvalidUtf8Boundary,
    /// Core cannot increment the revision.
    RevisionOverflow,
    /// Core cannot represent the replacement document length.
    DocumentLengthOverflow,
    /// Per-operation raw candidate budget exceeded.
    CandidateLimitExceeded,
    /// Old plus staged retained cache budget exceeded.
    CacheLimitExceeded,
    /// Materialized result budget exceeded.
    ResultLimitExceeded,
    /// Logical transaction payload budget exceeded.
    TransactionStorageLimitExceeded,
    /// Core resource arithmetic overflowed.
    ResourceAccountingOverflow,
    /// Core matching/cache/transaction invariant failed.
    ExecutionInvariantViolated,
    /// Future Core error not understood by this adapter.
    UnknownCoreError,
    /// A u64 construction budget is not representable by target usize.
    LimitOutOfRange,
    /// A u64 edit endpoint is not representable by target usize.
    OffsetOutOfRange,
    /// An earlier panic poisoned the adapter mutex; panic recovery is unsupported.
    SessionPoisoned,
}

/// Structured construction/edit failure, with Core's lifecycle observed after errors.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Error)]
pub enum IncrementalError {
    /// Failure without a partially committed report or source-text diagnostic.
    Rejected {
        /// Stable symbolic reason.
        code: IncrementalErrorCode,
        /// Whether further edits are blocked; construction failures create no Object.
        terminal: bool,
        /// Supplied revision for a Core stale-revision failure.
        expected_revision: Option<u64>,
        /// Current revision for a Core stale-revision failure.
        actual_revision: Option<u64>,
        /// Core resource limit, if applicable.
        limit: Option<u64>,
        /// Core required logical bytes, if applicable.
        required: Option<u64>,
        /// Original Core diagnostic or explicit adapter representation failure.
        detail: String,
    },
}

impl std::fmt::Display for IncrementalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self::Rejected { detail, .. } = self;
        f.write_str(detail)
    }
}
impl std::error::Error for IncrementalError {}

impl IncrementalError {
    pub(crate) fn from_core(error: cribra::IncrementalError, terminal: bool) -> Self {
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
        let (expected_revision, actual_revision) = match error {
            Core::StaleRevision { expected, actual } => (Some(expected), Some(actual)),
            _ => (None, None),
        };
        let (limit, required) = match error {
            Core::CandidateLimitExceeded { limit }
            | Core::CacheLimitExceeded { limit }
            | Core::ResultLimitExceeded { limit } => (Some(limit as u64), None),
            Core::TransactionStorageLimitExceeded { required, limit } => {
                (Some(limit as u64), Some(required as u64))
            }
            _ => (None, None),
        };
        Self::Rejected {
            code,
            terminal,
            expected_revision,
            actual_revision,
            limit,
            required,
            detail: error.to_string(),
        }
    }

    fn adapter(code: IncrementalErrorCode, terminal: bool, detail: String) -> Self {
        Self::Rejected {
            code,
            terminal,
            expected_revision: None,
            actual_revision: None,
            limit: None,
            required: None,
            detail,
        }
    }
}

impl IncrementalLimits {
    pub(crate) fn into_core(self) -> Result<cribra::IncrementalLimits, IncrementalError> {
        fn budget(value: Option<u64>, field: &str) -> Result<Option<usize>, IncrementalError> {
            value
                .map(|value| {
                    usize::try_from(value).map_err(|_| IncrementalError::Rejected {
                        code: IncrementalErrorCode::LimitOutOfRange,
                        terminal: true,
                        expected_revision: None,
                        actual_revision: None,
                        limit: Some(value),
                        required: None,
                        detail: format!("{field} cannot be represented on this target"),
                    })
                })
                .transpose()
        }
        Ok(cribra::IncrementalLimits::default()
            .with_max_candidates(budget(self.max_candidates, "max_candidates")?)
            .with_max_retained_cache_records(budget(
                self.max_retained_cache_records,
                "max_retained_cache_records",
            )?)
            .with_max_materialized_results(budget(
                self.max_materialized_results,
                "max_materialized_results",
            )?)
            .with_max_transaction_storage_bytes(budget(
                self.max_transaction_storage_bytes,
                "max_transaction_storage_bytes",
            )?))
    }
}

/// An owned Core document with serialized transactional edits and snapshot reads.
///
/// Only Core retains the editable source. No source getter is exported. Each
/// call is synchronized; separate revision/report reads can observe intervening
/// edits; `snapshot` captures both atomically. No foreign callback or UniFFI
/// lowering runs while holding the mutex.
#[derive(uniffi::Object)]
pub struct IncrementalSession {
    source_id: SourceId,
    inner: Mutex<cribra::IncrementalSession>,
}

impl IncrementalSession {
    pub(crate) fn from_core(core: cribra::IncrementalSession, source_id: SourceId) -> Arc<Self> {
        Arc::new(Self {
            source_id,
            inner: Mutex::new(core),
        })
    }

    fn lock(&self) -> Result<MutexGuard<'_, cribra::IncrementalSession>, IncrementalError> {
        self.inner.lock().map_err(|_| {
            IncrementalError::adapter(
                IncrementalErrorCode::SessionPoisoned,
                true,
                "session mutex poisoned; panic recovery is unsupported".into(),
            )
        })
    }
}

#[uniffi::export]
impl IncrementalSession {
    /// Last committed Core revision, initially zero; readable after Core terminal errors.
    /// Only an adapter mutex poison can prevent this read.
    pub fn revision(&self) -> Result<u64, IncrementalError> {
        Ok(self.lock()?.revision())
    }

    /// Copies the last complete report without source text, including after Core failure.
    /// Only an adapter mutex poison can prevent this read.
    pub fn report(&self) -> Result<ScanReport, IncrementalError> {
        let (source_bytes, report) = {
            let core = self.lock()?;
            (core.source().len(), core.report().clone())
        };
        Ok(ScanReport::from_parts(
            &self.source_id,
            source_bytes,
            &report,
        ))
    }

    /// Atomically captures the last committed revision and matching report.
    /// Readable after Core failure, with the same poison error as existing getters.
    /// The lock is released before DTO projection and UniFFI lowering.
    pub fn snapshot(&self) -> Result<IncrementalSnapshot, IncrementalError> {
        let (revision, source_bytes, report) = {
            let core = self.lock()?;
            (core.revision(), core.source().len(), core.report().clone())
        };
        Ok(IncrementalSnapshot {
            revision,
            report: ScanReport::from_parts(&self.source_id, source_bytes, &report),
        })
    }

    /// Replaces zero-based UTF-8 byte range [start, end), returning the new revision.
    /// Core validates revision/range/boundaries and increments even for no-op edits.
    pub fn apply_edit(
        &self,
        expected_revision: u64,
        start: u64,
        end: u64,
        replacement: String,
    ) -> Result<u64, IncrementalError> {
        let mut core = self.lock()?;
        let endpoint = |value| {
            usize::try_from(value).map_err(|_| {
                IncrementalError::adapter(
                    IncrementalErrorCode::OffsetOutOfRange,
                    !core.is_active(),
                    "edit endpoint cannot be represented on this target".into(),
                )
            })
        };
        let range = endpoint(start)?..endpoint(end)?;
        let result = core.apply_edit(expected_revision, range, &replacement);
        let terminal = !core.is_active();
        drop(core);
        result.map_err(|error| IncrementalError::from_core(error, terminal))
    }

    /// Core activity; a poisoned mutex is always Failed and cannot resume edits.
    pub fn state(&self) -> IncrementalState {
        match self.lock() {
            Ok(core) if core.is_active() => IncrementalState::Active,
            _ => IncrementalState::Failed,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rare_core_error_conversions_keep_the_supplied_core_lifecycle() {
        for (error, code, terminal) in [
            (
                cribra::IncrementalError::RevisionOverflow,
                IncrementalErrorCode::RevisionOverflow,
                false,
            ),
            (
                cribra::IncrementalError::DocumentLengthOverflow,
                IncrementalErrorCode::DocumentLengthOverflow,
                false,
            ),
            (
                cribra::IncrementalError::ResourceAccountingOverflow,
                IncrementalErrorCode::ResourceAccountingOverflow,
                true,
            ),
            (
                cribra::IncrementalError::ExecutionInvariantViolated,
                IncrementalErrorCode::ExecutionInvariantViolated,
                true,
            ),
        ] {
            let IncrementalError::Rejected {
                code: actual_code,
                terminal: actual_terminal,
                detail,
                ..
            } = IncrementalError::from_core(error, terminal);
            assert_eq!(actual_code, code);
            assert_eq!(actual_terminal, terminal);
            assert_eq!(detail, error.to_string());
        }
    }

    #[test]
    fn mutex_poison_is_failed_without_attempting_core_panic_recovery() {
        let scanner = cribra::Scanner::builder().build().unwrap();
        let session = IncrementalSession::from_core(
            scanner.incremental_session("KEEP".into()).unwrap(),
            SourceId {
                value: "poison-test".into(),
            },
        );
        let other = Arc::clone(&session);
        let thread = std::thread::spawn(move || {
            let _guard = other.inner.lock().unwrap();
            panic!("test-only panic while holding the incremental mutex");
        });
        assert!(thread.join().is_err());
        assert_eq!(session.state(), IncrementalState::Failed);
        for error in [
            session.revision().unwrap_err(),
            session.report().unwrap_err(),
            session.snapshot().unwrap_err(),
            session.apply_edit(0, 0, 0, String::new()).unwrap_err(),
        ] {
            assert!(matches!(
                error,
                IncrementalError::Rejected {
                    code: IncrementalErrorCode::SessionPoisoned,
                    terminal: true,
                    ..
                }
            ));
        }
    }
}
