use std::sync::Arc;

use crate::{
    compiled_rule::{
        CandidateSink, CandidateView, CompiledRuleSet, ContextStoragePlanningError,
        ContextualStoragePlan, CoordinateStoragePlan, CoordinateStoragePlanningError,
        InternalFinding, MultiPatternStreamState, PatternStreamState, PendingValidation,
        PlannedPrefixCoordinatePin, ResourceLimitExceeded, StreamingEvent, StreamingStorageError,
        SuffixStreamState,
    },
    finding::Finding,
    location::Location,
    scanner::{AcceptedCandidate, normalize_candidates_by},
    source_coordinates::{CoordinateOverflow, SourceCoordinates},
    utf8_transport::{Utf8Transport, Utf8TransportError},
    validators::dispatch::{ValidatorStreamingRequirement, validate_candidate},
};

/// Mutable execution state owned by one logical source.
///
/// Streaming execution state belongs here rather than in the shared compiled
/// scanner configuration. Later streaming slices extend this boundary with the
/// concrete matcher, validator, candidate, normalization, and location state
/// they require.
#[derive(Debug)]
struct SourceExecutionState {
    accepted_bytes: usize,
    emitted_candidates: usize,
    transport: Utf8Transport,
    multi_pattern: MultiPatternStreamState,
    suffix: SuffixStreamState,
    patterns: Vec<PatternStreamState>,
}

/// Mutable execution state for one logical source.
///
/// A source session owns all state that may change while processing a single
/// logical source. The compiled rule set is shared immutable configuration and
/// must never contain source-local execution state.
#[derive(Debug)]
pub(crate) struct SourceSession {
    rules: Arc<CompiledRuleSet>,
    lifecycle: SourceLifecycle,
    state: SourceExecutionState,
    max_candidates: Option<usize>,
}

/// Optional source-local limits. None preserves unlimited execution.
///
/// Storage admission covers maximum logical streaming buffer payload only,
/// excluding spare capacity, allocator overhead, fixed metadata, compiled
/// matchers, library workspace, and caller-owned input and findings.
/// Validator context is not allocated or accounted for in this raw-candidate phase.
#[derive(Debug, Default, Copy, Clone, Eq, PartialEq)]
pub(crate) struct SourceLimits {
    pub(crate) max_candidates: Option<usize>,
    pub(crate) max_stream_storage_bytes: Option<usize>,
}

/// Lifecycle of one logical-source execution.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
enum SourceLifecycle {
    Active,
    Completed,
    Failed,
}

/// Source admission or execution failure.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub(crate) enum SourceSessionError {
    UnsupportedRuleSet,
    NotActive,
    InvalidUtf8(Utf8TransportError),
    ResourceLimitExceeded,
    StreamStorageLimitExceeded { required: usize, limit: usize },
    StreamStorageRequirementOverflow,
    SourceOffsetOverflow,
    MissingCandidateText,
    MissingSourceHistory,
    CandidateTextMismatch,
    ContextRetentionExceeded,
    InvalidPendingRange,
    PendingCapacityExceeded,
    CoordinateOverflow(CoordinateOverflow),
    MissingCoordinateHistory,
    MissingPrefixCoordinatePin,
    InvalidCoordinateOffset,
    CoordinateInvariantViolation,
    NotCompleted,
    AlreadyFinalized,
    RetainedResultLimitExceeded,
}

impl From<CoordinateOverflow> for SourceSessionError {
    fn from(error: CoordinateOverflow) -> Self {
        Self::CoordinateOverflow(error)
    }
}

impl From<Utf8TransportError> for SourceSessionError {
    fn from(error: Utf8TransportError) -> Self {
        Self::InvalidUtf8(error)
    }
}

impl SourceSession {
    pub(crate) fn new(rules: Arc<CompiledRuleSet>) -> Result<Self, SourceSessionError> {
        Self::new_with_limits(rules, SourceLimits::default())
    }

    /// Creates a session with a cumulative raw-candidate limit; None is unlimited.
    pub(crate) fn new_with_max_candidates(
        rules: Arc<CompiledRuleSet>,
        max_candidates: Option<usize>,
    ) -> Result<Self, SourceSessionError> {
        Self::new_with_limits(
            rules,
            SourceLimits {
                max_candidates,
                ..SourceLimits::default()
            },
        )
    }

    /// Admits the source before allocating execution state. Storage admission
    /// is static and does not enforce actual heap allocation capacity.
    pub(crate) fn new_with_limits(
        rules: Arc<CompiledRuleSet>,
        limits: SourceLimits,
    ) -> Result<Self, SourceSessionError> {
        if !rules.supports_streaming() || !rules.supports_streaming_semantics() {
            return Err(SourceSessionError::UnsupportedRuleSet);
        }

        if let Some(limit) = limits.max_stream_storage_bytes {
            let requirements =
                rules
                    .streaming_storage_requirements()
                    .map_err(|error| match error {
                        StreamingStorageError::ArithmeticOverflow => {
                            SourceSessionError::StreamStorageRequirementOverflow
                        }
                        StreamingStorageError::UnsupportedPattern(_) => {
                            SourceSessionError::UnsupportedRuleSet
                        }
                    })?;
            let required = requirements.total_buffer_bytes;
            if required > limit {
                return Err(SourceSessionError::StreamStorageLimitExceeded { required, limit });
            }
        }

        let patterns = rules.new_pattern_stream_states();

        Ok(Self {
            rules,
            lifecycle: SourceLifecycle::Active,
            max_candidates: limits.max_candidates,
            state: SourceExecutionState {
                accepted_bytes: 0,
                emitted_candidates: 0,
                transport: Utf8Transport::default(),
                multi_pattern: MultiPatternStreamState::default(),
                suffix: SuffixStreamState::default(),
                patterns,
            },
        })
    }

    pub(crate) fn accepted_bytes(&self) -> usize {
        self.state.accepted_bytes
    }

    pub(crate) fn scan_chunk(
        &mut self,
        chunk: &[u8],
        findings: &mut Vec<InternalFinding>,
    ) -> Result<(), SourceSessionError> {
        self.scan_chunk_with_ingestion(chunk, &mut |view| {
            findings.push(view.finding);
            Ok(())
        })
    }

    fn scan_chunk_with_ingestion(
        &mut self,
        chunk: &[u8],
        ingest: &mut impl FnMut(CandidateView<'_>) -> Result<(), SourceSessionError>,
    ) -> Result<(), SourceSessionError> {
        self.scan_chunk_with_context(chunk, &mut |view, _, _| {
            if let Some(view) = view {
                ingest(view)?;
            }
            Ok(())
        })
    }

    // None marks successful callback completion, before accepted_bytes advances.
    fn scan_chunk_with_context(
        &mut self,
        chunk: &[u8],
        ingest: &mut impl FnMut(
            Option<CandidateView<'_>>,
            &str,
            usize,
        ) -> Result<(), SourceSessionError>,
    ) -> Result<(), SourceSessionError> {
        self.scan_chunk_with_events(chunk, &mut |event, text, base| match event {
            Some(StreamingEvent::Candidate(view) | StreamingEvent::PrefixCompleted(view)) => {
                ingest(Some(view), text, base)
            }
            Some(StreamingEvent::PrefixDiscovered { .. }) => Ok(()),
            None => ingest(None, text, base),
        })
    }

    fn scan_chunk_with_events(
        &mut self,
        chunk: &[u8],
        ingest: &mut impl FnMut(
            Option<StreamingEvent<'_>>,
            &str,
            usize,
        ) -> Result<(), SourceSessionError>,
    ) -> Result<(), SourceSessionError> {
        self.ensure_active()?;

        let rules = &self.rules;
        let state = &mut self.state;
        let max_candidates = self.max_candidates;
        let mut execution_result: Result<(), SourceSessionError> = Ok(());

        let result = state.transport.push(chunk, |text| {
            // Transport callbacks are infallible. Preserve the first execution
            // failure and skip later callbacks from the same push.
            if execution_result.is_err() {
                return;
            }
            let bytes = text.as_bytes();
            let source_offset = state.accepted_bytes;
            let Some(source_end) = source_offset.checked_add(bytes.len()) else {
                execution_result = Err(SourceSessionError::SourceOffsetOverflow);
                return;
            };
            let mut ingestion_error = None;
            let mut callback = |event: StreamingEvent<'_>| {
                ingest(Some(event), text, source_offset).map_err(|error| {
                    ingestion_error = Some(error);
                    ResourceLimitExceeded
                })
            };
            let mut sink = CandidateSink::with_events(
                &mut callback,
                &mut state.emitted_candidates,
                max_candidates,
            );
            execution_result = (|| -> Result<(), ResourceLimitExceeded> {
                rules.scan_multi_pattern_stream_chunk(
                    &mut state.multi_pattern,
                    bytes,
                    source_offset,
                    &mut sink,
                )?;

                rules.scan_suffix_stream_chunk(
                    &mut state.suffix,
                    bytes,
                    source_offset,
                    &mut sink,
                )?;

                rules.scan_pattern_stream_chunk(
                    &mut state.patterns,
                    bytes,
                    source_offset,
                    &mut sink,
                )?;

                Ok(())
            })()
            .map_err(|_| ingestion_error.unwrap_or(SourceSessionError::ResourceLimitExceeded));
            if execution_result.is_ok() {
                execution_result = ingest(None, text, source_offset);
                if execution_result.is_ok() {
                    state.accepted_bytes = source_end;
                }
            }
        });

        if let Err(error) = execution_result {
            self.lifecycle = SourceLifecycle::Failed;
            return Err(error);
        }
        if let Err(error) = result {
            self.lifecycle = SourceLifecycle::Failed;
            return Err(SourceSessionError::InvalidUtf8(error));
        }

        Ok(())
    }

    #[cfg(test)]
    fn lifecycle(&self) -> SourceLifecycle {
        self.lifecycle
    }

    pub(crate) fn finish_scan(
        &mut self,
        findings: &mut Vec<InternalFinding>,
    ) -> Result<(), SourceSessionError> {
        self.finish_with_ingestion(&mut |view| {
            findings.push(view.finding);
            Ok(())
        })
    }

    fn finish_with_ingestion(
        &mut self,
        ingest: &mut impl FnMut(CandidateView<'_>) -> Result<(), SourceSessionError>,
    ) -> Result<(), SourceSessionError> {
        self.finish_with_context(&mut |view| {
            if let Some(view) = view {
                ingest(view)?;
            }
            Ok(())
        })
    }

    // None announces genuine EOF after successful transport finalization.
    fn finish_with_context(
        &mut self,
        ingest: &mut impl FnMut(Option<CandidateView<'_>>) -> Result<(), SourceSessionError>,
    ) -> Result<(), SourceSessionError> {
        self.finish_with_events(&mut |event| match event {
            Some(StreamingEvent::Candidate(view) | StreamingEvent::PrefixCompleted(view)) => {
                ingest(Some(view))
            }
            Some(StreamingEvent::PrefixDiscovered { .. }) => {
                Err(SourceSessionError::CoordinateInvariantViolation)
            }
            None => ingest(None),
        })
    }

    fn finish_with_events(
        &mut self,
        ingest: &mut impl FnMut(Option<StreamingEvent<'_>>) -> Result<(), SourceSessionError>,
    ) -> Result<(), SourceSessionError> {
        self.ensure_active()?;

        if let Err(error) = self.state.transport.finish() {
            self.lifecycle = SourceLifecycle::Failed;
            return Err(SourceSessionError::InvalidUtf8(error));
        }

        if let Err(error) = ingest(None) {
            self.lifecycle = SourceLifecycle::Failed;
            return Err(error);
        }

        let mut ingestion_error = None;
        let mut callback = |event: StreamingEvent<'_>| {
            ingest(Some(event)).map_err(|error| {
                ingestion_error = Some(error);
                ResourceLimitExceeded
            })
        };
        let mut sink = CandidateSink::with_events(
            &mut callback,
            &mut self.state.emitted_candidates,
            self.max_candidates,
        );
        let result: Result<(), ResourceLimitExceeded> = (|| {
            self.rules.finish_multi_pattern_stream(
                &mut self.state.multi_pattern,
                self.state.accepted_bytes,
                &mut sink,
            )?;

            self.rules
                .finish_suffix_stream(&mut self.state.suffix, &mut sink)?;

            self.rules
                .finish_pattern_stream(&mut self.state.patterns, &mut sink)?;
            Ok(())
        })();

        if result.is_err() {
            self.lifecycle = SourceLifecycle::Failed;
            return Err(ingestion_error.unwrap_or(SourceSessionError::ResourceLimitExceeded));
        }

        self.lifecycle = SourceLifecycle::Completed;

        Ok(())
    }

    fn ensure_active(&self) -> Result<(), SourceSessionError> {
        if self.lifecycle == SourceLifecycle::Active {
            Ok(())
        } else {
            Err(SourceSessionError::NotActive)
        }
    }

    #[cfg(test)]
    pub(crate) fn shares_rules_with(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.rules, &other.rules)
    }
}

/// Accepted, unnormalized candidate. No source text or metadata borrow is retained.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub(crate) struct ValidatedCandidate {
    pub(crate) finding: InternalFinding,
    pub(crate) confidence: crate::Confidence,
}

/// Caller-owned accepted output; coordinates are captured before validation.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub(crate) struct AnnotatedCandidate {
    pub(crate) finding: InternalFinding,
    pub(crate) confidence: crate::Confidence,
    pub(crate) line: usize,
    pub(crate) column: usize,
}

pub(crate) trait AcceptedOutput<C> {
    fn accept(
        &mut self,
        candidate: ValidatedCandidate,
        coordinates: C,
    ) -> Result<(), SourceSessionError>;

    fn callback_completed(&mut self, _text: &str, _base: usize) -> Result<(), SourceSessionError> {
        Ok(())
    }
}

impl AcceptedOutput<()> for Vec<ValidatedCandidate> {
    fn accept(&mut self, candidate: ValidatedCandidate, _: ()) -> Result<(), SourceSessionError> {
        self.push(candidate);
        Ok(())
    }
}

impl AcceptedOutput<SourceCoordinates> for Vec<AnnotatedCandidate> {
    fn accept(
        &mut self,
        candidate: ValidatedCandidate,
        coordinates: SourceCoordinates,
    ) -> Result<(), SourceSessionError> {
        self.push(AnnotatedCandidate {
            finding: candidate.finding,
            confidence: candidate.confidence,
            line: coordinates.line(),
            column: coordinates.column(),
        });
        Ok(())
    }
}

/// Opt-in candidate and bounded-context validation; no public findings or normalization.
/// Caller-owned accepted storage is separate from matcher logical-storage admission.
#[derive(Debug)]
pub(crate) struct ValidatedSourceSession {
    session: SourceSession,
    context: ContextRetention,
}

/// Logical payload is bounded by the checked contextual plan. String spare
/// capacity and caller-owned accepted records are outside this storage subtotal.
#[derive(Debug)]
struct ContextRetention<C = ()> {
    history: String,
    history_base: usize,
    history_limit: usize,
    scratch: String,
    scratch_limit: usize,
    plan: ContextualStoragePlan,
    pending: Vec<PendingValidation<C>>,
    #[cfg(test)]
    usage: ContextUsage,
}

#[cfg(test)]
#[derive(Debug, Default)]
struct ContextUsage {
    history: usize,
    scratch: usize,
    snapshots: usize,
    records_per_rule: Vec<usize>,
    snapshot_per_rule: Vec<usize>,
}

impl<C: Copy> ContextRetention<C> {
    fn new(plan: ContextualStoragePlan) -> Self {
        Self {
            history: String::new(),
            history_base: 0,
            history_limit: plan.history_bytes,
            scratch: String::new(),
            scratch_limit: plan.assembly_scratch_bytes,
            plan,
            pending: Vec::new(),
            #[cfg(test)]
            usage: ContextUsage::default(),
        }
    }
    #[cfg(test)]
    fn observe(&mut self) {
        self.usage.history = self.usage.history.max(self.history.len());
        self.usage.scratch = self.usage.scratch.max(self.scratch.len());
        self.usage.snapshots = self
            .usage
            .snapshots
            .max(self.pending.iter().map(|record| record.text.len()).sum());
        self.usage.records_per_rule.resize(self.plan.rules.len(), 0);
        self.usage
            .snapshot_per_rule
            .resize(self.plan.rules.len(), 0);
        for (index, plan) in self.plan.rules.iter().enumerate() {
            let records = self
                .pending
                .iter()
                .filter(|record| record.finding.rule_index() == plan.rule_index);
            self.usage.records_per_rule[index] =
                self.usage.records_per_rule[index].max(records.clone().count());
            self.usage.snapshot_per_rule[index] = self.usage.snapshot_per_rule[index]
                .max(records.map(|record| record.text.len()).max().unwrap_or(0));
        }
    }

    /// Append the exact after-window, clipping its end inward to UTF-8.
    fn append_after(
        &mut self,
        start: usize,
        mut end: usize,
        text: &str,
        base: usize,
    ) -> Result<(), SourceSessionError> {
        if start > end || start < self.history_base {
            return Err(SourceSessionError::InvalidPendingRange);
        }
        while end > start {
            let boundary = if end < base {
                self.history.is_char_boundary(end - self.history_base)
            } else {
                text.is_char_boundary(end - base)
            };
            if boundary {
                break;
            }
            end -= 1;
        }
        let length = self
            .scratch
            .len()
            .checked_add(end - start)
            .ok_or(SourceSessionError::StreamStorageRequirementOverflow)?;
        if length > self.scratch_limit {
            return Err(SourceSessionError::ContextRetentionExceeded);
        }
        if start < base {
            self.scratch.push_str(
                self.history
                    .get(start - self.history_base..end.min(base) - self.history_base)
                    .ok_or(SourceSessionError::MissingSourceHistory)?,
            );
        }
        if end > base {
            self.scratch.push_str(
                text.get(start.max(base) - base..end - base)
                    .ok_or(SourceSessionError::InvalidPendingRange)?,
            );
        }
        #[cfg(test)]
        self.observe();
        Ok(())
    }

    fn resolve_pending(
        &mut self,
        rules: &CompiledRuleSet,
        text: &str,
        base: usize,
        eof: bool,
        accepted: &mut impl AcceptedOutput<C>,
    ) -> Result<(), SourceSessionError> {
        #[cfg(test)]
        self.observe();
        #[cfg(test)]
        let mut snapshot_payload: usize = self.pending.iter().map(|record| record.text.len()).sum();
        let available_end = base
            .checked_add(text.len())
            .ok_or(SourceSessionError::SourceOffsetOverflow)?;
        let mut index = 0;
        while index < self.pending.len() {
            let record = &mut self.pending[index];
            let metadata = rules.metadata(record.finding.rule_index());
            let ValidatorStreamingRequirement::BoundedContext { after_bytes, .. } =
                metadata.validator().streaming_requirement()
            else {
                return Err(SourceSessionError::InvalidPendingRange);
            };
            if after_bytes == 0
                || record
                    .finding
                    .end()
                    .checked_add(after_bytes)
                    .ok_or(SourceSessionError::SourceOffsetOverflow)?
                    != record.after_end
            {
                return Err(SourceSessionError::InvalidPendingRange);
            }
            let range = record.candidate_range.clone();
            if range.start > range.end
                || record.text.get(range.clone()).is_none()
                || range.end - range.start
                    != record
                        .finding
                        .end()
                        .checked_sub(record.finding.start())
                        .ok_or(SourceSessionError::InvalidPendingRange)?
            {
                return Err(SourceSessionError::InvalidPendingRange);
            }
            let collected_end = record
                .finding
                .end()
                .checked_add(record.text.len() - range.end)
                .ok_or(SourceSessionError::SourceOffsetOverflow)?;
            if collected_end < base
                || collected_end > available_end
                || record.after_end < collected_end
            {
                return Err(SourceSessionError::InvalidPendingRange);
            }
            let mut end = record.after_end.min(available_end);
            while end > collected_end && !text.is_char_boundary(end - base) {
                end -= 1;
            }
            let extra = text
                .get(collected_end - base..end - base)
                .ok_or(SourceSessionError::InvalidPendingRange)?;
            let plan = self
                .plan
                .rules
                .iter()
                .find(|plan| plan.rule_index == record.finding.rule_index())
                .ok_or(SourceSessionError::InvalidPendingRange)?;
            if record
                .text
                .len()
                .checked_add(extra.len())
                .ok_or(SourceSessionError::StreamStorageRequirementOverflow)?
                > plan.snapshot_bytes
            {
                return Err(SourceSessionError::ContextRetentionExceeded);
            }
            record.text.push_str(extra);
            let ready = eof || available_end >= record.after_end;
            #[cfg(test)]
            {
                snapshot_payload += extra.len();
                self.usage.snapshots = self.usage.snapshots.max(snapshot_payload);
                let slot = self
                    .plan
                    .rules
                    .iter()
                    .position(|plan| plan.rule_index == record.finding.rule_index())
                    .unwrap();
                self.usage.snapshot_per_rule[slot] =
                    self.usage.snapshot_per_rule[slot].max(record.text.len());
            }
            if ready {
                let record = self.pending.remove(index);
                #[cfg(test)]
                {
                    snapshot_payload -= record.text.len();
                }
                let metadata = rules.metadata(record.finding.rule_index());
                if let Some(validation) = validate_candidate(
                    metadata.validator(),
                    &record.text,
                    record.candidate_range,
                    metadata.confidence(),
                ) {
                    accepted.accept(
                        ValidatedCandidate {
                            finding: record.finding,
                            confidence: validation.confidence(),
                        },
                        record.coordinates,
                    )?;
                }
            } else {
                index += 1;
            }
        }
        Ok(())
    }

    fn retain(&mut self, text: &str, base: usize) -> Result<(), SourceSessionError> {
        if self.history_limit != 0
            && self.history_base.checked_add(self.history.len()) != Some(base)
        {
            return Err(SourceSessionError::MissingSourceHistory);
        }
        let end = base
            .checked_add(text.len())
            .ok_or(SourceSessionError::SourceOffsetOverflow)?;
        if self.history_limit == 0 {
            self.history_base = end;
            return Ok(());
        }
        let keep = self
            .history_limit
            .checked_sub(3)
            .ok_or(SourceSessionError::ContextRetentionExceeded)?;
        if text.len() >= keep {
            let mut cut = text.len() - keep;
            while cut > 0 && !text.is_char_boundary(cut) {
                cut -= 1;
            }
            self.history.clear();
            self.history.push_str(&text[cut..]);
        } else {
            let wanted_old = keep - text.len();
            let mut cut = self.history.len().saturating_sub(wanted_old);
            while cut > 0 && !self.history.is_char_boundary(cut) {
                cut -= 1;
            }
            self.history.drain(..cut);
            self.history.push_str(text);
        }
        if self.history.len() > self.history_limit {
            return Err(SourceSessionError::ContextRetentionExceeded);
        }
        self.history_base = end
            .checked_sub(self.history.len())
            .ok_or(SourceSessionError::MissingSourceHistory)?;
        #[cfg(test)]
        self.observe();
        Ok(())
    }

    fn assemble(
        &mut self,
        view: CandidateView<'_>,
        before: usize,
        text: &str,
        base: usize,
    ) -> Result<usize, SourceSessionError> {
        let start = view.finding.start();
        let end = view.finding.end();
        let available_end = base
            .checked_add(text.len())
            .ok_or(SourceSessionError::SourceOffsetOverflow)?;
        if self.history_base.checked_add(self.history.len()) != Some(base)
            || end > available_end
            || start > end
        {
            return Err(SourceSessionError::MissingSourceHistory);
        }
        let mut left = start.saturating_sub(before);
        if left < self.history_base {
            return Err(SourceSessionError::MissingSourceHistory);
        }
        // Match ValidationContext's inward UTF-8 clipping, across the two borrows.
        while left < start {
            let byte = if left < base {
                self.history.as_bytes().get(left - self.history_base)
            } else {
                text.as_bytes().get(left - base)
            }
            .ok_or(SourceSessionError::MissingSourceHistory)?;
            if byte & 0xc0 != 0x80 {
                break;
            }
            left += 1;
        }
        let length = end - left;
        if length > self.scratch_limit {
            return Err(SourceSessionError::ContextRetentionExceeded);
        }
        self.scratch.clear();
        if left < base {
            let old_end = end.min(base);
            let part = self
                .history
                .get(left - self.history_base..old_end - self.history_base)
                .ok_or(SourceSessionError::MissingSourceHistory)?;
            self.scratch.push_str(part);
        }
        if end > base {
            let part = text
                .get(left.max(base) - base..end - base)
                .ok_or(SourceSessionError::MissingSourceHistory)?;
            self.scratch.push_str(part);
        }
        let local_start = start - left;
        let candidate = self
            .scratch
            .get(local_start..)
            .ok_or(SourceSessionError::MissingSourceHistory)?;
        if candidate != view.text.ok_or(SourceSessionError::MissingCandidateText)? {
            return Err(SourceSessionError::CandidateTextMismatch);
        }
        Ok(local_start)
    }
}

impl ValidatedSourceSession {
    pub(crate) fn new_with_limits(
        rules: Arc<CompiledRuleSet>,
        limits: SourceLimits,
    ) -> Result<Self, SourceSessionError> {
        if !rules.supports_streaming() || !rules.supports_streaming_semantics() {
            return Err(SourceSessionError::UnsupportedRuleSet);
        }
        let plan = rules
            .contextual_storage_plan()
            .map_err(|error| match error {
                ContextStoragePlanningError::ArithmeticOverflow => {
                    SourceSessionError::StreamStorageRequirementOverflow
                }
                ContextStoragePlanningError::UnsupportedRuleSet => {
                    SourceSessionError::UnsupportedRuleSet
                }
            })?;
        if let Some(limit) = limits.max_stream_storage_bytes {
            let matcher = rules
                .streaming_storage_requirements()
                .map_err(|error| match error {
                    StreamingStorageError::ArithmeticOverflow => {
                        SourceSessionError::StreamStorageRequirementOverflow
                    }
                    StreamingStorageError::UnsupportedPattern(_) => {
                        SourceSessionError::UnsupportedRuleSet
                    }
                })?;
            let required = matcher
                .total_buffer_bytes
                .checked_add(plan.persistent_bytes)
                .and_then(|value| value.checked_add(plan.assembly_scratch_bytes))
                .ok_or(SourceSessionError::StreamStorageRequirementOverflow)?;
            if required > limit {
                return Err(SourceSessionError::StreamStorageLimitExceeded { required, limit });
            }
        }
        Ok(Self {
            session: SourceSession::new_with_limits(
                rules,
                SourceLimits {
                    max_stream_storage_bytes: None,
                    ..limits
                },
            )?,
            context: ContextRetention {
                history: String::new(),
                history_base: 0,
                history_limit: plan.history_bytes,
                scratch: String::new(),
                scratch_limit: plan.assembly_scratch_bytes,
                plan,
                pending: Vec::new(),
                #[cfg(test)]
                usage: ContextUsage::default(),
            },
        })
    }

