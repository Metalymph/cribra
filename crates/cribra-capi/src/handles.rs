//! Opaque Rust-owned objects used by the native ABI.

use cribra::{ScanReport, ScanResults, Scanner, ScannerBuilder, transform::ShareBundle};

/// Opaque native scanner-builder handle.
pub struct CribraBuilder {
    pub(crate) inner: Option<ScannerBuilder>,
}

impl CribraBuilder {
    pub(crate) fn empty() -> Self {
        Self {
            inner: Some(ScannerBuilder::new()),
        }
    }
}

/// Opaque immutable scanner handle.
pub struct CribraScanner {
    pub(crate) inner: Scanner,
}

impl CribraScanner {
    pub(crate) fn new(inner: Scanner) -> Self {
        Self { inner }
    }
}

/// Opaque, exclusively accessed Core-owned editable UTF-8 document.
///
/// No adapter source buffer, revision counter or report cache is retained.
/// Ordinary Core terminal errors preserve the committed snapshot; a contained
/// panic permanently removes the Core session. Free with cribra_incremental_free.
pub struct CribraIncremental {
    pub(crate) inner: Option<cribra::IncrementalSession>,
}

impl CribraIncremental {
    pub(crate) fn new(inner: cribra::IncrementalSession) -> Self {
        Self { inner: Some(inner) }
    }

    pub(crate) fn with_core<T>(
        &mut self,
        operation: impl FnOnce(&mut cribra::IncrementalSession) -> T,
    ) -> Result<T, crate::CribraStatus> {
        let mut core = self
            .inner
            .take()
            .ok_or(crate::CRIBRA_INCREMENTAL_SESSION_INVALIDATED)?;
        // Restore on every normal return, including ordinary Core errors.
        // Unwinding drops Core and leaves None, forbidding panic recovery.
        let result = operation(&mut core);
        self.inner = Some(core);
        Ok(result)
    }
}

/// Opaque, exclusively accessed byte-oriented Core streaming session.
///
/// Owns no adapter source buffer. The handle remains valid after finish or a
/// terminal failure until freed with [`crate::cribra_stream_free`].
pub struct CribraStream {
    pub(crate) inner: Option<cribra::StreamSession>,
    pub(crate) source_bytes: usize,
}

impl CribraStream {
    pub(crate) fn new(inner: cribra::StreamSession) -> Self {
        Self {
            inner: Some(inner),
            source_bytes: 0,
        }
    }

    pub(crate) fn push(&mut self, chunk: &[u8]) -> Result<(), cribra::StreamError> {
        // Taking ownership first also leaves the handle inactive if Core panics.
        let mut inner = self.inner.take().ok_or(cribra::StreamError::NotActive)?;
        inner.push(chunk)?;
        self.source_bytes = self
            .source_bytes
            .checked_add(chunk.len())
            .ok_or(cribra::StreamError::SourceOffsetOverflow)?;
        self.inner = Some(inner);
        Ok(())
    }

    pub(crate) fn finish(&mut self) -> Result<CribraReport, cribra::StreamError> {
        let inner = self.inner.take().ok_or(cribra::StreamError::NotActive)?;
        Ok(CribraReport::new(inner.finish()?, self.source_bytes))
    }
}

/// Opaque immutable report handle.
///
/// v0.3.3 deliberately establishes report ownership only. The report read
/// surface is introduced by v0.3.4.
pub struct CribraReport {
    pub(crate) inner: ScanReport,
    pub(crate) source_bytes: usize,
}

impl CribraReport {
    pub(crate) fn new(inner: ScanReport, source_bytes: usize) -> Self {
        Self {
            inner,
            source_bytes,
        }
    }
}

/// Opaque Rust-owned transformed UTF-8 output.
///
/// Content is owned by Cribra until [`crate::cribra_output_free`] is called.
pub struct CribraOutput {
    pub(crate) inner: String,
}

impl CribraOutput {
    pub(crate) fn new(inner: String) -> Self {
        Self { inner }
    }
}

/// Opaque Rust-owned ordered batch results.
///
/// Keys are copied into Rust-owned `String` values. Reports retain metadata only;
/// original source text is never stored.
pub struct CribraBatchResults {
    pub(crate) inner: ScanResults<String>,
}

impl CribraBatchResults {
    pub(crate) fn new(inner: ScanResults<String>) -> Self {
        Self { inner }
    }
}

/// Opaque Rust-owned share-safe transformed batch.
///
/// The bundle owns cloned source keys, transformed UTF-8 content, and
/// share-safe manifest metadata. It never retains original source text.
pub struct CribraShareBundle {
    pub(crate) inner: ShareBundle<String>,
}

impl CribraShareBundle {
    pub(crate) fn new(inner: ShareBundle<String>) -> Self {
        Self { inner }
    }
}
