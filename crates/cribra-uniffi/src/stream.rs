//! Owned, synchronized access to Core's consuming byte-stream sessions.

use std::sync::{Arc, Mutex, MutexGuard};

use crate::{ScanReport, SourceId};

/// Observable session lifecycle. Terminal calls never reactivate a session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum StreamState {
    /// Accepts binary chunks and genuine EOF.
    Active,
    /// EOF succeeded and the complete report was returned exactly once.
    Finished,
    /// Execution failed; all working results and the Core session were dropped.
    Failed,
}

/// Optional Core budgets; `None` is unlimited, and zero is passed unchanged.
///
/// Storage is Core's logical executor payload, not total process heap usage.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct StreamLimits {
    /// Cumulative raw rule candidates, including validator rejections.
    pub max_candidates: Option<u64>,
    /// Static logical executor storage checked before Core execution allocation.
    pub max_stream_storage_bytes: Option<u64>,
    /// Retained findings plus review candidates, before normalization/suppression.
    pub max_retained_results: Option<u64>,
}

/// Stable symbolic codes. Numeric discriminants and diagnostic text are not stable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum StreamErrorCode {
    /// Core rejected streaming admission for this ruleset.
    UnsupportedRuleSet,
    /// The Object is already Finished or Failed.
    NotActive,
    /// Definite malformed UTF-8 in pushed bytes.
    InvalidUtf8,
    /// EOF inside an incomplete UTF-8 scalar.
    IncompleteUtf8,
    /// Core's raw candidate limit was exceeded.
    CandidateLimitExceeded,
    /// Core's retained result limit was exceeded.
    ResultLimitExceeded,
    /// Core's logical storage requirement exceeds the admission limit.
    StorageLimitExceeded,
    /// Core's storage requirement arithmetic overflowed.
    StorageRequirementOverflow,
    /// Absolute source byte offset overflowed.
    SourceOffsetOverflow,
    /// Core's source line or scalar column overflowed.
    CoordinateOverflow,
    /// Core execution invariant failed.
    ExecutionInvariantViolated,
    /// A future Core error not understood by this adapter; never retried.
    UnknownCoreError,
    /// An input u64 budget cannot be represented by this target's usize.
    LimitOutOfRange,
    /// An earlier panic poisoned the session mutex; execution cannot resume.
    SessionPoisoned,
}

/// Admission or terminal execution error with no partial report or source text.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Error)]
pub enum StreamError {
    /// Core failure, terminal misuse, or an explicit adapter representation failure.
    Rejected {
        /// Stable symbolic reason.
        code: StreamErrorCode,
        /// Core resource limit, or unrepresentable input budget, when applicable.
        limit: Option<u64>,
        /// Core's required logical storage bytes, when applicable.
        required: Option<u64>,
        /// Diagnostic detail; not a machine-readable contract.
        detail: String,
    },
}

impl std::fmt::Display for StreamError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self::Rejected { detail, .. } = self;
        f.write_str(detail)
    }
}

impl std::error::Error for StreamError {}

impl From<cribra::StreamError> for StreamError {
    fn from(error: cribra::StreamError) -> Self {
        use cribra::StreamError as Core;
        let (code, limit, required) = match error {
            Core::UnsupportedRuleSet => (StreamErrorCode::UnsupportedRuleSet, None, None),
            Core::NotActive => (StreamErrorCode::NotActive, None, None),
            Core::InvalidUtf8 => (StreamErrorCode::InvalidUtf8, None, None),
            Core::IncompleteUtf8 => (StreamErrorCode::IncompleteUtf8, None, None),
            Core::CandidateLimitExceeded { limit } => (
                StreamErrorCode::CandidateLimitExceeded,
                Some(limit as u64),
                None,
            ),
            Core::ResultLimitExceeded { limit } => (
                StreamErrorCode::ResultLimitExceeded,
                Some(limit as u64),
                None,
            ),
            Core::StorageLimitExceeded { required, limit } => (
                StreamErrorCode::StorageLimitExceeded,
                Some(limit as u64),
                Some(required as u64),
            ),
            Core::StorageRequirementOverflow => {
                (StreamErrorCode::StorageRequirementOverflow, None, None)
            }
            Core::SourceOffsetOverflow => (StreamErrorCode::SourceOffsetOverflow, None, None),
            Core::CoordinateOverflow => (StreamErrorCode::CoordinateOverflow, None, None),
            Core::ExecutionInvariantViolated => {
                (StreamErrorCode::ExecutionInvariantViolated, None, None)
            }
            _ => (StreamErrorCode::UnknownCoreError, None, None),
        };
        Self::Rejected {
            code,
            limit,
            required,
            detail: error.to_string(),
        }
    }
}