    pub(crate) fn scan_chunk(
        &mut self,
        chunk: &[u8],
        accepted: &mut Vec<ValidatedCandidate>,
    ) -> Result<(), SourceSessionError> {
        let rules = Arc::clone(&self.session.rules);
        let context = &mut self.context;
        self.session
            .scan_chunk_with_context(chunk, &mut |view, text, base| {
                context.resolve_pending(&rules, text, base, false, accepted)?;
                if let Some(view) = view {
                    Self::ingest_context(&rules, context, view, text, base, false, accepted)
                } else {
                    context.retain(text, base)
                }
            })
    }

    pub(crate) fn finish_scan(
        &mut self,
        accepted: &mut Vec<ValidatedCandidate>,
    ) -> Result<(), SourceSessionError> {
        let rules = Arc::clone(&self.session.rules);
        let base = self.session.accepted_bytes();
        let context = &mut self.context;
        self.session.finish_with_context(&mut |view| {
            if let Some(view) = view {
                Self::ingest_context(&rules, context, view, "", base, true, accepted)
            } else {
                context.resolve_pending(&rules, "", base, true, accepted)
            }
        })
    }

    #[cfg(test)]
    fn ingest_before(
        rules: &CompiledRuleSet,
        context: &mut ContextRetention,
        view: CandidateView<'_>,
        text: &str,
        base: usize,
        accepted: &mut Vec<ValidatedCandidate>,
    ) -> Result<(), SourceSessionError> {
        Self::ingest_context(rules, context, view, text, base, false, accepted)
    }

    fn ingest_context(
        rules: &CompiledRuleSet,
        context: &mut ContextRetention,
        view: CandidateView<'_>,
        text: &str,
        base: usize,
        eof: bool,
        accepted: &mut Vec<ValidatedCandidate>,
    ) -> Result<(), SourceSessionError> {
        Self::ingest_with_coordinates(rules, context, view, text, base, eof, (), accepted)
    }

    #[allow(clippy::too_many_arguments)] // Independent matcher/context/output borrows.
    fn ingest_with_coordinates<C: Copy>(
        rules: &CompiledRuleSet,
        context: &mut ContextRetention<C>,
        view: CandidateView<'_>,
        text: &str,
        base: usize,
        eof: bool,
        coordinates: C,
        accepted: &mut impl AcceptedOutput<C>,
    ) -> Result<(), SourceSessionError> {
        let metadata = rules.metadata(view.finding.rule_index());
        if let ValidatorStreamingRequirement::BoundedContext {
            before_bytes,
            after_bytes,
        } = metadata.validator().streaming_requirement()
        {
            let local_start = context.assemble(view, before_bytes, text, base)?;
            let candidate_range = local_start..context.scratch.len();
            let after_end = view
                .finding
                .end()
                .checked_add(after_bytes)
                .ok_or(SourceSessionError::SourceOffsetOverflow)?;
            let available_end = base
                .checked_add(text.len())
                .ok_or(SourceSessionError::SourceOffsetOverflow)?;
            context.append_after(view.finding.end(), after_end.min(available_end), text, base)?;
            if !eof && available_end < after_end {
                let plan = context
                    .plan
                    .rules
                    .iter()
                    .find(|plan| plan.rule_index == view.finding.rule_index())
                    .ok_or(SourceSessionError::InvalidPendingRange)?;
                if context
                    .pending
                    .iter()
                    .filter(|record| record.finding.rule_index() == view.finding.rule_index())
                    .count()
                    >= plan.max_pending_records
                {
                    return Err(SourceSessionError::PendingCapacityExceeded);
                }
                if context.scratch.len() > plan.snapshot_bytes {
                    return Err(SourceSessionError::ContextRetentionExceeded);
                }
                context.pending.push(PendingValidation {
                    finding: view.finding,
                    candidate_range,
                    after_end,
                    text: std::mem::take(&mut context.scratch),
                    coordinates,
                });
                #[cfg(test)]
                context.observe();
                return Ok(());
            }
            if let Some(validation) = validate_candidate(
                metadata.validator(),
                &context.scratch,
                candidate_range,
                metadata.confidence(),
            ) {
                accepted.accept(
                    ValidatedCandidate {
                        finding: view.finding,
                        confidence: validation.confidence(),
                    },
                    coordinates,
                )?;
            }
            // Retain capacity, but not candidate text after synchronous validation.
            context.scratch.clear();
            Ok(())
        } else {
            Self::ingest_into(rules, view, coordinates, accepted)
        }
    }

    fn ingest(
        rules: &CompiledRuleSet,
        view: CandidateView<'_>,
        accepted: &mut Vec<ValidatedCandidate>,
    ) -> Result<(), SourceSessionError> {
        Self::ingest_into(rules, view, (), accepted)
    }

    fn ingest_into<C: Copy>(
        rules: &CompiledRuleSet,
        view: CandidateView<'_>,
        coordinates: C,
        accepted: &mut impl AcceptedOutput<C>,
    ) -> Result<(), SourceSessionError> {
        let metadata = rules.metadata(view.finding.rule_index());
        let text = match metadata.validator().streaming_requirement() {
            ValidatorStreamingRequirement::NoValidation => "",
            ValidatorStreamingRequirement::CandidateOnly => {
                view.text.ok_or(SourceSessionError::MissingCandidateText)?
            }
            ValidatorStreamingRequirement::BoundedContext { .. } => {
                return Err(SourceSessionError::UnsupportedRuleSet);
            }
        };
        if let Some(validation) = validate_candidate(
            metadata.validator(),
            text,
            0..text.len(),
            metadata.confidence(),
        ) {
            accepted.accept(
                ValidatedCandidate {
                    finding: view.finding,
                    confidence: validation.confidence(),
                },
                coordinates,
            )?;
        }
        Ok(())
    }
}

/// Opt-in coordinate capture; output is caller-owned and remains unnormalized.
#[derive(Debug)]
pub(crate) struct AnnotatedSourceSession {
    session: SourceSession,
    context: ContextRetention<SourceCoordinates>,
    coordinates: CoordinateRetention,
    finalized: bool,
}

#[derive(Debug)]
struct CoordinateRetention {
    history: String,
    anchor: SourceCoordinates,
    cursor: SourceCoordinates,
    plan: CoordinateStoragePlan,
    pins: Vec<PlannedPrefixCoordinatePin>,
    // One monotone query cursor per compiled rule. Literal/suffix starts are
    // increasing; regex projections cannot precede the previous complete match
    // end. Prefix completions use pins. No source-sized coordinate index exists.
    rule_positions: Vec<SourceCoordinates>,
    #[cfg(test)]
    advanced_bytes: usize,
}

impl CoordinateRetention {
    fn position(
        &self,
        offset: usize,
        text: &str,
        base: usize,
    ) -> Result<SourceCoordinates, SourceSessionError> {
        if self.cursor.byte_offset() != base
            || self.anchor.byte_offset().checked_add(self.history.len()) != Some(base)
        {
            return Err(SourceSessionError::CoordinateInvariantViolation);
        }
        let end = base
            .checked_add(text.len())
            .ok_or(SourceSessionError::SourceOffsetOverflow)?;
        if offset > end {
            return Err(SourceSessionError::InvalidCoordinateOffset);
        }
        let mut position;
        if offset < base {
            let local = offset
                .checked_sub(self.anchor.byte_offset())
                .ok_or(SourceSessionError::MissingCoordinateHistory)?;
            let prior = self
                .history
                .get(..local)
                .ok_or(SourceSessionError::InvalidCoordinateOffset)?;
            position = self.anchor;
            position.checked_advance(prior)?;
        } else {
            if !text.is_char_boundary(offset - base) {
                return Err(SourceSessionError::InvalidCoordinateOffset);
            }
            position = self.cursor;
            position.checked_advance(&text[..offset - base])?;
        }
        Ok(position)
    }

    /// Replays each callback at most once per rule, independently of hit
    /// density, shared regex reach-back, or interleaved Aho match ordering.
    fn position_for_rule(
        &mut self,
        rule: crate::compiled_rule::RuleIndex,
        offset: usize,
        text: &str,
        base: usize,
    ) -> Result<SourceCoordinates, SourceSessionError> {
        if offset < base {
            return self.position(offset, text, base);
        }
        self.position(base, text, base)?;
        let cursor = self
            .rule_positions
            .get_mut(rule.get())
            .ok_or(SourceSessionError::CoordinateInvariantViolation)?;
        if cursor.byte_offset() < base {
            *cursor = self.cursor;
        }
        if cursor.byte_offset() > offset {
            return Err(SourceSessionError::CoordinateInvariantViolation);
        }
        let segment = text
            .get(cursor.byte_offset() - base..offset - base)
            .ok_or(SourceSessionError::InvalidCoordinateOffset)?;
        cursor.checked_advance(segment)?;
        #[cfg(test)]
        {
            self.advanced_bytes += segment.len();
        }
        Ok(*cursor)
    }

    fn capture_event<'a>(
        &mut self,
        event: StreamingEvent<'a>,
        text: &str,
        base: usize,
    ) -> Result<Option<(CandidateView<'a>, SourceCoordinates)>, SourceSessionError> {
        match event {
            StreamingEvent::PrefixDiscovered { rule_index, start } => {
                if self.pins.len() >= self.plan.max_prefix_pins
                    || self.pins.iter().any(|pin| pin.rule_index == rule_index)
                {
                    return Err(SourceSessionError::CoordinateInvariantViolation);
                }
                let coordinates = self.position_for_rule(rule_index, start, text, base)?;
                self.pins.push(PlannedPrefixCoordinatePin {
                    rule_index,
                    coordinates,
                });
                Ok(None)
            }
            StreamingEvent::Candidate(view) | StreamingEvent::PrefixCompleted(view) => {
                let end = base
                    .checked_add(text.len())
                    .ok_or(SourceSessionError::SourceOffsetOverflow)?;
                if view.finding.start() > view.finding.end() || view.finding.end() > end {
                    return Err(SourceSessionError::InvalidCoordinateOffset);
                }
                let coordinates = if matches!(event, StreamingEvent::PrefixCompleted(_)) {
                    let index = self
                        .pins
                        .iter()
                        .position(|pin| {
                            pin.rule_index == view.finding.rule_index()
                                && pin.coordinates.byte_offset() == view.finding.start()
                        })
                        .ok_or(SourceSessionError::MissingPrefixCoordinatePin)?;
                    self.pins.swap_remove(index).coordinates
                } else {
                    self.position_for_rule(
                        view.finding.rule_index(),
                        view.finding.start(),
                        text,
                        base,
                    )?
                };
                Ok(Some((view, coordinates)))
            }
        }
    }

    /// Retain only a UTF-8 tail. Advancing the anchor needs no replacement copy.
    fn retain(&mut self, text: &str, base: usize) -> Result<(), SourceSessionError> {
        self.position(base, text, base)?;
        let mut next = self.cursor;
        next.checked_advance(text)?;
        if self.plan.history_bytes == 0 {
            self.anchor = next;
        } else {
            let keep = self
                .plan
                .history_bytes
                .checked_sub(3)
                .ok_or(SourceSessionError::CoordinateInvariantViolation)?;
            if text.len() >= keep {
                let mut cut = text.len() - keep;
                while cut > 0 && !text.is_char_boundary(cut) {
                    cut -= 1;
                }
                let mut anchor = self.cursor;
                anchor.checked_advance(&text[..cut])?;
                self.history.clear();
                self.history.push_str(&text[cut..]);
                self.anchor = anchor;
            } else {
                let wanted = keep - text.len();
                let mut cut = self.history.len().saturating_sub(wanted);
                while cut > 0 && !self.history.is_char_boundary(cut) {
                    cut -= 1;
                }
                let mut anchor = self.anchor;
                anchor.checked_advance(&self.history[..cut])?;
                self.history.drain(..cut);
                self.history.push_str(text);
                self.anchor = anchor;
            }
        }
        self.cursor = next;
        if self.history.len() > self.plan.history_bytes
            || self.anchor.byte_offset().checked_add(self.history.len()) != Some(next.byte_offset())
        {
            return Err(SourceSessionError::CoordinateInvariantViolation);
        }
        Ok(())
    }
}

impl AnnotatedSourceSession {
    /// Finalizes once, after successful EOF. Both vectors belong to the caller:
    /// accepted records are normalized in place and public findings are appended.
    /// Records must be the complete accepted collection from this session.
    /// No source text or reorder queue is allocated; caller-owned output
    /// storage is outside executor logical-buffer admission.
    pub(crate) fn finalize_findings(
        &mut self,
        accepted: &mut Vec<AnnotatedCandidate>,
        findings: &mut Vec<Finding>,
    ) -> Result<(), SourceSessionError> {
        if self.session.lifecycle != SourceLifecycle::Completed {
            return Err(SourceSessionError::NotCompleted);
        }
        if self.finalized {
            return Err(SourceSessionError::AlreadyFinalized);
        }
        let rules = &self.session.rules;
        normalize_candidates_by(accepted, |candidate| AcceptedCandidate {
            metadata: rules.metadata(candidate.finding.rule_index()),
            start: candidate.finding.start(),
            end: candidate.finding.end(),
            confidence: candidate.confidence,
        });
        for candidate in accepted.iter() {
            let metadata = rules.metadata(candidate.finding.rule_index());
            let mut location =
                Location::from_span(candidate.finding.start(), candidate.finding.end());
            location.set_position(candidate.line, candidate.column);
            findings.push(Finding::new(
                metadata.id().clone(),
                location,
                metadata.severity(),
                candidate.confidence,
                metadata.remediation(),
            ));
        }
        self.finalized = true;
        Ok(())
    }

    /// Includes actual coordinate payload and annotated pending-record layout.
    /// Excludes spare capacity, allocator overhead and caller-owned accepted output.
    pub(crate) fn storage_requirement(
        rules: &CompiledRuleSet,
        context: &ContextualStoragePlan,
        coordinates: &CoordinateStoragePlan,
    ) -> Result<usize, SourceSessionError> {
        let overflow = SourceSessionError::StreamStorageRequirementOverflow;
        let extra_record = std::mem::size_of::<PendingValidation<SourceCoordinates>>()
            .checked_sub(std::mem::size_of::<PendingValidation>())
            .ok_or(overflow)?;
        let pending_extra = context.rules.iter().try_fold(0usize, |total, rule| {
            rule.max_pending_records
                .checked_mul(extra_record)
                .and_then(|bytes| total.checked_add(bytes))
                .ok_or(overflow)
        })?;
        let matcher = rules
            .streaming_storage_requirements()
            .map_err(|error| match error {
                StreamingStorageError::ArithmeticOverflow => overflow,
                StreamingStorageError::UnsupportedPattern(_) => {
                    SourceSessionError::UnsupportedRuleSet
                }
            })?;
        matcher
            .total_buffer_bytes
            .checked_add(context.persistent_bytes)
            .and_then(|value| value.checked_add(context.assembly_scratch_bytes))
            .and_then(|value| value.checked_add(coordinates.total_bytes))
            .and_then(|value| {
                rules
                    .len()
                    .checked_mul(std::mem::size_of::<SourceCoordinates>())
                    .and_then(|bytes| value.checked_add(bytes))
            })
            .and_then(|value| value.checked_add(pending_extra))
            .and_then(|value| {
                context
                    .rules
                    .len()
                    .checked_mul(std::mem::size_of::<crate::compiled_rule::ContextRulePlan>())
                    .and_then(|bytes| value.checked_add(bytes))
            })
            .and_then(|value| {
                rules
                    .new_pattern_state_metadata_bytes()
                    .and_then(|bytes| value.checked_add(bytes))
            })
            .ok_or(overflow)
    }

    pub(crate) fn required_storage(rules: &CompiledRuleSet) -> Result<usize, SourceSessionError> {
        let coordinates = rules
            .coordinate_storage_plan()
            .map_err(|error| match error {
                CoordinateStoragePlanningError::UnsupportedRuleSet => {
                    SourceSessionError::UnsupportedRuleSet
                }
                CoordinateStoragePlanningError::ArithmeticOverflow => {
                    SourceSessionError::StreamStorageRequirementOverflow
                }
            })?;
        let context = rules
            .contextual_storage_plan()
            .map_err(|error| match error {
                ContextStoragePlanningError::UnsupportedRuleSet => {
                    SourceSessionError::UnsupportedRuleSet
                }
                ContextStoragePlanningError::ArithmeticOverflow => {
                    SourceSessionError::StreamStorageRequirementOverflow
                }
            })?;
        Self::storage_requirement(rules, &context, &coordinates)
    }

    pub(crate) fn new_with_limits(
        rules: Arc<CompiledRuleSet>,
        limits: SourceLimits,
    ) -> Result<Self, SourceSessionError> {
        let coordinates = rules
            .coordinate_storage_plan()
            .map_err(|error| match error {
                CoordinateStoragePlanningError::UnsupportedRuleSet => {
                    SourceSessionError::UnsupportedRuleSet
                }
                CoordinateStoragePlanningError::ArithmeticOverflow => {
                    SourceSessionError::StreamStorageRequirementOverflow
                }
            })?;
        let context = rules
            .contextual_storage_plan()
            .map_err(|error| match error {
                ContextStoragePlanningError::UnsupportedRuleSet => {
                    SourceSessionError::UnsupportedRuleSet
                }
                ContextStoragePlanningError::ArithmeticOverflow => {
                    SourceSessionError::StreamStorageRequirementOverflow
                }
            })?;
        let required = Self::storage_requirement(&rules, &context, &coordinates)?;
        if let Some(limit) = limits.max_stream_storage_bytes
            && required > limit
        {
            return Err(SourceSessionError::StreamStorageLimitExceeded { required, limit });
        }
        // All admission and checked metadata calculations precede execution allocation.
        let rule_positions = vec![SourceCoordinates::default(); rules.len()];
        Ok(Self {
            session: SourceSession::new_with_limits(
                rules,
                SourceLimits {
                    max_stream_storage_bytes: None,
                    ..limits
                },
            )?,
            context: ContextRetention::new(context),
            coordinates: CoordinateRetention {
                history: String::new(),
                anchor: SourceCoordinates::default(),
                cursor: SourceCoordinates::default(),
                plan: coordinates,
                pins: Vec::new(),
                rule_positions,
                #[cfg(test)]
                advanced_bytes: 0,
            },
            finalized: false,
        })
    }

    pub(crate) fn scan_chunk(
        &mut self,
        chunk: &[u8],
        accepted: &mut impl AcceptedOutput<SourceCoordinates>,
    ) -> Result<(), SourceSessionError> {
        let rules = Arc::clone(&self.session.rules);
        let context = &mut self.context;
        let coordinates = &mut self.coordinates;
        let result = self
            .session
            .scan_chunk_with_events(chunk, &mut |event, text, base| {
                context.resolve_pending(&rules, text, base, false, accepted)?;
                if let Some(event) = event {
                    if let Some((view, position)) = coordinates.capture_event(event, text, base)? {
                        ValidatedSourceSession::ingest_with_coordinates(
                            &rules, context, view, text, base, false, position, accepted,
                        )?;
                    }
                } else {
                    accepted.callback_completed(text, base)?;
                    coordinates.retain(text, base)?;
                    context.retain(text, base)?;
                }
                Ok(())
            });
        if result.is_err() {
            self.coordinates.pins.clear();
        }
        result
    }

    pub(crate) fn finish_scan(
        &mut self,
        accepted: &mut impl AcceptedOutput<SourceCoordinates>,
    ) -> Result<(), SourceSessionError> {
        let rules = Arc::clone(&self.session.rules);
        let base = self.session.accepted_bytes();
        let context = &mut self.context;
        let coordinates = &mut self.coordinates;
        let result = self.session.finish_with_events(&mut |event| {
            if let Some(event) = event {
                if let Some((view, position)) = coordinates.capture_event(event, "", base)? {
                    ValidatedSourceSession::ingest_with_coordinates(
                        &rules, context, view, "", base, true, position, accepted,
                    )?;
                }
            } else {
                coordinates.position(base, "", base)?;
                context.resolve_pending(&rules, "", base, true, accepted)?;
            }
            Ok(())
        });
        self.coordinates.pins.clear();
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Rule, Scanner, Severity};

    fn assert_annotated_partitions(rules: Vec<Rule>, source: &str) {
        let mut builder = Scanner::builder();
        for rule in &rules {
            builder = builder.rule(rule.clone());
        }
        let scanner = builder.build().unwrap();
        let compiled = budget_rules(rules);
        let report = scanner.scan([((), source)]);
        // Accepted records precede priority resolution. Compare these against
        // whole-source validation, then compare normalized Finding values with
        // Scanner below, including its ordering and all public fields.
        let mut expected: Vec<_> = contextual_expected(&compiled, source)
            .iter()
            .map(|candidate| {
                let finding = candidate.finding;
                let mut position = SourceCoordinates::default();
                position
                    .checked_advance(&source[..finding.start()])
                    .unwrap();
                (
                    compiled
                        .metadata(finding.rule_index())
                        .id()
                        .as_str()
                        .to_owned(),
                    finding.start(),
                    finding.end(),
                    candidate.confidence,
                    position.line(),
                    position.column(),
                )
            })
            .collect();
        expected.sort();
        for size in [1, 2, 3, 7, 31, source.len().max(1)] {
            let mut session = AnnotatedSourceSession::new_with_limits(
                Arc::clone(&compiled),
                SourceLimits::default(),
            )
            .unwrap();
            let mut validated = ValidatedSourceSession::new_with_limits(
                Arc::clone(&compiled),
                SourceLimits::default(),
            )
            .unwrap();
            let mut raw = SourceSession::new(Arc::clone(&compiled)).unwrap();
            let mut accepted = Vec::new();
            let mut old = Vec::new();
            let mut raw_output = Vec::new();
            session.scan_chunk(b"", &mut accepted).unwrap();
            for chunk in source.as_bytes().chunks(size) {
                session.scan_chunk(chunk, &mut accepted).unwrap();
                validated.scan_chunk(chunk, &mut old).unwrap();
                raw.scan_chunk(chunk, &mut raw_output).unwrap();
                assert!(
                    session.coordinates.history.len() <= session.coordinates.plan.history_bytes
                );
                assert!(session.coordinates.pins.len() <= session.coordinates.plan.max_prefix_pins);
                assert_eq!(
                    session.session.state.emitted_candidates,
                    raw.state.emitted_candidates
                );
            }
            session.finish_scan(&mut accepted).unwrap();
            validated.finish_scan(&mut old).unwrap();
            raw.finish_scan(&mut raw_output).unwrap();
            assert_eq!(session.session.lifecycle, SourceLifecycle::Completed);
            assert!(session.coordinates.pins.is_empty());
            assert!(session.context.pending.is_empty());
            let mut actual: Vec<_> = accepted
                .iter()
                .map(|record| {
                    (
                        compiled
                            .metadata(record.finding.rule_index())
                            .id()
                            .as_str()
                            .to_owned(),
                        record.finding.start(),
                        record.finding.end(),
                        record.confidence,
                        record.line,
                        record.column,
                    )
                })
                .collect();
            actual.sort();
            assert_eq!(actual, expected, "partition {size}");
            let mut new_plain: Vec<_> = accepted
                .iter()
                .map(|record| {
                    (
                        record.finding.rule_index(),
                        record.finding.start(),
                        record.finding.end(),
                        record.confidence,
                    )
                })
                .collect();
            let mut old_plain: Vec<_> = old
                .into_iter()
                .map(|record| {
                    (
                        record.finding.rule_index(),
                        record.finding.start(),
                        record.finding.end(),
                        record.confidence,
                    )
                })
                .collect();
            new_plain.sort();
            old_plain.sort();
            assert_eq!(new_plain, old_plain);
            assert_eq!(session.session.state.emitted_candidates, raw_output.len());
            let mut findings = Vec::new();
            session
                .finalize_findings(&mut accepted, &mut findings)
                .unwrap();
            assert_eq!(
                findings,
                report
                    .findings()
                    .map(|(_, finding)| finding.clone())
                    .collect::<Vec<_>>(),
                "public Finding parity, partition {size}"
            );
            assert_eq!(
                session.scan_chunk(b"x", &mut Vec::new()),
                Err(SourceSessionError::NotActive)
            );
        }
    }

    #[test]
    fn annotated_retention_high_water_stays_within_checked_plans() {
        use crate::validators::dispatch::ValidatorKind;
        let rules = budget_rules(vec![
            Rule::literal("dense", "a", Severity::High).with_validator(ValidatorKind::Maven),
            Rule::captured_pattern("projected", "x(?P<value>a)", "value", Severity::High)
                .unwrap()
                .with_validator(ValidatorKind::Maven),
            Rule::prefix("prefix", "tok_", Severity::High),
        ]);
        let source = format!("tok_{}! {}", "x".repeat(50_000), "xa".repeat(500));
        for size in [1, 7, 127, source.len()] {
            let mut session = AnnotatedSourceSession::new_with_limits(
                Arc::clone(&rules),
                SourceLimits::default(),
            )
            .unwrap();
            let mut accepted = Vec::new();
            let mut coordinate_history = 0;
            let mut pins = 0;
            for chunk in source.as_bytes().chunks(size) {
                session.scan_chunk(chunk, &mut accepted).unwrap();
                coordinate_history = coordinate_history.max(session.coordinates.history.len());
                pins = pins.max(session.coordinates.pins.len());
                assert!(
                    session.coordinates.history.len() <= session.coordinates.plan.history_bytes
                );
                assert!(session.coordinates.pins.len() <= session.coordinates.plan.max_prefix_pins);
            }
            session.finish_scan(&mut accepted).unwrap();
            assert!(session.context.pending.is_empty());
            let usage = &session.context.usage;
            let plan = &session.context.plan;
            assert!(usage.history <= plan.history_bytes);
            assert!(usage.scratch <= plan.assembly_scratch_bytes);
            let mut snapshot_bound = 0;
            for (index, rule) in plan.rules.iter().enumerate() {
                assert!(usage.records_per_rule[index] <= rule.max_pending_records);
                assert!(usage.snapshot_per_rule[index] <= rule.snapshot_bytes);
                snapshot_bound += rule.max_pending_records * rule.snapshot_bytes;
            }
            assert!(usage.snapshots <= snapshot_bound);
            println!(
                "pending per-rule: observed={:?}, planned={:?}",
                usage.records_per_rule,
                plan.rules
                    .iter()
                    .map(|rule| rule.max_pending_records)
                    .collect::<Vec<_>>()
            );
            println!(
                "annotated partition={size}: history={}/{}, scratch={}/{}, snapshots={}/{snapshot_bound}, coordinate_history={coordinate_history}/{}, pins={pins}/{}",
                usage.history,
                plan.history_bytes,
                usage.scratch,
                plan.assembly_scratch_bytes,
                usage.snapshots,
                session.coordinates.plan.history_bytes,
                session.coordinates.plan.max_prefix_pins
            );
        }
    }

