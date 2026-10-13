//! Exact owned mutable-document scanning.
//!
//! Construction replays the authoritative pipeline; edits localize literal and
//! structural starts, with whole-source Prefix/Suffix/regex fallback. No regional
//! indexes or checkpoints are retained. Cached rejected and hidden records remain
//! available for revalidation and global authoritative finalization.
//!
//! The old and staged snapshots coexist until commit. Raw emissions are checked
//! before immediate ingestion; no raw candidate collection is retained. Controlled
//! document, cache, structural-span, accepted-projection and report payloads are
//! checked before allocation/insertion, including conservative Box conversion
//! destinations. Reservations survive until ownership drops or transfers.
//!
//! Logical payload excludes Vec spare capacity, allocator overhead, stack state,
//! shared compiled configuration and caller-owned clones. Opaque regex caches,
//! capture workspace and validator allocations (for example decoded credentials,
//! lowercase strings and parsing scratch) are outside this contract. No allocator
//! failure recovery or total heap-memory guarantee is provided.
//!
//! Cache size depends on all raw occurrences, including rejected and hidden
//! records. Copying the document, walking caches and global finalization remain
//! necessary; local enumeration does not promise sublinear total edit cost.

use std::{ops::Range, sync::Arc};

use crate::{
    Confidence, ScanReport, SensitiveCandidate,
    candidate_detection::{
        RECOVERY_LIKE_LEN, detect_owned_spans_with_emit, materialize_candidates_with_emit,
    },
    compiled_rule::{CompiledRuleSet, IncrementalWork, InternalFinding, WorkMetric},
    scanner::{AcceptedCandidate, finalize_report_accounted, validate_raw_candidate},
    validators::{contextual::ValidationContext, dispatch::ValidatorStreamingRequirement},
};

/// Optional per-operation limits for an owned mutable document.
///
/// Defaults are unlimited. Cache limits cover old and staged records together;
/// storage covers controlled logical payload, not capacity or total heap memory.
#[derive(Debug, Default, Copy, Clone, Eq, PartialEq)]
pub struct IncrementalLimits {
    max_candidates: Option<usize>,
    max_retained_cache_records: Option<usize>,
    max_materialized_results: Option<usize>,
    max_transaction_storage_bytes: Option<usize>,
}

impl IncrementalLimits {
    /// Limits newly emitted raw rule candidates per construction or edit.
    /// Rejections and duplicates count; reuse and revalidation do not.
    #[must_use]
    pub const fn with_max_candidates(mut self, limit: Option<usize>) -> Self {
        self.max_candidates = limit;
        self
    }
    /// Limits simultaneously owned rule and unsuppressed structural cache records.
    /// Old and staged records count, including rejected and normalization losers.
    #[must_use]
    pub const fn with_max_retained_cache_records(mut self, limit: Option<usize>) -> Self {
        self.max_retained_cache_records = limit;
        self
    }
    /// Limits constructed findings and structural result copies per operation.
    /// Normalization precedes finding construction; suppression refunds no tickets.
    #[must_use]
    pub const fn with_max_materialized_results(mut self, limit: Option<usize>) -> Self {
        self.max_materialized_results = limit;
        self
    }
    /// Limits conservative peak logical transaction payload, including the old snapshot.
    /// Excludes spare capacity, allocator overhead, compiled configuration, opaque
    /// regex/validator workspace, stack metadata and caller-owned report clones.
    #[must_use]
    pub const fn with_max_transaction_storage_bytes(mut self, limit: Option<usize>) -> Self {
        self.max_transaction_storage_bytes = limit;
        self
    }
}

/// Recoverable edit errors or terminal resource/execution failures.
/// Errors never contain source text; terminal failures preserve the old snapshot.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[non_exhaustive]
pub enum IncrementalError {
    /// An earlier terminal failure prevents further edits.
    NotActive,
    /// The edit targets a different revision.
    StaleRevision {
        /// Supplied revision.
        expected: u64,
        /// Current revision.
        actual: u64,
    },
    /// The byte range is reversed or outside the document.
    InvalidRange,
    /// A range endpoint splits a UTF-8 scalar.
    InvalidUtf8Boundary,
    /// The revision cannot be incremented.
    RevisionOverflow,
    /// The replacement length is not representable by a contiguous Rust string.
    DocumentLengthOverflow,
    /// A raw emission would exceed the per-operation limit.
    CandidateLimitExceeded {
        /// Configured limit.
        limit: usize,
    },
    /// Old and staged cache ownership would exceed the limit.
    CacheLimitExceeded {
        /// Configured limit.
        limit: usize,
    },
    /// Result materialization would exceed the per-operation limit.
    ResultLimitExceeded {
        /// Configured limit.
        limit: usize,
    },
    /// Conservative logical transaction payload would exceed the limit.
    TransactionStorageLimitExceeded {
        /// Required logical bytes at this point.
        required: usize,
        /// Configured limit.
        limit: usize,
    },
    /// A resource count or payload requirement cannot be represented.
    ResourceAccountingOverflow,
    /// Matching, cache or transaction invariants were violated.
    ExecutionInvariantViolated,
}

impl std::fmt::Display for IncrementalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "incremental scan failed: {self:?}")
    }
}
impl std::error::Error for IncrementalError {}

/// Checks happen before ownership acquisition. Reservations remain live until
/// buffers drop or transfer, including simultaneous conversion destinations.
struct Operation {
    limits: IncrementalLimits,
    raw: usize,
    cache: usize,
    results: usize,
    bytes: usize,
    peak: usize,
    peak_cache: usize,
    #[cfg(test)]
    observed_peak: usize,
}

fn payload<T>(count: usize) -> Result<usize, IncrementalError> {
    count
        .checked_mul(std::mem::size_of::<T>())
        .filter(|bytes| *bytes <= isize::MAX as usize)
        .ok_or(IncrementalError::ResourceAccountingOverflow)
}
fn sum(a: usize, b: usize) -> Result<usize, IncrementalError> {
    a.checked_add(b)
        .ok_or(IncrementalError::ResourceAccountingOverflow)
}

impl Operation {
    fn new(
        limits: IncrementalLimits,
        bytes: usize,
        cache: usize,
    ) -> Result<Self, IncrementalError> {
        let mut operation = Self {
            limits,
            raw: 0,
            cache: 0,
            results: 0,
            bytes: 0,
            peak: 0,
            peak_cache: 0,
            #[cfg(test)]
            observed_peak: bytes,
        };
        operation.cache_records(cache)?;
        operation.bytes(bytes)?;
        Ok(operation)
    }
    fn raw(&mut self) -> Result<(), IncrementalError> {
        let next = sum(self.raw, 1)?;
        if let Some(limit) = self.limits.max_candidates
            && next > limit
        {
            return Err(IncrementalError::CandidateLimitExceeded { limit });
        }
        self.raw = next;
        Ok(())
    }
    fn cache_records(&mut self, count: usize) -> Result<(), IncrementalError> {
        let next = sum(self.cache, count)?;
        if let Some(limit) = self.limits.max_retained_cache_records
            && next > limit
        {
            return Err(IncrementalError::CacheLimitExceeded { limit });
        }
        self.cache = next;
        self.peak_cache = self.peak_cache.max(next);
        Ok(())
    }
    fn reserve<T>(&mut self, count: usize) -> Result<(), IncrementalError> {
        self.bytes(payload::<T>(count)?)
    }
    fn release<T>(&mut self, count: usize) -> Result<(), IncrementalError> {
        self.release_bytes(payload::<T>(count)?)
    }
}

pub(crate) trait ReportAccounting {
    #[cfg(test)]
    fn observe_gap(&mut self, _gap: usize) -> Result<(), IncrementalError> {
        Ok(())
    }
    fn results(&mut self, count: usize) -> Result<(), IncrementalError>;
    fn bytes(&mut self, bytes: usize) -> Result<(), IncrementalError>;
    fn release_bytes(&mut self, bytes: usize) -> Result<(), IncrementalError>;
}
impl ReportAccounting for () {
    fn results(&mut self, _: usize) -> Result<(), IncrementalError> {
        Ok(())
    }
    fn bytes(&mut self, _: usize) -> Result<(), IncrementalError> {
        Ok(())
    }
    fn release_bytes(&mut self, _: usize) -> Result<(), IncrementalError> {
        Ok(())
    }
}
impl ReportAccounting for Operation {
    #[cfg(test)]
    fn observe_gap(&mut self, gap: usize) -> Result<(), IncrementalError> {
        let observed = self
            .bytes
            .checked_sub(gap)
            .ok_or(IncrementalError::ExecutionInvariantViolated)?;
        self.observed_peak = self.observed_peak.max(observed);
        assert!(self.observed_peak <= self.peak);
        Ok(())
    }

