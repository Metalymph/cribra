//! Reusable synchronous Core scanner owned through UniFFI's object lifecycle.

use std::sync::Arc;

use crate::{IncrementalError, IncrementalLimits, IncrementalSession};
use crate::{RuleMetadata, ScanReport, ScanResults, ScanSource, ScannerBuildError, ScannerConfig};
use crate::{SourceId, StreamError, StreamLimits, StreamSession};

/// A reusable Core scanner with default or explicitly configured rules.
///
/// UniFFI manages shared ownership and releases the Core scanner when the last
/// object reference is dropped. Calls keep no input text or previous results.
#[derive(uniffi::Object)]
pub struct Scanner {
    pub(crate) inner: cribra::Scanner,
}

#[uniffi::export]
impl Scanner {
    /// Compiles the Core's default built-in rules once for repeated scans.
    #[uniffi::constructor]
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            inner: cribra::Scanner::default(),
        })
    }

    /// Builds a scanner from explicit built-in/custom selection.
    ///
    /// All rule construction, validation and compilation are delegated to Core.
    /// Returns a structured Core construction error if configuration fails.
    #[uniffi::constructor]
    pub fn configured(config: ScannerConfig) -> Result<Arc<Self>, ScannerBuildError> {
        Ok(Arc::new(Self {
            inner: config.build_core()?,
        }))
    }

    /// Returns active rule metadata in Core order, without matcher payloads.
    pub fn active_rules(&self) -> Vec<RuleMetadata> {
        self.inner.rule_metadata().map(RuleMetadata::from).collect()
    }

    /// Creates an owned, revisioned Core document with unlimited optional budgets.
    pub fn incremental(
        &self,
        source: ScanSource,
    ) -> Result<Arc<IncrementalSession>, IncrementalError> {
        let core = self
            .inner
            .incremental_session(source.text)
            .map_err(|error| IncrementalError::from_core(error, true))?;
        Ok(IncrementalSession::from_core(
            core,
            source.id,
            self.inner.clone(),
        ))
    }

    /// Creates a Core incremental document with preventive per-operation limits.
    pub fn incremental_with_limits(
        &self,
        source: ScanSource,
        limits: IncrementalLimits,
    ) -> Result<Arc<IncrementalSession>, IncrementalError> {
        let core = self
            .inner
            .incremental_session_with_limits(source.text, limits.into_core()?)
            .map_err(|error| IncrementalError::from_core(error, true))?;
        Ok(IncrementalSession::from_core(
            core,
            source.id,
            self.inner.clone(),
        ))
    }

    /// Opens a Core streaming session with unlimited optional resource budgets.
    /// Unsupported rulesets fail admission; no whole-source fallback is used.
    pub fn stream(&self, source_id: SourceId) -> Result<Arc<StreamSession>, StreamError> {
        Ok(StreamSession::from_core(
            self.inner.stream()?,
            source_id,
            self.inner.clone(),
        ))
    }

    /// Opens a streaming session with Core-enforced per-source resource limits.
    pub fn stream_with_limits(
        &self,
        source_id: SourceId,
        limits: StreamLimits,
    ) -> Result<Arc<StreamSession>, StreamError> {
        let core = self.inner.stream_with_limits(limits.into_core()?)?;
        Ok(StreamSession::from_core(
            core,
            source_id,
            self.inner.clone(),
        ))
    }

    /// Scans one identified UTF-8 source without retaining its contents.
    pub fn scan(&self, source: ScanSource) -> ScanReport {
        let results = self.inner.scan([(source.id, source.text.as_str())]);
        let entry = &results.as_slice()[0];
        ScanReport::from_parts(
            entry.key(),
            entry.source_bytes(),
            entry.report(),
            Some(&self.inner),
        )
    }

    /// Scans sources in input order, preserving IDs including duplicates.
    ///
    /// Findings retain Core order. Empty input produces no reports and a zero
    /// summary. No source contents are included in the returned records.
    pub fn scan_many(&self, sources: Vec<ScanSource>) -> ScanResults {
        let results = self.inner.scan(
            sources
                .iter()
                .map(|source| (&source.id, source.text.as_str())),
        );
        ScanResults {
            reports: results
                .iter()
                .map(|entry| {
                    ScanReport::from_parts(
                        entry.key(),
                        entry.source_bytes(),
                        entry.report(),
                        Some(&self.inner),
                    )
                })
                .collect(),
            summary: results.summary().into(),
        }
    }
}

#[cfg(feature = "fuse")]
#[uniffi::export]
impl Scanner {
    /// Delegates independent source scans directly to Core's parallel scanner.
    ///
    /// Returns the same owned DTOs and input ordering as `scan_many`, including
    /// duplicate/Unicode IDs, finding order, candidate counts and UTF-8 locations.
    /// This synchronous call uses Core's current Rayon pool, without adapter
    /// scheduling, a dedicated pool, source chunking or a sequential fallback.
    pub fn parallel_scan(&self, sources: Vec<ScanSource>) -> ScanResults {
        let results = self.inner.parallel_scan(
            sources
                .iter()
                .map(|source| (&source.id, source.text.as_str())),
        );
        ScanResults {
            reports: results
                .iter()
                .map(|entry| {
                    ScanReport::from_parts(
                        entry.key(),
                        entry.source_bytes(),
                        entry.report(),
                        Some(&self.inner),
                    )
                })
                .collect(),
            summary: results.summary().into(),
        }
    }
}