    #[test]
    fn dense_coordinate_lookup_uses_per_rule_cursors_and_linear_work() {
        let rules = budget_rules(vec![
            Rule::literal("one", "x", Severity::High),
            Rule::literal("overlap", "xx", Severity::High),
            Rule::pattern("large-bound", "y{100000}", Severity::High).unwrap(),
        ]);
        let mut session =
            AnnotatedSourceSession::new_with_limits(rules, SourceLimits::default()).unwrap();
        let source = "x".repeat(100_000);
        session
            .scan_chunk(source.as_bytes(), &mut Vec::new())
            .unwrap();
        let work = session.coordinates.advanced_bytes;
        println!(
            "coordinate lookup: {work} advanced bytes, source={}, rules={}",
            source.len(),
            session.coordinates.rule_positions.len()
        );
        assert!(work < source.len() * 10, "coordinate replay: {work}");
        assert!(session.coordinates.history.len() <= session.coordinates.plan.history_bytes);
        assert!(session.coordinates.pins.is_empty());
    }

    #[test]
    fn finding_finalization_requires_successful_eof_and_runs_once() {
        let rules = budget_rules(vec![Rule::literal("literal", "value", Severity::High)]);
        let mut session =
            AnnotatedSourceSession::new_with_limits(rules, SourceLimits::default()).unwrap();
        let mut accepted = Vec::new();
        let mut findings = Vec::new();
        assert_eq!(
            session.finalize_findings(&mut accepted, &mut findings),
            Err(SourceSessionError::NotCompleted)
        );
        session.scan_chunk(b"value", &mut accepted).unwrap();
        let prior = accepted.clone();
        assert_eq!(
            session.finalize_findings(&mut accepted, &mut findings),
            Err(SourceSessionError::NotCompleted)
        );
        assert_eq!(accepted, prior);
        assert!(findings.is_empty());
        session.finish_scan(&mut accepted).unwrap();
        // Existing caller-owned output is appended to, not cleared.
        let mut location = Location::from_span(0, 5);
        location.set_position(1, 1);
        let expected = Finding::new(
            "literal".into(),
            location,
            Severity::High,
            crate::Confidence::High,
            None,
        );
        findings.push(expected.clone());
        session
            .finalize_findings(&mut accepted, &mut findings)
            .unwrap();
        assert_eq!(findings, vec![expected.clone(), expected]);
        assert_eq!(
            session.finalize_findings(&mut accepted, &mut findings),
            Err(SourceSessionError::AlreadyFinalized)
        );
        assert_eq!(findings.len(), 2);
        assert_eq!(accepted, prior);
        assert_eq!(
            session.scan_chunk(b"value", &mut accepted),
            Err(SourceSessionError::NotActive)
        );
        assert_eq!(
            session.finish_scan(&mut accepted),
            Err(SourceSessionError::NotActive)
        );
    }