    fn results(&mut self, count: usize) -> Result<(), IncrementalError> {
        let next = sum(self.results, count)?;
        if let Some(limit) = self.limits.max_materialized_results
            && next > limit
        {
            return Err(IncrementalError::ResultLimitExceeded { limit });
        }
        self.results = next;
        Ok(())
    }
    fn bytes(&mut self, count: usize) -> Result<(), IncrementalError> {
        let next = sum(self.bytes, count)?;
        if let Some(limit) = self.limits.max_transaction_storage_bytes
            && next > limit
        {
            return Err(IncrementalError::TransactionStorageLimitExceeded {
                required: next,
                limit,
            });
        }
        self.bytes = next;
        self.peak = self.peak.max(next);
        Ok(())
    }
    fn release_bytes(&mut self, count: usize) -> Result<(), IncrementalError> {
        self.bytes = self
            .bytes
            .checked_sub(count)
            .ok_or(IncrementalError::ExecutionInvariantViolated)?;
        Ok(())
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
enum Lifecycle {
    Active,
    Failed,
}

/// One raw emission, before normalization. None denotes validator rejection.
/// Rule metadata and candidate text are never copied into cache records.
#[derive(Debug, Copy, Clone)]
struct CachedCandidate {
    finding: InternalFinding,
    confidence: Option<Confidence>,
}

struct Snapshot {
    source: String,
    revision: u64,
    per_rule: Vec<Vec<CachedCandidate>>,
    structural: Vec<SensitiveCandidate>,
    report: ScanReport,
    #[cfg(test)]
    work: IncrementalWork,
    #[cfg(test)]
    peak_bytes: usize,
    #[cfg(test)]
    peak_cache: usize,
    #[cfg(test)]
    observed_peak: usize,
}

impl Snapshot {
    fn replay(
        rules: &CompiledRuleSet,
        source: String,
        revision: u64,
        operation: &mut Operation,
    ) -> Result<Self, IncrementalError> {
        operation.reserve::<Vec<CachedCandidate>>(rules.len())?;
        let mut per_rule = vec![Vec::new(); rules.len()];
        rules.scan_with_emit(&source, &mut |finding| {
            ingest(rules, &source, &mut per_rule, operation, finding)
        })?;
        let mut spans = Vec::new();
        detect_owned_spans_with_emit(&source, 0..source.len(), None, &mut |start, end| {
            stage_span(operation, &mut spans, start, end)
        })?;
        Self::assemble(
            rules,
            source,
            revision,
            per_rule,
            spans,
            IncrementalWork::default(),
            operation,
        )
    }

    #[cfg(test)]
    fn from_emissions(
        rules: &CompiledRuleSet,
        source: String,
        revision: u64,
        raw: Vec<InternalFinding>,
    ) -> Result<Self, IncrementalError> {
        let mut operation = Operation::new(IncrementalLimits::default(), source.len(), 0)?;
        operation.reserve::<Vec<CachedCandidate>>(rules.len())?;
        let mut per_rule = vec![Vec::new(); rules.len()];
        for finding in raw {
            ingest(rules, &source, &mut per_rule, &mut operation, finding)?;
        }
        Self::assemble(
            rules,
            source,
            revision,
            per_rule,
            Vec::new(),
            IncrementalWork::default(),
            &mut operation,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn assemble(
        rules: &CompiledRuleSet,
        source: String,
        revision: u64,
        per_rule: Vec<Vec<CachedCandidate>>,
        spans: Vec<(usize, usize)>,
        work: IncrementalWork,
        operation: &mut Operation,
    ) -> Result<Self, IncrementalError> {
        let span_count = spans.len();
        // Each span already owns a cache ticket. Materialization transfers that
        // identity, but both byte payloads count until the span buffer is dropped.
        operation.reserve::<SensitiveCandidate>(span_count)?;
        let mut structural = Vec::with_capacity(span_count);
        materialize_candidates_with_emit(&source, spans, &mut |candidate| {
            structural.push(candidate);
            Ok(())
        })?;
        operation.release::<(usize, usize)>(span_count)?;
        let count = per_rule
            .iter()
            .flatten()
            .filter(|record| record.confidence.is_some())
            .count();
        operation.reserve::<AcceptedCandidate<'_>>(count)?;
        let accepted = per_rule
            .iter()
            .flatten()
            .filter_map(|record| {
                record.confidence.map(|confidence| AcceptedCandidate {
                    metadata: rules.metadata(record.finding.rule_index()),
                    start: record.finding.start(),
                    end: record.finding.end(),
                    confidence,
                })
            })
            .collect();
        #[cfg(test)]
        operation.observe_gap(0)?;
        let report = finalize_report_accounted(
            &source,
            accepted,
            |budget| {
                budget.results(structural.len())?;
                budget.reserve::<SensitiveCandidate>(structural.len())?;
                Ok(structural.clone())
            },
            operation,
        )?;
        #[cfg(not(test))]
        let _ = work;
        Ok(Self {
            source,
            revision,
            per_rule,
            structural,
            report,
            #[cfg(test)]
            work,
            #[cfg(test)]
            peak_bytes: operation.peak,
            #[cfg(test)]
            peak_cache: operation.peak_cache,
            #[cfg(test)]
            observed_peak: operation.observed_peak,
        })
    }

    fn cache_len(&self) -> Result<usize, IncrementalError> {
        self.per_rule
            .iter()
            .try_fold(self.structural.len(), |total, records| {
                sum(total, records.len())
            })
    }
    fn payload(&self) -> Result<usize, IncrementalError> {
        let mut bytes = sum(
            self.source.len(),
            payload::<Vec<CachedCandidate>>(self.per_rule.len())?,
        )?;
        for records in &self.per_rule {
            bytes = sum(bytes, payload::<CachedCandidate>(records.len())?)?;
        }
        bytes = sum(bytes, payload::<SensitiveCandidate>(self.structural.len())?)?;
        bytes = sum(bytes, payload::<crate::Finding>(self.report.len())?)?;
        sum(
            bytes,
            payload::<SensitiveCandidate>(self.report.candidate_len())?,
        )
    }

    fn update(
        &self,
        rules: &CompiledRuleSet,
        lengths: &[Option<usize>],
        source: String,
        revision: u64,
        edit: EditGeometry,
        operation: &mut Operation,
    ) -> Result<Self, IncrementalError> {
        let invariant = IncrementalError::ExecutionInvariantViolated;
        if self.per_rule.len() != rules.len() || lengths.len() != rules.len() {
            return Err(invariant);
        }
        let mut work = IncrementalWork::default();
        operation.reserve::<Vec<CachedCandidate>>(rules.len())?;
        operation.reserve::<Range<usize>>(rules.len())?;
        let mut per_rule = vec![Vec::new(); rules.len()];
        let mut owners = vec![0..0; rules.len()];
        for (index, records) in self.per_rule.iter().enumerate() {
            let Some(length) = lengths[index] else {
                continue;
            };
            let old_owner = edit.literal_starts(self.source.len(), length, edit.b)?;
            owners[index] = edit.literal_starts(source.len(), length, edit.c)?;
            for record in records {
                let old = record.finding;
                check_emission(
                    rules,
                    &self.source,
                    old.rule_index().get(),
                    old.start()..old.end(),
                )?;
                if old.rule_index().get() != index
                    || old.end().checked_sub(old.start()) != Some(length)
                {
                    return Err(invariant);
                }
                if old_owner.contains(&old.start()) {
                    continue;
                }
                let span = edit.map_span(old.start()..old.end())?;
                check_emission(rules, &source, index, span.clone())?;
                let finding = InternalFinding::new(old.rule_index(), span.start, span.end);
                let requirement = rules
                    .metadata(old.rule_index())
                    .validator()
                    .streaming_requirement();
                operation.cache_records(1)?;
                operation.reserve::<CachedCandidate>(1)?;
                let confidence =
                    if context_unchanged(&self.source, &source, old, finding, edit, requirement)? {
                        work.record(WorkMetric::ReusedRecords, 1).ok_or(invariant)?;
                        record.confidence
                    } else {
                        work.record(WorkMetric::RevalidatedCandidates, 1)
                            .ok_or(invariant)?;
                        validate_raw_candidate(rules, &source, finding)
                            .map(|candidate| candidate.confidence)
                    };
                per_rule[index].push(CachedCandidate {
                    finding,
                    confidence,
                });
            }
        }
        let mut regenerated = 0;
        let mut emit = |finding| {
            ingest(rules, &source, &mut per_rule, operation, finding)?;
            regenerated = sum(regenerated, 1)?;
            Ok(())
        };
        let nonempty = owners.iter().filter(|range| !range.is_empty());
        if let Some(left) = nonempty.clone().map(|range| range.start).min() {
            let right_start = nonempty.map(|range| range.end).max().ok_or(invariant)?;
            let max_length = lengths.iter().flatten().copied().max().ok_or(invariant)?;
            // Include every complete owned needle, not just its start. This byte
            // bound may fall inside a scalar; Input never slices a str.
            let right = right_start
                .checked_add(
                    (max_length - 1).min(source.len().checked_sub(right_start).ok_or(invariant)?),
                )
                .ok_or(invariant)?;
            rules.scan_incremental_literals(&source, left..right, &owners, &mut emit, &mut work)?;
        }
        rules.scan_incremental_fallback(&source, &mut emit, &mut work)?;
        work.record(WorkMetric::RegeneratedCandidates, regenerated)
            .ok_or(invariant)?;
        drop(owners);
        operation.release::<Range<usize>>(rules.len())?;
        // Within a literal rule length is fixed, so start order is also the
        // original overlapping enumeration order. Never deduplicate records.
        for (index, records) in per_rule.iter_mut().enumerate() {
            if lengths[index].is_some() {
                records
                    .sort_unstable_by_key(|record| (record.finding.start(), record.finding.end()));
            }
        }
        let old_structural = edit.structural_starts(self.source.len(), edit.b)?;
        let new_structural = edit.structural_starts(source.len(), edit.c)?;
        let mut spans = Vec::new();
        for candidate in &self.structural {
            let location = candidate.location();
            let span = location.start()..location.end();
            if self.source.get(span.clone()).is_none()
                || span.end.checked_sub(span.start) != Some(RECOVERY_LIKE_LEN)
            {
                return Err(invariant);
            }
            if !old_structural.contains(&span.start) {
                let mapped = edit.map_span(span)?;
                if source.get(mapped.clone()).is_none() {
                    return Err(invariant);
                }
                work.record(WorkMetric::ReusedRecords, 1).ok_or(invariant)?;
                stage_span(operation, &mut spans, mapped.start, mapped.end)?;
            }
        }
        detect_owned_spans_with_emit(
            &source,
            new_structural,
            Some(&mut work),
            &mut |start, end| stage_span(operation, &mut spans, start, end),
        )?;
        spans.sort_unstable();
        if spans.windows(2).any(|pair| pair[0].0 >= pair[1].0) {
            return Err(invariant);
        }
        Self::assemble(rules, source, revision, per_rule, spans, work, operation)
    }
}

fn ingest(
    rules: &CompiledRuleSet,
    source: &str,
    per_rule: &mut [Vec<CachedCandidate>],
    operation: &mut Operation,
    finding: InternalFinding,
) -> Result<(), IncrementalError> {
    operation.raw()?;
    check_emission(
        rules,
        source,
        finding.rule_index().get(),
        finding.start()..finding.end(),
    )?;
    operation.cache_records(1)?;
    operation.reserve::<CachedCandidate>(1)?;
    let confidence =
        validate_raw_candidate(rules, source, finding).map(|candidate| candidate.confidence);
    per_rule[finding.rule_index().get()].push(CachedCandidate {
        finding,
        confidence,
    });
    Ok(())
}
fn stage_span(
    operation: &mut Operation,
    spans: &mut Vec<(usize, usize)>,
    start: usize,
    end: usize,
) -> Result<(), IncrementalError> {
    operation.cache_records(1)?;
    operation.reserve::<(usize, usize)>(1)?;
    spans.push((start, end));
    Ok(())
}

/// A revisioned UTF-8 document with immutable scanner configuration.
/// No report or cache borrow can outlive a mutable edit of this session.
/// Every successful edit publishes a complete report equivalent to whole-source
/// scanning. Literal/structural matching is localized; other matchers replay.
/// Recoverable input errors preserve activity. Resource/internal failures make
/// edits terminal while the previous complete source/report remain readable.
/// Allocation failure and panic recovery are outside this contract.
pub struct IncrementalSession {
    limits: IncrementalLimits,
    rules: Arc<CompiledRuleSet>,
    lifecycle: Lifecycle,
    literal_lengths: Vec<Option<usize>>,
    current: Snapshot,
}

impl std::fmt::Debug for IncrementalSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IncrementalSession")
            .field("active", &self.is_active())
            .field("revision", &self.revision())
            .field("source_bytes", &self.source().len())
            .finish_non_exhaustive()
    }
}

impl IncrementalSession {
    pub(crate) fn new(
        rules: Arc<CompiledRuleSet>,
        source: String,
    ) -> Result<Self, IncrementalError> {
        Self::new_with_limits(rules, source, IncrementalLimits::default())
    }
    pub(crate) fn new_with_limits(
        rules: Arc<CompiledRuleSet>,
        source: String,
        limits: IncrementalLimits,
    ) -> Result<Self, IncrementalError> {
        let mut operation = Operation::new(limits, source.len(), 0)?;
        operation.reserve::<Option<usize>>(rules.len())?;
        let literal_lengths = rules.incremental_literal_lengths();
        let current = Snapshot::replay(&rules, source, 0, &mut operation)?;
        Ok(Self {
            limits,
            rules,
            lifecycle: Lifecycle::Active,
            literal_lengths,
            current,
        })
    }

    /// Whether further edits can be attempted.
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.lifecycle == Lifecycle::Active
    }

    /// Borrows the last committed UTF-8 document, including after terminal failure.
    #[must_use]
    pub fn source(&self) -> &str {
        &self.current.source
    }

    /// Returns the last committed revision; the initial revision is zero.
    #[must_use]
    pub fn revision(&self) -> u64 {
        self.current.revision
    }

    /// Borrows the last complete report; no partial staged report is exposed.
    #[must_use]
    pub fn report(&self) -> &ScanReport {
        &self.current.report
    }

    /// Replaces a half-open byte range transactionally. Every successful edit,
    /// including an unchanged replacement, increments the revision once.
    /// Invalid inputs leave the session active; resource or invariant failure
    /// is terminal but leaves the previous complete snapshot readable. Allocation
    /// failure and panic recovery are outside this contract.
    pub fn apply_edit(
        &mut self,
        expected_revision: u64,
        range: Range<usize>,
        replacement: &str,
    ) -> Result<u64, IncrementalError> {
        if self.lifecycle != Lifecycle::Active {
            return Err(IncrementalError::NotActive);
        }
        if expected_revision != self.revision() {
            return Err(IncrementalError::StaleRevision {
                expected: expected_revision,
                actual: self.revision(),
            });
        }
        if range.start > range.end || range.end > self.source().len() {
            return Err(IncrementalError::InvalidRange);
        }
        if !self.source().is_char_boundary(range.start)
            || !self.source().is_char_boundary(range.end)
        {
            return Err(IncrementalError::InvalidUtf8Boundary);
        }
        let revision = self
            .revision()
            .checked_add(1)
            .ok_or(IncrementalError::RevisionOverflow)?;
        let length = replacement_length(
            self.source().len(),
            range.end - range.start,
            replacement.len(),
        )?;
        let edit = EditGeometry {
            a: range.start,
            b: range.end,
            c: range
                .start
                .checked_add(replacement.len())
                .ok_or(IncrementalError::DocumentLengthOverflow)?,
        };
        let admission = (|| {
            let baseline = sum(
                self.current.payload()?,
                payload::<Option<usize>>(self.literal_lengths.len())?,
            )?;
            Operation::new(self.limits, baseline, self.current.cache_len()?)
        })();
        let mut operation = match admission {
            Ok(operation) => operation,
            Err(error) => return self.commit_replay(Err(error)),
        };
        let staged = (|| {
            operation.bytes(length)?;
            let mut source = String::with_capacity(length);
            source.push_str(&self.source()[..range.start]);
            source.push_str(replacement);
            source.push_str(&self.source()[range.end..]);
            self.current.update(
                &self.rules,
                &self.literal_lengths,
                source,
                revision,
                edit,
                &mut operation,
            )
        })();
        // The old snapshot drops inside commit; all old/staged reservations
        // remain live until that release (or staged cleanup after failure).
        let result = self.commit_replay(staged);
        debug_assert!(operation.bytes <= operation.peak);
        result
    }

    fn commit_replay(
        &mut self,
        staged: Result<Snapshot, IncrementalError>,
    ) -> Result<u64, IncrementalError> {
        match staged {
            Ok(snapshot) => {
                // Publish only after matching, validation and finalization succeed;
                // failures must leave the last complete revision readable.
                self.current = snapshot;
                Ok(self.revision())
            }
            Err(error) => {
                self.lifecycle = Lifecycle::Failed;
                Err(error)
            }
        }
    }
}

/// Old [a,b) becomes new [a,c). Prefix endpoints do not move; suffix
/// endpoints use c + (offset-b), checked without signed displacement casts.
#[derive(Copy, Clone)]
struct EditGeometry {
    a: usize,
    b: usize,
    c: usize,
}

impl EditGeometry {
    fn is_empty(self) -> bool {
        self.a == self.b && self.a == self.c
    }