impl StreamLimits {
    pub(crate) fn into_core(self) -> Result<cribra::StreamLimits, StreamError> {
        fn budget(value: Option<u64>, field: &str) -> Result<Option<usize>, StreamError> {
            value
                .map(|value| {
                    usize::try_from(value).map_err(|_| StreamError::Rejected {
                        code: StreamErrorCode::LimitOutOfRange,
                        limit: Some(value),
                        required: None,
                        detail: format!("{field} cannot be represented on this target"),
                    })
                })
                .transpose()
        }
        Ok(cribra::StreamLimits::default()
            .with_max_candidates(budget(self.max_candidates, "max_candidates")?)
            .with_max_stream_storage_bytes(budget(
                self.max_stream_storage_bytes,
                "max_stream_storage_bytes",
            )?)
            .with_max_retained_results(budget(self.max_retained_results, "max_retained_results")?))
    }
}

struct Inner {
    session: Option<cribra::StreamSession>,
    state: StreamState,
    source_bytes: usize,
}

impl Inner {
    fn fail(&mut self) {
        self.session = None;
        self.state = StreamState::Failed;
    }
}

/// One Core stream, independent of the Scanner's foreign reference lifetime.
///
/// Calls are serialized. Callers must supply chunks in source order. `push`
/// returns no report; `finish` consumes the inner Core session exactly once.
#[derive(uniffi::Object)]
pub struct StreamSession {
    scanner: cribra::Scanner,
    source_id: SourceId,
    inner: Mutex<Inner>,
}

impl StreamSession {
    pub(crate) fn from_core(
        session: cribra::StreamSession,
        source_id: SourceId,
        scanner: cribra::Scanner,
    ) -> Arc<Self> {
        Arc::new(Self {
            scanner,
            source_id,
            inner: Mutex::new(Inner {
                session: Some(session),
                state: StreamState::Active,
                source_bytes: 0,
            }),
        })
    }

    fn lock(&self) -> Result<MutexGuard<'_, Inner>, StreamError> {
        self.inner.lock().map_err(|poisoned| {
            poisoned.into_inner().fail();
            StreamError::Rejected {
                code: StreamErrorCode::SessionPoisoned,
                limit: None,
                required: None,
                detail: "session mutex poisoned; execution discarded".into(),
            }
        })
    }
}

#[uniffi::export]
impl StreamSession {
    /// Observes Active, Finished or Failed without exposing working results.
    pub fn state(&self) -> StreamState {
        self.lock()
            .map(|inner| inner.state)
            .unwrap_or(StreamState::Failed)
    }

    /// Feeds raw bytes (`Data` in Swift); UTF-8 carry and validation belong to Core.
    /// Empty chunks are no-ops only while Active. Any Core error is terminal.
    pub fn push(&self, chunk: Vec<u8>) -> Result<(), StreamError> {
        let mut inner = self.lock()?;
        let result = inner
            .session
            .as_mut()
            .ok_or(cribra::StreamError::NotActive)?
            .push(&chunk);
        if let Err(error) = result {
            inner.fail();
            return Err(error.into());
        }
        // Core accepted these bytes. Count only their length for the existing
        // report DTO; no input text is copied or independently decoded.
        match inner.source_bytes.checked_add(chunk.len()) {
            Some(total) => inner.source_bytes = total,
            None => {
                inner.fail();
                return Err(cribra::StreamError::SourceOffsetOverflow.into());
            }
        }
        Ok(())
    }

    /// Signals genuine EOF and transfers a complete report exactly once.
    /// Failure returns no report. Later calls return NotActive without changing
    /// the existing terminal state.
    pub fn finish(&self) -> Result<ScanReport, StreamError> {
        let mut inner = self.lock()?;
        let session = inner.session.take().ok_or(cribra::StreamError::NotActive)?;
        inner.state = StreamState::Failed;
        let core_report = session.finish()?;
        let report = ScanReport::from_parts(
            &self.source_id,
            inner.source_bytes,
            &core_report,
            Some(&self.scanner),
        );
        inner.state = StreamState::Finished;
        Ok(report)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn poisoned_mutex_discards_core_and_exposes_failed_state() {
        let scanner = cribra::Scanner::builder().build().unwrap();
        let session = StreamSession::from_core(
            scanner.stream().unwrap(),
            SourceId {
                value: "poison-test".into(),
            },
            scanner.clone(),
        );
        let other = Arc::clone(&session);
        let panic = std::thread::spawn(move || {
            let _guard = other.inner.lock().unwrap();
            panic!("test-only injected panic while holding the session mutex");
        });
        assert!(panic.join().is_err());
        assert_eq!(session.state(), StreamState::Failed);
        assert!(matches!(
            session.finish(),
            Err(StreamError::Rejected {
                code: StreamErrorCode::SessionPoisoned,
                ..
            })
        ));
        let inner = session.inner.lock().err().unwrap().into_inner();
        assert!(inner.session.is_none());
    }
}