    #[test]
    fn finding_finalization_matches_priority_confidence_and_overlap_semantics() {
        use crate::{Confidence, Remediation, validators::dispatch::ValidatorKind};
        let source = "é\r\npostgres://password:AlphaBravo123";
        let rules = vec![
            Rule::literal("z-high-confidence", "AlphaBravo123", Severity::Low)
                .with_validator(ValidatorKind::DatabaseConnection)
                .with_remediation(Remediation::RemoveSensitiveValue),
            Rule::literal("a-medium-confidence", "AlphaBravo123", Severity::Critical)
                .with_validator(ValidatorKind::Password),
            Rule::literal("lower-priority", "AlphaBravo123", Severity::Critical),
            Rule::literal("partial-overlap", "AlphaBravo", Severity::High),
        ];
        let mut builder = Scanner::builder();
        for rule in &rules {
            builder = builder.rule(rule.clone());
        }
        let report = builder.build().unwrap().scan([((), source)]);
        let expected: Vec<_> = report
            .findings()
            .map(|(_, finding)| finding.clone())
            .collect();
        assert_eq!(
            expected
                .iter()
                .map(|finding| finding.rule_id().as_str())
                .collect::<Vec<_>>(),
            [
                "partial-overlap",
                "z-high-confidence",
                "a-medium-confidence"
            ]
        );
        assert_eq!(expected[1].confidence(), Confidence::High);
        assert_eq!(expected[2].confidence(), Confidence::Medium);
        assert_eq!(
            expected[1].remediation(),
            Some(Remediation::RemoveSensitiveValue)
        );
        assert_annotated_partitions(rules, source);
        // Exercise actual removal in the caller's accepted vector, independently
        // of the pre-normalization parity helper above.
        let rules = budget_rules(vec![
            Rule::literal("generic", "AlphaBravo123", Severity::Critical),
            Rule::literal("validated", "AlphaBravo123", Severity::Low)
                .with_validator(ValidatorKind::Password),
        ]);
        let mut session =
            AnnotatedSourceSession::new_with_limits(rules, SourceLimits::default()).unwrap();
        let mut accepted = Vec::new();
        session
            .scan_chunk(b"password=AlphaBravo123", &mut accepted)
            .unwrap();
        session.finish_scan(&mut accepted).unwrap();
        assert_eq!(accepted.len(), 2);
        let mut findings = Vec::new();
        session
            .finalize_findings(&mut accepted, &mut findings)
            .unwrap();
        assert_eq!(accepted.len(), 1);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].rule_id().as_str(), "validated");
    }

    #[test]
    fn finding_finalization_preserves_equal_priority_and_all_sort_keys() {
        let rules = vec![
            Rule::literal("z-high", "abc", Severity::High),
            Rule::literal("low", "abc", Severity::Low),
            Rule::literal("a-high", "abc", Severity::High),
            Rule::literal("short", "ab", Severity::Low),
            Rule::literal("overlap", "bc", Severity::Critical),
        ];
        let mut builder = Scanner::builder();
        for rule in &rules {
            builder = builder.rule(rule.clone());
        }
        let report = builder.build().unwrap().scan([((), "abc")]);
        assert_eq!(
            report
                .findings()
                .map(|(_, finding)| finding.rule_id().as_str())
                .collect::<Vec<_>>(),
            ["short", "a-high", "z-high", "low", "overlap"]
        );
        assert_annotated_partitions(rules, "abc\r\n€abc");
    }

    #[test]
    fn finding_finalization_does_not_deduplicate_identical_records() {
        let mut session = AnnotatedSourceSession::new_with_limits(
            budget_rules(vec![Rule::literal("same", "abc", Severity::High)]),
            SourceLimits::default(),
        )
        .unwrap();
        let mut accepted = Vec::new();
        session.scan_chunk(b"abc", &mut accepted).unwrap();
        session.finish_scan(&mut accepted).unwrap();
        accepted.push(accepted[0]);
        let mut findings = Vec::new();
        session
            .finalize_findings(&mut accepted, &mut findings)
            .unwrap();
        assert_eq!(accepted.len(), 2);
        assert_eq!(findings.len(), 2);
        assert_eq!(findings[0], findings[1]);
    }

    #[test]
    fn finding_finalization_candidate_only_projection_suffix_and_eof_parity() {
        use crate::validators::dispatch::ValidatorKind;
        let iban = "DE89370400440532013000";
        assert_annotated_partitions(
            vec![
                Rule::literal("iban", iban, Severity::Low).with_validator(ValidatorKind::Iban),
                Rule::suffix("suffix", "3000", Severity::High).with_validator(ValidatorKind::Iban),
                Rule::captured_pattern(
                    "capture",
                    r"KEY=(?P<value>DE[0-9]{20})",
                    "value",
                    Severity::Critical,
                )
                .unwrap()
                .with_validator(ValidatorKind::Iban),
                Rule::pattern("eof", r"DE[0-9]{20}\z", Severity::High)
                    .unwrap()
                    .with_validator(ValidatorKind::Iban),
                Rule::literal("rejected", "invalid-iban", Severity::High)
                    .with_validator(ValidatorKind::Iban),
            ],
            &format!("é\rKEY={iban}!\ninvalid-iban\r\n{iban}"),
        );
    }

    #[test]
    fn annotated_matcher_coordinates_match_scanner_across_utf8_partitions() {
        let rules = vec![
            Rule::literal("literal", "éX", Severity::High),
            Rule::literal("newline", "\r\n", Severity::High),
            Rule::suffix("suffix", "_end", Severity::High),
            Rule::pattern("delayed", r"A[0-9]{1,3}B", Severity::High).unwrap(),
            Rule::pattern("lf", r"(?m:^lf$)", Severity::High).unwrap(),
            Rule::pattern("crlf", r"(?mR:^line$)", Severity::High).unwrap(),
            Rule::captured_pattern(
                "capture",
                r"key=(?P<value>é[0-9]{1,2})",
                "value",
                Severity::High,
            )
            .unwrap(),
            Rule::captured_pattern("empty", r"q(?P<value>)", "value", Severity::High).unwrap(),
            Rule::captured_pattern("eof", r"tail(?P<value>)\z", "value", Severity::High).unwrap(),
        ];
        assert_annotated_partitions(
            rules,
            "a€éX\r\néX\r lf\nlf\nline\r\nA123B key=é12 q token_end! final_end tail",
        );
        assert_annotated_partitions(vec![], "");
    }

    #[test]
    fn annotated_contextual_coordinates_survive_deferred_validation_and_eof() {
        use crate::validators::dispatch::ValidatorKind;
        let value = "CorrectHorseBatteryStaple";
        let rules = vec![
            Rule::literal("maven", value, Severity::High).with_validator(ValidatorKind::Maven),
            Rule::suffix("suffix", "Staple", Severity::High).with_validator(ValidatorKind::Maven),
            Rule::captured_pattern(
                "projected",
                r"<password>(?P<value>CorrectHorseBatteryStaple)</password>",
                "value",
                Severity::High,
            )
            .unwrap()
            .with_validator(ValidatorKind::Maven),
            Rule::literal("before", "AlphaBravo123", Severity::High)
                .with_validator(ValidatorKind::Password),
            Rule::literal("iban", "GB82WEST12345698765432", Severity::High)
                .with_validator(ValidatorKind::Iban),
        ];
        let source = format!(
            "é🦀\r\npassword=AlphaBravo123\n<servers><server><password>{value}</password></server></servers>\nGB82WEST12345698765432"
        );
        assert_annotated_partitions(rules, &source);
        let compiled = budget_rules(vec![
            Rule::literal("maven", value, Severity::High).with_validator(ValidatorKind::Maven),
        ]);
        let mut session =
            AnnotatedSourceSession::new_with_limits(compiled, SourceLimits::default()).unwrap();
        let mut accepted = Vec::new();
        session
            .scan_chunk(
                format!("é\n<servers><server>{value}").as_bytes(),
                &mut accepted,
            )
            .unwrap();
        assert!(accepted.is_empty());
        assert_eq!(session.context.pending.len(), 1);
        let pinned = session.context.pending[0].coordinates;
        session
            .scan_chunk(b"</server></servers>\n\n", &mut accepted)
            .unwrap();
        session.finish_scan(&mut accepted).unwrap();
        assert_eq!(
            (accepted[0].line, accepted[0].column),
            (pinned.line(), pinned.column())
        );
    }

    #[test]
    fn annotated_prefix_pins_outlive_history_and_reset_at_boundaries() {
        let rules = vec![
            Rule::prefix("prefix", "tok_", Severity::High),
            Rule::prefix("overlap", "tok", Severity::High),
        ];
        let source = format!(
            "é\r\n{}!\ntok_second",
            "tok_".to_owned() + &"a".repeat(8000)
        );
        assert_annotated_partitions(rules.clone(), &source);
        let mut session =
            AnnotatedSourceSession::new_with_limits(budget_rules(rules), SourceLimits::default())
                .unwrap();
        let mut accepted = Vec::new();
        session
            .scan_chunk("é\r\ntok_".as_bytes(), &mut accepted)
            .unwrap();
        assert_eq!(session.coordinates.pins.len(), 2);
        for _ in 0..20 {
            session.scan_chunk(&[b'a'; 512], &mut accepted).unwrap();
        }
        assert!(session.coordinates.anchor.byte_offset() > 5);
        assert_eq!(session.coordinates.pins.len(), 2);
        assert!(
            session
                .coordinates
                .pins
                .iter()
                .all(|pin| (pin.coordinates.line(), pin.coordinates.column()) == (2, 1))
        );
        session.scan_chunk(b"!", &mut accepted).unwrap();
        assert!(session.coordinates.pins.is_empty());
        session.scan_chunk(b"\ntok_last", &mut accepted).unwrap();
        assert_eq!(session.coordinates.pins.len(), 2);
        session.finish_scan(&mut accepted).unwrap();
        assert!(session.coordinates.pins.is_empty());
        assert_eq!(
            (
                accepted.last().unwrap().line,
                accepted.last().unwrap().column
            ),
            (3, 1)
        );
    }

    #[test]
    fn annotated_large_callback_retains_only_planned_tail() {
        let source = format!("{}éX\r\n{}éX", "🦀".repeat(20_000), " ".repeat(20_000));
        let rule = Rule::literal("literal", "éX", Severity::High);
        let scanner = Scanner::builder().rule(rule.clone()).build().unwrap();
        let mut session = AnnotatedSourceSession::new_with_limits(
            budget_rules(vec![rule]),
            SourceLimits::default(),
        )
        .unwrap();
        let mut accepted = Vec::new();
        session
            .scan_chunk(source.as_bytes(), &mut accepted)
            .unwrap();
        assert!(session.coordinates.history.len() <= session.coordinates.plan.history_bytes);
        assert!(session.coordinates.history.len() < 10);
        session.finish_scan(&mut accepted).unwrap();
        for (actual, (_, expected)) in accepted
            .iter()
            .zip(scanner.scan([((), source.as_str())]).findings())
        {
            assert_eq!(
                (actual.line, actual.column),
                (expected.location().line(), expected.location().column())
            );
        }
        let mut findings = Vec::new();
        session
            .finalize_findings(&mut accepted, &mut findings)
            .unwrap();
        assert_eq!(
            findings,
            scanner
                .scan([((), source.as_str())])
                .findings()
                .map(|(_, finding)| finding.clone())
                .collect::<Vec<_>>()
        );
    }

    fn assert_annotated_terminal(
        session: &mut AnnotatedSourceSession,
        accepted: &mut Vec<AnnotatedCandidate>,
    ) {
        assert_eq!(session.session.lifecycle, SourceLifecycle::Failed);
        assert!(session.coordinates.pins.is_empty());
        let prior = accepted.clone();
        let mut findings = Vec::new();
        assert_eq!(
            session.finalize_findings(accepted, &mut findings),
            Err(SourceSessionError::NotCompleted)
        );
        assert!(findings.is_empty());
        assert_eq!(*accepted, prior);
        assert_eq!(
            session.scan_chunk(b"more", accepted),
            Err(SourceSessionError::NotActive)
        );
        assert_eq!(
            session.finish_scan(accepted),
            Err(SourceSessionError::NotActive)
        );
        assert_eq!(*accepted, prior);
    }

    #[test]
    fn annotated_checked_overflows_and_partial_output_are_terminal() {
        for (cursor, text, error) in [
            (
                SourceCoordinates::from_parts(0, usize::MAX, 1),
                b"\n".as_slice(),
                SourceSessionError::CoordinateOverflow(CoordinateOverflow::Line),
            ),
            (
                SourceCoordinates::from_parts(0, 1, usize::MAX),
                b"x".as_slice(),
                SourceSessionError::CoordinateOverflow(CoordinateOverflow::Column),
            ),
            (
                SourceCoordinates::from_parts(usize::MAX, 1, 1),
                b"x".as_slice(),
                SourceSessionError::SourceOffsetOverflow,
            ),
        ] {
            let mut session = AnnotatedSourceSession::new_with_limits(
                budget_rules(vec![]),
                SourceLimits::default(),
            )
            .unwrap();
            session.session.state.accepted_bytes = cursor.byte_offset();
            session.coordinates.cursor = cursor;
            session.coordinates.anchor = cursor;
            let mut accepted = Vec::new();
            assert_eq!(session.scan_chunk(text, &mut accepted), Err(error));
            assert_annotated_terminal(&mut session, &mut accepted);
        }
        let mut session = AnnotatedSourceSession::new_with_limits(
            budget_rules(vec![Rule::literal("x", "x", Severity::High)]),
            SourceLimits::default(),
        )
        .unwrap();
        let cursor = SourceCoordinates::from_parts(0, 1, usize::MAX);
        session.coordinates.cursor = cursor;
        session.coordinates.anchor = cursor;
        let mut accepted = Vec::new();
        assert_eq!(
            session.scan_chunk(b"x", &mut accepted),
            Err(SourceSessionError::CoordinateOverflow(
                CoordinateOverflow::Column
            ))
        );
        assert_eq!(accepted.len(), 1); // Partial caller-owned output is retained.
        assert_annotated_terminal(&mut session, &mut accepted);
    }

    #[test]
    fn annotated_missing_history_and_prefix_pins_fail_explicitly() {
        for eof in [false, true] {
            let rule = if eof {
                Rule::pattern("regex", r"abc\z", Severity::High).unwrap()
            } else {
                Rule::literal("literal", "abc", Severity::High)
            };
            let mut session = AnnotatedSourceSession::new_with_limits(
                budget_rules(vec![rule]),
                SourceLimits::default(),
            )
            .unwrap();
            let mut accepted = Vec::new();
            session
                .scan_chunk(if eof { b"abc" } else { b"ab" }, &mut accepted)
                .unwrap();
            assert!(accepted.is_empty());
            session.coordinates.history.clear();
            session.coordinates.anchor = session.coordinates.cursor;
            let result = if eof {
                session.finish_scan(&mut accepted)
            } else {
                session.scan_chunk(b"c", &mut accepted)
            };
            assert_eq!(result, Err(SourceSessionError::MissingCoordinateHistory));
            assert_annotated_terminal(&mut session, &mut accepted);
        }
        for eof in [false, true] {
            let mut session = AnnotatedSourceSession::new_with_limits(
                budget_rules(vec![Rule::prefix("prefix", "tok_", Severity::High)]),
                SourceLimits::default(),
            )
            .unwrap();
            let mut accepted = Vec::new();
            session.scan_chunk(b"tok_open", &mut accepted).unwrap();
            session.coordinates.pins.clear();
            let result = if eof {
                session.finish_scan(&mut accepted)
            } else {
                session.scan_chunk(b"!", &mut accepted)
            };
            assert_eq!(result, Err(SourceSessionError::MissingPrefixCoordinatePin));
            assert_annotated_terminal(&mut session, &mut accepted);
        }
    }

    #[test]
    fn annotated_invalid_offsets_and_coordinate_invariants_fail_explicitly() {
        let mut session = AnnotatedSourceSession::new_with_limits(
            budget_rules(vec![Rule::literal("literal", "é", Severity::High)]),
            SourceLimits::default(),
        )
        .unwrap();
        let coordinates = &mut session.coordinates;
        assert_eq!(
            session
                .session
                .scan_chunk_with_events("é".as_bytes(), &mut |event, text, base| {
                    if let Some(StreamingEvent::Candidate(view)) = event {
                        coordinates.capture_event(StreamingEvent::Candidate(view), text, base)?;
                        coordinates.position(1, text, base)?;
                    }
                    Ok(())
                }),
            Err(SourceSessionError::InvalidCoordinateOffset)
        );
        assert_annotated_terminal(&mut session, &mut Vec::new());
        let mut session =
            AnnotatedSourceSession::new_with_limits(budget_rules(vec![]), SourceLimits::default())
                .unwrap();
        session.coordinates.cursor = SourceCoordinates::from_parts(1, 1, 1);
        assert_eq!(
            session.scan_chunk(b"x", &mut Vec::new()),
            Err(SourceSessionError::CoordinateInvariantViolation)
        );
        assert_annotated_terminal(&mut session, &mut Vec::new());
    }

    #[test]
    fn annotated_budget_utf8_and_execution_error_precedence_are_preserved() {
        use crate::validators::dispatch::ValidatorKind;
        let rules = budget_rules(vec![
            Rule::literal("rejected", "bad", Severity::High).with_validator(ValidatorKind::Iban),
        ]);
        let mut session = AnnotatedSourceSession::new_with_limits(
            rules,
            SourceLimits {
                max_candidates: Some(1),
                ..SourceLimits::default()
            },
        )
        .unwrap();
        let mut accepted = Vec::new();
        session.scan_chunk(b"bad", &mut accepted).unwrap();
        assert!(accepted.is_empty());
        assert_eq!(
            session.scan_chunk(b"bad", &mut accepted),
            Err(SourceSessionError::ResourceLimitExceeded)
        );
        assert_eq!(session.session.state.emitted_candidates, 1);
        assert_annotated_terminal(&mut session, &mut accepted);
        for budget in [None, Some(0)] {
            let rules = budget_rules(vec![Rule::literal("literal", "€", Severity::High)]);
            let mut session = AnnotatedSourceSession::new_with_limits(
                rules,
                SourceLimits {
                    max_candidates: budget,
                    ..SourceLimits::default()
                },
            )
            .unwrap();
            let cursor = SourceCoordinates::from_parts(0, 1, usize::MAX);
            session.coordinates.cursor = cursor;
            session.coordinates.anchor = cursor;
            session.scan_chunk(&[0xe2, 0x82], &mut Vec::new()).unwrap();
            let error = if budget.is_some() {
                SourceSessionError::ResourceLimitExceeded
            } else {
                SourceSessionError::CoordinateOverflow(CoordinateOverflow::Column)
            };
            assert_eq!(
                session.scan_chunk(&[0xac, 0xff], &mut Vec::new()),
                Err(error)
            );
            assert_annotated_terminal(&mut session, &mut Vec::new());
        }
        for eof in [false, true] {
            let mut session = AnnotatedSourceSession::new_with_limits(
                budget_rules(vec![]),
                SourceLimits::default(),
            )
            .unwrap();
            let error = if eof {
                session.scan_chunk(&[0xe2], &mut Vec::new()).unwrap();
                session.finish_scan(&mut Vec::new())
            } else {
                session.scan_chunk(&[0xff], &mut Vec::new())
            };
            assert!(matches!(error, Err(SourceSessionError::InvalidUtf8(_))));
            assert_annotated_terminal(&mut session, &mut Vec::new());
        }
    }

    #[test]
    fn annotated_storage_admission_includes_history_pins_and_deferred_coordinates() {
        use crate::validators::dispatch::ValidatorKind;
        let rules = budget_rules(vec![
            Rule::prefix("prefix", "tok_", Severity::High),
            Rule::literal("context", "CorrectHorseBatteryStaple", Severity::High)
                .with_validator(ValidatorKind::Maven),
        ]);
        let context = rules.contextual_storage_plan().unwrap();
        let mut coordinates = rules.coordinate_storage_plan().unwrap();
        let required =
            AnnotatedSourceSession::storage_requirement(&rules, &context, &coordinates).unwrap();
        let original = rules
            .streaming_storage_requirements()
            .unwrap()
            .total_buffer_bytes
            + context.persistent_bytes
            + context.assembly_scratch_bytes;
        let extra_metadata: usize = context
            .rules
            .iter()
            .map(|rule| {
                rule.max_pending_records
                    * (std::mem::size_of::<PendingValidation<SourceCoordinates>>()
                        - std::mem::size_of::<PendingValidation>())
            })
            .sum();
        assert_eq!(
            required,
            original
                + coordinates.total_bytes
                + extra_metadata
                + rules.len() * std::mem::size_of::<SourceCoordinates>()
                + context.rules.len()
                    * std::mem::size_of::<crate::compiled_rule::ContextRulePlan>()
                + rules.new_pattern_state_metadata_bytes().unwrap()
        );
        assert!(required > original);
        let mut exact = AnnotatedSourceSession::new_with_limits(
            Arc::clone(&rules),
            SourceLimits {
                max_candidates: Some(1),
                max_stream_storage_bytes: Some(required),
            },
        )
        .unwrap();
        let mut accepted = Vec::new();
        exact.scan_chunk(b"tok_value", &mut accepted).unwrap();
        exact.finish_scan(&mut accepted).unwrap();
        assert_eq!(accepted.len(), 1);
        assert!(
            matches!(AnnotatedSourceSession::new_with_limits(Arc::clone(&rules), SourceLimits { max_stream_storage_bytes: Some(required - 1), ..SourceLimits::default() }), Err(SourceSessionError::StreamStorageLimitExceeded { required: value, limit }) if value == required && limit == required - 1)
        );
        ValidatedSourceSession::new_with_limits(
            Arc::clone(&rules),
            SourceLimits {
                max_stream_storage_bytes: Some(original),
                ..SourceLimits::default()
            },
        )
        .unwrap();
        SourceSession::new_with_limits(
            Arc::clone(&rules),
            SourceLimits {
                max_stream_storage_bytes: Some(
                    rules
                        .streaming_storage_requirements()
                        .unwrap()
                        .total_buffer_bytes,
                ),
                ..SourceLimits::default()
            },
        )
        .unwrap();
        coordinates.total_bytes = usize::MAX;
        assert_eq!(
            AnnotatedSourceSession::storage_requirement(&rules, &context, &coordinates),
            Err(SourceSessionError::StreamStorageRequirementOverflow)
        );
        let unsupported = budget_rules(vec![
            Rule::pattern("unbounded", "a+", Severity::High).unwrap(),
        ]);
        assert!(matches!(
            AnnotatedSourceSession::new_with_limits(unsupported, SourceLimits::default()),
            Err(SourceSessionError::UnsupportedRuleSet)
        ));
    }

    #[test]
    fn audit_empty_complete_capture_requires_explicit_rejection() {
        for pattern in ["(?P<value>)", "(?P<value>a?)", r"(?P<value>\b)"] {
            assert!(matches!(
                Rule::captured_pattern("empty", pattern, "value", Severity::High),
                Err(crate::RuleError::PatternMatchesEmpty)
            ));
        }
    }

    #[test]
    fn audit_scanner_oracle_chunking_and_context_high_water_marks() {
        use crate::validators::dispatch::ValidatorKind;
        let password = "AlphaBravo123";
        let maven = "CorrectHorseBatteryStaple";
        let bearer = "ComposerBearerToken_123456";
        let rules = vec![
            Rule::literal("opaque", "opaque", Severity::High),
            Rule::prefix("prefix", "tok_", Severity::High),
            Rule::literal("password-shadow", password, Severity::Low),
            Rule::literal("password", password, Severity::High)
                .with_validator(ValidatorKind::Password),
            Rule::suffix("suffix", "123", Severity::High).with_validator(ValidatorKind::Password),
            Rule::literal("iban", "GB82WEST12345698765432", Severity::High)
                .with_validator(ValidatorKind::Iban),
            Rule::captured_pattern(
                "maven",
                r"<password>(?P<value>CorrectHorseBatteryStaple)</password>",
                "value",
                Severity::High,
            )
            .unwrap()
            .with_validator(ValidatorKind::Maven),
            Rule::literal("composer", bearer, Severity::High)
                .with_validator(ValidatorKind::ComposerBearerToken),
            Rule::literal("swift", "ZuluDelta456", Severity::High)
                .with_validator(ValidatorKind::SwiftPm),
            Rule::captured_pattern("eof", r"(?P<value>DeltaEcho456)\z", "value", Severity::High)
                .unwrap()
                .with_validator(ValidatorKind::Password),
            // A zero-width projected capture with a non-empty complete match is safe.
            Rule::captured_pattern("empty-projection", r"o(?P<value>)", "value", Severity::High)
                .unwrap(),
        ];
        let mut builder = Scanner::builder();
        for rule in &rules {
            builder = builder.rule(rule.clone());
        }
        let scanner = builder.build().unwrap();
        let compiled = budget_rules(rules);
        let fixture = format!(
            "é opaque tok_token password={password}! GB82WEST12345698765432\n<servers><server><password>{maven}</password></server></servers>\n{{\"bearer\":{{\"repo.example\":\"{bearer}\"}}}}\nSWIFTPM_REGISTRY_TOKEN=ZuluDelta456\npassword=DeltaEcho456"
        );
        for (label, source) in [
            ("small", fixture.clone()),
            (
                "large",
                format!(
                    "{}{}\n{}\npassword=DeltaEcho456",
                    "é".repeat(50_000),
                    fixture,
                    " ".repeat(20_000)
                ),
            ),
        ] {
            let report = scanner.scan([((), source.as_str())]); // delegates to Scanner::scan_source
            let mut oracle: Vec<_> = report
                .findings()
                .map(|(_, finding)| {
                    (
                        finding.rule_id().as_str().to_owned(),
                        finding.location().start(),
                        finding.location().end(),
                        finding.confidence(),
                    )
                })
                .collect();
            oracle.sort();
            let expected = contextual_expected(&compiled, &source);
            let mut raw = Vec::new();
            compiled.scan(&source, &mut raw);
            for (partition, sizes) in [
                ("single", vec![source.len()]),
                ("bytes", vec![1]),
                ("irregular", vec![1, 13, 2, 4093, 3, 7]),
            ] {
                let plan = compiled.contextual_storage_plan().unwrap();
                let required = compiled
                    .streaming_storage_requirements()
                    .unwrap()
                    .total_buffer_bytes
                    + plan.persistent_bytes
                    + plan.assembly_scratch_bytes;
                let mut session = ValidatedSourceSession::new_with_limits(
                    Arc::clone(&compiled),
                    SourceLimits {
                        max_candidates: Some(raw.len()),
                        max_stream_storage_bytes: Some(required),
                    },
                )
                .unwrap();
                let mut accepted = Vec::new();
                let mut offset = 0;
                let mut step = 0;
                while offset < source.len() {
                    let end = (offset + sizes[step % sizes.len()]).min(source.len());
                    session
                        .scan_chunk(&source.as_bytes()[offset..end], &mut accepted)
                        .unwrap();
                    offset = end;
                    step += 1;
                }
                session.finish_scan(&mut accepted).unwrap();
                assert!(session.context.pending.is_empty());
                assert_eq!(session.session.state.emitted_candidates, raw.len());
                accepted.sort_by_key(|candidate| {
                    (
                        candidate.finding.rule_index(),
                        candidate.finding.start(),
                        candidate.finding.end(),
                    )
                });
                assert_eq!(accepted, expected, "{label}/{partition}");
                // Normalization suppresses lower-priority exact-span candidates,
                // preserving equal-priority and partially overlapping records.
                let mut normalized: Vec<_> = accepted
                    .iter()
                    .filter(|candidate| {
                        !accepted.iter().any(|other| {
                            other.finding.start() == candidate.finding.start()
                                && other.finding.end() == candidate.finding.end()
                                && compiled.metadata(other.finding.rule_index()).priority()
                                    > compiled.metadata(candidate.finding.rule_index()).priority()
                        })
                    })
                    .map(|candidate| {
                        (
                            compiled
                                .metadata(candidate.finding.rule_index())
                                .id()
                                .as_str()
                                .to_owned(),
                            candidate.finding.start(),
                            candidate.finding.end(),
                            candidate.confidence,
                        )
                    })
                    .collect();
                normalized.sort();
                assert_eq!(normalized, oracle, "{label}/{partition}");
                let usage = &session.context.usage;
                assert!(usage.history <= plan.history_bytes);
                assert!(usage.scratch <= plan.assembly_scratch_bytes);
                let snapshot_bound: usize = plan
                    .rules
                    .iter()
                    .map(|rule| rule.max_pending_records * rule.snapshot_bytes)
                    .sum();
                assert!(usage.snapshots <= snapshot_bound);
                println!(
                    "pending per-rule: observed={:?}, planned={:?}",
                    usage.records_per_rule,
                    plan.rules
                        .iter()
                        .map(|rule| rule.max_pending_records)
                        .collect::<Vec<_>>()
                );
                for (index, rule) in plan.rules.iter().enumerate() {
                    assert!(usage.records_per_rule[index] <= rule.max_pending_records);
                    assert!(usage.snapshot_per_rule[index] <= rule.snapshot_bytes);
                }
                eprintln!(
                    "context audit {label}/{partition}: history={}/{}, scratch={}/{}, snapshots={}/{}, records={:?}, per_snapshot={:?}",
                    usage.history,
                    plan.history_bytes,
                    usage.scratch,
                    plan.assembly_scratch_bytes,
                    usage.snapshots,
                    snapshot_bound,
                    usage.records_per_rule,
                    usage.snapshot_per_rule
                );
            }
        }
    }

    #[test]
    fn audit_projected_empty_and_delayed_capture_pending_cardinality() {
        use crate::validators::dispatch::ValidatorKind;
        for (pattern, source) in [
            (r"(?P<value>)a", "a".repeat(4100)),
            (r"(?P<value>a)bbb", "abbb".repeat(1100)),
        ] {
            let rule = Rule::captured_pattern("capture", pattern, "value", Severity::High)
                .unwrap()
                .with_validator(ValidatorKind::Maven);
            let rules = budget_rules(vec![rule]);
            let mut raw = Vec::new();
            rules.scan(&source, &mut raw);
            let expected = contextual_expected(&rules, &source);
            for size in [1, 17, source.len()] {
                let mut session = ValidatedSourceSession::new_with_limits(
                    Arc::clone(&rules),
                    SourceLimits {
                        max_candidates: Some(raw.len()),
                        ..SourceLimits::default()
                    },
                )
                .unwrap();
                let mut accepted = Vec::new();
                for chunk in source.as_bytes().chunks(size) {
                    session.scan_chunk(chunk, &mut accepted).unwrap();
                }
                session.finish_scan(&mut accepted).unwrap();
                assert_eq!(accepted, expected);
                assert_eq!(session.session.state.emitted_candidates, raw.len());
                let plan = &session.context.plan.rules[0];
                assert!(session.context.usage.records_per_rule[0] <= plan.max_pending_records);
                assert!(session.context.usage.snapshot_per_rule[0] <= plan.snapshot_bytes);
                eprintln!(
                    "capture audit {pattern}/{size}: records={}/{}, snapshot={}/{}",
                    session.context.usage.records_per_rule[0],
                    plan.max_pending_records,
                    session.context.usage.snapshot_per_rule[0],
                    plan.snapshot_bytes
                );
                // Raw enumeration is independently checked, not inferred from rejected validation output.
                let mut raw_session = SourceSession::new(Arc::clone(&rules)).unwrap();
                let mut observed = Vec::new();
                for chunk in source.as_bytes().chunks(size) {
                    raw_session.scan_chunk(chunk, &mut observed).unwrap();
                }
                raw_session.finish_scan(&mut observed).unwrap();
                assert_eq!(observed, raw);
            }
        }
    }

    #[test]
    fn audit_partial_source_windows_match_scanner_decisions() {
        use crate::validators::dispatch::ValidatorKind;
        let value = "AlphaBravo123";
        let cases = [
            (ValidatorKind::Password, format!("password={value}")),
            (
                ValidatorKind::Password,
                format!("password={} {value}", "x".repeat(160)),
            ),
            (
                ValidatorKind::Password,
                format!("€{}password={value}", " ".repeat(150)),
            ),
            (
                ValidatorKind::Maven,
                format!("<servers><server>{value}</server></servers>"),
            ),
            (
                ValidatorKind::Maven,
                format!(
                    "<servers><server>{}{value}</server></servers>",
                    " ".repeat(4096)
                ),
            ),
            (
                ValidatorKind::Maven,
                format!(
                    "<servers><server>{value}{}€</server></servers>",
                    " ".repeat(4095)
                ),
            ),
            (ValidatorKind::Maven, value.to_owned()),
            (
                ValidatorKind::ComposerBearerToken,
                format!(r#"{{"bearer":{{"repo.example":"{value}"}}}}"#),
            ),
            (
                ValidatorKind::ComposerBearerToken,
                format!(
                    r#"{{"padding":"{}","bearer":{{"repo.example":"{value}"}}}}"#,
                    "x".repeat(16_384)
                ),
            ),
            (
                ValidatorKind::ComposerBearerToken,
                format!(
                    r#"{{"bearer":{{"repo.example":"{value}{}"}}}}"#,
                    "x".repeat(16_384)
                ),
            ),
        ];
        for (kind, source) in cases {
            let rule = Rule::literal("window", value, Severity::High).with_validator(kind);
            let compiled = budget_rules(vec![rule.clone()]);
            let scanner = Scanner::builder().rule(rule).build().unwrap();
            let report = scanner.scan([((), source.as_str())]);
            let expected = contextual_expected(&compiled, &source);
            let actual: Vec<_> = report
                .findings()
                .map(|(_, finding)| {
                    (
                        finding.location().start(),
                        finding.location().end(),
                        finding.confidence(),
                    )
                })
                .collect();
            assert_eq!(
                actual,
                expected
                    .iter()
                    .map(|record| (
                        record.finding.start(),
                        record.finding.end(),
                        record.confidence
                    ))
                    .collect::<Vec<_>>()
            );
            assert_context_partitions(compiled, &source);
        }
    }

    fn contextual_expected(rules: &CompiledRuleSet, source: &str) -> Vec<ValidatedCandidate> {
        let mut raw = Vec::new();
        rules.scan(source, &mut raw);
        let mut accepted: Vec<_> = raw
            .into_iter()
            .filter_map(|finding| {
                let metadata = rules.metadata(finding.rule_index());
                validate_candidate(
                    metadata.validator(),
                    source,
                    finding.start()..finding.end(),
                    metadata.confidence(),
                )
                .map(|validation| ValidatedCandidate {
                    finding,
                    confidence: validation.confidence(),
                })
            })
            .collect();
        accepted.sort_by_key(|candidate| {
            (
                candidate.finding.rule_index(),
                candidate.finding.start(),
                candidate.finding.end(),
            )
        });
        accepted
    }

    fn assert_context_partitions(rules: Arc<CompiledRuleSet>, source: &str) {
        let expected = contextual_expected(&rules, source);
        let scanner = Scanner::new(Arc::clone(&rules));
        let expected_report = scanner
            .scan([((), source)])
            .single_report()
            .unwrap()
            .clone();
        let partitions: Vec<Vec<&[u8]>> = if source.len() < 256 {
            (0..=source.len())
                .map(|split| vec![&source.as_bytes()[..split], &source.as_bytes()[split..]])
                .collect()
        } else {
            [1, 2, 7, 31, 4095, 4096, 4097, source.len()]
                .into_iter()
                .map(|size| source.as_bytes().chunks(size).collect())
                .collect()
        };
        for chunks in partitions {
            let mut public = scanner.stream().unwrap();
            let mut session = ValidatedSourceSession::new_with_limits(
                Arc::clone(&rules),
                SourceLimits::default(),
            )
            .unwrap();
            let mut accepted = Vec::new();
            for chunk in chunks {
                public.push(chunk).unwrap();
                session.scan_chunk(chunk, &mut accepted).unwrap();
                for plan in &session.context.plan.rules {
                    let records: Vec<_> = session
                        .context
                        .pending
                        .iter()
                        .filter(|record| record.finding.rule_index() == plan.rule_index)
                        .collect();
                    assert!(records.len() <= plan.max_pending_records);
                    assert!(
                        records
                            .iter()
                            .all(|record| record.text.len() <= plan.snapshot_bytes)
                    );
                }
            }
            session.finish_scan(&mut accepted).unwrap();
            assert_eq!(public.finish().unwrap(), expected_report);
            assert!(session.context.pending.is_empty());
            assert_eq!(session.session.lifecycle, SourceLifecycle::Completed);
            accepted.sort_by_key(|candidate| {
                (
                    candidate.finding.rule_index(),
                    candidate.finding.start(),
                    candidate.finding.end(),
                )
            });
            assert_eq!(accepted, expected, "source={source}");
        }
    }

    #[test]
    fn after_context_matchers_match_whole_source_across_partitions_and_eof() {
        use crate::validators::dispatch::ValidatorKind;
        let value = "CorrectHorseBatteryStaple";
        let source = format!("é<servers><server><password>{value}</password></server></servers>");
        for rule in [
            Rule::literal("literal", value, Severity::High),
            Rule::suffix("suffix", "Staple", Severity::High),
            Rule::pattern("regex", value, Severity::High).unwrap(),
            Rule::captured_pattern(
                "capture",
                "<password>(?P<value>CorrectHorseBatteryStaple)</password>",
                "value",
                Severity::High,
            )
            .unwrap(),
            Rule::captured_pattern(
                "eof-capture",
                r"<password>(?P<value>CorrectHorseBatteryStaple)</password></server></servers>\z",
                "value",
                Severity::High,
            )
            .unwrap(),
        ] {
            let rule = rule.with_validator(ValidatorKind::Maven);
            let rules = budget_rules(vec![rule.clone()]);
            let expected = contextual_expected(&rules, &source);
            assert_eq!(expected.len(), 1);
            let scanner = Scanner::builder().rule(rule).build().unwrap();
            let scan = scanner.scan([((), source.as_str())]);
            assert_eq!(scan.findings().count(), 1);
            assert_eq!(
                scan.findings().next().unwrap().1.confidence(),
                expected[0].confidence
            );
            assert_context_partitions(Arc::clone(&rules), &source);
            // A genuine EOF without closing tags must reject, even for suffix EOF emissions.
            assert_context_partitions(rules, &format!("<servers><server>{value}"));
        }
    }

    #[test]
    fn all_after_context_validator_kinds_preserve_acceptance_and_confidence() {
        use crate::validators::dispatch::ValidatorKind;
        let value = "CorrectHorseBatteryStaple";
        let verifier = "$apr1$hfT7jp2q$2VbDVlM1QY3wP1uQYxJYB/";
        let cases = [
            (
                ValidatorKind::Maven,
                value,
                format!("<servers><server>{value}</server></servers>"),
            ),
            (
                ValidatorKind::SystemPasswordVerifier,
                verifier,
                format!("alice:{verifier}"),
            ),
            (
                ValidatorKind::ComposerHttpBasicPassword,
                value,
                format!(
                    r#"{{"http-basic":{{"repo.example":{{"username":"alice","password":"{value}"}}}}}}"#
                ),
            ),
            (
                ValidatorKind::ComposerBearerToken,
                value,
                format!(r#"{{"bearer":{{"repo.example":"{value}"}}}}"#),
            ),
            (
                ValidatorKind::ComposerBitbucketConsumerSecret,
                value,
                format!(
                    r#"{{"bitbucket-oauth":{{"bitbucket.org":{{"consumer-key":"key","consumer-secret":"{value}"}}}}}}"#
                ),
            ),
            (
                ValidatorKind::ComposerForgejoToken,
                value,
                format!(
                    r#"{{"forgejo-token":{{"forgejo.example.org":{{"username":"alice","token":"{value}"}}}}}}"#
                ),
            ),
            (
                ValidatorKind::Nuget,
                value,
                format!(
                    r#"<packageSourceCredentials><Feed><add key="ClearTextPassword" value="{value}" /></Feed></packageSourceCredentials>"#
                ),
            ),
        ];
        let mut mixed_rules = Vec::new();
        let mut mixed_source = String::new();
        for (index, (kind, needle, source)) in cases.into_iter().enumerate() {
            let rule = Rule::literal(format!("context-{index}"), needle, Severity::High)
                .with_validator(kind);
            let rules = budget_rules(vec![rule.clone()]);
            let expected = contextual_expected(&rules, &source);
            assert_eq!(expected.len(), 1, "{kind:?}");
            let scanner = Scanner::builder().rule(rule.clone()).build().unwrap();
            let result = scanner.scan([((), source.as_str())]);
            assert_eq!(result.findings().count(), 1, "{kind:?}");
            assert_eq!(
                result.findings().next().unwrap().1.confidence(),
                expected[0].confidence
            );
            assert_context_partitions(rules, &source);
            mixed_rules.push(rule);
            mixed_source.push_str(&source);
            mixed_source.push('\n');
        }
        assert_context_partitions(budget_rules(mixed_rules), &mixed_source);
    }

    #[test]
    fn after_context_uses_full_callback_and_waits_for_complete_lookahead() {
        use crate::validators::dispatch::ValidatorKind;
        let value = "CorrectHorseBatteryStaple";
        let rules = budget_rules(vec![
            Rule::literal("maven", value, Severity::High).with_validator(ValidatorKind::Maven),
        ]);
        let before = format!("<servers><server>{value}");
        let tail = format!("</server></servers>{}", " ".repeat(4096));
        let source = format!("{before}{tail}");
        let mut session =
            ValidatedSourceSession::new_with_limits(Arc::clone(&rules), SourceLimits::default())
                .unwrap();
        let mut accepted = Vec::new();
        session
            .scan_chunk(before.as_bytes(), &mut accepted)
            .unwrap();
        assert_eq!(session.context.pending.len(), 1);
        assert!(accepted.is_empty());
        session
            .scan_chunk(&tail.as_bytes()[..30], &mut accepted)
            .unwrap();
        assert!(accepted.is_empty()); // Syntactically sufficient tags are not the complete window.
        session
            .scan_chunk(&tail.as_bytes()[30..], &mut accepted)
            .unwrap();
        assert_eq!(accepted, contextual_expected(&rules, &source));
        assert!(session.context.pending.is_empty());
        session.finish_scan(&mut accepted).unwrap();
        let mut immediate =
            ValidatedSourceSession::new_with_limits(rules, SourceLimits::default()).unwrap();
        let mut actual = Vec::new();
        immediate
            .scan_chunk(source.as_bytes(), &mut actual)
            .unwrap();
        assert_eq!(actual, accepted);
        assert!(immediate.context.pending.is_empty());
    }

    #[test]
    fn after_context_clips_utf8_inward_and_preserves_eof() {
        use crate::validators::dispatch::ValidatorKind;
        let value = "CorrectHorseBatteryStaple";
        let rules = budget_rules(vec![
            Rule::literal("maven", value, Severity::High).with_validator(ValidatorKind::Maven),
        ]);
        for padding in [4093, 4094, 4095] {
            let source = format!(
                "<servers><server>{value}{}€</server></servers>",
                " ".repeat(padding)
            );
            assert_context_partitions(Arc::clone(&rules), &source);
        }
        let source = format!("<servers><server>{value}€</server></servers>");
        assert_context_partitions(rules, &source);
    }

    #[test]
    fn dense_pending_candidates_obey_per_rule_and_payload_bounds() {
        use crate::validators::dispatch::ValidatorKind;
        let rules = budget_rules(vec![
            Rule::literal("literal", "a", Severity::High).with_validator(ValidatorKind::Maven),
            Rule::captured_pattern("capture", "(?P<value>a)", "value", Severity::High)
                .unwrap()
                .with_validator(ValidatorKind::Maven),
        ]);
        let source = "a".repeat(4200);
        let mut session =
            ValidatedSourceSession::new_with_limits(Arc::clone(&rules), SourceLimits::default())
                .unwrap();
        let mut accepted = Vec::new();
        let mut peak = 0;
        for chunk in source.as_bytes().chunks(1000) {
            session.scan_chunk(chunk, &mut accepted).unwrap();
            peak = peak.max(session.context.pending.len());
            for plan in &session.context.plan.rules {
                let records: Vec<_> = session
                    .context
                    .pending
                    .iter()
                    .filter(|record| record.finding.rule_index() == plan.rule_index)
                    .collect();
                assert!(records.len() <= plan.max_pending_records);
                assert!(
                    records
                        .iter()
                        .all(|record| record.text.len() <= plan.snapshot_bytes)
                );
            }
            let payload: usize = session
                .context
                .pending
                .iter()
                .map(|record| record.text.len() + std::mem::size_of::<PendingValidation>())
                .sum();
            assert!(
                payload + session.context.history.len() <= session.context.plan.persistent_bytes
            );
        }
        assert!(peak >= 8000, "exercise dense, overlapping snapshots");
        session.finish_scan(&mut accepted).unwrap();
        assert!(session.context.pending.is_empty());
        assert_eq!(accepted, contextual_expected(&rules, &source));
        let mut whole = Vec::new();
        rules.scan(&source, &mut whole);
        assert_eq!(session.session.state.emitted_candidates, whole.len());
    }

    #[test]
    fn contextual_storage_admission_accounts_for_snapshots_and_metadata() {
        use crate::validators::dispatch::ValidatorKind;
        let value = "CorrectHorseBatteryStaple";
        let rules = budget_rules(vec![
            Rule::literal("maven", value, Severity::High).with_validator(ValidatorKind::Maven),
        ]);
        let plan = rules.contextual_storage_plan().unwrap();
        let matcher = rules
            .streaming_storage_requirements()
            .unwrap()
            .total_buffer_bytes;
        let required = matcher + plan.persistent_bytes + plan.assembly_scratch_bytes;
        assert!(plan.persistent_bytes > plan.history_bytes);
        assert_eq!(
            ValidatedSourceSession::new_with_limits(
                Arc::clone(&rules),
                SourceLimits {
                    max_candidates: Some(1),
                    max_stream_storage_bytes: Some(required - 1),
                }
            )
            .unwrap_err(),
            SourceSessionError::StreamStorageLimitExceeded {
                required,
                limit: required - 1
            }
        );
        let mut session = ValidatedSourceSession::new_with_limits(
            Arc::clone(&rules),
            SourceLimits {
                max_candidates: Some(1),
                max_stream_storage_bytes: Some(required),
            },
        )
        .unwrap();
        let source = format!("<servers><server>{value}</server></servers>");
        let mut accepted = Vec::new();
        for chunk in source.as_bytes().chunks(3) {
            session.scan_chunk(chunk, &mut accepted).unwrap();
        }
        session.finish_scan(&mut accepted).unwrap();
        assert_eq!(accepted, contextual_expected(&rules, &source));
        assert_eq!(session.session.state.emitted_candidates, 1);
        // Raw admission retains its matcher-only accounting.
        assert!(
            SourceSession::new_with_limits(
                rules,
                SourceLimits {
                    max_stream_storage_bytes: Some(matcher),
                    ..SourceLimits::default()
                }
            )
            .is_ok()
        );
    }

    #[test]
    fn pending_range_capacity_and_offset_failures_are_terminal() {
        use crate::validators::dispatch::ValidatorKind;
        let rules = budget_rules(vec![
            Rule::literal("maven", "a", Severity::High).with_validator(ValidatorKind::Maven),
        ]);
        for capacity in [true, false] {
            let mut session = ValidatedSourceSession::new_with_limits(
                Arc::clone(&rules),
                SourceLimits::default(),
            )
            .unwrap();
            let mut accepted = Vec::new();
            let result = if capacity {
                session.context.plan.rules[0].max_pending_records = 0;
                session.scan_chunk(b"a", &mut accepted)
            } else {
                session.scan_chunk(b"a", &mut accepted).unwrap();
                session.context.pending[0].candidate_range.end = usize::MAX;
                session.finish_scan(&mut accepted)
            };
            assert_eq!(
                result,
                Err(if capacity {
                    SourceSessionError::PendingCapacityExceeded
                } else {
                    SourceSessionError::InvalidPendingRange
                })
            );
            assert!(accepted.is_empty());
            assert_eq!(session.session.lifecycle, SourceLifecycle::Failed);
            assert_eq!(
                session.scan_chunk(b"a", &mut accepted),
                Err(SourceSessionError::NotActive)
            );
            assert_eq!(
                session.finish_scan(&mut accepted),
                Err(SourceSessionError::NotActive)
            );
        }
        let mut session =
            ValidatedSourceSession::new_with_limits(Arc::clone(&rules), SourceLimits::default())
                .unwrap();
        let context = &mut session.context;
        let mut accepted = Vec::new();
        let result = session
            .session
            .scan_chunk_with_context(b"a", &mut |view, text, base| {
                if let Some(view) = view {
                    ValidatedSourceSession::ingest_context(
                        &rules,
                        context,
                        CandidateView {
                            finding: view.finding,
                            text: None,
                        },
                        text,
                        base,
                        false,
                        &mut accepted,
                    )
                } else {
                    Ok(())
                }
            });
        assert_eq!(result, Err(SourceSessionError::MissingCandidateText));
        assert!(accepted.is_empty());
        assert_eq!(session.session.lifecycle, SourceLifecycle::Failed);
        assert_eq!(
            session.finish_scan(&mut accepted),
            Err(SourceSessionError::NotActive)
        );
        let mut session =
            ValidatedSourceSession::new_with_limits(rules, SourceLimits::default()).unwrap();
        session.session.state.accepted_bytes = usize::MAX - 1;
        session.context.history = " ".repeat(4096);
        session.context.history_base = usize::MAX - 4097;
        assert_eq!(
            session.scan_chunk(b"a", &mut Vec::new()),
            Err(SourceSessionError::SourceOffsetOverflow)
        );
        assert_eq!(session.session.lifecycle, SourceLifecycle::Failed);
    }

    #[test]
    fn deferred_validation_preserves_raw_budget_and_transport_error_precedence() {
        use crate::validators::dispatch::ValidatorKind;
        let rules = budget_rules(vec![
            Rule::literal("maven", "a", Severity::High).with_validator(ValidatorKind::Maven),
        ]);
        let mut seed_findings = Vec::new();
        rules.scan("a", &mut seed_findings);
        let seed = ValidatedCandidate {
            finding: seed_findings[0],
            confidence: crate::Confidence::High,
        };
        for input in [b"aaa".as_slice(), "a".repeat(200_000).as_bytes()] {
            let mut session = ValidatedSourceSession::new_with_limits(
                Arc::clone(&rules),
                SourceLimits {
                    max_candidates: Some(1),
                    ..SourceLimits::default()
                },
            )
            .unwrap();
            let mut accepted = vec![seed];
            assert_eq!(
                session.scan_chunk(input, &mut accepted),
                Err(SourceSessionError::ResourceLimitExceeded)
            );
            assert_eq!(session.session.state.emitted_candidates, 1);
            assert!(session.context.pending.len() <= 1);
            assert_eq!(accepted, vec![seed]);
            assert_eq!(
                session.finish_scan(&mut accepted),
                Err(SourceSessionError::NotActive)
            );
        }
        let mut session =
            ValidatedSourceSession::new_with_limits(Arc::clone(&rules), SourceLimits::default())
                .unwrap();
        let mut accepted = Vec::new();
        session.scan_chunk(b"a", &mut accepted).unwrap();
        session.scan_chunk(&[0xe2], &mut accepted).unwrap();
        assert!(matches!(
            session.finish_scan(&mut accepted),
            Err(SourceSessionError::InvalidUtf8(_))
        ));
        assert!(accepted.is_empty());
        assert_eq!(session.session.lifecycle, SourceLifecycle::Failed);
        let mut session =
            ValidatedSourceSession::new_with_limits(rules, SourceLimits::default()).unwrap();
        session.scan_chunk(b"a", &mut accepted).unwrap();
        session.context.pending[0].candidate_range.end = usize::MAX;
        // Carry completion invokes ingestion before transport discovers the malformed byte.
        session.scan_chunk(&[0xc3], &mut accepted).unwrap();
        assert_eq!(
            session.scan_chunk(&[0xa9, b'a', 0xff], &mut accepted),
            Err(SourceSessionError::InvalidPendingRange)
        );
        assert_eq!(session.session.lifecycle, SourceLifecycle::Failed);
    }

    #[test]
    fn before_context_matches_whole_source_across_utf8_partitions_and_eof() {
        use crate::validators::dispatch::ValidatorKind;
        let value = "AlphaBravo123";
        let cases = [
            (
                Rule::literal("password", value, Severity::High)
                    .with_validator(ValidatorKind::Password),
                format!("password={value}"),
            ),
            (
                Rule::literal("rejected", value, Severity::High)
                    .with_validator(ValidatorKind::Password),
                format!("other={value}"),
            ),
            (
                Rule::literal("swift", value, Severity::High)
                    .with_validator(ValidatorKind::SwiftPm),
                format!("é\nSWIFTPM_REGISTRY_TOKEN={value}"),
            ),
            (
                Rule::literal("cargo", value, Severity::High)
                    .with_validator(ValidatorKind::CargoRegistry),
                format!("[registries.private]\ntoken={value}"),
            ),
            (
                Rule::literal("netrc", value, Severity::High).with_validator(ValidatorKind::Netrc),
                format!("machine private.example login admin password {value}"),
            ),
            (
                Rule::pattern("delayed", "AlphaBravo123(?:x{0,16})", Severity::High)
                    .unwrap()
                    .with_validator(ValidatorKind::Password),
                format!("password={value}!{}", "y".repeat(64)),
            ),
            (
                Rule::captured_pattern(
                    "capture",
                    "password=(?P<value>AlphaBravo123)",
                    "value",
                    Severity::High,
                )
                .unwrap()
                .with_validator(ValidatorKind::Password),
                format!("épassword={value}"),
            ),
            (
                Rule::suffix("suffix", "123", Severity::High)
                    .with_validator(ValidatorKind::Password),
                format!("password={value}! password={value}"),
            ),
            (
                Rule::literal("clipped", value, Severity::High)
                    .with_validator(ValidatorKind::Password),
                format!("€{}password={value}", " ".repeat(150)),
            ),
        ];
        for (rule, source) in cases {
            let rules = budget_rules(vec![rule.clone()]);
            let mut raw = Vec::new();
            rules.scan(&source, &mut raw);
            let expected: Vec<_> = raw
                .into_iter()
                .filter_map(|finding| {
                    let metadata = rules.metadata(finding.rule_index());
                    validate_candidate(
                        metadata.validator(),
                        &source,
                        finding.start()..finding.end(),
                        metadata.confidence(),
                    )
                    .map(|validation| ValidatedCandidate {
                        finding,
                        confidence: validation.confidence(),
                    })
                })
                .collect();
            let scanner = Scanner::builder().rule(rule).build().unwrap();
            let result = scanner.scan([((), source.as_str())]);
            assert_eq!(result.findings().count(), expected.len());
            assert_eq!(
                result
                    .findings()
                    .map(|(_, finding)| finding.confidence())
                    .collect::<Vec<_>>(),
                expected
                    .iter()
                    .map(|candidate| candidate.confidence)
                    .collect::<Vec<_>>()
            );
            for split in 0..=source.len() {
                let mut session = ValidatedSourceSession::new_with_limits(
                    Arc::clone(&rules),
                    SourceLimits::default(),
                )
                .unwrap();
                let mut accepted = Vec::new();
                session
                    .scan_chunk(&source.as_bytes()[..split], &mut accepted)
                    .unwrap();
                session
                    .scan_chunk(&source.as_bytes()[split..], &mut accepted)
                    .unwrap();
                session.finish_scan(&mut accepted).unwrap();
                accepted.sort_by_key(|record| record.finding.start());
                assert_eq!(accepted, expected, "split={split}, source={source}");
                assert!(session.context.history.len() <= session.context.history_limit);
                assert!(session.context.scratch.is_empty());
            }
        }
    }

    #[test]
    fn validated_history_storage_admission_and_large_callbacks_are_bounded() {
        use crate::validators::dispatch::ValidatorKind;
        let rules = budget_rules(vec![
            Rule::literal("password", "AlphaBravo123", Severity::High)
                .with_validator(ValidatorKind::Password),
            Rule::literal("swift", "ZuluDelta456", Severity::High)
                .with_validator(ValidatorKind::SwiftPm),
        ]);
        let matcher = rules
            .streaming_storage_requirements()
            .unwrap()
            .total_buffer_bytes;
        let plan = rules.contextual_storage_plan().unwrap();
        let required = matcher + plan.history_bytes + plan.assembly_scratch_bytes;
        assert_eq!(
            ValidatedSourceSession::new_with_limits(
                Arc::clone(&rules),
                SourceLimits {
                    max_candidates: None,
                    max_stream_storage_bytes: Some(required - 1),
                }
            )
            .unwrap_err(),
            SourceSessionError::StreamStorageLimitExceeded {
                required,
                limit: required - 1,
            }
        );
        // Raw admission retains its matcher-only interpretation.
        assert!(
            SourceSession::new_with_limits(
                Arc::clone(&rules),
                SourceLimits {
                    max_candidates: None,
                    max_stream_storage_bytes: Some(matcher),
                }
            )
            .is_ok()
        );
        let mut session = ValidatedSourceSession::new_with_limits(
            rules,
            SourceLimits {
                max_candidates: Some(2),
                max_stream_storage_bytes: Some(required),
            },
        )
        .unwrap();
        let source = format!(
            "{}password=AlphaBravo123\nSWIFTPM_REGISTRY_TOKEN=ZuluDelta456",
            "é".repeat(100_000)
        );
        let mut accepted = Vec::new();
        session
            .scan_chunk(source.as_bytes(), &mut accepted)
            .unwrap();
        session.finish_scan(&mut accepted).unwrap();
        assert_eq!(accepted.len(), 2);
        assert!(session.context.history.len() <= plan.history_bytes);
        assert!(session.context.scratch.capacity() < source.len());
        assert!(session.context.history.capacity() < source.len());
    }

    #[test]
    fn history_loss_and_text_mismatch_are_terminal_execution_errors() {
        use crate::validators::dispatch::ValidatorKind;
        for missing in [true, false] {
            let rules = budget_rules(vec![
                Rule::literal("password", "AlphaBravo123", Severity::High)
                    .with_validator(ValidatorKind::Password),
            ]);
            let mut session = ValidatedSourceSession::new_with_limits(
                Arc::clone(&rules),
                SourceLimits::default(),
            )
            .unwrap();
            let mut accepted = Vec::new();
            session.scan_chunk(b"password=", &mut accepted).unwrap();
            if missing {
                session.context.history.clear();
                session.context.history_base = session.session.accepted_bytes();
            }
            let context = &mut session.context;
            let result = session.session.scan_chunk_with_context(
                b"AlphaBravo123",
                &mut |view, text, base| {
                    if let Some(view) = view {
                        let view = if missing {
                            view
                        } else {
                            CandidateView {
                                text: Some("XXXXXXXXXXXXX"),
                                ..view
                            }
                        };
                        ValidatedSourceSession::ingest_before(
                            &rules,
                            context,
                            view,
                            text,
                            base,
                            &mut accepted,
                        )
                    } else {
                        context.retain(text, base)
                    }
                },
            );
            assert_eq!(
                result,
                Err(if missing {
                    SourceSessionError::MissingSourceHistory
                } else {
                    SourceSessionError::CandidateTextMismatch
                })
            );
            assert!(accepted.is_empty());
            assert_eq!(session.session.lifecycle, SourceLifecycle::Failed);
            assert_eq!(
                session.scan_chunk(b"", &mut accepted),
                Err(SourceSessionError::NotActive)
            );
            assert_eq!(
                session.finish_scan(&mut accepted),
                Err(SourceSessionError::NotActive)
            );
        }
    }

    #[test]
    fn validated_candidates_match_whole_source_decisions_across_all_byte_splits() {
        use crate::{Confidence, validators::dispatch::ValidatorKind};
        let iban = "DE89370400440532013000";
        let jwt = "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.Signature123";
        for (rule, source, count, confidence) in [
            (
                Rule::literal("none", "é", Severity::High),
                "xéé".to_owned(),
                2,
                Confidence::High,
            ),
            (
                Rule::prefix("prefix", "tok_", Severity::High),
                "é tok_eof".to_owned(),
                1,
                Confidence::High,
            ),
            (
                Rule::literal("iban", iban, Severity::High).with_validator(ValidatorKind::Iban),
                format!("é {iban}"),
                1,
                Confidence::High,
            ),
            (
                Rule::literal("invalid", "not-an-iban", Severity::High)
                    .with_validator(ValidatorKind::Iban),
                "é not-an-iban".to_owned(),
                0,
                Confidence::High,
            ),
            (
                Rule::suffix("suffix", "3000", Severity::High).with_validator(ValidatorKind::Iban),
                format!("é {iban}"),
                1,
                Confidence::High,
            ),
            (
                Rule::pattern("regex", r"DE[0-9]{20}\z", Severity::High)
                    .unwrap()
                    .with_validator(ValidatorKind::Iban),
                format!("é {iban}"),
                1,
                Confidence::High,
            ),
            (
                Rule::captured_pattern(
                    "capture",
                    r"KEY=(?P<value>DE[0-9]{20})",
                    "value",
                    Severity::High,
                )
                .unwrap()
                .with_validator(ValidatorKind::Iban),
                format!("é KEY={iban}"),
                1,
                Confidence::High,
            ),
            (
                Rule::literal("jwt", jwt, Severity::High).with_validator(ValidatorKind::Jwt),
                format!("é {jwt}"),
                1,
                Confidence::Medium,
            ),
        ] {
            let scanner = Scanner::builder().rule(rule.clone()).build().unwrap();
            let report = scanner.scan([((), source.as_str())]);
            assert_eq!(report.findings().count(), count);
            let rules = budget_rules(vec![rule]);
            let mut raw_whole = Vec::new();
            rules.scan(&source, &mut raw_whole);
            let expected: Vec<_> = raw_whole
                .iter()
                .filter_map(|&finding| {
                    let metadata = rules.metadata(finding.rule_index());
                    validate_candidate(
                        metadata.validator(),
                        &source,
                        finding.start()..finding.end(),
                        metadata.confidence(),
                    )
                    .map(|result| ValidatedCandidate {
                        finding,
                        confidence: result.confidence(),
                    })
                })
                .collect();
            for split in 0..=source.len() {
                let mut session = ValidatedSourceSession::new_with_limits(
                    Arc::clone(&rules),
                    SourceLimits {
                        max_candidates: Some(raw_whole.len()),
                        max_stream_storage_bytes: Some(
                            rules
                                .streaming_storage_requirements()
                                .unwrap()
                                .total_buffer_bytes,
                        ),
                    },
                )
                .unwrap();
                let mut accepted = Vec::new();
                session
                    .scan_chunk(&source.as_bytes()[..split], &mut accepted)
                    .unwrap();
                session
                    .scan_chunk(&source.as_bytes()[split..], &mut accepted)
                    .unwrap();
                session.finish_scan(&mut accepted).unwrap();
                accepted
                    .sort_by_key(|candidate| (candidate.finding.start(), candidate.finding.end()));
                assert_eq!(accepted, expected);
                assert!(
                    accepted
                        .iter()
                        .all(|candidate| candidate.confidence == confidence)
                );
                assert_eq!(session.session.state.emitted_candidates, raw_whole.len());
                assert_eq!(session.session.lifecycle, SourceLifecycle::Completed);
                assert_eq!(
                    session.scan_chunk(b"", &mut accepted),
                    Err(SourceSessionError::NotActive)
                );
                assert_eq!(
                    session.finish_scan(&mut accepted),
                    Err(SourceSessionError::NotActive)
                );
            }
        }
    }

    #[test]
    fn validated_admission_keeps_raw_context_support_and_all_existing_gates() {
        use crate::validators::dispatch::ValidatorKind;
        let contextual = budget_rules(vec![
            Rule::literal("context", "abc", Severity::High).with_validator(ValidatorKind::Maven),
        ]);
        assert!(SourceSession::new(Arc::clone(&contextual)).is_ok());
        let plan = contextual.contextual_storage_plan().unwrap();
        let required = contextual
            .streaming_storage_requirements()
            .unwrap()
            .total_buffer_bytes
            + plan.persistent_bytes
            + plan.assembly_scratch_bytes;
        assert_eq!(
            ValidatedSourceSession::new_with_limits(
                contextual,
                SourceLimits {
                    max_candidates: Some(0),
                    max_stream_storage_bytes: Some(0),
                }
            )
            .unwrap_err(),
            SourceSessionError::StreamStorageLimitExceeded { required, limit: 0 }
        );
        for rule in [
            Rule::prefix("prefix", "tok_", Severity::High).with_validator(ValidatorKind::Iban),
            Rule::suffix("suffix", "!", Severity::High),
            Rule::pattern("regex", "a+", Severity::High).unwrap(),
            Rule::pattern("aws.secret-access-key", "abc", Severity::High)
                .unwrap()
                .with_validator(ValidatorKind::Aws),
        ] {
            assert_eq!(
                ValidatedSourceSession::new_with_limits(
                    budget_rules(vec![rule]),
                    SourceLimits::default()
                )
                .unwrap_err(),
                SourceSessionError::UnsupportedRuleSet
            );
        }
    }

    #[test]
    fn rejected_candidates_consume_cumulative_raw_budget_before_validation() {
        use crate::validators::dispatch::ValidatorKind;
        let rules = budget_rules(vec![
            Rule::literal("invalid", "bad", Severity::High).with_validator(ValidatorKind::Iban),
        ]);
        let required = rules
            .streaming_storage_requirements()
            .unwrap()
            .total_buffer_bytes;
        let mut session = ValidatedSourceSession::new_with_limits(
            rules,
            SourceLimits {
                max_candidates: Some(2),
                max_stream_storage_bytes: Some(required),
            },
        )
        .unwrap();
        let mut accepted = Vec::new();
        session.scan_chunk(b"bad ", &mut accepted).unwrap();
        accepted.clear();
        session.scan_chunk(b"bad ", &mut accepted).unwrap();
        assert!(accepted.is_empty());
        assert_eq!(
            session.scan_chunk(b"bad", &mut accepted),
            Err(SourceSessionError::ResourceLimitExceeded)
        );
        assert_eq!(session.session.state.emitted_candidates, 2);
        assert_eq!(session.session.lifecycle, SourceLifecycle::Failed);
        assert_eq!(
            session.finish_scan(&mut accepted),
            Err(SourceSessionError::NotActive)
        );
        assert_eq!(
            session.scan_chunk(b"", &mut accepted),
            Err(SourceSessionError::NotActive)
        );
    }

    #[test]
    fn validated_execution_failures_are_terminal_including_eof_budget() {
        // Use a real emitted candidate to avoid constructing private finding fields.
        let rules = budget_rules(vec![Rule::literal("literal", "a", Severity::High)]);
        let mut raw = Vec::new();
        rules.scan("a", &mut raw);
        let mut accepted = vec![ValidatedCandidate {
            finding: raw[0],
            confidence: crate::Confidence::Low,
        }];
        let existing = accepted.clone();
        for chunk in [vec![0xff], vec![0xe2]] {
            let mut session = ValidatedSourceSession::new_with_limits(
                Arc::clone(&rules),
                SourceLimits::default(),
            )
            .unwrap();
            let result = session.scan_chunk(&chunk, &mut accepted);
            if chunk[0] == 0xe2 {
                result.unwrap();
                assert_eq!(
                    session.finish_scan(&mut accepted),
                    Err(SourceSessionError::InvalidUtf8(
                        Utf8TransportError::Incomplete
                    ))
                );
            } else {
                assert_eq!(
                    result,
                    Err(SourceSessionError::InvalidUtf8(
                        Utf8TransportError::Malformed
                    ))
                );
            }
            assert_eq!(accepted, existing);
            assert_eq!(session.session.lifecycle, SourceLifecycle::Failed);
            assert_eq!(
                session.finish_scan(&mut accepted),
                Err(SourceSessionError::NotActive)
            );
        }
        let mut session = ValidatedSourceSession::new_with_limits(
            budget_rules(vec![Rule::suffix("suffix", "_end", Severity::High)]),
            SourceLimits {
                max_candidates: Some(0),
                ..SourceLimits::default()
            },
        )
        .unwrap();
        session.scan_chunk(b"abc_end", &mut accepted).unwrap();
        assert_eq!(
            session.finish_scan(&mut accepted),
            Err(SourceSessionError::ResourceLimitExceeded)
        );
        assert_eq!(accepted, existing);
        assert_eq!(session.session.lifecycle, SourceLifecycle::Failed);
        let mut session =
            ValidatedSourceSession::new_with_limits(budget_rules(vec![]), SourceLimits::default())
                .unwrap();
        session.session.state.accepted_bytes = usize::MAX;
        assert_eq!(
            session.scan_chunk(b"a", &mut accepted),
            Err(SourceSessionError::SourceOffsetOverflow)
        );
        assert_eq!(
            session.finish_scan(&mut accepted),
            Err(SourceSessionError::NotActive)
        );
        assert_eq!(accepted, existing);
    }

    #[test]
    fn missing_text_is_explicit_terminal_and_preserves_execution_error_precedence() {
        use crate::validators::dispatch::ValidatorKind;
        for eof in [false, true] {
            let rules = budget_rules(vec![
                Rule::pattern("iban", r"DE[0-9]{20}\z", Severity::High)
                    .unwrap()
                    .with_validator(ValidatorKind::Iban),
            ]);
            let mut session = SourceSession::new(Arc::clone(&rules)).unwrap();
            let mut accepted = Vec::new();
            let mut ingest = |view: CandidateView<'_>| {
                ValidatedSourceSession::ingest(
                    &rules,
                    CandidateView { text: None, ..view },
                    &mut accepted,
                )
            };
            if eof {
                session
                    .scan_chunk(b"DE89370400440532013000", &mut Vec::new())
                    .unwrap();
                assert_eq!(
                    session.finish_with_ingestion(&mut ingest),
                    Err(SourceSessionError::MissingCandidateText)
                );
            } else {
                // Inject absent text at ingestion; normal matcher views supply it.
                let literal = budget_rules(vec![
                    Rule::literal("iban", "€", Severity::High).with_validator(ValidatorKind::Iban),
                ]);
                session = SourceSession::new(Arc::clone(&literal)).unwrap();
                session.scan_chunk(&[0xe2], &mut Vec::new()).unwrap();
                let mut ingest = |view: CandidateView<'_>| {
                    ValidatedSourceSession::ingest(
                        &literal,
                        CandidateView { text: None, ..view },
                        &mut accepted,
                    )
                };
                assert_eq!(
                    // Completed carry invokes ingestion before malformed remainder.
                    session.scan_chunk_with_ingestion(&[0x82, 0xac, 0xff], &mut ingest),
                    Err(SourceSessionError::MissingCandidateText)
                );
            }
            assert!(accepted.is_empty());
            assert_eq!(session.lifecycle, SourceLifecycle::Failed);
            assert_eq!(
                session.scan_chunk(b"", &mut Vec::new()),
                Err(SourceSessionError::NotActive)
            );
            assert_eq!(
                session.finish_scan(&mut Vec::new()),
                Err(SourceSessionError::NotActive)
            );
        }
    }

    #[test]
    fn semantic_admission_preserves_raw_candidates_and_existing_limits() {
        use crate::validators::dispatch::ValidatorKind;

        let rules = budget_rules(vec![
            Rule::literal("none", "abc", Severity::High),
            Rule::literal("candidate", "abc", Severity::High).with_validator(ValidatorKind::GitHub),
            Rule::suffix("before", "abc", Severity::High).with_validator(ValidatorKind::Password),
            Rule::pattern("after", "abc", Severity::High)
                .unwrap()
                .with_validator(ValidatorKind::Maven),
        ]);
        assert!(rules.supports_streaming());
        assert!(rules.supports_streaming_semantics());
        let required = rules
            .streaming_storage_requirements()
            .unwrap()
            .total_buffer_bytes;
        let source = "é abc!";
        let mut whole = Vec::new();
        rules.scan(source, &mut whole);
        assert_eq!(whole.len(), 4);
        // These are deliberately invalid validator inputs. Admission must not
        // accidentally turn raw-candidate execution into validation.
        for split in 0..=source.len() {
            let mut session = SourceSession::new_with_limits(
                Arc::clone(&rules),
                SourceLimits {
                    max_candidates: Some(4),
                    max_stream_storage_bytes: Some(required),
                },
            )
            .unwrap();
            let mut findings = Vec::new();
            session
                .scan_chunk(&source.as_bytes()[..split], &mut findings)
                .unwrap();
            session
                .scan_chunk(&source.as_bytes()[split..], &mut findings)
                .unwrap();
            session.finish_scan(&mut findings).unwrap();
            assert_eq!(projected_findings(&findings), projected_findings(&whole));
            assert_eq!(session.lifecycle(), SourceLifecycle::Completed);
        }
        assert_eq!(
            SourceSession::new_with_limits(
                Arc::clone(&rules),
                SourceLimits {
                    max_candidates: None,
                    max_stream_storage_bytes: Some(required - 1),
                },
            )
            .unwrap_err(),
            SourceSessionError::StreamStorageLimitExceeded {
                required,
                limit: required - 1
            }
        );
        let mut session = SourceSession::new_with_max_candidates(rules, Some(0)).unwrap();
        let mut findings = Vec::new();
        assert_eq!(
            session.scan_chunk(b"abc!", &mut findings),
            Err(SourceSessionError::ResourceLimitExceeded)
        );
        assert!(findings.is_empty());
        assert_eq!(session.lifecycle(), SourceLifecycle::Failed);
        assert_eq!(
            session.finish_scan(&mut findings),
            Err(SourceSessionError::NotActive)
        );
    }

    fn budget_rules(rules: Vec<Rule>) -> Arc<CompiledRuleSet> {
        Arc::new(CompiledRuleSet::compile(rules).unwrap())
    }

    #[test]
    fn source_offset_exact_boundary_can_complete() {
        // Empty rules avoid fabricating matcher history at a synthetic offset.
        let mut session = SourceSession::new(budget_rules(vec![])).unwrap();
        session.state.accepted_bytes = usize::MAX - 3;
        let mut findings = Vec::new();
        session.scan_chunk(b"abc", &mut findings).unwrap();
        assert_eq!(session.accepted_bytes(), usize::MAX);
        assert_eq!(session.lifecycle(), SourceLifecycle::Active);
        session.finish_scan(&mut findings).unwrap();
        assert_eq!(session.lifecycle(), SourceLifecycle::Completed);
        assert!(findings.is_empty());
        assert_terminal_session_unchanged(&mut session, &mut findings);
    }

    #[test]
    fn source_offset_overflow_precedes_all_matchers_and_preserves_findings() {
        let rules = budget_rules(vec![
            Rule::literal("literal", "secret", Severity::High),
            Rule::prefix("prefix", "secret", Severity::High),
            Rule::suffix("suffix", "secret", Severity::High),
            Rule::pattern("regex", "secret", Severity::High).unwrap(),
            Rule::captured_pattern("capture", r"(?P<value>secret)", "value", Severity::High)
                .unwrap(),
        ]);
        let mut findings = Vec::new();
        rules.scan("secret", &mut findings);
        let existing = findings.clone();
        assert_eq!(existing.len(), 5);
        for candidate_limit in [None, Some(0)] {
            let mut session =
                SourceSession::new_with_max_candidates(Arc::clone(&rules), candidate_limit)
                    .unwrap();
            session.state.accepted_bytes = usize::MAX - 2;
            let state = format!("{:?}", session.state);
            assert_eq!(
                session.scan_chunk(b"secret", &mut findings),
                Err(SourceSessionError::SourceOffsetOverflow)
            );
            assert_eq!(
                format!("{:?}", session.state),
                state,
                "overflow must precede matcher mutation"
            );
            assert_eq!(findings, existing);
            assert_eq!(session.state.emitted_candidates, 0);
            assert_eq!(session.accepted_bytes(), usize::MAX - 2);
            assert_eq!(session.lifecycle(), SourceLifecycle::Failed);
            assert_terminal_session_unchanged(&mut session, &mut findings);
        }
    }

    #[test]
    fn source_offset_empty_input_does_not_overflow() {
        let mut session = SourceSession::new(budget_rules(vec![])).unwrap();
        session.state.accepted_bytes = usize::MAX;
        let state = format!("{:?}", session.state);
        let mut findings = Vec::new();
        for _ in 0..3 {
            session.scan_chunk(b"", &mut findings).unwrap();
            assert_eq!(format!("{:?}", session.state), state);
            assert_eq!(session.lifecycle(), SourceLifecycle::Active);
        }
        session.finish_scan(&mut findings).unwrap();
        assert_eq!(session.lifecycle(), SourceLifecycle::Completed);
        assert!(findings.is_empty());
    }

    #[test]
    fn source_offset_carried_scalar_uses_full_callback_length() {
        for scalar in ["é", "€", "🦀"] {
            for split in 1..scalar.len() {
                for overflow in [false, true] {
                    let mut session = SourceSession::new(budget_rules(vec![])).unwrap();
                    let initial = usize::MAX - scalar.len() + usize::from(overflow);
                    session.state.accepted_bytes = initial;
                    let mut findings = Vec::new();
                    session
                        .scan_chunk(&scalar.as_bytes()[..split], &mut findings)
                        .unwrap();
                    assert_eq!(
                        session.accepted_bytes(),
                        initial,
                        "carry is not delivered input"
                    );
                    let result = session.scan_chunk(&scalar.as_bytes()[split..], &mut findings);
                    if overflow {
                        assert_eq!(result, Err(SourceSessionError::SourceOffsetOverflow));
                        assert_eq!(session.accepted_bytes(), initial);
                        assert_eq!(session.lifecycle(), SourceLifecycle::Failed);
                        assert_terminal_session_unchanged(&mut session, &mut findings);
                    } else {
                        result.unwrap();
                        assert_eq!(session.accepted_bytes(), usize::MAX);
                        session.finish_scan(&mut findings).unwrap();
                        assert_eq!(session.lifecycle(), SourceLifecycle::Completed);
                    }
                    assert!(findings.is_empty());
                }
            }
        }
    }

    #[test]
    fn source_offset_overflow_skips_later_callbacks_and_precedes_transport_error() {
        for tail in [b"\x80x".as_slice(), b"\x80\xFF"] {
            let mut session = SourceSession::new(budget_rules(vec![Rule::literal(
                "literal",
                "x",
                Severity::High,
            )]))
            .unwrap();
            session.state.accepted_bytes = usize::MAX - 3;
            let mut findings = Vec::new();
            session
                .scan_chunk(&[0xF0, 0x9F, 0xA6], &mut findings)
                .unwrap();
            let matcher_state = format!("{:?}", session.state.multi_pattern);
            // Four carried scalar bytes overflow. A later one-byte callback
            // would fit the unchanged counter but must not run its literal.
            assert_eq!(
                session.scan_chunk(tail, &mut findings),
                Err(SourceSessionError::SourceOffsetOverflow)
            );
            assert_eq!(format!("{:?}", session.state.multi_pattern), matcher_state);
            assert_eq!(session.accepted_bytes(), usize::MAX - 3);
            assert_eq!(session.state.emitted_candidates, 0);
            assert!(findings.is_empty());
            assert_eq!(session.lifecycle(), SourceLifecycle::Failed);
            assert_terminal_session_unchanged(&mut session, &mut findings);
        }
    }

    #[test]
    fn source_offset_guard_preserves_first_candidate_failure() {
        for tail in [b"\x80xxxxx".as_slice(), b"\x80\xFF"] {
            let mut session = SourceSession::new_with_max_candidates(
                budget_rules(vec![
                    Rule::literal("scalar", "🦀", Severity::High),
                    Rule::literal("later", "x", Severity::High),
                ]),
                Some(0),
            )
            .unwrap();
            session.state.accepted_bytes = usize::MAX - 4;
            let mut findings = Vec::new();
            session
                .scan_chunk(&[0xF0, 0x9F, 0xA6], &mut findings)
                .unwrap();
            // The callback end is representable, so candidate exhaustion wins,
            // including when a later callback would overflow or transport
            // subsequently encounters malformed bytes.
            assert_eq!(
                session.scan_chunk(tail, &mut findings),
                Err(SourceSessionError::ResourceLimitExceeded)
            );
            assert_eq!(session.accepted_bytes(), usize::MAX - 4);
            assert_eq!(session.state.emitted_candidates, 0);
            assert!(findings.is_empty());
            assert_eq!(session.lifecycle(), SourceLifecycle::Failed);
            assert_terminal_session_unchanged(&mut session, &mut findings);
        }
    }

    #[test]
    fn source_offset_later_callback_checks_updated_absolute_end() {
        let mut session = SourceSession::new(budget_rules(vec![])).unwrap();
        session.state.accepted_bytes = usize::MAX - 4;
        let mut findings = Vec::new();
        session
            .scan_chunk(&[0xF0, 0x9F, 0xA6], &mut findings)
            .unwrap();
        // The carried scalar reaches MAX successfully; the second callback
        // from the same push must check against that updated counter.
        assert_eq!(
            session.scan_chunk(b"\x80x", &mut findings),
            Err(SourceSessionError::SourceOffsetOverflow)
        );
        assert_eq!(session.accepted_bytes(), usize::MAX);
        assert_eq!(session.lifecycle(), SourceLifecycle::Failed);
        assert!(findings.is_empty());
        assert_terminal_session_unchanged(&mut session, &mut findings);
    }

    #[test]
    fn source_offset_near_boundary_incomplete_utf8_retains_transport_semantics() {
        let mut session = SourceSession::new(budget_rules(vec![])).unwrap();
        session.state.accepted_bytes = usize::MAX;
        let mut findings = Vec::new();
        session
            .scan_chunk(&[0xF0, 0x9F, 0xA6], &mut findings)
            .unwrap();
        session.scan_chunk(b"", &mut findings).unwrap();
        assert_eq!(session.accepted_bytes(), usize::MAX);
        assert_eq!(session.lifecycle(), SourceLifecycle::Active);
        assert_eq!(
            session.finish_scan(&mut findings),
            Err(SourceSessionError::InvalidUtf8(
                Utf8TransportError::Incomplete
            ))
        );
        assert_eq!(session.lifecycle(), SourceLifecycle::Failed);
        assert!(findings.is_empty());
        assert_terminal_session_unchanged(&mut session, &mut findings);
    }

    #[test]
    fn source_offset_guard_preserves_ordinary_utf8_offsets_and_eof_parity() {
        let rules = budget_rules(vec![
            Rule::literal("literal", "secret", Severity::High),
            Rule::prefix("prefix", "tok_", Severity::High),
            Rule::suffix("suffix", "_end", Severity::High),
            Rule::captured_pattern(
                "capture",
                r"KEY=(?P<value>[A-Z]{2})!",
                "value",
                Severity::High,
            )
            .unwrap(),
            Rule::pattern("word", r"\bsecret\b", Severity::High).unwrap(),
            Rule::pattern("lf", r"(?m)^secret$", Severity::High).unwrap(),
            Rule::pattern("crlf", r"(?mR)^secret$", Severity::High).unwrap(),
            Rule::pattern("eof", r"secret\z", Severity::High).unwrap(),
        ]);
        // Exercise token completion at EOF as well as an EOF-only regex match.
        for source in [
            "é🦀 secret KEY=AB!\r\nsecret\nsecret tok_value_end",
            "é🦀 secret KEY=AB!\r\nsecret\nsecret tok_value_end!\nsecret",
        ] {
            let mut whole = Vec::new();
            rules.scan(source, &mut whole);
            for partition in [1, 2, 3, 7, source.len()] {
                let mut session = SourceSession::new(Arc::clone(&rules)).unwrap();
                let mut findings = Vec::new();
                for chunk in source.as_bytes().chunks(partition) {
                    session.scan_chunk(chunk, &mut findings).unwrap();
                }
                session.finish_scan(&mut findings).unwrap();
                assert_eq!(session.accepted_bytes(), source.len());
                assert_eq!(projected_findings(&findings), projected_findings(&whole));
                assert_eq!(session.lifecycle(), SourceLifecycle::Completed);
                assert_terminal_session_unchanged(&mut session, &mut findings);
            }
        }
    }

    fn storage_rule_sets() -> Vec<Vec<Rule>> {
        vec![
            vec![],
            vec![Rule::literal("literal", "secret", Severity::High)],
            vec![Rule::prefix("prefix", "tok_", Severity::High)],
            vec![Rule::suffix("suffix", "_end", Severity::High)],
            vec![Rule::pattern("regex", r"(?mR)^secret$", Severity::High).unwrap()],
            vec![
                Rule::literal("literal", "secret", Severity::High),
                Rule::prefix("prefix", "tok_", Severity::High),
                Rule::suffix("suffix", "_end", Severity::High),
                Rule::captured_pattern(
                    "capture",
                    r"KEY=(?P<value>[A-Z]{2})!",
                    "value",
                    Severity::High,
                )
                .unwrap(),
                Rule::pattern("word", r"\bsecret\b", Severity::High).unwrap(),
                Rule::pattern("lf", r"(?m)^secret$", Severity::High).unwrap(),
                Rule::pattern("crlf", r"(?mR)^secret$", Severity::High).unwrap(),
                Rule::pattern("eof", r"secret\z", Severity::High).unwrap(),
            ],
        ]
    }

    #[test]
    fn storage_admission_accepts_exact_requirement_and_rejects_one_byte_less() {
        for rules in storage_rule_sets() {
            let rules = budget_rules(rules);
            let required = rules
                .streaming_storage_requirements()
                .unwrap()
                .total_buffer_bytes;
            for limit in [required, usize::MAX] {
                let mut session = SourceSession::new_with_limits(
                    Arc::clone(&rules),
                    SourceLimits {
                        max_candidates: None,
                        max_stream_storage_bytes: Some(limit),
                    },
                )
                .unwrap();
                assert_eq!(session.lifecycle(), SourceLifecycle::Active);
                assert_eq!(session.accepted_bytes(), 0);
                let mut findings = Vec::new();
                session.finish_scan(&mut findings).unwrap();
                assert_eq!(session.lifecycle(), SourceLifecycle::Completed);
                assert!(findings.is_empty());
            }
            if required == 0 {
                assert!(rules.is_empty());
            } else {
                for limit in [0, required - 1] {
                    assert_eq!(
                        SourceSession::new_with_limits(
                            Arc::clone(&rules),
                            SourceLimits {
                                max_candidates: None,
                                max_stream_storage_bytes: Some(limit),
                            }
                        )
                        .unwrap_err(),
                        SourceSessionError::StreamStorageLimitExceeded { required, limit }
                    );
                }
            }
        }
    }

    #[test]
    fn storage_admitted_and_unlimited_sessions_preserve_utf8_and_eof_parity() {
        let source = format!(
            "é🦀 secret KEY=AB! tok_{}! {}_end!\r\nsecret\nsecret",
            "abc".repeat(1500),
            "a".repeat(4092)
        );
        for rules in storage_rule_sets() {
            let rules = budget_rules(rules);
            let required = rules
                .streaming_storage_requirements()
                .unwrap()
                .total_buffer_bytes;
            let mut whole = Vec::new();
            rules.scan(&source, &mut whole);
            let expected = projected_findings(&whole);
            for storage in [None, Some(required)] {
                for candidate_limit in [None, Some(expected.len())] {
                    for partition in [1, 2, 7, 4093, source.len()] {
                        let mut session = SourceSession::new_with_limits(
                            Arc::clone(&rules),
                            SourceLimits {
                                max_candidates: candidate_limit,
                                max_stream_storage_bytes: storage,
                            },
                        )
                        .unwrap();
                        let mut findings = Vec::new();
                        for chunk in source.as_bytes().chunks(partition) {
                            session.scan_chunk(chunk, &mut findings).unwrap();
                        }
                        session.finish_scan(&mut findings).unwrap();
                        assert_eq!(
                            projected_findings(&findings),
                            expected,
                            "storage={storage:?}, candidates={candidate_limit:?}, partition={partition}"
                        );
                        assert_eq!(session.accepted_bytes(), source.len());
                        assert_eq!(session.lifecycle(), SourceLifecycle::Completed);
                        assert_terminal_session_unchanged(&mut session, &mut findings);
                    }
                }
            }
        }
    }

    #[test]
    fn storage_admission_preserves_unsupported_ruleset_error() {
        for pattern in ["a+", r"(?:\Aabc|def)"] {
            let rules = budget_rules(vec![
                Rule::pattern("unsupported", pattern, Severity::High).unwrap(),
            ]);
            for storage in [None, Some(0), Some(usize::MAX)] {
                assert_eq!(
                    SourceSession::new_with_limits(
                        Arc::clone(&rules),
                        SourceLimits {
                            max_candidates: Some(0),
                            max_stream_storage_bytes: storage,
                        }
                    )
                    .unwrap_err(),
                    SourceSessionError::UnsupportedRuleSet
                );
            }
        }
    }

    #[test]
    fn storage_admission_preserves_terminal_candidate_failures_in_chunk_and_eof() {
        for rule in [
            Rule::literal("literal", "secret", Severity::High),
            Rule::prefix("prefix", "secret", Severity::High),
            Rule::suffix("suffix", "secret", Severity::High),
            Rule::captured_pattern("capture", r"(?P<value>secret)\z", "value", Severity::High)
                .unwrap(),
        ] {
            let rules = budget_rules(vec![rule]);
            let required = rules
                .streaming_storage_requirements()
                .unwrap()
                .total_buffer_bytes;
            let mut findings = Vec::new();
            rules.scan("secret", &mut findings);
            let existing = findings.clone();
            assert_eq!(existing.len(), 1);
            let mut session = SourceSession::new_with_limits(
                rules,
                SourceLimits {
                    max_candidates: Some(0),
                    max_stream_storage_bytes: Some(required),
                },
            )
            .unwrap();
            let scan = session.scan_chunk(b"secret", &mut findings);
            let failure = if scan.is_ok() {
                session.finish_scan(&mut findings)
            } else {
                scan
            };
            assert_eq!(failure, Err(SourceSessionError::ResourceLimitExceeded));
            assert_eq!(findings, existing);
            assert_eq!(session.state.emitted_candidates, 0);
            assert_eq!(session.lifecycle(), SourceLifecycle::Failed);
            assert_terminal_session_unchanged(&mut session, &mut findings);
        }
    }

    #[test]
    fn storage_admission_preserves_terminal_utf8_failure() {
        let rules = budget_rules(vec![]);
        for bytes in [&[0xFF][..], &[0xF0, 0x9F][..]] {
            let mut session = SourceSession::new_with_limits(
                Arc::clone(&rules),
                SourceLimits {
                    max_candidates: Some(0),
                    max_stream_storage_bytes: Some(0),
                },
            )
            .unwrap();
            let mut findings = Vec::new();
            let scan = session.scan_chunk(bytes, &mut findings);
            let failure = if scan.is_ok() {
                session.finish_scan(&mut findings)
            } else {
                scan
            };
            assert!(matches!(failure, Err(SourceSessionError::InvalidUtf8(_))));
            assert_eq!(session.lifecycle(), SourceLifecycle::Failed);
            assert_terminal_session_unchanged(&mut session, &mut findings);
        }
    }

    #[test]
    fn candidate_budget_unlimited_and_exact_limit_preserve_raw_parity() {
        let rules = budget_rules(vec![
            Rule::literal("literal", "secret", Severity::High),
            Rule::literal("same-span", "secret", Severity::High),
            Rule::prefix("prefix", "tok", Severity::High),
            Rule::suffix("suffix", "_end", Severity::High),
            Rule::pattern("word", r"\bsecret\b", Severity::High).unwrap(),
            Rule::pattern("line", r"(?mR)^secret$", Severity::High).unwrap(),
            Rule::captured_pattern(
                "capture",
                r"KEY=(?P<value>[A-Z]{2})!",
                "value",
                Severity::High,
            )
            .unwrap(),
            Rule::pattern("eof", r"secret\z", Severity::High).unwrap(),
        ]);
        let source = "🦀 secret KEY=AB! tokvalue_end!\r\nsecret\r\nsecret";
        let mut whole = Vec::new();
        rules.scan(source, &mut whole);
        let expected = projected_findings(&whole);
        assert!(expected.len() > 8);
        for limit in [None, Some(expected.len())] {
            for chunk_size in [1, 7, source.len()] {
                let mut session =
                    SourceSession::new_with_max_candidates(Arc::clone(&rules), limit).unwrap();
                let mut findings = Vec::new();
                for chunk in source.as_bytes().chunks(chunk_size) {
                    session.scan_chunk(chunk, &mut findings).unwrap();
                }
                session.finish_scan(&mut findings).unwrap();
                assert_eq!(projected_findings(&findings), expected);
                assert_eq!(session.state.emitted_candidates, expected.len());
                assert_eq!(session.lifecycle(), SourceLifecycle::Completed);
            }
        }
    }

    #[test]
    fn candidate_budget_zero_allows_candidate_free_sessions() {
        let rules = budget_rules(vec![Rule::literal("literal", "secret", Severity::High)]);
        for source in ["", "nothing🦀", "secret"] {
            let mut session =
                SourceSession::new_with_max_candidates(Arc::clone(&rules), Some(0)).unwrap();
            let mut findings = Vec::new();
            let result = session.scan_chunk(source.as_bytes(), &mut findings);
            assert!(findings.is_empty());
            assert_eq!(session.state.emitted_candidates, 0);
            if source == "secret" {
                assert_eq!(result, Err(SourceSessionError::ResourceLimitExceeded));
                assert_eq!(session.lifecycle(), SourceLifecycle::Failed);
                assert_terminal_session_unchanged(&mut session, &mut findings);
            } else {
                result.unwrap();
                session.finish_scan(&mut findings).unwrap();
                assert_eq!(session.lifecycle(), SourceLifecycle::Completed);
            }
        }
    }

    #[test]
    fn candidate_budget_checks_every_chunk_emission_path_before_append() {
        for (rule, chunks) in [
            (
                Rule::literal("literal", "secret", Severity::High),
                vec![b"secret!".as_slice()],
            ),
            (
                Rule::literal("crossing", "secret", Severity::High),
                vec![b"sec".as_slice(), b"ret!"],
            ),
            (
                Rule::prefix("prefix", "tok", Severity::High),
                vec![b"tokvalue!".as_slice()],
            ),
            (
                Rule::prefix("active", "tok", Severity::High),
                vec![b"tok".as_slice(), b"value!"],
            ),
            (
                Rule::suffix("suffix", "_end", Severity::High),
                vec![b"value_".as_slice(), b"end!"],
            ),
            (
                Rule::pattern("regex", r"secret[A-Z]{2}", Severity::High).unwrap(),
                vec![b"secretAB padding padding".as_slice()],
            ),
            (
                Rule::captured_pattern(
                    "capture",
                    r"KEY=(?P<value>[A-Z]{2})!",
                    "value",
                    Severity::High,
                )
                .unwrap(),
                vec![b"KEY=AB! padding padding".as_slice()],
            ),
        ] {
            let mut session =
                SourceSession::new_with_max_candidates(budget_rules(vec![rule]), Some(0)).unwrap();
            let mut findings = Vec::new();
            let mut result = Ok(());
            for chunk in chunks {
                result = session.scan_chunk(chunk, &mut findings);
                if result.is_err() {
                    break;
                }
            }
            assert_eq!(result, Err(SourceSessionError::ResourceLimitExceeded));
            assert!(findings.is_empty());
            assert_eq!(session.state.emitted_candidates, 0);
            assert_eq!(session.lifecycle(), SourceLifecycle::Failed);
            assert_terminal_session_unchanged(&mut session, &mut findings);
        }
    }

    #[test]
    fn candidate_budget_counts_eof_emissions_and_exact_limit_succeeds() {
        for (rule, source) in [
            (Rule::prefix("prefix", "tok", Severity::High), "tokvalue"),
            (Rule::suffix("suffix", "_end", Severity::High), "value_end"),
            (
                Rule::pattern("regex", r"secret[A-Z]{1,4}", Severity::High).unwrap(),
                "secretAB",
            ),
            (
                Rule::pattern("end", r"secret\z", Severity::High).unwrap(),
                "secret",
            ),
            (
                Rule::pattern("both", r"\Asecret\z", Severity::High).unwrap(),
                "secret",
            ),
            (
                Rule::captured_pattern(
                    "capture",
                    r"KEY=(?P<value>[A-Z]{2})!\z",
                    "value",
                    Severity::High,
                )
                .unwrap(),
                "KEY=AB!",
            ),
        ] {
            let rules = budget_rules(vec![rule]);
            let mut whole = Vec::new();
            rules.scan(source, &mut whole);
            assert_eq!(whole.len(), 1);
            for limit in [0, 1] {
                let mut session =
                    SourceSession::new_with_max_candidates(Arc::clone(&rules), Some(limit))
                        .unwrap();
                let mut findings = Vec::new();
                session
                    .scan_chunk(source.as_bytes(), &mut findings)
                    .unwrap();
                assert!(findings.is_empty());
                let result = session.finish_scan(&mut findings);
                if limit == 0 {
                    assert_eq!(result, Err(SourceSessionError::ResourceLimitExceeded));
                    assert!(findings.is_empty());
                    assert_eq!(session.lifecycle(), SourceLifecycle::Failed);
                    assert_terminal_session_unchanged(&mut session, &mut findings);
                } else {
                    result.unwrap();
                    assert_eq!(projected_findings(&findings), projected_findings(&whole));
                    assert_eq!(session.lifecycle(), SourceLifecycle::Completed);
                }
                assert_eq!(session.state.emitted_candidates, limit);
            }
        }
    }

    #[test]
    fn candidate_budget_counter_is_independent_of_caller_vec_changes() {
        let rules = budget_rules(vec![Rule::literal("literal", "x", Severity::High)]);
        let mut session =
            SourceSession::new_with_max_candidates(Arc::clone(&rules), Some(2)).unwrap();
        let mut findings = Vec::new();
        rules.scan("xxxxxxxxxx", &mut findings);
        session.scan_chunk(b"x", &mut findings).unwrap();
        assert_eq!(findings.len(), 11);
        assert_eq!(session.state.emitted_candidates, 1);
        findings.clear();
        session.scan_chunk(b"x", &mut findings).unwrap();
        assert_eq!(findings.len(), 1);
        assert_eq!(session.state.emitted_candidates, 2);
        findings = Vec::new();
        rules.scan("xxx", &mut findings);
        let before = projected_findings(&findings);
        assert_eq!(
            session.scan_chunk(b"x", &mut findings),
            Err(SourceSessionError::ResourceLimitExceeded)
        );
        assert_eq!(projected_findings(&findings), before);
        assert_eq!(session.state.emitted_candidates, 2);
        assert_eq!(session.lifecycle(), SourceLifecycle::Failed);
        assert_terminal_session_unchanged(&mut session, &mut findings);
    }

    #[test]
    fn candidate_budget_is_shared_across_engines_chunks_and_eof() {
        let rules = budget_rules(vec![
            Rule::literal("literal", "first", Severity::High),
            Rule::prefix("prefix", "tok", Severity::High),
            Rule::suffix("suffix", "_end", Severity::High),
            Rule::captured_pattern(
                "capture",
                r"(?P<value>secret)_end\z",
                "value",
                Severity::High,
            )
            .unwrap(),
        ]);
        let mut session = SourceSession::new_with_max_candidates(rules, Some(3)).unwrap();
        let mut findings = Vec::new();
        session.scan_chunk(b"first ", &mut findings).unwrap();
        assert_eq!(session.state.emitted_candidates, 1);
        findings.clear();
        session.scan_chunk(b"toksecret_end", &mut findings).unwrap();
        assert!(findings.is_empty());
        assert_eq!(
            session.finish_scan(&mut findings),
            Err(SourceSessionError::ResourceLimitExceeded)
        );
        // Prefix and suffix consume the remaining two slots; projected regex
        // emission is rejected before append, even after the caller cleared Vec.
        assert_eq!(findings.len(), 2);
        assert_eq!(session.state.emitted_candidates, 3);
        assert_eq!(session.lifecycle(), SourceLifecycle::Failed);
        assert_terminal_session_unchanged(&mut session, &mut findings);
    }

    #[test]
    fn candidate_budget_counts_distinct_rules_emitting_same_span() {
        let mut session = SourceSession::new_with_max_candidates(
            budget_rules(vec![
                Rule::literal("one", "x", Severity::High),
                Rule::literal("two", "x", Severity::High),
                Rule::literal("three", "x", Severity::High),
            ]),
            Some(2),
        )
        .unwrap();
        let mut findings = Vec::new();
        assert_eq!(
            session.scan_chunk(b"x", &mut findings),
            Err(SourceSessionError::ResourceLimitExceeded)
        );
        assert_eq!(findings.len(), 2);
        assert_eq!(session.state.emitted_candidates, 2);
        assert!(
            findings
                .iter()
                .all(|finding| (finding.start(), finding.end()) == (0, 1))
        );
        assert_ne!(findings[0].rule_index(), findings[1].rule_index());
        assert_terminal_session_unchanged(&mut session, &mut findings);
    }

    #[test]
    fn candidate_budget_large_single_chunk_stops_at_limit() {
        let source = "x ".repeat(1024 * 1024);
        for rule in [
            Rule::literal("literal", "x", Severity::High),
            Rule::prefix("prefix", "x", Severity::High),
            Rule::suffix("suffix", "x", Severity::High),
            Rule::pattern("regex", "x", Severity::High).unwrap(),
            Rule::captured_pattern("capture", r"(?P<value>x)", "value", Severity::High).unwrap(),
        ] {
            let mut session =
                SourceSession::new_with_max_candidates(budget_rules(vec![rule]), Some(7)).unwrap();
            let mut findings = Vec::new();
            assert_eq!(
                session.scan_chunk(source.as_bytes(), &mut findings),
                Err(SourceSessionError::ResourceLimitExceeded)
            );
            assert_eq!(findings.len(), 7);
            assert_eq!(session.state.emitted_candidates, 7);
            for (index, finding) in findings.iter().enumerate() {
                assert_eq!((finding.start(), finding.end()), (2 * index, 2 * index + 1));
            }
            assert_eq!(session.lifecycle(), SourceLifecycle::Failed);
            assert_terminal_session_unchanged(&mut session, &mut findings);
        }
    }

    #[test]
    fn candidate_budget_preserves_terminal_utf8_errors() {
        let rules = budget_rules(vec![Rule::literal("scalar", "🦀", Severity::High)]);
        let mut session =
            SourceSession::new_with_max_candidates(Arc::clone(&rules), Some(1)).unwrap();
        let mut findings = Vec::new();
        session
            .scan_chunk(&[0xF0, 0x9F, 0xA6], &mut findings)
            .unwrap();
        assert_eq!(
            session.scan_chunk(&[0x80, 0xFF], &mut findings),
            Err(SourceSessionError::InvalidUtf8(
                Utf8TransportError::Malformed
            ))
        );
        assert_eq!(findings.len(), 1);
        assert_eq!(session.state.emitted_candidates, 1);
        assert_eq!(session.lifecycle(), SourceLifecycle::Failed);
        assert_terminal_session_unchanged(&mut session, &mut findings);

        let mut session = SourceSession::new_with_max_candidates(rules, Some(0)).unwrap();
        let mut findings = Vec::new();
        session.scan_chunk(&[0xF0], &mut findings).unwrap();
        assert_eq!(
            session.finish_scan(&mut findings),
            Err(SourceSessionError::InvalidUtf8(
                Utf8TransportError::Incomplete
            ))
        );
        assert_terminal_session_unchanged(&mut session, &mut findings);
    }

    #[test]
    fn candidate_budget_failure_skips_later_transport_callbacks() {
        let mut session = SourceSession::new_with_max_candidates(
            budget_rules(vec![
                Rule::literal("scalar", "🦀", Severity::High),
                Rule::literal("literal", "secret", Severity::High),
            ]),
            Some(0),
        )
        .unwrap();
        let mut findings = Vec::new();
        session
            .scan_chunk(&[0xF0, 0x9F, 0xA6], &mut findings)
            .unwrap();
        assert_eq!(
            session.scan_chunk(b"\x80secret", &mut findings),
            Err(SourceSessionError::ResourceLimitExceeded)
        );
        assert!(findings.is_empty());
        assert_eq!(session.state.emitted_candidates, 0);
        assert_eq!(session.accepted_bytes(), 0);
        assert_terminal_session_unchanged(&mut session, &mut findings);
    }

    #[test]
    fn candidate_budget_does_not_count_absent_projected_captures() {
        let rules = budget_rules(vec![
            Rule::captured_pattern("capture", r"(?:X|(?P<value>Y))", "value", Severity::High)
                .unwrap(),
        ]);
        let mut session = SourceSession::new_with_max_candidates(rules, Some(0)).unwrap();
        let mut findings = Vec::new();
        session.scan_chunk(b"XXX", &mut findings).unwrap();
        session.finish_scan(&mut findings).unwrap();
        assert!(findings.is_empty());
        assert_eq!(session.state.emitted_candidates, 0);
        assert_eq!(session.lifecycle(), SourceLifecycle::Completed);
    }

    #[test]
    fn sessions_share_immutable_compiled_configuration() {
        let scanner = Scanner::builder()
            .rule(Rule::literal("literal", "secret", Severity::High))
            .build()
            .expect("rule should compile");

        let first = scanner
            .source_session()
            .expect("rule set should support streaming");
        let second = scanner
            .source_session()
            .expect("rule set should support streaming");

        assert!(first.shares_rules_with(&second));
    }

    #[test]
    fn session_scans_literal_across_chunk_boundary() {
        let rules = Arc::new(
            CompiledRuleSet::compile(vec![Rule::literal("secret", "secret", Severity::High)])
                .unwrap(),
        );

        let mut session = SourceSession::new(rules).expect("rule set should support streaming");
        let mut findings = Vec::new();

        session.scan_chunk(b"xxsec", &mut findings).unwrap();
        session.scan_chunk(b"retyy", &mut findings).unwrap();
        session.finish_scan(&mut findings).unwrap();

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].start(), 2);
        assert_eq!(findings[0].end(), 8);
        assert_eq!(session.accepted_bytes(), 10);
        assert_eq!(session.lifecycle(), SourceLifecycle::Completed);
    }

    #[test]
    fn session_finishes_active_prefix_at_end_of_source() {
        let rules = Arc::new(
            CompiledRuleSet::compile(vec![Rule::prefix("token", "ghp_", Severity::Critical)])
                .unwrap(),
        );

        let mut session = SourceSession::new(rules).expect("rule set should support streaming");
        let mut findings = Vec::new();

        session.scan_chunk(b"xx gh", &mut findings).unwrap();
        session.scan_chunk(b"p_secret", &mut findings).unwrap();

        assert!(findings.is_empty());

        session.finish_scan(&mut findings).unwrap();

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].start(), 3);
        assert_eq!(findings[0].end(), 13);
    }

    #[test]
    fn sessions_keep_stream_matcher_state_independent() {
        let rules = Arc::new(
            CompiledRuleSet::compile(vec![Rule::literal("secret", "secret", Severity::High)])
                .unwrap(),
        );

        let mut first =
            SourceSession::new(Arc::clone(&rules)).expect("rule set should support streaming");
        let mut second = SourceSession::new(rules).expect("rule set should support streaming");

        let mut first_findings = Vec::new();
        let mut second_findings = Vec::new();

        first.scan_chunk(b"sec", &mut first_findings).unwrap();
        second.scan_chunk(b"xxxx", &mut second_findings).unwrap();

        first.scan_chunk(b"ret", &mut first_findings).unwrap();
        second.scan_chunk(b"secret", &mut second_findings).unwrap();

        first.finish_scan(&mut first_findings).unwrap();
        second.finish_scan(&mut second_findings).unwrap();

        assert_eq!(first_findings.len(), 1);
        assert_eq!(first_findings[0].start(), 0);
        assert_eq!(first_findings[0].end(), 6);

        assert_eq!(second_findings.len(), 1);
        assert_eq!(second_findings[0].start(), 4);
        assert_eq!(second_findings[0].end(), 10);
    }

    #[test]
    fn session_preserves_offsets_when_utf8_scalar_is_split() {
        let rules = Arc::new(
            CompiledRuleSet::compile(vec![Rule::literal("secret", "secret", Severity::High)])
                .unwrap(),
        );

        let source = "🦀secret";
        let bytes = source.as_bytes();

        for split in 0..=bytes.len() {
            let mut session =
                SourceSession::new(Arc::clone(&rules)).expect("rule set should support streaming");
            let mut findings = Vec::new();

            session.scan_chunk(&bytes[..split], &mut findings).unwrap();
            session.scan_chunk(&bytes[split..], &mut findings).unwrap();
            session.finish_scan(&mut findings).unwrap();

            assert_eq!(findings.len(), 1, "split at byte {split}");
            assert_eq!(findings[0].start(), 4, "split at byte {split}");
            assert_eq!(findings[0].end(), 10, "split at byte {split}");
            assert_eq!(session.accepted_bytes(), source.len());
        }
    }

    #[test]
    fn session_matches_with_single_byte_transport() {
        let rules = Arc::new(
            CompiledRuleSet::compile(vec![
                Rule::literal("secret", "secret", Severity::High),
                Rule::prefix("github", "ghp_", Severity::Critical),
            ])
            .unwrap(),
        );

        let source = "🦀 secret xx ghp_value!";
        let mut session = SourceSession::new(rules).expect("rule set should support streaming");
        let mut findings = Vec::new();

        for byte in source.as_bytes() {
            session
                .scan_chunk(std::slice::from_ref(byte), &mut findings)
                .unwrap();
        }

        session.finish_scan(&mut findings).unwrap();

        let spans = findings
            .iter()
            .map(|finding| (finding.start(), finding.end()))
            .collect::<Vec<_>>();

        assert!(spans.contains(&(5, 11)));
        assert!(spans.contains(&(15, 24)));
        assert_eq!(session.accepted_bytes(), source.len());
    }

    #[test]
    fn session_rejects_incomplete_utf8_before_matcher_eof() {
        let rules = Arc::new(
            CompiledRuleSet::compile(vec![
                Rule::prefix("github", "ghp_", Severity::Critical),
                Rule::suffix("suffix", "_secret", Severity::High),
                Rule::pattern("eof", r"secret\z", Severity::High).unwrap(),
            ])
            .unwrap(),
        );

        let mut session = SourceSession::new(rules).expect("rule set should support streaming");
        let mut findings = Vec::new();

        session.scan_chunk(b"ghp_secret", &mut findings).unwrap();
        session
            .scan_chunk(&[0xF0, 0x9F, 0xA6], &mut findings)
            .unwrap();

        let state_before_eof = format!("{:?}", session.state);

        assert_eq!(
            session.finish_scan(&mut findings),
            Err(SourceSessionError::InvalidUtf8(
                Utf8TransportError::Incomplete
            ))
        );

        assert!(findings.is_empty());
        assert_eq!(
            format!("{:?}", session.state),
            state_before_eof,
            "failed transport finish must not flush matcher state"
        );
        assert_eq!(session.lifecycle(), SourceLifecycle::Failed);
        assert_terminal_session_unchanged(&mut session, &mut findings);
    }

    fn assert_terminal_session_unchanged(
        session: &mut SourceSession,
        findings: &mut Vec<InternalFinding>,
    ) {
        assert_ne!(session.lifecycle(), SourceLifecycle::Active);
        let lifecycle = session.lifecycle();
        // The state types intentionally have no equality API. Their existing
        // Debug implementations let this test snapshot every owned state.
        let state = format!("{:?}", session.state);
        let previous_findings = format!("{findings:?}");
        for chunk in [
            b"secret ghp_value! token_secret!".as_slice(),
            b"",
            &[0x80],
            &[0xFF],
        ] {
            assert_eq!(
                session.scan_chunk(chunk, findings),
                Err(SourceSessionError::NotActive)
            );
            assert_eq!(
                session.finish_scan(findings),
                Err(SourceSessionError::NotActive)
            );
            assert_eq!(session.lifecycle(), lifecycle);
            assert_eq!(format!("{:?}", session.state), state);
            assert_eq!(format!("{findings:?}"), previous_findings);
        }
    }

    #[test]
    fn session_becomes_terminal_on_malformed_first_chunk() {
        let rules = Arc::new(
            CompiledRuleSet::compile(vec![Rule::literal("literal", "secret", Severity::High)])
                .unwrap(),
        );
        for chunk in [
            b"\xFF".as_slice(),
            b"secret\x80",
            b"\xC0\xAF",
            b"\xED\xA0\x80",
        ] {
            let mut session = SourceSession::new(Arc::clone(&rules)).unwrap();
            let mut findings = Vec::new();
            assert_eq!(
                session.scan_chunk(chunk, &mut findings),
                Err(SourceSessionError::InvalidUtf8(
                    Utf8TransportError::Malformed
                ))
            );
            assert_eq!(session.lifecycle(), SourceLifecycle::Failed);
            assert_terminal_session_unchanged(&mut session, &mut findings);
        }
    }

    #[test]
    fn session_becomes_terminal_on_malformed_chunk_after_valid_chunks() {
        let rules = Arc::new(
            CompiledRuleSet::compile(vec![
                Rule::literal("literal", "secret", Severity::High),
                Rule::prefix("prefix", "ghp_", Severity::Critical),
                Rule::suffix("suffix", "_end", Severity::High),
                Rule::pattern("eof", r"end\z", Severity::High).unwrap(),
            ])
            .unwrap(),
        );
        let mut session = SourceSession::new(rules).unwrap();
        let mut findings = Vec::new();
        session.scan_chunk(b"sec", &mut findings).unwrap();
        session
            .scan_chunk(b"ret ghp_pending_end", &mut findings)
            .unwrap();
        assert_eq!(findings.len(), 1);
        let accepted_bytes = session.accepted_bytes();
        assert_eq!(
            session.scan_chunk(&[0xFF], &mut findings),
            Err(SourceSessionError::InvalidUtf8(
                Utf8TransportError::Malformed
            ))
        );
        assert_eq!(session.lifecycle(), SourceLifecycle::Failed);
        assert_eq!(session.accepted_bytes(), accepted_bytes);
        assert_eq!(findings.len(), 1);
        assert_terminal_session_unchanged(&mut session, &mut findings);
    }

    #[test]
    fn session_becomes_terminal_after_carried_scalar_then_malformed_bytes() {
        let rules = Arc::new(
            CompiledRuleSet::compile(vec![Rule::literal("scalar", "🦀", Severity::High)]).unwrap(),
        );
        let mut session = SourceSession::new(rules).unwrap();
        let mut findings = Vec::new();
        session
            .scan_chunk(&[0xF0, 0x9F, 0xA6], &mut findings)
            .unwrap();
        assert_eq!(session.accepted_bytes(), 0);
        assert!(findings.is_empty());
        assert_eq!(
            session.scan_chunk(&[0x80, 0xFF], &mut findings),
            Err(SourceSessionError::InvalidUtf8(
                Utf8TransportError::Malformed
            ))
        );
        assert_eq!(session.lifecycle(), SourceLifecycle::Failed);
        // Resolving the carried scalar invokes the callback before the
        // malformed remainder is rejected. No rollback is required.
        assert_eq!(session.accepted_bytes(), "🦀".len());
        assert_eq!(findings.len(), 1);
        assert_eq!((findings[0].start(), findings[0].end()), (0, 4));
        assert_terminal_session_unchanged(&mut session, &mut findings);
    }

    #[test]
    fn completed_session_cannot_resume() {
        let rules = Arc::new(
            CompiledRuleSet::compile(vec![Rule::prefix("prefix", "ghp_", Severity::Critical)])
                .unwrap(),
        );
        let mut session = SourceSession::new(rules).unwrap();
        let mut findings = Vec::new();
        session.scan_chunk(b"ghp_secret", &mut findings).unwrap();
        assert_eq!(session.lifecycle(), SourceLifecycle::Active);
        session.finish_scan(&mut findings).unwrap();
        assert_eq!(session.lifecycle(), SourceLifecycle::Completed);
        assert_eq!(findings.len(), 1);
        assert_eq!((findings[0].start(), findings[0].end()), (0, 10));
        assert_terminal_session_unchanged(&mut session, &mut findings);
    }

    #[test]
    fn session_scans_suffix_across_chunk_boundary() {
        let rules = Arc::new(
            CompiledRuleSet::compile(vec![Rule::suffix("suffix", "_end", Severity::High)]).unwrap(),
        );

        let mut session = SourceSession::new(rules).expect("rule set should support streaming");
        let mut findings = Vec::new();

        session.scan_chunk(b"before ab", &mut findings).unwrap();

        assert!(findings.is_empty());

        session.scan_chunk(b"c_end!", &mut findings).unwrap();

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].start(), 7);
        assert_eq!(findings[0].end(), 14);
    }

    #[test]
    fn session_finishes_suffix_at_end_of_source() {
        let rules = Arc::new(
            CompiledRuleSet::compile(vec![Rule::suffix("suffix", "_end", Severity::High)]).unwrap(),
        );

        let mut session = SourceSession::new(rules).expect("rule set should support streaming");
        let mut findings = Vec::new();

        session.scan_chunk(b"xx abc_end", &mut findings).unwrap();

        assert!(findings.is_empty());

        session.finish_scan(&mut findings).unwrap();

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].start(), 3);
        assert_eq!(findings[0].end(), 10);
    }

    #[test]
    fn session_rejects_oversized_suffix_token() {
        let rules = Arc::new(
            CompiledRuleSet::compile(vec![Rule::suffix("suffix", "_end", Severity::High)]).unwrap(),
        );

        let source = format!("{}_end", "a".repeat(4096));

        let mut session = SourceSession::new(rules).expect("rule set should support streaming");
        let mut findings = Vec::new();

        for chunk in source.as_bytes().chunks(127) {
            session.scan_chunk(chunk, &mut findings).unwrap();
        }

        session.finish_scan(&mut findings).unwrap();

        assert!(findings.is_empty());
    }

    #[test]
    fn session_recovers_suffix_matching_after_oversized_token() {
        let rules = Arc::new(
            CompiledRuleSet::compile(vec![Rule::suffix("suffix", "_end", Severity::High)]).unwrap(),
        );

        let oversized = format!("{}_end", "a".repeat(4096));
        let source = format!("{oversized}! valid_end!");

        let mut session = SourceSession::new(rules).expect("rule set should support streaming");
        let mut findings = Vec::new();

        for chunk in source.as_bytes().chunks(113) {
            session.scan_chunk(chunk, &mut findings).unwrap();
        }

        session.finish_scan(&mut findings).unwrap();

        assert_eq!(findings.len(), 1);

        let expected_start = oversized.len() + 2;

        assert_eq!(findings[0].start(), expected_start);
        assert_eq!(findings[0].end(), expected_start + "valid_end".len());
    }

    #[test]
    fn streamed_suffix_matches_whole_source_semantics() {
        let rules = Arc::new(
            CompiledRuleSet::compile(vec![Rule::suffix("suffix", "_end", Severity::High)]).unwrap(),
        );

        let source = "first_end! nope_endx second_end third_end";

        let mut whole = Vec::new();
        rules.scan(source, &mut whole);

        let mut session =
            SourceSession::new(Arc::clone(&rules)).expect("rule set should support streaming");
        let mut streamed = Vec::new();

        for chunk in source.as_bytes().chunks(3) {
            session.scan_chunk(chunk, &mut streamed).unwrap();
        }

        session.finish_scan(&mut streamed).unwrap();

        let project =
            |finding: &InternalFinding| (finding.start(), finding.end(), finding.rule_index());

        let mut whole = whole.iter().map(project).collect::<Vec<_>>();
        let mut streamed = streamed.iter().map(project).collect::<Vec<_>>();

        whole.sort_unstable();
        streamed.sort_unstable();

        assert_eq!(streamed, whole);
    }

    #[test]
    fn streamed_suffix_is_equivalent_across_every_two_chunk_partition() {
        let rules = Arc::new(
            CompiledRuleSet::compile(vec![Rule::suffix("suffix", "_end", Severity::High)]).unwrap(),
        );

        let source = "🦀 first_end! nope_endx second_end third_end";

        let mut whole = Vec::new();
        rules.scan(source, &mut whole);

        let project =
            |finding: &InternalFinding| (finding.start(), finding.end(), finding.rule_index());

        let mut expected = whole.iter().map(project).collect::<Vec<_>>();
        expected.sort_unstable();

        for split in 0..=source.len() {
            if !source.is_char_boundary(split) {
                continue;
            }

            let mut session =
                SourceSession::new(Arc::clone(&rules)).expect("rule set should support streaming");
            let mut streamed = Vec::new();

            session
                .scan_chunk(&source.as_bytes()[..split], &mut streamed)
                .unwrap();
            session
                .scan_chunk(&source.as_bytes()[split..], &mut streamed)
                .unwrap();
            session.finish_scan(&mut streamed).unwrap();

            let mut actual = streamed.iter().map(project).collect::<Vec<_>>();
            actual.sort_unstable();

            assert_eq!(actual, expected, "split at byte {split}");
        }
    }

    #[test]
    fn streamed_suffix_is_equivalent_across_every_byte_partition() {
        let rules = Arc::new(
            CompiledRuleSet::compile(vec![Rule::suffix("suffix", "_end", Severity::High)]).unwrap(),
        );

        let source = "🦀 first_end! café second_end";

        let mut whole = Vec::new();
        rules.scan(source, &mut whole);

        let project =
            |finding: &InternalFinding| (finding.start(), finding.end(), finding.rule_index());

        let mut expected = whole.iter().map(project).collect::<Vec<_>>();
        expected.sort_unstable();

        for split in 0..=source.len() {
            let mut session =
                SourceSession::new(Arc::clone(&rules)).expect("rule set should support streaming");
            let mut streamed = Vec::new();

            session
                .scan_chunk(&source.as_bytes()[..split], &mut streamed)
                .unwrap();
            session
                .scan_chunk(&source.as_bytes()[split..], &mut streamed)
                .unwrap();
            session.finish_scan(&mut streamed).unwrap();

            let mut actual = streamed.iter().map(project).collect::<Vec<_>>();
            actual.sort_unstable();

            assert_eq!(actual, expected, "split at raw byte {split}");
        }
    }

    #[test]
    fn session_matches_suffix_with_single_byte_transport() {
        let rules = Arc::new(
            CompiledRuleSet::compile(vec![Rule::suffix("suffix", "_end", Severity::High)]).unwrap(),
        );

        let source = "🦀 alpha_end! beta_end gamma";
        let mut session = SourceSession::new(rules).expect("rule set should support streaming");
        let mut findings = Vec::new();

        for byte in source.as_bytes() {
            session
                .scan_chunk(std::slice::from_ref(byte), &mut findings)
                .unwrap();
        }

        session.finish_scan(&mut findings).unwrap();

        let spans = findings
            .iter()
            .map(|finding| (finding.start(), finding.end()))
            .collect::<Vec<_>>();

        assert_eq!(spans, vec![(5, 14), (16, 24)]);
    }

    fn projected_findings(
        findings: &[InternalFinding],
    ) -> Vec<(usize, usize, crate::compiled_rule::RuleIndex)> {
        let mut projected = findings
            .iter()
            .map(|finding| (finding.start(), finding.end(), finding.rule_index()))
            .collect::<Vec<_>>();

        projected.sort_unstable();
        projected
    }

    #[test]
    fn session_stream_matches_whole_source_with_all_supported_matchers() {
        let rules = Arc::new(
            CompiledRuleSet::compile(vec![
                Rule::literal("literal", "secret", Severity::High),
                Rule::prefix("prefix", "ghp_", Severity::Critical),
                Rule::suffix("suffix", "_token", Severity::High),
                Rule::pattern("pattern", r"key_[A-Z]{4}", Severity::High).unwrap(),
            ])
            .unwrap(),
        );

        let source = "🦀 secret xx ghp_value! yy alpha_token zz key_ABCD end";

        let mut whole = Vec::new();
        rules.scan(source, &mut whole);

        let mut session =
            SourceSession::new(Arc::clone(&rules)).expect("rule set should support streaming");
        let mut streamed = Vec::new();

        let chunks: &[&[u8]] = &[
            "🦀 se".as_bytes(),
            b"cret xx gh",
            b"p_val",
            b"ue! yy alpha_",
            b"token zz key_",
            b"AB",
            b"CD end",
        ];

        for chunk in chunks {
            session.scan_chunk(chunk, &mut streamed).unwrap();
        }

        session.finish_scan(&mut streamed).unwrap();

        assert_eq!(projected_findings(&streamed), projected_findings(&whole),);

        assert_eq!(session.accepted_bytes(), source.len());
        assert_eq!(session.lifecycle(), SourceLifecycle::Completed);
    }

    #[test]
    fn session_stream_matches_whole_source_for_every_valid_split() {
        let rules = Arc::new(
            CompiledRuleSet::compile(vec![
                Rule::literal("literal", "secret", Severity::High),
                Rule::prefix("prefix", "ghp_", Severity::Critical),
                Rule::suffix("suffix", "_token", Severity::High),
                Rule::pattern("pattern", r"key_[A-Z]{4}", Severity::High).unwrap(),
            ])
            .unwrap(),
        );

        let source = "🦀 secret xx ghp_value! yy alpha_token zz key_ABCD 🦀";

        let mut whole = Vec::new();
        rules.scan(source, &mut whole);
        let expected = projected_findings(&whole);

        for split in 0..=source.len() {
            if !source.is_char_boundary(split) {
                continue;
            }

            let mut session =
                SourceSession::new(Arc::clone(&rules)).expect("rule set should support streaming");
            let mut streamed = Vec::new();

            session
                .scan_chunk(&source.as_bytes()[..split], &mut streamed)
                .unwrap();

            session
                .scan_chunk(&source.as_bytes()[split..], &mut streamed)
                .unwrap();

            session.finish_scan(&mut streamed).unwrap();

            assert_eq!(
                projected_findings(&streamed),
                expected,
                "stream differs from whole-source scan at split {split}",
            );

            assert_eq!(session.accepted_bytes(), source.len());
        }
    }

    #[test]
    fn session_stream_matches_whole_source_with_single_byte_transport() {
        let rules = Arc::new(
            CompiledRuleSet::compile(vec![
                Rule::literal("literal", "secret", Severity::High),
                Rule::prefix("prefix", "ghp_", Severity::Critical),
                Rule::suffix("suffix", "_token", Severity::High),
                Rule::pattern("pattern", r"key_[A-Z]{4}", Severity::High).unwrap(),
            ])
            .unwrap(),
        );

        let source = "🦀 secret xx ghp_value! yy alpha_token zz key_ABCD 🦀";

        let mut whole = Vec::new();
        rules.scan(source, &mut whole);
        let expected = projected_findings(&whole);

        let mut session =
            SourceSession::new(Arc::clone(&rules)).expect("rule set should support streaming");
        let mut streamed = Vec::new();

        for byte in source.as_bytes() {
            session
                .scan_chunk(std::slice::from_ref(byte), &mut streamed)
                .unwrap();
        }

        session.finish_scan(&mut streamed).unwrap();

        assert_eq!(projected_findings(&streamed), expected);
        assert_eq!(session.accepted_bytes(), source.len());
        assert_eq!(session.lifecycle(), SourceLifecycle::Completed);
    }

    #[test]
    fn source_session_streams_all_supported_matcher_kinds_across_chunks() {
        let rules = Arc::new(
            CompiledRuleSet::compile(vec![
                Rule::literal("literal", "literal_secret", Severity::High),
                Rule::suffix("suffix", "_suffix", Severity::High),
                Rule::captured_pattern(
                    "pattern",
                    r"KEY=(?P<value>[A-Z]{4})!",
                    "value",
                    Severity::High,
                )
                .unwrap(),
            ])
            .unwrap(),
        );

        let source = "🦀 literal_secret token_suffix KEY=ABCD! end";

        let mut expected = Vec::new();
        rules.scan(source, &mut expected);

        let bytes = source.as_bytes();

        // Deliberately split inside the leading UTF-8 scalar and inside
        // the bounded captured pattern.
        let pattern_start = source.find("KEY=ABCD!").unwrap();

        let chunks = [
            &bytes[..2],
            &bytes[2..pattern_start + 6],
            &bytes[pattern_start + 6..],
        ];

        let mut session =
            SourceSession::new(Arc::clone(&rules)).expect("rule set should support streaming");
        let mut streamed = Vec::new();

        for chunk in chunks {
            session.scan_chunk(chunk, &mut streamed).unwrap();
        }

        session.finish_scan(&mut streamed).unwrap();

        let project =
            |finding: &InternalFinding| (finding.start(), finding.end(), finding.rule_index());

        let mut expected = expected.iter().map(project).collect::<Vec<_>>();
        let mut streamed = streamed.iter().map(project).collect::<Vec<_>>();

        expected.sort_unstable();
        streamed.sort_unstable();

        assert_eq!(streamed, expected);
        assert_eq!(session.accepted_bytes(), source.len());
    }

    #[test]
    fn session_rejects_rule_set_with_unsupported_streaming_pattern() {
        let rule = Rule::pattern("unbounded", r"secret_[A-Z]+", Severity::High)
            .expect("pattern should compile");

        let rules =
            Arc::new(CompiledRuleSet::compile(vec![rule]).expect("rule set should compile"));

        let error =
            SourceSession::new(rules).expect_err("unbounded pattern must reject streaming session");

        assert_eq!(error, SourceSessionError::UnsupportedRuleSet);
    }

    #[test]
    fn session_accepts_rule_set_with_bounded_streaming_pattern() {
        let rule = Rule::pattern("bounded", r"secret_[A-Z]{4}", Severity::High)
            .expect("pattern should compile");

        let rules =
            Arc::new(CompiledRuleSet::compile(vec![rule]).expect("rule set should compile"));

        SourceSession::new(rules).expect("bounded pattern should support streaming");
    }

    fn assert_pattern_stream_parity(pattern: &str, source: &str) {
        let rule = Rule::pattern("word-boundary", pattern, Severity::High)
            .expect("pattern should compile");

        let rules =
            Arc::new(CompiledRuleSet::compile(vec![rule]).expect("rule set should compile"));

        let mut expected = Vec::new();
        rules.scan(source, &mut expected);
        let expected = projected_findings(&expected);

        // Every possible two-chunk partition, including UTF-8 boundaries.
        for split in 0..=source.len() {
            let mut session = SourceSession::new(Arc::clone(&rules))
                .expect("bounded word-boundary pattern must support streaming");

            let mut actual = Vec::new();

            session
                .scan_chunk(&source.as_bytes()[..split], &mut actual)
                .expect("first chunk should succeed");

            session
                .scan_chunk(&source.as_bytes()[split..], &mut actual)
                .expect("second chunk should succeed");

            session
                .finish_scan(&mut actual)
                .expect("finish should succeed");

            assert_eq!(
                projected_findings(&actual),
                expected,
                "pattern={pattern:?}, source={source:?}, split={split}",
            );
        }

        // One byte per transport chunk.
        let mut session = SourceSession::new(Arc::clone(&rules))
            .expect("bounded word-boundary pattern must support streaming");

        let mut actual = Vec::new();

        for byte in source.as_bytes() {
            session
                .scan_chunk(std::slice::from_ref(byte), &mut actual)
                .expect("single-byte chunk should succeed");
        }

        session
            .finish_scan(&mut actual)
            .expect("finish should succeed");

        assert_eq!(
            projected_findings(&actual),
            expected,
            "single-byte pattern={pattern:?}, source={source:?}",
        );
    }

    #[test]
    fn word_boundary_streaming_preserves_ascii_semantics() {
        for source in [
            "secret",
            " secret ",
            "secretX",
            "Xsecret",
            "secret secret",
            "secretX secret",
            "secret_secret",
            "secret\nsecret",
            "secret!",
            "!secret",
            "secreté",
            "ésecret",
        ] {
            assert_pattern_stream_parity(r"(?-u:\b)secret(?-u:\b)", source);
        }
    }

    #[test]
    fn word_boundary_streaming_preserves_unicode_semantics() {
        for source in [
            "secret",
            " secret ",
            "secretX",
            "Xsecret",
            "secret secret",
            "secreté",
            "ésecret",
            "🦀secret🦀",
            "secret🦀secret",
            "αsecretβ",
            "secret\u{0301}",
            "secret\nsecret",
        ] {
            assert_pattern_stream_parity(r"\bsecret\b", source);
        }
    }

    #[test]
    fn word_boundary_streaming_preserves_non_overlapping_matches() {
        for source in [
            "abc",
            "abcabc",
            "abc abc",
            "abcabcabc",
            "abcéabc",
            "abc!abc",
        ] {
            assert_pattern_stream_parity(r"\b(?:abc|abcabc)\b", source);
        }
    }

    #[test]
    fn word_boundary_streaming_preserves_context_after_repeated_trimming() {
        let sources = [
            format!("{}secret ", "X".repeat(256)),
            format!("{}secret ", "é".repeat(128)),
            format!("{}secret ", "🦀".repeat(128)),
            format!("{} secretX secret ", "x".repeat(256)),
            format!("{}secret secretX secret ", "α".repeat(128)),
            "Xsecret secret Xsecret secret".repeat(64),
        ];

        for source in &sources {
            assert_pattern_stream_parity(r"\bsecret\b", source);
            assert_pattern_stream_parity(r"(?-u:\b)secret(?-u:\b)", source);
        }
    }

    #[test]
    fn word_boundary_streaming_preserves_leftmost_selection() {
        let patterns = [
            r"\b(?:a|ab|abc)\b",
            r"\b(?:abc|ab|a)\b",
            r"\b(?:abc|abcabc)\b",
            r"\b(?:abcabc|abc)\b",
            r"\b(?:foo|foobar)\b",
            r"\b(?:foobar|foo)\b",
            r"\b(?:ab|abc|abcd)\b",
        ];

        let sources = [
            "a ab abc abcd".to_owned(),
            "abc abcabc abc".to_owned(),
            "abcabc abc abcabc".to_owned(),
            "foobar foo foobar".to_owned(),
            "foo foobar foo".to_owned(),
            "abcX abc abcabc".to_owned(),
            "Xabc abc Xabc".to_owned(),
            "αabcβ abc 🦀abc🦀".to_owned(),
            "abc abcabc abc abcabc".repeat(32),
        ];

        for pattern in patterns {
            for source in &sources {
                assert_pattern_stream_parity(pattern, source);
            }
        }
    }

    #[test]
    fn word_boundary_streaming_preserves_negated_boundaries() {
        for pattern in [
            r"\Bsecret\B",
            r"(?-u:\B)secret(?-u:\B)",
            r"\Bsecret\b",
            r"\bsecret\B",
        ] {
            for source in [
                "secret",
                "XsecretY",
                "Xsecret",
                "secretY",
                " secret ",
                "ésecreté",
                "🦀secret🦀",
                "XsecretY secret Zsecret",
            ] {
                assert_pattern_stream_parity(pattern, source);
            }
        }
    }

    #[test]
    fn word_boundary_streaming_preserves_directional_boundaries() {
        for pattern in [
            r"\b{start}secret\b{end}",
            r"\b{start-half}secret\b{end-half}",
            r"\b{start}secret",
            r"secret\b{end}",
        ] {
            for source in [
                "secret",
                "XsecretY",
                " secret ",
                "secret secret",
                "ésecret",
                "secreté",
                "🦀secret🦀",
                "secret\u{0301}",
            ] {
                assert_pattern_stream_parity(pattern, source);
            }
        }
    }

    #[test]
    fn word_boundary_streaming_differential_regression() {
        // Fixed seed: failures must be reproducible without external dependencies.
        let mut state = 0xC71B_A5E5_D52A_2026_u64;

        fn next(state: &mut u64) -> u64 {
            *state ^= *state << 13;
            *state ^= *state >> 7;
            *state ^= *state << 17;
            *state
        }

        const FRAGMENTS: &[&str] = &[
            "", "a", "b", "x", "abc", "abcabc", "secret", "secretX", "Xsecret", " ", "  ", "\n",
            "\r\n", "_", "-", "!", ".", "é", "α", "β", "🦀", "\u{0301}",
        ];

        const PATTERNS: &[&str] = &[
            r"\bsecret\b",
            r"(?-u:\b)secret(?-u:\b)",
            r"\Bsecret\B",
            r"\b(?:abc|abcabc)\b",
            r"\b(?:abcabc|abc)\b",
            r"\b(?:a|ab|abc)\b",
            r"\b{start}abc\b{end}",
            r"\b{start-half}abc\b{end-half}",
        ];

        for case in 0..128 {
            let mut source = String::new();

            // Variable-length sources with a mixture of ASCII, Unicode,
            // delimiters, partial candidates, and adjacent candidates.
            let fragments = 8 + (next(&mut state) % 32) as usize;

            for _ in 0..fragments {
                let index = (next(&mut state) as usize) % FRAGMENTS.len();
                source.push_str(FRAGMENTS[index]);
            }

            for pattern in PATTERNS {
                assert_pattern_stream_parity(pattern, &source);
            }

            // Include the case index in the failure context when debugging.
            // The fixed seed and generation order make every case reproducible.
            let _ = case;
        }
    }

    #[test]
    fn text_anchor_streaming_preserves_absolute_start() {
        for pattern in [
            r"\Asecret",
            r"\A(?:secret|secretX)",
            r"\A(?:secretX|secret)",
            r"\Asecret\b",
            r"\A(?P<value>secret)",
        ] {
            for source in [
                "",
                "secret",
                "secretX",
                "secret secret",
                "Xsecret",
                "Xsecret secret",
                " secret",
                "ésecret",
                "🦀secret",
                "secret\nsecret",
                "secret\r\nsecret",
            ] {
                assert_pattern_stream_parity(pattern, source);
            }
        }
    }

    #[test]
    fn text_anchor_streaming_preserves_absolute_end() {
        for pattern in [
            r"secret\z",
            r"(?:secret|Xsecret)\z",
            r"(?:secretX|secret)\z",
            r"secret\b\z",
            r"(?P<value>secret)\z",
        ] {
            for source in [
                "",
                "secret",
                "Xsecret",
                "secretX",
                "secret secret",
                "secretXsecret",
                "secret\n",
                "secret\r\n",
                "ésecret",
                "🦀secret",
                "secret🦀",
                "secreté",
            ] {
                assert_pattern_stream_parity(pattern, source);
            }
        }
    }

    #[test]
    fn text_anchor_streaming_preserves_both_absolute_anchors() {
        for pattern in [
            r"\Asecret\z",
            r"\A(?:secret|secretX)\z",
            r"\A(?:secretX|secret)\z",
            r"\A(?P<value>secret)\z",
        ] {
            for source in [
                "",
                "secret",
                "secretX",
                "Xsecret",
                "secret secret",
                "secret\n",
                "ésecret",
                "🦀secret",
            ] {
                assert_pattern_stream_parity(pattern, source);
            }
        }
    }

    #[test]
    fn text_anchor_streaming_does_not_rebase_source_start() {
        for pattern in [r"\Asecret", r"\A(?:secret|secretX)", r"\Asecret\b"] {
            for source in [
                format!("{}secret", "x".repeat(256)),
                format!("{}secret", "é".repeat(128)),
                format!("{}secret", "🦀".repeat(128)),
                format!("{}secret secret", "x".repeat(256)),
            ] {
                assert_pattern_stream_parity(pattern, &source);
            }
        }
    }

    #[test]
    fn absolute_start_streaming_resolves_without_eof() {
        for pattern in [
            r"\Asecret",
            r"\A(?:secretX|secret)",
            r"\A(?:secret|secretX)",
            r"\Asecret\b",
        ] {
            let source = format!("secret{}", "x".repeat(256));
            assert_pattern_stream_parity(pattern, &source);
        }
    }

    #[test]
    fn absolute_end_streaming_long_source_parity() {
        for pattern in [
            r"secret\z",
            r"\bsecret\z",
            r"\Bsecret\z",
            r"(?:secret|Xsecret)\z",
            r"(?P<value>secret)\z",
        ] {
            for prefix in [
                "x".repeat(256),
                " ".repeat(256),
                "é".repeat(128),
                "🦀".repeat(128),
                "x ".repeat(128),
            ] {
                for suffix in [
                    "secret",
                    "Xsecret",
                    " secret",
                    "secretX",
                    "secret\n",
                    "secret🦀",
                ] {
                    let source = format!("{prefix}{suffix}");
                    assert_pattern_stream_parity(pattern, &source);
                }
            }
        }
    }

    #[test]
    fn absolute_end_streaming_alternative_parity() {
        for pattern in [
            r"(?:secret|Xsecret)\z",
            r"(?:Xsecret|secret)\z",
            r"(?:secretX|secret)\z",
            r"(?:secret|secretX)\z",
            r"(?:abc|abcabc)\z",
            r"(?:abcabc|abc)\z",
        ] {
            for source in [
                "secret",
                "Xsecret",
                "secretX",
                "abc",
                "abcabc",
                "xabcabc",
                "abcabcabc",
                "secretsecret",
                "Xsecretsecret",
            ] {
                assert_pattern_stream_parity(pattern, source);
            }
        }
    }

    #[test]
    fn both_absolute_anchors_reject_oversized_sources() {
        let sources = [
            "secretXsecret".to_owned(),
            "Xsecret".to_owned(),
            "secret\n".to_owned(),
            "secret🦀".to_owned(),
            "🦀secret".to_owned(),
            "secret".repeat(100),
        ];

        for pattern in [
            r"\Asecret\z",
            r"\A(?:secret|secretX)\z",
            r"\A(?P<value>secret)\z",
            r"\Asecret\b\z",
        ] {
            for source in &sources {
                assert_pattern_stream_parity(pattern, source);
            }
        }
    }

    #[test]
    fn line_anchor_lf_streaming_parity() {
        let patterns = [
            r"(?m)^secret",
            r"(?m)secret$",
            r"(?m)^secret$",
            r"(?m)^secret[A-Z]{2}$",
            r"(?m)^(?:secret|token)$",
            r"(?m)^(?:secretX|secret)$",
            r"(?m)^secret\b",
            r"(?m)\bsecret$",
        ];

        let sources = [
            "",
            "secret",
            "secret\n",
            "\nsecret",
            "\nsecret\n",
            "xsecret",
            "secretX",
            "x\nsecret\ny",
            "secret\nsecret",
            "secret\r\n",
            "x\r\nsecret\r\n",
            "secretAB\n",
            "token\nsecret",
            "é\nsecret",
            "🦀\nsecret",
        ];

        for pattern in patterns {
            for source in sources {
                assert_pattern_stream_parity(pattern, source);
            }
        }
    }

    #[test]
    fn line_anchor_crlf_streaming_parity() {
        let patterns = [
            r"(?mR)^secret",
            r"(?mR)secret$",
            r"(?mR)^secret$",
            r"(?mR)^secret[A-Z]{2}$",
            r"(?mR)^(?:secret|token)$",
            r"(?mR)^secret\b",
            r"(?mR)\bsecret$",
        ];

        let sources = [
            "",
            "secret",
            "secret\n",
            "secret\r",
            "secret\r\n",
            "\r\nsecret",
            "\r\nsecret\r\n",
            "x\r\nsecret\r\ny",
            "secret\r\nsecret",
            "x\nsecret\ny",
            "x\rsecret\ry",
            "secret\r\r\n",
            "é\r\nsecret",
            "🦀\r\nsecret",
        ];

        for pattern in patterns {
            for source in sources {
                assert_pattern_stream_parity(pattern, source);
            }
        }
    }

    #[test]
    fn mixed_required_absolute_line_streaming_parity() {
        for pattern in [
            r"(?m)\Asecret$",
            r"(?mR)\Asecret$",
            r"(?m)\A^secret",
            r"(?mR)\A^secret",
            r"(?m)^secret\z",
            r"(?mR)^secret\z",
            r"(?m)secret$\z",
            r"(?mR)secret$\z",
            r"(?m)\A^secret$\z",
            r"(?mR)\A^secret$\z",
            r"(?m)\Asecret\n^token$",
            r"(?mR)\Asecret\r\n^token$",
            r"(?m)^secret$\ntoken\z",
            r"(?mR)^secret$\r\ntoken\z",
            r"(?m)\Asecret$\n^token\z",
            r"(?mR)\Asecret$\r\n^token\z",
            r"(?m)\A\bsecret\b$",
            r"(?mR)\A\bsecret\b$",
            r"(?m)^\bsecret\b\z",
            r"(?mR)^\bsecret\b\z",
            r"(?m)\A^(?:secretX|secret)$",
            r"(?mR)\A^(?:secret|secretX)$",
            r"(?m)^(?:secret|secretX)\z",
            r"(?mR)^(?:secretX|secret)\z",
            r"(?m)\A(?:secret$|secretX)",
            r"(?mR)(?:^secret|secretX)\z",
            r"(?mR)\Asecret\r$",
            r"(?mR)^\nsecret\z",
        ] {
            for source in [
                "",
                "secret",
                "secretX",
                "secret\n",
                "secret\r",
                "secret\r\n",
                "secret\nX",
                "secret\rX",
                "\nsecret",
                "\rsecret",
                "\r\nsecret",
                "secret\nsecret",
                "secret\n token",
                "secret\ntoken",
                "secret\r\ntoken",
                "secret\rtoken",
                "secret\ntoken\n",
                "secret\r\ntoken\r\n",
                "é\nsecret",
                "🦀\r\nsecret",
                "secreté",
            ] {
                assert_pattern_stream_parity(pattern, source);
            }
        }
    }

    #[test]
    fn mixed_required_absolute_line_trimming_parity() {
        for pattern in [
            r"(?m)^secret\z",
            r"(?mR)^secret\z",
            r"(?m)^\bsecret\b\z",
            r"(?mR)^\bsecret\b\z",
            r"(?m)^secret$\ntoken\z",
            r"(?mR)^secret$\r\ntoken\z",
        ] {
            for prefix in [
                "x".repeat(96),
                "é".repeat(48),
                "🦀".repeat(24),
                "secret\r\nxsecret\n".repeat(8),
            ] {
                for suffix in [
                    "secret",
                    "\nsecret",
                    "\rsecret",
                    "\r\nsecret",
                    "\nsecret\ntoken",
                    "\r\nsecret\r\ntoken",
                    "\nsecretX",
                ] {
                    assert_pattern_stream_parity(pattern, &(prefix.clone() + suffix));
                }
            }
        }
        for pattern in [
            r"(?m)\Asecret$",
            r"(?mR)\Asecret$",
            r"(?m)\A^secret$\z",
            r"(?mR)\A^secret$\z",
        ] {
            for source in [
                "secret\n".to_owned() + &"🦀\r\nsecret".repeat(64),
                "secretX".to_owned() + &"\nsecret".repeat(64),
            ] {
                assert_pattern_stream_parity(pattern, &source);
            }
        }
    }

    #[test]
    fn mixed_required_absolute_line_capture_parity() {
        for pattern in [
            r"(?m)\A^KEY=(?P<value>[A-Z]{2,4})!$",
            r"(?mR)\A^KEY=(?P<value>[A-Z]{2,4})!$",
            r"(?m)^KEY=(?P<value>[A-Z]{2,4})!\z",
            r"(?mR)^KEY=(?P<value>[A-Z]{2,4})!\z",
            r"(?m)\A^KEY=(?P<value>[A-Z]{2,4})!$\z",
            r"(?mR)\A^KEY=(?P<value>[A-Z]{2,4})!$\z",
            r"(?mR)\A(?:^KEY=AB!|KEY=(?P<value>AB)!$)",
            r"(?mR)(?:^KEY=AB!|(?P<value>AB)!$)\z",
        ] {
            let rule =
                Rule::captured_pattern("mixed-capture", pattern, "value", Severity::High).unwrap();
            let rules = Arc::new(CompiledRuleSet::compile(vec![rule]).unwrap());
            for source in [
                "KEY=AB!".to_owned(),
                "KEY=ABCD!\r\nKEY=XY!".to_owned(),
                "KEY=AB!KEY=XY!".to_owned(),
                "KEY=AB!\rKEY=XY!\nKEY=CD!".to_owned(),
                "🦀\r\nKEY=AB!\r\nXY!".to_owned(),
                "éKEY=AB!".to_owned(),
                "KEY=AB!\r\n".to_owned(),
                "x🦀\r\n".repeat(64) + "KEY=AB!",
            ] {
                let mut expected = Vec::new();
                rules.scan(&source, &mut expected);
                let expected = projected_findings(&expected);
                let bytes = source.as_bytes();
                let partitions = (0..=bytes.len())
                    .map(|split| vec![&bytes[..split], &bytes[split..]])
                    .chain(std::iter::once(bytes.chunks(1).collect()));
                for chunks in partitions {
                    let mut session = SourceSession::new(Arc::clone(&rules)).unwrap();
                    let mut findings = Vec::new();
                    for chunk in chunks {
                        session.scan_chunk(chunk, &mut findings).unwrap();
                    }
                    session.finish_scan(&mut findings).unwrap();
                    assert_eq!(
                        projected_findings(&findings),
                        expected,
                        "mixed capture parity: pattern={pattern}, source={source:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn mixed_required_absolute_line_eof_behavior() {
        for pattern in [
            r"(?m)^secret\z",
            r"(?mR)^secret\z",
            r"(?m)\A^secret$\z",
            r"(?mR)\A^secret$\z",
        ] {
            let rules = Arc::new(
                CompiledRuleSet::compile(vec![
                    Rule::pattern("mixed-eof", pattern, Severity::High).unwrap(),
                ])
                .unwrap(),
            );
            for suffix in ["", "X", "\r", "\n", "\r\n"] {
                let mut session = SourceSession::new(Arc::clone(&rules)).unwrap();
                let mut findings = Vec::new();
                session.scan_chunk(b"secret", &mut findings).unwrap();
                assert!(findings.is_empty(), "absolute-end match must wait for EOF");
                session
                    .scan_chunk(suffix.as_bytes(), &mut findings)
                    .unwrap();
                assert!(findings.is_empty(), "absolute-end match must wait for EOF");
                session.finish_scan(&mut findings).unwrap();
                let mut expected = Vec::new();
                rules.scan(&("secret".to_owned() + suffix), &mut expected);
                assert_eq!(projected_findings(&findings), projected_findings(&expected));
                assert_eq!(findings.len(), usize::from(suffix.is_empty()));
            }
        }

        for pattern in [r"(?m)\Asecret$", r"(?mR)\Asecret$"] {
            let rules = Arc::new(
                CompiledRuleSet::compile(vec![
                    Rule::pattern("mixed-start", pattern, Severity::High).unwrap(),
                ])
                .unwrap(),
            );
            let mut session = SourceSession::new(rules).unwrap();
            let mut findings = Vec::new();
            session.scan_chunk(b"secret", &mut findings).unwrap();
            assert!(
                findings.is_empty(),
                "temporary end must not resolve a line-end match"
            );
            session
                .scan_chunk(b"\nxxxxxxxxxxxxxxxx\nsecret", &mut findings)
                .unwrap();
            assert_eq!(
                findings.len(),
                1,
                "stable absolute-start match can resolve before EOF"
            );
            session.finish_scan(&mut findings).unwrap();
            assert_eq!(
                findings.len(),
                1,
                "absolute start must not rebase or duplicate at EOF"
            );
            assert_eq!((findings[0].start(), findings[0].end()), (0, 6));
        }
    }

    #[test]
    fn line_anchor_crlf_alternative_parity() {
        for pattern in [
            r"(?mR)^(?:abc|abcabc)$",
            r"(?mR)^(?:abcabc|abc)$",
            r"(?mR)(?:^abc|abc$)",
            r"(?mR)(?:abc$|^abc)",
            r"(?mR)(?:^abcabc|abc)",
            r"(?mR)(?:abc|^abcabc)",
        ] {
            for source in [
                "abc",
                "abcabc",
                "abcabcabc",
                "abc\r\nabcabc",
                "abcabc\rabc",
                "abcabc\nabc",
                "xabc\r\n",
                "\r\nabc",
                "abc\r",
                "abc\n",
            ] {
                assert_pattern_stream_parity(pattern, source);
            }
        }
    }

    #[test]
    fn line_anchor_crlf_repeated_trimming_parity() {
        let source = "secret\r\nxsecret\rsecret\n🦀\r\nsecret\résecret\n".repeat(32) + "secret";
        for pattern in [
            r"(?mR)^secret",
            r"(?mR)secret$",
            r"(?mR)^secret$",
            r"(?mR)^\bsecret\b$",
            r"(?mR)(?:^secret|secret$)",
        ] {
            assert_pattern_stream_parity(pattern, &source);
        }

        // A trimmed window must not manufacture a source-start assertion.
        for prefix in ["x".repeat(256), "é".repeat(128), "🦀".repeat(128)] {
            for suffix in ["secret", "\rsecret", "\nsecret", "\r\nsecret"] {
                assert_pattern_stream_parity(r"(?mR)^secret", &(prefix.clone() + suffix));
            }
        }
    }

    #[test]
    fn line_anchor_crlf_pair_interior_and_eof_parity() {
        // Neither anchor may match between the CR and LF of a pair.
        for pattern in [r"(?mR)^\nsecret", r"(?mR)secret\r$"] {
            for source in ["\nsecret", "\r\nsecret", "secret\r", "secret\r\n"] {
                assert_pattern_stream_parity(pattern, source);
            }
        }

        for pattern in [r"(?mR)secret$", r"(?mR)^secret$", r"(?mR)\bsecret$"] {
            for source in [
                "secret",
                "secretX",
                "secreté",
                "secret🦀",
                "secret\r",
                "secret\n",
                "secret\r\n",
                "secret\rX",
                "secret\r\nX",
                "secret\r\r\n",
            ] {
                assert_pattern_stream_parity(pattern, source);
            }
        }

        let rules = Arc::new(
            CompiledRuleSet::compile(vec![
                Rule::pattern("crlf-eof", r"(?mR)^secret$", Severity::High).unwrap(),
            ])
            .unwrap(),
        );
        let mut session = SourceSession::new(rules).unwrap();
        let mut findings = Vec::new();
        session.scan_chunk(b"secret", &mut findings).unwrap();
        assert!(
            findings.is_empty(),
            "temporary end must not resolve an EOF match"
        );
        session.finish_scan(&mut findings).unwrap();
        assert_eq!(findings.len(), 1);
        assert_eq!((findings[0].start(), findings[0].end()), (0, 6));
    }

    #[test]
    fn line_anchor_streaming_alternative_parity() {
        let patterns = [
            r"(?m)^(?:abc|abcabc)$",
            r"(?m)^(?:abcabc|abc)$",
            r"(?m)(?:^abc|abc$)",
            r"(?m)(?:abc$|^abc)",
        ];

        let sources = [
            "abc",
            "abcabc",
            "abc\nabc",
            "abcabc\nabc",
            "xabc\n",
            "\nabc",
            "abc\r\n",
            "x\r\nabc\r\n",
            "abcabcabc",
            "abc\nabcabc",
        ];

        for pattern in patterns {
            for source in sources {
                assert_pattern_stream_parity(pattern, source);
            }
        }
    }

    #[test]
    fn line_anchor_streaming_repeated_trimming_parity() {
        let mut source = String::new();

        for index in 0..128 {
            if index % 3 == 0 {
                source.push_str("secret\r\n");
            } else if index % 3 == 1 {
                source.push_str("xsecret\n");
            } else {
                source.push_str("🦀secret\r\n");
            }
        }

        source.push_str("secret");

        for pattern in [r"(?m)^secret", r"(?m)secret$", r"(?m)^secret$"] {
            assert_pattern_stream_parity(pattern, &source);
        }
    }

    #[test]
    fn line_anchor_lf_does_not_rebase_window_start() {
        let sources = [
            format!("{}secret", "x".repeat(256)),
            format!("{}secret", "é".repeat(128)),
            format!("{}secret", "🦀".repeat(128)),
            format!("{}secret", "x".repeat(256) + "\n"),
            format!("{}\nsecret", "x".repeat(256)),
            format!("{}\r\nsecret", "x".repeat(256)),
            format!("{}secret\nsecret", "x".repeat(256)),
        ];

        for pattern in [r"(?m)^secret", r"(?m)^secret\b", r"(?m)^(?:secret|token)"] {
            for source in &sources {
                assert_pattern_stream_parity(pattern, source);
            }
        }
    }

    #[test]
    fn line_anchor_lf_does_not_accept_temporary_buffer_end() {
        for pattern in [r"(?m)secret$", r"(?m)^secret$", r"(?m)\bsecret$"] {
            for source in [
                "secretX",
                "secretXY",
                "secret\nX",
                "secret\r\nX",
                "xsecretX",
                "secret🦀",
                "secreté",
            ] {
                assert_pattern_stream_parity(pattern, source);
            }
        }
    }

    #[test]
    fn bounded_streaming_preserves_non_overlapping_selection() {
        for pattern in [
            r"(?:abc|bc)",
            r"(?:abc|bcd)",
            r"(?:abcabc|abc)",
            r"(?:abc|abcabc)",
            r"(?:secretX|secret)",
        ] {
            for source in [
                "abc",
                "abcabc",
                "abcd",
                "abcdbc",
                "abcabcabc",
                "secretXsecret",
                "secretsecretX",
                "xabcabcabc",
            ] {
                assert_pattern_stream_parity(pattern, source);
            }
        }
    }
}