    fn literal_starts(
        self,
        source_len: usize,
        length: usize,
        end: usize,
    ) -> Result<Range<usize>, IncrementalError> {
        if length == 0 {
            return Err(IncrementalError::ExecutionInvariantViolated);
        }
        if self.is_empty() || source_len < length {
            return Ok(0..0);
        }
        // Insertion: old upper=a, new upper=c. Deletion: old upper=b,
        // new upper=a. Replacement: old upper=b, new upper=c. Intersect
        // with complete-match start domain [0,N-L+1). L=1 has no halo.
        // Crossing starts need L-1 bytes of left halo; new starts inside [a,c)
        // are also owned. Old suffix starts at b survive and shift to c, even
        // for L=1 or insertion into an empty document.
        // Saturation here implements max(0,a-L+1), not offset shifting.
        let lo = self.a.saturating_sub(length - 1);
        let hi = end.min(source_len - length + 1);
        Ok(if lo < hi { lo..hi } else { 0..0 })
    }

    fn structural_starts(
        self,
        source_len: usize,
        end: usize,
    ) -> Result<Range<usize>, IncrementalError> {
        if self.is_empty() || source_len < RECOVERY_LIKE_LEN {
            return Ok(0..0);
        }
        // Unlike a literal, a structural token depends on adjacent boundary
        // bytes: include the token ending at a and the start exactly at end.
        let domain_end = source_len - RECOVERY_LIKE_LEN + 1;
        let lo = self.a.saturating_sub(RECOVERY_LIKE_LEN);
        // Clipping before addition prevents overflow at the representable EOF.
        let hi = if end >= domain_end {
            domain_end
        } else {
            end.checked_add(1)
                .ok_or(IncrementalError::ExecutionInvariantViolated)?
        };
        Ok(if lo < hi { lo..hi } else { 0..0 })
    }

    fn map_suffix(self, offset: usize) -> Result<usize, IncrementalError> {
        offset
            .checked_sub(self.b)
            .and_then(|relative| self.c.checked_add(relative))
            .ok_or(IncrementalError::ExecutionInvariantViolated)
    }

    fn map_span(self, span: Range<usize>) -> Result<Range<usize>, IncrementalError> {
        if span.start > span.end {
            return Err(IncrementalError::ExecutionInvariantViolated);
        }
        if self.is_empty() || span.end <= self.a {
            Ok(span)
        } else if span.start >= self.b {
            Ok(self.map_suffix(span.start)?..self.map_suffix(span.end)?)
        } else {
            Err(IncrementalError::ExecutionInvariantViolated)
        }
    }
}

fn dependency_envelope(
    source: &str,
    finding: InternalFinding,
    before: usize,
    after: usize,
) -> Range<usize> {
    let context = ValidationContext::new(source, finding.start()..finding.end());
    finding.start() - context.before_window(before).len()
        ..finding.end() + context.after_window(after).len()
}

fn context_unchanged(
    old_source: &str,
    new_source: &str,
    old: InternalFinding,
    new: InternalFinding,
    edit: EditGeometry,
    requirement: ValidatorStreamingRequirement,
) -> Result<bool, IncrementalError> {
    let ValidatorStreamingRequirement::BoundedContext {
        before_bytes,
        after_bytes,
    } = requirement
    else {
        return Ok(true);
    };
    // Equal candidate bytes alone do not preserve contextual decisions. Clipped
    // envelopes must map exactly, including source-edge and UTF-8 clipping changes.
    let old_envelope = dependency_envelope(old_source, old, before_bytes, after_bytes);
    let new_envelope = dependency_envelope(new_source, new, before_bytes, after_bytes);
    if edit.is_empty() {
        return Ok(old_envelope == new_envelope);
    }
    if old.end() <= edit.a {
        Ok(old_envelope.end <= edit.a && new_envelope == old_envelope)
    } else if old.start() >= edit.b && old_envelope.start >= edit.b {
        Ok(new_envelope
            == (edit.map_suffix(old_envelope.start)?..edit.map_suffix(old_envelope.end)?))
    } else {
        Ok(false)
    }
}

fn check_emission(
    rules: &CompiledRuleSet,
    source: &str,
    rule_index: usize,
    range: Range<usize>,
) -> Result<(), IncrementalError> {
    if rule_index >= rules.len() || source.get(range).is_none() {
        return Err(IncrementalError::ExecutionInvariantViolated);
    }
    Ok(())
}

fn replacement_length(
    current: usize,
    removed: usize,
    inserted: usize,
) -> Result<usize, IncrementalError> {
    current
        .checked_sub(removed)
        .and_then(|remaining| remaining.checked_add(inserted))
        .filter(|length| *length <= isize::MAX as usize)
        .ok_or(IncrementalError::DocumentLengthOverflow)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Rule, Scanner, Severity, builtins, validators::dispatch::ValidatorKind};

    fn scanner(rules: Vec<Rule>) -> Scanner {
        Scanner::builder().rules(rules).build().unwrap()
    }

    fn parity(scanner: &Scanner, session: &IncrementalSession) {
        let results = scanner.scan([((), session.source())]);
        assert_eq!(session.report(), results.single_report().unwrap());
        // Check the entire pre-normalization cache, including hidden/rejected
        // records, against authoritative replay, not just the public report.
        let replay = Snapshot::replay(
            &session.rules,
            session.source().to_owned(),
            session.revision(),
            &mut Operation::new(IncrementalLimits::default(), session.source().len(), 0).unwrap(),
        )
        .unwrap();
        let records = |snapshot: &Snapshot| {
            snapshot
                .per_rule
                .iter()
                .map(|records| {
                    records
                        .iter()
                        .map(|record| (record.finding, record.confidence))
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(records(&session.current), records(&replay));
        assert_eq!(session.current.structural, replay.structural);
    }

    fn edit(scanner: &Scanner, session: &mut IncrementalSession, range: Range<usize>, text: &str) {
        let previous = session.revision();
        assert_eq!(session.apply_edit(previous, range, text), Ok(previous + 1));
        parity(scanner, session);
    }

    #[derive(Debug, Eq, PartialEq)]
    struct Observation {
        source: String,
        revision: u64,
        cache: Vec<(usize, usize, usize, Option<Confidence>)>,
        structural: Vec<SensitiveCandidate>,
        report: ScanReport,
    }

    fn observe(session: &IncrementalSession) -> Observation {
        Observation {
            source: session.source().to_owned(),
            revision: session.revision(),
            cache: session
                .current
                .per_rule
                .iter()
                .flatten()
                .map(|record| {
                    (
                        record.finding.rule_index().get(),
                        record.finding.start(),
                        record.finding.end(),
                        record.confidence,
                    )
                })
                .collect(),
            structural: session.current.structural.clone(),
            report: session.report().clone(),
        }
    }

    #[test]
    fn replacements_cover_empty_bof_eof_unicode_and_line_endings() {
        let scanner = scanner(vec![Rule::literal("literal", "SEC", Severity::High)]);
        let mut session = scanner.incremental_session(String::new()).unwrap();
        assert_eq!(session.revision(), 0);
        parity(&scanner, &session);
        edit(&scanner, &mut session, 0..0, "é\rSEC\n🦀\r\n");
        edit(&scanner, &mut session, 0..2, "€");
        let end = session.source().len();
        edit(&scanner, &mut session, end..end, "SEC");
        edit(&scanner, &mut session, 0..0, "\n");
        let start = session.source().find('🦀').unwrap();
        edit(&scanner, &mut session, start..start + 4, "α\nβ");
        let end = session.source().len();
        edit(&scanner, &mut session, 0..end, "");
        edit(&scanner, &mut session, 0..0, "");
    }

    #[test]
    fn cache_preserves_overlapping_literals_and_equal_priority_candidates() {
        let scanner = scanner(vec![
            Rule::literal("first", "aa", Severity::High),
            Rule::literal("second", "aa", Severity::Low),
        ]);
        let mut session = scanner.incremental_session("aaaa".into()).unwrap();
        assert_eq!(session.current.per_rule[0].len(), 3);
        assert_eq!(session.current.per_rule[1].len(), 3);
        assert_eq!(session.report().len(), 6);
        parity(&scanner, &session);
        edit(&scanner, &mut session, 0..0, "a");
        assert_eq!(session.report().len(), 8);
        edit(&scanner, &mut session, 1..4, "b");
    }

    #[test]
    fn rejected_and_lower_priority_records_survive_and_reappear() {
        let value = "CorrectHorseBatteryStaple";
        let scanner = scanner(vec![
            Rule::literal("fallback", value, Severity::Low),
            Rule::literal("contextual", value, Severity::High)
                .with_validator(ValidatorKind::Password),
        ]);
        let mut session = scanner
            .incremental_session(format!("label={value}"))
            .unwrap();
        assert_eq!(session.current.per_rule[1][0].confidence, None);
        assert_eq!(
            session.report().findings()[0].rule_id().as_str(),
            "fallback"
        );
        assert_eq!(
            session.current.per_rule[0][0].confidence,
            Some(Confidence::High)
        );
        edit(&scanner, &mut session, 0..5, "password");
        assert_eq!(session.report().len(), 1);
        assert_eq!(
            session.report().findings()[0].rule_id().as_str(),
            "contextual"
        );
        assert_eq!(session.current.per_rule[0].len(), 1);
        assert_eq!(
            session.current.per_rule[1][0].confidence,
            Some(Confidence::Medium)
        );
        edit(&scanner, &mut session, 0..8, "label");
        assert_eq!(
            session.report().findings()[0].rule_id().as_str(),
            "fallback"
        );
    }

    #[test]
    fn suppressed_structural_cache_reappears_after_context_edit() {
        let code = "ABCD-EFGH-IJKL-MNOP";
        let scanner = scanner(vec![
            Rule::literal("password", code, Severity::High).with_validator(ValidatorKind::Password),
        ]);
        let mut session = scanner
            .incremental_session(format!("password={code}"))
            .unwrap();
        assert_eq!(session.current.structural.len(), 1);
        assert_eq!(session.report().candidate_len(), 0);
        assert_eq!(session.report().len(), 1);
        edit(&scanner, &mut session, 0..8, "ordinary");
        assert_eq!(session.report().len(), 0);
        assert_eq!(session.report().candidate_len(), 1);
    }

    #[test]
    fn candidate_only_validation_rebuilds_acceptance_and_rejection() {
        let scanner = scanner(vec![
            Rule::pattern("pesel", r"\d{11}", Severity::High)
                .unwrap()
                .with_validator(ValidatorKind::Pesel),
        ]);
        let mut session = scanner
            .incremental_session("44051401458 44051401459".into())
            .unwrap();
        assert_eq!(session.current.per_rule[0].len(), 2);
        assert_eq!(
            session.current.per_rule[0][0].confidence,
            Some(Confidence::High)
        );
        assert_eq!(session.current.per_rule[0][1].confidence, None);
        parity(&scanner, &session);
        edit(&scanner, &mut session, 22..23, "8");
        assert_eq!(session.report().len(), 2);
    }

    #[test]
    fn after_context_validation_observes_new_eof_and_closing_markup() {
        let value = "CorrectHorseBatteryStaple";
        let scanner = scanner(vec![
            Rule::literal("maven", value, Severity::High).with_validator(ValidatorKind::Maven),
        ]);
        let mut session = scanner
            .incremental_session(format!("<settings><servers><server><password>{value}"))
            .unwrap();
        assert_eq!(session.current.per_rule[0][0].confidence, None);
        let end = session.source().len();
        edit(
            &scanner,
            &mut session,
            end..end,
            "</password></server></servers></settings>",
        );
        assert_eq!(
            session.current.per_rule[0][0].confidence,
            Some(Confidence::High)
        );
        assert_eq!(session.report().len(), 1);
        let end = session.source().len();
        edit(
            &scanner,
            &mut session,
            end - "</servers></settings>".len()..end,
            "",
        );
        assert_eq!(session.report().len(), 0);
    }

    #[test]
    fn prefixes_suffixes_and_regex_enumeration_use_whole_source_semantics() {
        let scanner = scanner(vec![
            Rule::prefix("prefix", "pre", Severity::High),
            Rule::suffix("suffix", "!", Severity::Low),
            Rule::suffix("self-overlap", "!!", Severity::Low),
            Rule::pattern("bounded", "aa", Severity::High).unwrap(),
            Rule::pattern("unbounded", r"x[^\n]+", Severity::High).unwrap(),
            Rule::pattern("crlf", r"(?mR)^END$", Severity::High).unwrap(),
            Rule::pattern("absolute", r"\Apre", Severity::High).unwrap(),
        ]);
        let mut session = scanner
            .incremental_session("preTOKEN a! !!!!! aaaaaa xlong\r\nEND\r\n".into())
            .unwrap();
        parity(&scanner, &session);
        edit(&scanner, &mut session, 0..0, "a");
        edit(&scanner, &mut session, 1..1, " ");
        let end = session.source().len();
        edit(&scanner, &mut session, end..end, "pre");
    }

    #[test]
    fn projected_empty_and_absent_captures_are_preserved() {
        let scanner = scanner(vec![
            Rule::captured_pattern(
                "projected",
                r"key=(?P<value>[^;]+);",
                "value",
                Severity::High,
            )
            .unwrap(),
            Rule::captured_pattern("empty", r"a(?P<value>)b", "value", Severity::High).unwrap(),
            Rule::captured_pattern("absent", r"(?:x(?P<value>y)|z)", "value", Severity::Low)
                .unwrap(),
        ]);
        let mut session = scanner
            .incremental_session("é\r\nkey=🦀; ab z".into())
            .unwrap();
        assert_eq!(
            session.current.per_rule[1][0].finding.start(),
            session.current.per_rule[1][0].finding.end()
        );
        assert!(session.current.per_rule[2].is_empty());
        parity(&scanner, &session);
        edit(&scanner, &mut session, 0..0, "\r\n");
        let end = session.source().len();
        edit(&scanner, &mut session, end - 1..end, "xy");
        assert_eq!(session.current.per_rule[2].len(), 1);
    }

    #[test]
    fn per_rule_prefilter_duplicates_remain_observable() {
        let value = "CorrectHorseBatteryStaple";
        let scanner = scanner(vec![
            Rule::captured_pattern(
                "generic.password-field",
                r"(?:password|pwd)[^=]*=(?P<value>[^\s]+)",
                "value",
                Severity::High,
            )
            .unwrap()
            .with_validator(ValidatorKind::Password),
        ]);
        let mut session = scanner
            .incremental_session(format!("password pwd={value}"))
            .unwrap();
        assert_eq!(session.current.per_rule[0].len(), 2);
        assert_eq!(session.report().len(), 2);
        parity(&scanner, &session);
        edit(&scanner, &mut session, 0..9, "");
        assert_eq!(session.report().len(), 1);
        edit(&scanner, &mut session, 0..0, "password ");
        assert_eq!(session.report().len(), 2);
    }

    #[test]
    fn recoverable_errors_preserve_entire_snapshot_and_error_precedence() {
        let scanner = scanner(vec![Rule::literal("literal", "SECRET", Severity::High)]);
        let mut session = scanner.incremental_session("éSECRET".into()).unwrap();
        let before = observe(&session);
        let invalid = [
            (
                1,
                0..0,
                IncrementalError::StaleRevision {
                    expected: 1,
                    actual: 0,
                },
            ),
            (
                0,
                Range { start: 3, end: 1 },
                IncrementalError::InvalidRange,
            ),
            (0, 0..usize::MAX, IncrementalError::InvalidRange),
            (0, 1..1, IncrementalError::InvalidUtf8Boundary),
            (0, 0..1, IncrementalError::InvalidUtf8Boundary),
        ];
        for (revision, range, error) in invalid {
            assert_eq!(
                session.apply_edit(revision, range, "replacement"),
                Err(error)
            );
            assert_eq!(observe(&session), before);
            assert_eq!(session.lifecycle, Lifecycle::Active);
        }
        edit(&scanner, &mut session, 0..2, "🦀");
        let before = observe(&session);
        assert_eq!(
            session.apply_edit(0, 0..0, ""),
            Err(IncrementalError::StaleRevision {
                expected: 0,
                actual: 1
            })
        );
        assert_eq!(observe(&session), before);
    }

    #[test]
    fn arithmetic_overflow_is_checked_without_large_allocations() {
        assert_eq!(
            replacement_length(isize::MAX as usize, 1, 1),
            Ok(isize::MAX as usize)
        );
        assert_eq!(
            replacement_length(usize::MAX, 1, 1),
            Err(IncrementalError::DocumentLengthOverflow)
        );
        assert_eq!(
            replacement_length(usize::MAX, 0, 1),
            Err(IncrementalError::DocumentLengthOverflow)
        );
        let scanner = scanner(vec![]);
        let mut session = scanner.incremental_session("é".into()).unwrap();
        session.current.revision = u64::MAX;
        let before = observe(&session);
        assert_eq!(
            session.apply_edit(u64::MAX, 0..0, "a"),
            Err(IncrementalError::RevisionOverflow)
        );
        assert_eq!(observe(&session), before);
        assert_eq!(session.lifecycle, Lifecycle::Active);
        assert_eq!(
            session.apply_edit(u64::MAX, 1..1, "a"),
            Err(IncrementalError::InvalidUtf8Boundary)
        );
    }

    #[test]
    fn malformed_replay_emissions_fail_before_validation_and_are_terminal() {
        let rules = Arc::new(
            CompiledRuleSet::compile(vec![Rule::literal("literal", "SECRET", Severity::High)])
                .unwrap(),
        );
        let mut session = IncrementalSession::new(Arc::clone(&rules), "éSECRET".into()).unwrap();
        let before = observe(&session);
        for (index, range) in [
            (1, 0..0),
            (0, 1..2),
            (0, 9..10),
            (0, Range { start: 3, end: 2 }),
        ] {
            assert_eq!(
                check_emission(&rules, "éSECRET", index, range),
                Err(IncrementalError::ExecutionInvariantViolated)
            );
        }
        // Use real matcher emissions with synthetic inconsistent source history.
        // The first emission validates; the second is outside the staged source.
        let mut raw = Vec::new();
        rules.scan("SECRET SECRET", &mut raw);
        assert_eq!(raw.len(), 2);
        let staged = Snapshot::from_emissions(&rules, "SECRET é".into(), 1, raw);
        assert_eq!(
            session.commit_replay(staged),
            Err(IncrementalError::ExecutionInvariantViolated)
        );
        assert_eq!(observe(&session), before);
        assert_eq!(session.lifecycle, Lifecycle::Failed);
        assert_eq!(
            session.apply_edit(0, 0..0, "valid"),
            Err(IncrementalError::NotActive)
        );
        assert_eq!(observe(&session), before);
    }

    #[test]
    fn full_portfolios_and_mixed_validators_have_report_parity_after_every_edit() {
        let combined: Vec<_> = builtins::CURRENT
            .iter()
            .chain(builtins::personal::CURRENT)
            .chain(builtins::financial::CURRENT)
            .copied()
            .collect();
        for portfolio in [
            builtins::CURRENT,
            builtins::personal::CURRENT,
            builtins::financial::CURRENT,
            combined.as_slice(),
        ] {
            let scanner = Scanner::builder().builtins(portfolio).build().unwrap();
            let mut session = scanner
                .incremental_session(
                    "password=CorrectHorseBatteryStaple\nABCD-EFGH-IJKL-MNOP\npesel=44051401458\n"
                        .into(),
                )
                .unwrap();
            parity(&scanner, &session);
            edit(&scanner, &mut session, 0..0, "é🦀\r\n");
            let start = session.source().find("password").unwrap();
            edit(&scanner, &mut session, start..start + 8, "ordinary");
            let end = session.source().len();
            edit(
                &scanner,
                &mut session,
                end..end,
                "iban=GB82 WEST 1234 5698 7654 32\r",
            );
            let end = session.source().len();
            edit(&scanner, &mut session, 0..end, "");
        }
    }

    #[test]
    fn literal_ownership_intervals_cover_edit_edges_and_representable_limits() {
        for (old_len, new_len, a, b, c, length, old_expected, new_expected) in [
            (0, 1, 0, 0, 1, 1, 0..0, 0..1),
            (1, 2, 0, 0, 1, 1, 0..0, 0..1),
            (3, 2, 1, 2, 1, 1, 1..2, 0..0),
            (3, 2, 1, 2, 1, 2, 0..2, 0..1),
            (4, 5, 2, 2, 3, 2, 1..2, 1..3),
            (5, 4, 2, 3, 2, 2, 1..3, 1..2),
            (3, 3, 1, 2, 2, 1, 1..2, 1..2),
            (3, 3, 1, 1, 1, 2, 0..0, 0..0),
            (1, 2, 1, 1, 2, 2, 0..0, 0..1),
            (usize::MAX, usize::MAX, 0, 1, 1, 1, 0..1, 0..1),
        ] {
            let geometry = EditGeometry { a, b, c };
            assert_eq!(
                geometry.literal_starts(old_len, length, b).unwrap(),
                old_expected
            );
            assert_eq!(
                geometry.literal_starts(new_len, length, c).unwrap(),
                new_expected
            );
        }
        let geometry = EditGeometry {
            a: usize::MAX,
            b: usize::MAX,
            c: usize::MAX,
        };
        assert_eq!(geometry.map_span(0..usize::MAX), Ok(0..usize::MAX));
        assert_eq!(geometry.structural_starts(usize::MAX, usize::MAX), Ok(0..0));
        assert_eq!(
            EditGeometry {
                a: 0,
                b: 1,
                c: usize::MAX
            }
            .map_suffix(2),
            Err(IncrementalError::ExecutionInvariantViolated)
        );
    }

    #[test]
    fn exhaustive_literal_edits_preserve_overlaps_duplicates_and_cache_ownership() {
        let needles = [
            "a", "b", "aa", "aaa", "aba", "ab", "ba", "aa", "é", "🦀", "éa",
        ];
        let scanner = scanner(
            needles
                .iter()
                .enumerate()
                .map(|(i, needle)| Rule::literal(format!("literal-{i}"), *needle, Severity::High))
                .collect(),
        );
        let mut sources = vec![String::new(), "éa".into(), "🦀éa".into(), "aéaba".into()];
        for length in 1..=6 {
            for bits in 0..1usize << length {
                sources.push(
                    (0..length)
                        .map(|i| if bits & (1 << i) == 0 { 'a' } else { 'b' })
                        .collect(),
                );
            }
        }
        let mut checks = 0;
        for old in sources {
            let boundaries: Vec<_> = (0..=old.len())
                .filter(|&p| old.is_char_boundary(p))
                .collect();
            for &a in &boundaries {
                for &b in boundaries.iter().filter(|&&b| b >= a) {
                    for replacement in ["", "a", "b", "aa", "ab", "é", "🦀"] {
                        let mut session = scanner.incremental_session(old.clone()).unwrap();
                        edit(&scanner, &mut session, a..b, replacement);
                        checks += 1;
                    }
                }
            }
        }
        assert_eq!(checks, 19_964);
    }

    #[test]
    fn literal_stats_measure_actual_local_work_and_exclude_prefix_lengths() {
        let scanner = scanner(vec![Rule::literal("literal", "x", Severity::High)]);
        let source = format!("x{}x", "a".repeat(10_000));
        let mut session = scanner.incremental_session(source).unwrap();
        edit(&scanner, &mut session, 5_000..5_001, "b");
        let work = &session.current.work;
        assert_eq!(work.get(WorkMetric::LiteralBytes), 1);
        assert_eq!(work.get(WorkMetric::WholeRuleFallbacks), 0);
        assert_eq!(work.get(WorkMetric::ReusedRecords), 2);
        assert_eq!(work.get(WorkMetric::RegeneratedCandidates), 0);
        edit(&scanner, &mut session, 0..0, "x");
        assert_eq!(session.current.work.get(WorkMetric::ReusedRecords), 2);
        assert_eq!(
            session.current.work.get(WorkMetric::RegeneratedCandidates),
            1
        );
        edit(&scanner, &mut session, 1..1, "");
        assert_eq!(session.current.work.get(WorkMetric::LiteralBytes), 0);
        assert_eq!(session.current.work.get(WorkMetric::ReusedRecords), 3);

        let mixed = self::scanner(vec![
            Rule::literal("literal", "x", Severity::High),
            Rule::prefix("prefix", "p".repeat(4096), Severity::High),
            Rule::suffix("suffix", "!", Severity::High),
            Rule::pattern("regex", "x", Severity::High).unwrap(),
        ]);
        let mut session = mixed.incremental_session("x ".into()).unwrap();
        edit(&mixed, &mut session, 1..1, "x");
        let work = &session.current.work;
        assert_eq!(work.get(WorkMetric::LiteralBytes), 1);
        assert_eq!(work.get(WorkMetric::PrefixPasses), 1);
        assert_eq!(work.get(WorkMetric::PrefixBytes), session.source().len());
        assert_eq!(work.get(WorkMetric::WholeRuleFallbacks), 3);
        assert_eq!(work.get(WorkMetric::RegeneratedCandidates), 3);
    }

    #[test]
    fn contextual_reuse_checks_both_effective_windows_and_utf8_clipping() {
        let value = "CorrectHorseBatteryStaple";
        let scanner = scanner(vec![
            Rule::literal("password", value, Severity::High)
                .with_validator(ValidatorKind::Password),
        ]);
        let mut session = scanner
            .incremental_session(format!(
                "é{}password={value}{}",
                " ".repeat(300),
                " ".repeat(300)
            ))
            .unwrap();
        edit(&scanner, &mut session, 0..2, "🦀");
        assert_eq!(session.current.work.get(WorkMetric::ReusedRecords), 1);
        assert_eq!(
            session.current.work.get(WorkMetric::RevalidatedCandidates),
            0
        );
        let key = session.source().find("password").unwrap();
        edit(&scanner, &mut session, key..key + 8, "ordinary");
        assert_eq!(
            session.current.work.get(WorkMetric::RevalidatedCandidates),
            1
        );
        assert_eq!(session.current.per_rule[0][0].confidence, None);
        let end = session.source().len();
        edit(&scanner, &mut session, end..end, "é");
        assert_eq!(session.current.work.get(WorkMetric::ReusedRecords), 1);

        // CandidateOnly rejection is cached just like acceptance.
        let candidate_only = self::scanner(vec![
            Rule::literal("pesel", "44051401459", Severity::High)
                .with_validator(ValidatorKind::Pesel),
        ]);
        let mut session = candidate_only
            .incremental_session("44051401459".into())
            .unwrap();
        edit(&candidate_only, &mut session, 0..0, "é");
        assert_eq!(session.current.work.get(WorkMetric::ReusedRecords), 1);
        assert_eq!(session.current.per_rule[0][0].confidence, None);

        // The unchanged suffix bytes alone do not imply unchanged clipped context.
        let rules =
            CompiledRuleSet::compile(vec![Rule::literal("literal", "x", Severity::High)]).unwrap();
        let mut raw = Vec::new();
        rules.scan("éx", &mut raw);
        let old = raw[0];
        let new = InternalFinding::new(old.rule_index(), 4, 5);
        assert!(
            !context_unchanged(
                "éx",
                "🦀x",
                old,
                new,
                EditGeometry { a: 0, b: 2, c: 4 },
                ValidatorStreamingRequirement::BoundedContext {
                    before_bytes: 3,
                    after_bytes: 0
                }
            )
            .unwrap()
        );
        let new = InternalFinding::new(old.rule_index(), 2, 3);
        assert!(
            !context_unchanged(
                "éx",
                "éxy",
                old,
                new,
                EditGeometry { a: 3, b: 3, c: 4 },
                ValidatorStreamingRequirement::BoundedContext {
                    before_bytes: 0,
                    after_bytes: 3
                }
            )
            .unwrap()
        );
    }

    #[test]
    fn structural_localization_covers_neighbors_tokens_and_utf8() {
        let scanner = scanner(vec![]);
        let code = "ABCD-EFGH-IJKL-MNOP";
        for old in [
            code.to_owned(),
            format!(" {code} "),
            format!("é{code}🦀 {code}"),
            format!("x{code}-"),
        ] {
            let boundaries: Vec<_> = (0..=old.len())
                .filter(|&p| old.is_char_boundary(p))
                .collect();
            for &a in &boundaries {
                for b in [a, (a + 1).min(old.len()), old.len()] {
                    if !old.is_char_boundary(b) {
                        continue;
                    }
                    for replacement in ["", "X", "-", "_", "é", code] {
                        let mut session = scanner.incremental_session(old.clone()).unwrap();
                        edit(&scanner, &mut session, a..b, replacement);
                    }
                }
            }
        }
        let mut session = scanner
            .incremental_session(format!("{code}{} {code}", " ".repeat(10_000)))
            .unwrap();
        edit(&scanner, &mut session, 5_000..5_001, "_");
        assert_eq!(session.current.work.get(WorkMetric::ReusedRecords), 2);
        assert_eq!(
            session.current.work.get(WorkMetric::StructuralPredicates),
            0
        );
        assert!(session.current.work.get(WorkMetric::StructuralBytes) < 60);
    }

    #[test]
    fn selective_invariant_failure_is_terminal_and_does_not_commit_staged_state() {
        let scanner = scanner(vec![Rule::literal("literal", "x", Severity::High)]);
        let mut session = scanner.incremental_session("x".into()).unwrap();
        session.literal_lengths[0] = Some(2);
        let before = observe(&session);
        assert_eq!(
            session.apply_edit(0, 1..1, "x"),
            Err(IncrementalError::ExecutionInvariantViolated)
        );
        assert_eq!(observe(&session), before);
        assert_eq!(session.lifecycle, Lifecycle::Failed);
        assert_eq!(
            session.apply_edit(0, 0..0, ""),
            Err(IncrementalError::NotActive)
        );
    }

    #[test]
    fn regional_enumeration_owns_starts_not_halo_matches_or_pattern_slots() {
        let rules = CompiledRuleSet::compile(vec![
            Rule::literal("aa-first", "aa", Severity::High),
            Rule::literal("aa-second", "aa", Severity::High),
            Rule::literal("unicode", "é", Severity::High),
            Rule::prefix("prefix", "a", Severity::High),
        ])
        .unwrap();
        let source = "éaaaa";
        let owners = vec![2..4, 3..4, 0..0, 0..0];
        let mut raw = Vec::new();
        let mut work = IncrementalWork::default();
        // Begin inside é, include end=5 exactly, and exclude valid halo starts.
        rules
            .scan_incremental_literals(
                source,
                1..5,
                &owners,
                &mut |finding| {
                    raw.push(finding);
                    Ok(())
                },
                &mut work,
            )
            .unwrap();
        let mut full = Vec::new();
        rules.scan(source, &mut full);
        full.retain(|finding| owners[finding.rule_index().get()].contains(&finding.start()));
        assert_eq!(raw, full);
        assert_eq!(raw.len(), 3);
        assert!(raw.iter().any(|finding| finding.end() == 5));
    }

    #[test]
    fn every_contextual_validator_reuses_the_authoritative_dispatch() {
        use ValidatorKind::*;
        let kinds = [
            Aws,
            Azure,
            Gcp,
            NpmRegistry,
            RubyGemsHost,
            NhsNumber,
            Ssn,
            CardVerificationCode,
            OtpProvisioningSecret,
            Pan,
            Password,
            SensitiveHash,
            GenericCredential,
            HttpBasic,
            DatabaseConnection,
            SwiftPm,
            Gradle,
            CargoRegistry,
            Pypi,
            WireGuard,
            DockerRegistry,
            Netrc,
            SwiftPmNetrc,
            Maven,
            SystemPasswordVerifier,
            ComposerHttpBasicPassword,
            ComposerBearerToken,
            ComposerBitbucketConsumerSecret,
            ComposerForgejoToken,
            Nuget,
        ];
        let value = "CorrectHorseBatteryStaple";
        let scanner = scanner(
            kinds
                .iter()
                .enumerate()
                .map(|(i, &kind)| {
                    assert!(matches!(
                        kind.streaming_requirement(),
                        ValidatorStreamingRequirement::BoundedContext { .. }
                    ));
                    Rule::literal(format!("contextual-{i}"), value, Severity::High)
                        .with_validator(kind)
                })
                .collect(),
        );
        let mut session = scanner
            .incremental_session(format!("é password={value} 🦀"))
            .unwrap();
        // Source bytes are unchanged, but these 30 before windows cross the edit.
        edit(&scanner, &mut session, 3..3, "ordinary ");
        assert_eq!(
            session.current.work.get(WorkMetric::RevalidatedCandidates),
            30
        );
        let end = session.source().len();
        edit(
            &scanner,
            &mut session,
            end..end,
            "</password></server></servers></settings>",
        );
        // Only the seven before+after dispatch kinds must be revalidated here.
        assert_eq!(
            session.current.work.get(WorkMetric::RevalidatedCandidates),
            7
        );
        assert_eq!(session.current.work.get(WorkMetric::ReusedRecords), 23);
    }

    #[test]
    fn deterministic_mixed_edit_sequences_preserve_report_and_cache_parity() {
        let value = "CorrectHorseBatteryStaple";
        let scanner = scanner(vec![
            Rule::literal("overlap", "aa", Severity::High),
            Rule::literal("password", value, Severity::High)
                .with_validator(ValidatorKind::Password),
            Rule::literal("maven", value, Severity::High).with_validator(ValidatorKind::Maven),
            Rule::prefix("prefix", "pre", Severity::High),
            Rule::suffix("suffix", "!", Severity::Low),
            Rule::captured_pattern("capture", r"key=(?P<value>[^;]*);", "value", Severity::High)
                .unwrap(),
            Rule::captured_pattern(
                "generic.password-field",
                r"(?:password|pwd)[^=]*=(?P<value>[^\s]+)",
                "value",
                Severity::High,
            )
            .unwrap()
            .with_validator(ValidatorKind::Password),
        ]);
        let mut session = scanner
            .incremental_session(format!(
                "é
aaaaaa preTOKEN a! key=; password pwd={value}
ABCD-EFGH-IJKL-MNOP"
            ))
            .unwrap();
        let replacements = [
            "",
            "aa",
            "é",
            "🦀
",
            "preTOKEN ",
            "key=;",
            "-",
            "password=",
            value,
        ];
        for step in 0..90 {
            let boundaries: Vec<_> = (0..=session.source().len())
                .filter(|&p| session.source().is_char_boundary(p))
                .collect();
            let first = step * 17 % boundaries.len();
            let last = (first + step % 4).min(boundaries.len() - 1);
            edit(
                &scanner,
                &mut session,
                boundaries[first]..boundaries[last],
                replacements[step % replacements.len()],
            );
        }
    }

    #[test]
    fn checked_shifting_clipping_and_work_accounting_reject_overflow() {
        let geometry = EditGeometry {
            a: usize::MAX - 1,
            b: usize::MAX,
            c: usize::MAX,
        };
        assert_eq!(
            geometry.literal_starts(usize::MAX, 1, geometry.b),
            Ok(usize::MAX - 1..usize::MAX)
        );
        assert_eq!(
            geometry.structural_starts(usize::MAX, geometry.b),
            Ok(usize::MAX - 20..usize::MAX - 18)
        );
        assert_eq!(
            geometry.literal_starts(10, 0, 5),
            Err(IncrementalError::ExecutionInvariantViolated)
        );
        assert_eq!(
            geometry.map_suffix(0),
            Err(IncrementalError::ExecutionInvariantViolated)
        );
        assert_eq!(
            geometry.map_span(Range { start: 3, end: 2 }),
            Err(IncrementalError::ExecutionInvariantViolated)
        );
        let mut work = IncrementalWork::default();
        assert_eq!(work.record(WorkMetric::LiteralBytes, usize::MAX), Some(()));
        assert_eq!(work.record(WorkMetric::LiteralBytes, 1), None);
        assert_eq!(work.get(WorkMetric::LiteralBytes), usize::MAX);
    }
    #[test]
    fn resource_zero_limits_allow_empty_execution() {
        let scanner = scanner(vec![]);
        let limits = IncrementalLimits::default()
            .with_max_candidates(Some(0))
            .with_max_retained_cache_records(Some(0))
            .with_max_materialized_results(Some(0))
            .with_max_transaction_storage_bytes(Some(0));
        let mut session = scanner
            .incremental_session_with_limits(String::new(), limits)
            .unwrap();
        assert_eq!(session.current.peak_bytes, 0);
        assert_eq!(session.apply_edit(0, 0..0, ""), Ok(1));
        assert_eq!(
            session.apply_edit(1, 0..0, "a"),
            Err(IncrementalError::TransactionStorageLimitExceeded {
                required: 1,
                limit: 0
            })
        );
        assert!(!session.is_active());
        assert_eq!(session.source(), "");
        assert_eq!(session.revision(), 1);
    }

    #[test]
    fn resource_peak_cache_ownership_counts_separate_copies_not_shared_metadata() {
        let scanner = scanner(vec![Rule::literal("literal", "x", Severity::High)]);
        let limits = IncrementalLimits::default()
            .with_max_retained_cache_records(Some(2))
            .with_max_candidates(Some(1));
        let mut session = scanner
            .incremental_session_with_limits("x".into(), limits)
            .unwrap();
        assert_eq!(session.current.peak_cache, 1);
        // A second session shares compiled metadata, never this session's cache.
        let _other = scanner.incremental_session("x".into()).unwrap();
        for revision in 0..3 {
            assert_eq!(session.apply_edit(revision, 0..0, ""), Ok(revision + 1));
            assert_eq!(session.current.peak_cache, 2);
        }
        session.limits = limits.with_max_retained_cache_records(Some(1));
        let before = observe(&session);
        assert_eq!(
            session.apply_edit(3, 0..0, ""),
            Err(IncrementalError::CacheLimitExceeded { limit: 1 })
        );
        assert_eq!(observe(&session), before);
        assert_eq!(
            session.apply_edit(3, 0..0, ""),
            Err(IncrementalError::NotActive)
        );
    }

    #[test]
    fn resource_emission_checks_precede_accumulation_for_every_fallback() {
        let rules = [
            Rule::literal("literal", "x", Severity::High),
            Rule::prefix("prefix", "x", Severity::High),
            Rule::suffix("suffix", "x", Severity::High),
            Rule::pattern("regex", "x", Severity::High).unwrap(),
            Rule::captured_pattern("capture", "x(?P<value>)", "value", Severity::High).unwrap(),
            Rule::pattern("reject", "[0-9]{11}", Severity::High)
                .unwrap()
                .with_validator(ValidatorKind::Pesel),
            builtins::PASSWORD_FIELD.to_rule().unwrap(),
        ];
        for rule in rules {
            let scanner = scanner(vec![rule]);
            let compiled = scanner.incremental_session(String::new()).unwrap().rules;
            let source = "password=CorrectHorseBatteryStaple\nx x 44051401459 44051401459\npassword=CorrectHorseBatteryStaple";
            let mut operation = Operation::new(
                IncrementalLimits::default().with_max_candidates(Some(1)),
                0,
                0,
            )
            .unwrap();
            let mut cache = vec![Vec::new(); compiled.len()];
            let mut callbacks = 0;
            let error = compiled.scan_with_emit(source, &mut |finding| {
                callbacks += 1;
                ingest(&compiled, source, &mut cache, &mut operation, finding)
            });
            assert_eq!(
                error,
                Err(IncrementalError::CandidateLimitExceeded { limit: 1 })
            );
            assert_eq!(callbacks, 2);
            assert_eq!(operation.raw, 1);
            assert_eq!(cache.iter().map(Vec::len).sum::<usize>(), 1);
        }
    }

    #[test]
    fn resource_raw_limit_is_per_operation_and_includes_rejections() {
        let scanner = scanner(vec![
            Rule::pattern("pesel", "[0-9]{11}", Severity::High)
                .unwrap()
                .with_validator(ValidatorKind::Pesel),
        ]);
        let limits = IncrementalLimits::default()
            .with_max_candidates(Some(1))
            .with_max_materialized_results(Some(0));
        let mut session = scanner
            .incremental_session_with_limits("44051401459".into(), limits)
            .unwrap();
        assert!(session.report().is_empty());
        assert_eq!(session.current.per_rule[0].len(), 1);
        assert_eq!(session.apply_edit(0, 0..0, ""), Ok(1)); // full regex replay gets a fresh raw budget
        let old = observe(&session);
        assert_eq!(
            session.apply_edit(1, 11..11, " 44051401459"),
            Err(IncrementalError::CandidateLimitExceeded { limit: 1 })
        );
        assert_eq!(observe(&session), old);
        assert!(!session.is_active());
    }

    #[test]
    fn resource_reused_and_revalidated_records_do_not_emit_raw_candidates() {
        let value = "CorrectHorseBatteryStaple";
        let scanner = scanner(vec![
            Rule::literal("password", value, Severity::High)
                .with_validator(ValidatorKind::Password),
        ]);
        let mut session = scanner
            .incremental_session(format!("label={value}"))
            .unwrap();
        session.limits = IncrementalLimits::default().with_max_candidates(Some(0));
        assert_eq!(session.apply_edit(0, 0..5, "password"), Ok(1));
        assert_eq!(
            session.current.work.get(WorkMetric::RevalidatedCandidates),
            1
        );
        parity(&scanner, &session);
        assert_eq!(session.apply_edit(1, 8..8, ""), Ok(2));
        assert_eq!(session.current.work.get(WorkMetric::ReusedRecords), 1);
    }

    #[test]
    fn resource_results_count_suppressed_structural_copies_without_refund() {
        let code = "QRST-UVWX-YZ12-3456";
        let scanner = scanner(vec![Rule::literal("code", code, Severity::High)]);
        let limits = IncrementalLimits::default()
            .with_max_retained_cache_records(Some(2))
            .with_max_materialized_results(Some(2));
        let session = scanner
            .incremental_session_with_limits(code.into(), limits)
            .unwrap();
        assert_eq!(session.report().len(), 1);
        assert_eq!(session.report().candidate_len(), 0);
        assert_eq!(session.current.structural.len(), 1);
        assert_eq!(session.current.peak_cache, 2);
        assert!(matches!(
            scanner.incremental_session_with_limits(
                code.into(),
                limits.with_max_materialized_results(Some(1))
            ),
            Err(IncrementalError::ResultLimitExceeded { limit: 1 })
        ));
        assert!(matches!(
            scanner.incremental_session_with_limits(
                code.into(),
                limits.with_max_retained_cache_records(Some(1))
            ),
            Err(IncrementalError::CacheLimitExceeded { limit: 1 })
        ));
    }

    #[test]
    fn resource_normalization_losers_keep_cache_tickets_not_materialization_tickets() {
        let scanner = scanner(vec![
            Rule::literal("generic", "CorrectHorseBatteryStaple", Severity::Low),
            Rule::literal("validated", "CorrectHorseBatteryStaple", Severity::High)
                .with_validator(ValidatorKind::Password),
        ]);
        let limits = IncrementalLimits::default()
            .with_max_candidates(Some(2))
            .with_max_retained_cache_records(Some(2))
            .with_max_materialized_results(Some(1));
        let session = scanner
            .incremental_session_with_limits("password=CorrectHorseBatteryStaple".into(), limits)
            .unwrap();
        assert_eq!(session.current.cache_len().unwrap(), 2);
        assert_eq!(session.report().len(), 1);
        assert!(matches!(
            scanner.incremental_session_with_limits(
                session.source().into(),
                limits.with_max_retained_cache_records(Some(1))
            ),
            Err(IncrementalError::CacheLimitExceeded { limit: 1 })
        ));
    }

    #[test]
    fn resource_storage_boundaries_and_live_payload_reconcile_after_transfers() {
        let scanner = scanner(vec![Rule::literal("literal", "x", Severity::High)]);
        let text = "x QRST-UVWX-YZ12-3456";
        let session = scanner.incremental_session(text.into()).unwrap();
        let compiled = Arc::clone(&session.rules);
        let required = session.current.peak_bytes;
        assert!(required > session.current.payload().unwrap());
        let limits =
            IncrementalLimits::default().with_max_transaction_storage_bytes(Some(required));
        scanner
            .incremental_session_with_limits(text.into(), limits)
            .unwrap();
        assert!(matches!(
            scanner.incremental_session_with_limits(
                text.into(),
                limits.with_max_transaction_storage_bytes(Some(required - 1))
            ),
            Err(IncrementalError::TransactionStorageLimitExceeded { .. })
        ));
        let baseline = sum(
            session.current.payload().unwrap(),
            payload::<Option<usize>>(compiled.len()).unwrap(),
        )
        .unwrap();
        let mut operation = Operation::new(
            IncrementalLimits::default(),
            baseline,
            session.current.cache_len().unwrap(),
        )
        .unwrap();
        let next = format!("é\n{text}");
        operation.bytes(next.len()).unwrap();
        let staged = session
            .current
            .update(
                &compiled,
                &session.literal_lengths,
                next,
                1,
                EditGeometry { a: 0, b: 0, c: 3 },
                &mut operation,
            )
            .unwrap();
        assert_eq!(operation.bytes, baseline + staged.payload().unwrap());
        assert_eq!(
            operation.cache,
            session.current.cache_len().unwrap() + staged.cache_len().unwrap()
        );
        let required = operation.peak;
        let mut exact = scanner.incremental_session(text.into()).unwrap();
        exact.limits = limits.with_max_transaction_storage_bytes(Some(required));
        assert_eq!(exact.apply_edit(0, 0..0, "é\n"), Ok(1));
        parity(&scanner, &exact);
        let mut below = scanner.incremental_session(text.into()).unwrap();
        below.limits = limits.with_max_transaction_storage_bytes(Some(required - 1));
        let old = observe(&below);
        assert!(matches!(
            below.apply_edit(0, 0..0, "é\n"),
            Err(IncrementalError::TransactionStorageLimitExceeded { .. })
        ));
        assert_eq!(observe(&below), old);
        assert!(!below.is_active());
    }

    #[test]
    fn resource_preflight_errors_and_checked_overflow_are_deterministic() {
        assert_eq!(
            payload::<CachedCandidate>(usize::MAX),
            Err(IncrementalError::ResourceAccountingOverflow)
        );
        assert_eq!(
            payload::<u8>(isize::MAX as usize + 1),
            Err(IncrementalError::ResourceAccountingOverflow)
        );
        assert_eq!(
            sum(usize::MAX, 1),
            Err(IncrementalError::ResourceAccountingOverflow)
        );
        let mut operation = Operation::new(IncrementalLimits::default(), usize::MAX, 0).unwrap();
        assert_eq!(
            operation.reserve::<u8>(1),
            Err(IncrementalError::ResourceAccountingOverflow)
        );
        assert_eq!(operation.bytes, usize::MAX);
        operation.raw = usize::MAX;
        assert_eq!(
            operation.raw(),
            Err(IncrementalError::ResourceAccountingOverflow)
        );
        let scanner = scanner(vec![Rule::literal("x", "x", Severity::High)]);
        let mut session = scanner.incremental_session("x".into()).unwrap();
        session.limits = IncrementalLimits::default().with_max_transaction_storage_bytes(Some(0));
        assert!(matches!(
            session.apply_edit(1, Range { start: 2, end: 1 }, ""),
            Err(IncrementalError::StaleRevision { .. })
        ));
        assert!(session.is_active());
        assert_eq!(
            session.apply_edit(0, 0..0, ""),
            Err(IncrementalError::TransactionStorageLimitExceeded {
                required: session.current.payload().unwrap() + payload::<Option<usize>>(1).unwrap(),
                limit: 0
            })
        );
        assert_eq!(
            session.apply_edit(0, 0..0, ""),
            Err(IncrementalError::NotActive)
        );
        assert!(!format!("{session:?}").contains("source: "));
    }

    #[test]
    fn resource_large_fallback_stops_without_source_sized_raw_collection() {
        let scanner = scanner(vec![Rule::pattern("regex", "x", Severity::High).unwrap()]);
        let compiled = scanner.incremental_session(String::new()).unwrap().rules;
        let source = "x".repeat(1024 * 1024);
        let mut operation = Operation::new(
            IncrementalLimits::default().with_max_candidates(Some(3)),
            0,
            0,
        )
        .unwrap();
        let mut cache = vec![Vec::new()];
        let mut work = IncrementalWork::default();
        assert_eq!(
            compiled.scan_incremental_fallback(
                &source,
                &mut |finding| ingest(&compiled, &source, &mut cache, &mut operation, finding),
                &mut work
            ),
            Err(IncrementalError::CandidateLimitExceeded { limit: 3 })
        );
        assert_eq!(cache[0].len(), 3);
        assert_eq!(operation.bytes, payload::<CachedCandidate>(3).unwrap());
    }

    #[test]
    fn resource_diagnostics_report_owned_payload_and_avoided_work() {
        for (name, rules, source) in [
            (
                "sparse-literal",
                vec![Rule::literal("secret", "SECRET", Severity::High)],
                format!("{}SECRET", "ordinary value\n".repeat(4400)),
            ),
            (
                "dense-literal",
                vec![Rule::literal("aa", "aa", Severity::High)],
                "a".repeat(65536),
            ),
            ("structural", vec![], "QRST-UVWX-YZ12-3456 ".repeat(3276)),
        ] {
            let scanner = scanner(rules);
            let mut session = scanner.incremental_session(source).unwrap();
            let construction = session.current.peak_bytes;
            let at = session.source().len() / 2;
            session.apply_edit(0, at..at, "x").unwrap();
            parity(&scanner, &session);
            assert!(session.current.peak_bytes >= session.current.payload().unwrap());
            assert!(session.current.observed_peak <= session.current.peak_bytes);
            assert!(session.current.peak_cache >= session.current.cache_len().unwrap());
            println!(
                "D4 {name}: source={} construct_bytes={construction} edit_bytes={} observed_bytes={} peak_cache={} local_bytes={} structural_bytes={} reused={} revalidated={} regenerated={} fallback_rules={}",
                session.source().len(),
                session.current.peak_bytes,
                session.current.observed_peak,
                session.current.peak_cache,
                session.current.work.get(WorkMetric::LiteralBytes),
                session.current.work.get(WorkMetric::StructuralBytes),
                session.current.work.get(WorkMetric::ReusedRecords),
                session.current.work.get(WorkMetric::RevalidatedCandidates),
                session.current.work.get(WorkMetric::RegeneratedCandidates),
                session.current.work.get(WorkMetric::WholeRuleFallbacks)
            );
        }
    }
}
