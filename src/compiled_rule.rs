use std::collections::BTreeMap;

use aho_corasick::{AhoCorasick, AhoCorasickBuilder, Input, MatchKind};
use regex::Regex;

use crate::{
    confidence::Confidence,
    remediation::Remediation,
    rule::{AbsoluteAnchor, Matcher, PatternContext, Rule, RuleId, RuleKind},
    scanner_builder::ScannerBuildError,
    severity::Severity,
    validators::dispatch::ValidatorKind,
};

/// Test instrumentation is updated by actual execution paths. Production builds
/// retain no counters: this type is zero-sized and record() is a no-op.
#[derive(Debug, Default, Clone)]
pub(crate) struct IncrementalWork {
    #[cfg(test)]
    counts: [usize; 11],
}

#[derive(Copy, Clone)]
pub(crate) enum WorkMetric {
    LiteralBytes,
    StructuralBytes,
    StructuralPredicates,
    PrefixPasses,
    PrefixBytes,
    GatePasses,
    GateBytes,
    WholeRuleFallbacks,
    ReusedRecords,
    RevalidatedCandidates,
    RegeneratedCandidates,
}

impl IncrementalWork {
    pub(crate) fn record(&mut self, metric: WorkMetric, count: usize) -> Option<()> {
        #[cfg(test)]
        {
            let slot = &mut self.counts[metric as usize];
            *slot = slot.checked_add(count)?;
        }
        #[cfg(not(test))]
        let _ = (metric, count);
        Some(())
    }

    #[cfg(test)]
    pub(crate) fn get(&self, metric: WorkMetric) -> usize {
        self.counts[metric as usize]
    }
}

/// Exact text is borrowed only for synchronous ingestion. Prefixes may omit it.
#[derive(Debug, Copy, Clone)]
pub(crate) struct CandidateView<'text> {
    pub(crate) finding: InternalFinding,
    pub(crate) text: Option<&'text str>,
}

type CandidateCallback<'a> =
    dyn for<'text> FnMut(CandidateView<'text>) -> Result<(), ResourceLimitExceeded> + 'a;

/// Prefix discovery does not emit or consume a raw candidate.
#[derive(Copy, Clone)]
pub(crate) enum StreamingEvent<'text> {
    Candidate(CandidateView<'text>),
    PrefixDiscovered { rule_index: RuleIndex, start: usize },
    PrefixCompleted(CandidateView<'text>),
}

type EventCallback<'a> =
    dyn for<'text> FnMut(StreamingEvent<'text>) -> Result<(), ResourceLimitExceeded> + 'a;

enum CandidateTarget<'a> {
    #[cfg_attr(not(test), allow(dead_code))]
    Raw(&'a mut Vec<InternalFinding>),
    #[cfg_attr(not(test), allow(dead_code))]
    Callback(&'a mut CandidateCallback<'a>),
    Events(&'a mut EventCallback<'a>),
}

/// Preventive limit on raw streaming candidates, independent of output Vec length.
pub(crate) struct CandidateSink<'a> {
    target: CandidateTarget<'a>,
    emitted: &'a mut usize,
    max_candidates: Option<usize>,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
/// An attempted raw candidate emission exceeds the configured session limit.
pub(crate) struct ResourceLimitExceeded;

impl<'a> CandidateSink<'a> {
    pub(crate) fn with_events(
        callback: &'a mut EventCallback<'a>,
        emitted: &'a mut usize,
        max_candidates: Option<usize>,
    ) -> Self {
        Self {
            target: CandidateTarget::Events(callback),
            emitted,
            max_candidates,
        }
    }

    fn prefix_discovered(
        &mut self,
        rule_index: RuleIndex,
        start: usize,
    ) -> Result<(), ResourceLimitExceeded> {
        if let CandidateTarget::Events(callback) = &mut self.target {
            callback(StreamingEvent::PrefixDiscovered { rule_index, start })?;
        }
        Ok(())
    }

    fn complete_prefix(&mut self, finding: InternalFinding) -> Result<(), ResourceLimitExceeded> {
        self.emit_candidate(
            CandidateView {
                finding,
                text: None,
            },
            true,
        )
    }
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn new(
        findings: &'a mut Vec<InternalFinding>,
        emitted: &'a mut usize,
        max_candidates: Option<usize>,
    ) -> Self {
        Self {
            target: CandidateTarget::Raw(findings),
            emitted,
            max_candidates,
        }
    }

    /// The higher-ranked callback cannot retain a matcher-owned text borrow.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn with_callback(
        callback: &'a mut CandidateCallback<'a>,
        emitted: &'a mut usize,
        max_candidates: Option<usize>,
    ) -> Self {
        Self {
            target: CandidateTarget::Callback(callback),
            emitted,
            max_candidates,
        }
    }

    pub(crate) fn emit(&mut self, finding: InternalFinding) -> Result<(), ResourceLimitExceeded> {
        self.emit_view(CandidateView {
            finding,
            text: None,
        })
    }

    pub(crate) fn emit_view(
        &mut self,
        view: CandidateView<'_>,
    ) -> Result<(), ResourceLimitExceeded> {
        self.emit_candidate(view, false)
    }

    fn emit_candidate(
        &mut self,
        view: CandidateView<'_>,
        prefix_completion: bool,
    ) -> Result<(), ResourceLimitExceeded> {
        if self
            .max_candidates
            .is_some_and(|limit| *self.emitted >= limit)
        {
            return Err(ResourceLimitExceeded);
        }
        debug_assert!(
            view.text
                .is_none_or(|text| { text.len() == view.finding.end - view.finding.start })
        );
        // Finite limits are checked above, so their count is always exact.
        // Saturation keeps None unlimited even at the counter's numeric ceiling.
        // Count admitted raw candidates even if ingestion subsequently fails.
        *self.emitted = self.emitted.saturating_add(1);
        match &mut self.target {
            CandidateTarget::Raw(findings) => {
                findings.push(view.finding);
                Ok(())
            }
            CandidateTarget::Callback(callback) => callback(view),
            CandidateTarget::Events(callback) => callback(if prefix_completion {
                StreamingEvent::PrefixCompleted(view)
            } else {
                StreamingEvent::Candidate(view)
            }),
        }
    }
}

#[cfg(test)]
fn with_unlimited_sink(
    findings: &mut Vec<InternalFinding>,
    scan: impl FnOnce(&mut CandidateSink<'_>) -> Result<(), ResourceLimitExceeded>,
) {
    let mut emitted = 0;
    scan(&mut CandidateSink::new(findings, &mut emitted, None)).unwrap();
}

#[cfg(test)]
mod candidate_sink_tests {
    use super::*;

    #[test]
    fn candidate_sink_checks_limit_before_append_and_counter_increment() {
        let mut findings = vec![InternalFinding::new(RuleIndex::new(0), 10, 11)];
        let mut emitted = usize::MAX - 1;
        let mut sink = CandidateSink::new(&mut findings, &mut emitted, Some(usize::MAX));
        sink.emit(InternalFinding::new(RuleIndex::new(0), 0, 1))
            .unwrap();
        assert_eq!(
            sink.emit(InternalFinding::new(RuleIndex::new(0), 1, 2)),
            Err(ResourceLimitExceeded)
        );
        assert_eq!(emitted, usize::MAX);
        assert_eq!(findings.len(), 2);
        assert_eq!((findings[0].start(), findings[0].end()), (10, 11));
        assert_eq!((findings[1].start(), findings[1].end()), (0, 1));
    }

    #[test]
    fn candidate_sink_none_remains_unlimited_at_counter_ceiling() {
        let mut findings = Vec::new();
        let mut emitted = usize::MAX;
        let mut sink = CandidateSink::new(&mut findings, &mut emitted, None);
        sink.emit(InternalFinding::new(RuleIndex::new(0), 0, 1))
            .unwrap();
        assert_eq!(emitted, usize::MAX);
        assert_eq!(findings.len(), 1);
    }
}

/// Compact index into the immutable rule metadata table.
///
/// Findings carry this index while scanning instead of cloning a `RuleId` and
/// copying rule metadata for every match.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub(crate) struct RuleIndex(u32);

impl RuleIndex {
    #[inline]
    pub(crate) const fn new(index: u32) -> Self {
        Self(index)
    }

    #[inline]
    pub(crate) const fn get(self) -> usize {
        self.0 as usize
    }
}

/// Immutable metadata shared by every finding produced by one rule.
#[derive(Debug)]
pub(crate) struct CompiledRuleMetadata {
    id: RuleId,
    kind: RuleKind,
    severity: Severity,
    confidence: Confidence,
    validator: ValidatorKind,
    remediation: Option<Remediation>,
}

impl CompiledRuleMetadata {
    pub(crate) const fn id(&self) -> &RuleId {
        &self.id
    }

    pub(crate) const fn severity(&self) -> Severity {
        self.severity
    }

    pub(crate) const fn confidence(&self) -> Confidence {
        self.confidence
    }

    pub(crate) const fn validator(&self) -> ValidatorKind {
        self.validator
    }

    pub(crate) const fn remediation(&self) -> Option<Remediation> {
        self.remediation
    }

    /// Returns the normalization priority for findings produced by this rule.
    ///
    /// Provider-specific validators outrank generic contextual detectors when
    /// both accept the exact same source span. The value is internal and may
    /// evolve without affecting the public API.
    pub(crate) const fn priority(&self) -> u16 {
        match self.validator {
            ValidatorKind::None => 0,
            ValidatorKind::GenericCredential => 100,
            ValidatorKind::Password
            | ValidatorKind::SensitiveHash
            | ValidatorKind::DatabaseConnection
            | ValidatorKind::HttpBasic
            | ValidatorKind::WireGuard
            | ValidatorKind::DockerRegistry
            | ValidatorKind::NpmRegistry
            | ValidatorKind::Netrc
            | ValidatorKind::SystemPasswordVerifier => 200,
            ValidatorKind::Jwt | ValidatorKind::Nats => 300,
            ValidatorKind::CodiceFiscale
            | ValidatorKind::Pesel
            | ValidatorKind::NhsNumber
            | ValidatorKind::Ssn
            | ValidatorKind::CardVerificationCode
            | ValidatorKind::OtpProvisioningSecret
            | ValidatorKind::Tailscale
            | ValidatorKind::GitHub
            | ValidatorKind::GitLab
            | ValidatorKind::Stripe
            | ValidatorKind::Pan
            | ValidatorKind::Iban
            | ValidatorKind::Cloudflare
            | ValidatorKind::Slack
            | ValidatorKind::Telegram
            | ValidatorKind::Aws
            | ValidatorKind::Azure
            | ValidatorKind::Gcp
            | ValidatorKind::CargoRegistry
            | ValidatorKind::Pypi
            | ValidatorKind::RubyGems
            | ValidatorKind::RubyGemsHost
            | ValidatorKind::SwiftPm
            | ValidatorKind::SwiftPmNetrc
            | ValidatorKind::Gradle
            | ValidatorKind::Nuget
            | ValidatorKind::Maven
            | ValidatorKind::ComposerHttpBasicPassword
            | ValidatorKind::ComposerBearerToken
            | ValidatorKind::ComposerBitbucketConsumerSecret
            | ValidatorKind::ComposerForgejoToken => 500,
        }
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
enum MultiPatternKind {
    Literal,
    Prefix,
}

#[derive(Debug)]
struct MultiPatternRule {
    rule_index: RuleIndex,
    kind: MultiPatternKind,
    needle: Box<str>,
}

/// Shared multi-pattern executor for literal and prefix rules.
///
/// The source is traversed once for all needles in this group. Prefix rules
/// then extend only the candidates reported by the automaton.
#[derive(Debug)]
struct MultiPatternEngine {
    automaton: AhoCorasick,
    rules: Box<[MultiPatternRule]>,
    max_needle_len: usize,
}

impl MultiPatternEngine {
    #[inline]
    const fn max_overlap_len(&self) -> usize {
        self.max_needle_len.saturating_sub(1)
    }

    fn compile(rules: Vec<MultiPatternRule>) -> Result<Option<Self>, ScannerBuildError> {
        if rules.is_empty() {
            return Ok(None);
        }

        let max_needle_len = rules
            .iter()
            .map(|rule| rule.needle.len())
            .max()
            .unwrap_or(0);

        let automaton = AhoCorasickBuilder::new()
            .match_kind(MatchKind::Standard)
            .build(rules.iter().map(|rule| rule.needle.as_ref()))
            .map_err(ScannerBuildError::AutomatonBuild)?;

        Ok(Some(Self {
            automaton,
            rules: rules.into_boxed_slice(),
            max_needle_len,
        }))
    }

    // Full and regional scans share emission semantics. Input retains the
    // complete haystack: prefix boundaries never observe artificial EOF.
    fn scan_selected(
        &self,
        input: Input<'_>,
        emit: &mut impl FnMut(
            InternalFinding,
        ) -> Result<(), crate::incremental_session::IncrementalError>,
        mut owns: impl FnMut(&MultiPatternRule, usize) -> bool,
    ) -> Result<(), crate::incremental_session::IncrementalError> {
        let bytes = input.haystack();
        for matched in self.automaton.find_overlapping_iter(input.clone()) {
            let rule = &self.rules[matched.pattern().as_usize()];
            let start = matched.start();
            if !owns(rule, start) {
                continue;
            }
            let mut end = matched.end();
            if rule.kind == MultiPatternKind::Prefix {
                if start > 0 && is_token_byte(bytes[start - 1]) {
                    continue;
                }
                while end < bytes.len() && is_token_byte(bytes[end]) {
                    end += 1;
                }
            }
            emit(InternalFinding::new(rule.rule_index, start, end))?;
        }
        Ok(())
    }

    fn update_stream_overlap(&self, state: &mut MultiPatternStreamState, chunk: &[u8]) {
        let keep = self.max_overlap_len();

        if keep == 0 {
            if let Some(&last) = chunk.last() {
                state.byte_before_overlap = Some(last);
            }

            state.overlap.clear();
            return;
        }

        let old_overlap_len = state.overlap.len();
        let total_len = old_overlap_len + chunk.len();

        if total_len <= keep {
            state.overlap.extend_from_slice(chunk);
            return;
        }

        let remove = total_len - keep;

        if remove <= old_overlap_len {
            state.byte_before_overlap = Some(state.overlap[remove - 1]);
            state.overlap.drain(..remove);
            state.overlap.extend_from_slice(chunk);
            return;
        }

        let removed_from_chunk = remove - old_overlap_len;

        state.byte_before_overlap = if removed_from_chunk == 0 {
            state.overlap.last().copied()
        } else {
            Some(chunk[removed_from_chunk - 1])
        };

        state.overlap.clear();
        state
            .overlap
            .extend_from_slice(&chunk[removed_from_chunk..]);
    }

    fn scan_stream_chunk_with_sink(
        &self,
        state: &mut MultiPatternStreamState,
        chunk: &[u8],
        source_offset: usize,
        findings: &mut CandidateSink<'_>,
    ) -> Result<(), ResourceLimitExceeded> {
        // 1. Resolve prefixes already active before this chunk.
        Self::continue_active_prefixes_with_sink(state, chunk, source_offset, findings)?;

        let overlap_len = state.overlap.len();

        debug_assert!(source_offset >= overlap_len);

        // 2. Discover literal + prefix needles wholly inside this chunk.
        for matched in self.automaton.find_overlapping_iter(chunk) {
            let rule = &self.rules[matched.pattern().as_usize()];
            let absolute_start = source_offset + matched.start();

            match rule.kind {
                MultiPatternKind::Literal => {
                    findings.emit_view(CandidateView {
                        finding: InternalFinding::new(
                            rule.rule_index,
                            absolute_start,
                            source_offset + matched.end(),
                        ),
                        text: Some(&rule.needle),
                    })?;
                }

                MultiPatternKind::Prefix => {
                    let left_is_token = if matched.start() == 0 {
                        state
                            .overlap
                            .last()
                            .copied()
                            .or(state.byte_before_overlap)
                            .is_some_and(is_token_byte)
                    } else {
                        is_token_byte(chunk[matched.start() - 1])
                    };

                    if left_is_token {
                        continue;
                    }

                    self.accept_stream_prefix(
                        state,
                        rule,
                        absolute_start,
                        source_offset + matched.end(),
                        &chunk[matched.end()..],
                        findings,
                    )?;
                }
            }
        }

        // 3. Discover needles crossing the old/new boundary.
        if overlap_len != 0 && !chunk.is_empty() {
            let right_len = chunk.len().min(self.max_overlap_len());
            let mut boundary = Vec::with_capacity(overlap_len + right_len);

            boundary.extend_from_slice(&state.overlap);
            boundary.extend_from_slice(&chunk[..right_len]);

            let boundary_base = source_offset - overlap_len;

            for matched in self.automaton.find_overlapping_iter(&boundary) {
                if matched.start() >= overlap_len || matched.end() <= overlap_len {
                    continue;
                }

                let rule = &self.rules[matched.pattern().as_usize()];
                let absolute_start = boundary_base + matched.start();
                let absolute_end = boundary_base + matched.end();

                match rule.kind {
                    MultiPatternKind::Literal => {
                        findings.emit_view(CandidateView {
                            finding: InternalFinding::new(
                                rule.rule_index,
                                absolute_start,
                                absolute_end,
                            ),
                            text: Some(&rule.needle),
                        })?;
                    }

                    MultiPatternKind::Prefix => {
                        let left_is_token = if matched.start() == 0 {
                            state.byte_before_overlap.is_some_and(is_token_byte)
                        } else {
                            is_token_byte(boundary[matched.start() - 1])
                        };

                        if left_is_token {
                            continue;
                        }

                        let suffix_start = matched.end() - overlap_len;

                        self.accept_stream_prefix(
                            state,
                            rule,
                            absolute_start,
                            absolute_end,
                            &chunk[suffix_start..],
                            findings,
                        )?;
                    }
                }
            }
        }

        // 4. Mutate overlap only after every matcher has observed the old boundary.
        self.update_stream_overlap(state, chunk);
        Ok(())
    }

    fn continue_active_prefixes_with_sink(
        state: &mut MultiPatternStreamState,
        chunk: &[u8],
        source_offset: usize,
        findings: &mut CandidateSink<'_>,
    ) -> Result<(), ResourceLimitExceeded> {
        let mut index = 0;

        while index < state.active_prefixes.len() {
            let active = state.active_prefixes[index];

            let token_len = chunk
                .iter()
                .position(|&byte| !is_token_byte(byte))
                .unwrap_or(chunk.len());

            if token_len == chunk.len() {
                index += 1;
                continue;
            }

            findings.complete_prefix(InternalFinding::new(
                active.rule_index,
                active.source_start,
                source_offset + token_len,
            ))?;

            state.active_prefixes.swap_remove(index);
        }
        Ok(())
    }

    fn accept_stream_prefix(
        &self,
        state: &mut MultiPatternStreamState,
        rule: &MultiPatternRule,
        source_start: usize,
        needle_end: usize,
        suffix: &[u8],
        findings: &mut CandidateSink<'_>,
    ) -> Result<(), ResourceLimitExceeded> {
        let token_len = suffix
            .iter()
            .position(|&byte| !is_token_byte(byte))
            .unwrap_or(suffix.len());

        if token_len == suffix.len() {
            if !state.active_prefixes.iter().any(|active| {
                active.rule_index == rule.rule_index && active.source_start == source_start
            }) {
                findings.prefix_discovered(rule.rule_index, source_start)?;
                state.active_prefixes.push(ActivePrefix {
                    rule_index: rule.rule_index,
                    source_start,
                });
            }

            return Ok(());
        }

        findings.emit(InternalFinding::new(
            rule.rule_index,
            source_start,
            needle_end + token_len,
        ))
    }

    fn finish_stream_with_sink(
        state: &mut MultiPatternStreamState,
        source_end: usize,
        findings: &mut CandidateSink<'_>,
    ) -> Result<(), ResourceLimitExceeded> {
        for active in state.active_prefixes.drain(..) {
            findings.complete_prefix(InternalFinding::new(
                active.rule_index,
                active.source_start,
                source_end,
            ))?;
        }
        Ok(())
    }

    #[cfg(test)]
    fn scan_stream_chunk(
        &self,
        state: &mut MultiPatternStreamState,
        chunk: &[u8],
        source_offset: usize,
        findings: &mut Vec<InternalFinding>,
    ) {
        with_unlimited_sink(findings, |sink| {
            self.scan_stream_chunk_with_sink(state, chunk, source_offset, sink)
        });
    }

    #[cfg(test)]
    fn continue_active_prefixes(
        state: &mut MultiPatternStreamState,
        chunk: &[u8],
        source_offset: usize,
        findings: &mut Vec<InternalFinding>,
    ) {
        with_unlimited_sink(findings, |sink| {
            Self::continue_active_prefixes_with_sink(state, chunk, source_offset, sink)
        });
    }

    #[cfg(test)]
    fn finish_stream(
        state: &mut MultiPatternStreamState,
        source_end: usize,
        findings: &mut Vec<InternalFinding>,
    ) {
        with_unlimited_sink(findings, |sink| {
            Self::finish_stream_with_sink(state, source_end, sink)
        });
    }
}

/// Incremental execution state for the shared literal/prefix matcher.
///
/// The compiled [`MultiPatternEngine`] remains immutable and reusable across
/// sources. Each streaming source owns one instance of this state.
///
/// `overlap` retains only the bounded suffix of already consumed input that
/// may participate in a needle crossing the next chunk boundary. It never
/// retains complete historical chunks.
///
/// `active_prefixes` contains prefix rules whose configured prefix has already
/// matched but whose token has not yet reached a terminating boundary.
#[derive(Debug, Default)]
pub(crate) struct MultiPatternStreamState {
    overlap: Vec<u8>,

    /// Byte immediately preceding `overlap`, when one exists.
    ///
    /// This is retained only for evaluating the left token boundary of a
    /// prefix candidate whose start coincides with the beginning of overlap.
    byte_before_overlap: Option<u8>,

    /// Prefix matches whose token continues beyond the latest consumed chunk.
    active_prefixes: Vec<ActivePrefix>,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
struct ActivePrefix {
    rule_index: RuleIndex,
    source_start: usize,
}

/// Maximum byte length of a token eligible for suffix matching.
///
/// Suffix matching extends backwards to the beginning of the containing token.
/// Bounding the token extent makes the same semantics implementable with
/// bounded source-local state during incremental execution.
///
/// Tokens longer than this limit are not eligible for suffix findings; they are
/// never reported as truncated spans.
const MAX_SUFFIX_TOKEN_LEN: usize = 4096;

#[derive(Debug)]
struct SuffixRule {
    rule_index: RuleIndex,
    suffix: Box<str>,
}

impl SuffixRule {
    fn scan_with_emit(
        &self,
        source: &str,
        emit: &mut impl FnMut(
            InternalFinding,
        ) -> Result<(), crate::incremental_session::IncrementalError>,
    ) -> Result<(), crate::incremental_session::IncrementalError> {
        let bytes = source.as_bytes();

        for (suffix_start, _) in source.match_indices(self.suffix.as_ref()) {
            let end = suffix_start + self.suffix.len();

            if end < bytes.len() && is_token_byte(bytes[end]) {
                continue;
            }

            let mut start = suffix_start;

            while start > 0 && end - start < MAX_SUFFIX_TOKEN_LEN && is_token_byte(bytes[start - 1])
            {
                start -= 1;
            }

            // Reaching the bound while the token still continues to the left
            // means the complete token exceeds the supported suffix extent.
            if start > 0 && end - start == MAX_SUFFIX_TOKEN_LEN && is_token_byte(bytes[start - 1]) {
                continue;
            }

            emit(InternalFinding::new(self.rule_index, start, end))?;
        }
        Ok(())
    }

    fn scan_stream_token(
        &self,
        token_start: usize,
        token: &[u8],
        findings: &mut CandidateSink<'_>,
    ) -> Result<(), ResourceLimitExceeded> {
        let suffix = self.suffix.as_bytes();

        if token.ends_with(suffix) {
            findings.emit_view(CandidateView {
                finding: InternalFinding::new(
                    self.rule_index,
                    token_start,
                    token_start + token.len(),
                ),
                text: Some(std::str::from_utf8(token).expect("suffix tokens are ASCII")),
            })?;
        }
        Ok(())
    }
}

/// Incremental execution state for suffix rules.
///
/// Suffix matching needs bounded history from the current token so that a
/// suffix candidate can recover the complete token start without retaining
/// arbitrary source history.
///
/// A token that grows beyond [`MAX_SUFFIX_TOKEN_LEN`] becomes ineligible for
/// suffix findings until the next token boundary. It is never represented as a
/// truncated token.
#[derive(Debug, Default)]
pub(crate) struct SuffixStreamState {
    /// Bytes retained for the current token while it remains eligible.
    token: Vec<u8>,

    /// Absolute byte offset at which the current token begins.
    token_start: usize,

    /// Whether the current token exceeded the supported suffix extent.
    oversized: bool,
}

impl SuffixStreamState {
    /// Consumes one source chunk and yields each complete eligible token.
    ///
    /// Only token bytes are retained. Once a token exceeds
    /// [`MAX_SUFFIX_TOKEN_LEN`], its retained bytes are discarded and the
    /// token remains ineligible until its terminating boundary.
    fn push_chunk_with_sink(
        &mut self,
        chunk: &[u8],
        source_offset: usize,
        mut on_token: impl FnMut(usize, &[u8]) -> Result<(), ResourceLimitExceeded>,
    ) -> Result<(), ResourceLimitExceeded> {
        for (index, &byte) in chunk.iter().enumerate() {
            let absolute_offset = source_offset + index;

            if is_token_byte(byte) {
                if self.token.is_empty() && !self.oversized {
                    self.token_start = absolute_offset;
                }

                if self.oversized {
                    continue;
                }

                if self.token.len() < MAX_SUFFIX_TOKEN_LEN {
                    self.token.push(byte);
                } else {
                    self.token.clear();
                    self.oversized = true;
                }

                continue;
            }

            self.finish_token(&mut on_token)?;
        }
        Ok(())
    }

    /// Treats end-of-source as a token boundary.
    fn finish_with_sink(
        &mut self,
        mut on_token: impl FnMut(usize, &[u8]) -> Result<(), ResourceLimitExceeded>,
    ) -> Result<(), ResourceLimitExceeded> {
        self.finish_token(&mut on_token)
    }

    fn finish_token(
        &mut self,
        on_token: &mut impl FnMut(usize, &[u8]) -> Result<(), ResourceLimitExceeded>,
    ) -> Result<(), ResourceLimitExceeded> {
        if !self.oversized && !self.token.is_empty() {
            on_token(self.token_start, &self.token)?;
        }

        self.token.clear();
        self.oversized = false;
        Ok(())
    }

    #[cfg(test)]
    fn push_chunk(
        &mut self,
        chunk: &[u8],
        source_offset: usize,
        mut on_token: impl FnMut(usize, &[u8]),
    ) {
        self.push_chunk_with_sink(chunk, source_offset, |start, token| {
            on_token(start, token);
            Ok(())
        })
        .unwrap();
    }

    #[cfg(test)]
    fn finish(&mut self, mut on_token: impl FnMut(usize, &[u8])) {
        self.finish_with_sink(|start, token| {
            on_token(start, token);
            Ok(())
        })
        .unwrap();
    }
}

#[cfg(test)]
mod suffix_tests {
    use super::*;
    use crate::{Rule, Severity};

    fn scan(source: &str) -> Vec<(usize, usize)> {
        let compiled =
            CompiledRuleSet::compile(vec![Rule::suffix("suffix", "_end", Severity::High)])
                .expect("rule should compile");

        let mut findings = Vec::new();
        compiled.scan(source, &mut findings);

        findings
            .into_iter()
            .map(|finding| (finding.start(), finding.end()))
            .collect()
    }

    #[test]
    fn suffix_stream_emits_complete_token_at_boundary() {
        let mut state = SuffixStreamState::default();
        let mut tokens = Vec::new();

        state.push_chunk(b"abc_end!", 0, |start, token| {
            tokens.push((start, token.to_vec()));
        });

        assert_eq!(tokens, vec![(0, b"abc_end".to_vec())]);
        assert!(state.token.is_empty());
        assert!(!state.oversized);
    }

    #[test]
    fn suffix_stream_preserves_token_across_chunks() {
        let mut state = SuffixStreamState::default();
        let mut tokens = Vec::new();

        state.push_chunk(b"before ab", 0, |start, token| {
            tokens.push((start, token.to_vec()));
        });
        state.push_chunk(b"c_end!", 9, |start, token| {
            tokens.push((start, token.to_vec()));
        });

        assert_eq!(
            tokens,
            vec![(0, b"before".to_vec()), (7, b"abc_end".to_vec()),]
        );
    }

    #[test]
    fn suffix_stream_finishes_token_at_end_of_source() {
        let mut state = SuffixStreamState::default();
        let mut tokens = Vec::new();

        state.push_chunk(b"abc_end", 0, |start, token| {
            tokens.push((start, token.to_vec()));
        });

        assert!(tokens.is_empty());

        state.finish(|start, token| {
            tokens.push((start, token.to_vec()));
        });

        assert_eq!(tokens, vec![(0, b"abc_end".to_vec())]);
    }

    #[test]
    fn suffix_stream_accepts_token_at_maximum_extent() {
        let source = vec![b'a'; MAX_SUFFIX_TOKEN_LEN];

        let mut state = SuffixStreamState::default();
        let mut tokens = Vec::new();

        state.push_chunk(&source, 0, |start, token| {
            tokens.push((start, token.len()));
        });
        state.finish(|start, token| {
            tokens.push((start, token.len()));
        });

        assert_eq!(tokens, vec![(0, MAX_SUFFIX_TOKEN_LEN)]);
        assert!(!state.oversized);
    }

    #[test]
    fn suffix_stream_discards_token_beyond_maximum_extent() {
        let source = vec![b'a'; MAX_SUFFIX_TOKEN_LEN + 1];

        let mut state = SuffixStreamState::default();
        let mut tokens = Vec::new();

        state.push_chunk(&source, 0, |start, token| {
            tokens.push((start, token.len()));
        });

        assert!(tokens.is_empty());
        assert!(state.oversized);
        assert!(state.token.is_empty());

        state.finish(|start, token| {
            tokens.push((start, token.len()));
        });

        assert!(tokens.is_empty());
        assert!(!state.oversized);
        assert!(state.token.is_empty());
    }

    #[test]
    fn suffix_stream_recovers_after_oversized_token() {
        let mut source = vec![b'a'; MAX_SUFFIX_TOKEN_LEN + 1];
        source.extend_from_slice(b"!valid_end!");

        let mut state = SuffixStreamState::default();
        let mut tokens = Vec::new();

        state.push_chunk(&source, 0, |start, token| {
            tokens.push((start, token.to_vec()));
        });

        assert_eq!(
            tokens,
            vec![(MAX_SUFFIX_TOKEN_LEN + 2, b"valid_end".to_vec())]
        );
    }

    #[test]
    fn suffix_stream_bound_is_independent_of_chunking() {
        let source = vec![b'a'; MAX_SUFFIX_TOKEN_LEN + 1];

        for split in 0..=source.len() {
            let mut state = SuffixStreamState::default();
            let mut emitted = 0;

            state.push_chunk(&source[..split], 0, |_, _| {
                emitted += 1;
            });
            state.push_chunk(&source[split..], split, |_, _| {
                emitted += 1;
            });

            assert_eq!(emitted, 0, "split at byte {split}");
            assert!(
                state.token.len() <= MAX_SUFFIX_TOKEN_LEN,
                "split at byte {split}"
            );

            state.finish(|_, _| {
                emitted += 1;
            });

            assert_eq!(emitted, 0, "split at byte {split}");
        }
    }

    #[test]
    fn suffix_accepts_token_at_maximum_extent() {
        let source = format!("{}{}", "a".repeat(MAX_SUFFIX_TOKEN_LEN - 4), "_end");

        assert_eq!(source.len(), MAX_SUFFIX_TOKEN_LEN);
        assert_eq!(scan(&source), vec![(0, MAX_SUFFIX_TOKEN_LEN)]);
    }

    #[test]
    fn suffix_rejects_token_beyond_maximum_extent() {
        let source = format!("{}{}", "a".repeat(MAX_SUFFIX_TOKEN_LEN - 3), "_end");

        assert_eq!(source.len(), MAX_SUFFIX_TOKEN_LEN + 1);
        assert!(scan(&source).is_empty());
    }

    #[test]
    fn suffix_accepts_token_below_maximum_extent() {
        let source = format!("{}{}", "a".repeat(MAX_SUFFIX_TOKEN_LEN - 5), "_end");

        assert_eq!(source.len(), MAX_SUFFIX_TOKEN_LEN - 1);
        assert_eq!(scan(&source), vec![(0, MAX_SUFFIX_TOKEN_LEN - 1)]);
    }

    #[test]
    fn suffix_requires_trailing_token_boundary() {
        assert!(scan("abc_endx").is_empty());
        assert_eq!(scan("abc_end!"), vec![(0, 7)]);
    }

    #[test]
    fn suffix_recovers_start_after_left_boundary() {
        assert_eq!(scan("before abc_end!"), vec![(7, 14)]);
    }

    #[test]
    fn oversized_token_does_not_hide_later_valid_token() {
        let oversized = format!("{}{}", "a".repeat(MAX_SUFFIX_TOKEN_LEN), "_end");
        let source = format!("{oversized} valid_end!");

        assert_eq!(
            scan(&source),
            vec![(oversized.len() + 1, oversized.len() + 10)]
        );
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
enum PatternStreamPlan {
    Bounded {
        max_match_len: usize,
        context: PatternStreamContext,
    },

    Unsupported,
}

#[derive(Debug, Copy, Clone, Default, Eq, PartialEq)]
struct PatternStreamContext {
    history_bytes: usize,
    lookahead_bytes: usize,
    requires_text_start: bool,
    requires_text_end: bool,
}

impl PatternStreamPlan {
    fn compile(max_match_len: Option<usize>, context: PatternContext) -> Self {
        match max_match_len {
            Some(max_match_len) if context.is_context_free() => Self::Bounded {
                max_match_len,
                context: PatternStreamContext::default(),
            },

            Some(max_match_len) if context.is_word_only() => Self::Bounded {
                max_match_len,
                context: PatternStreamContext {
                    history_bytes: 4,
                    lookahead_bytes: 4,
                    requires_text_start: false,
                    requires_text_end: false,
                },
            },

            Some(max_match_len)
                if context.absolute_start() == AbsoluteAnchor::None
                    && context.absolute_end() == AbsoluteAnchor::Required
                    && context.has_only_text_end_and_word_context() =>
            {
                Self::Bounded {
                    max_match_len,
                    context: PatternStreamContext {
                        history_bytes: 4,
                        lookahead_bytes: 0,
                        requires_text_start: false,
                        requires_text_end: true,
                    },
                }
            }

            Some(max_match_len)
                if context.absolute_start() == AbsoluteAnchor::Required
                    && context.absolute_end() == AbsoluteAnchor::Required
                    && context.has_only_absolute_anchors_and_word_context() =>
            {
                Self::Bounded {
                    max_match_len,
                    context: PatternStreamContext {
                        history_bytes: 0,
                        lookahead_bytes: 0,
                        requires_text_start: true,
                        requires_text_end: true,
                    },
                }
            }

            Some(max_match_len)
                if context.absolute_start() == AbsoluteAnchor::Required
                    && context.absolute_end() == AbsoluteAnchor::None
                    && context.has_only_text_start_and_word_context() =>
            {
                Self::Bounded {
                    max_match_len,
                    context: PatternStreamContext {
                        history_bytes: 0,
                        lookahead_bytes: 4,
                        requires_text_start: true,
                        requires_text_end: false,
                    },
                }
            }

            // Required absolute anchors retain the real start or defer to
            // the real EOF. Local line/word assertions use the same bounded
            // history and lookahead as standalone line contexts.
            Some(max_match_len)
                if context.has_only_uniform_line_and_absolute_context()
                    && context.absolute_start() != AbsoluteAnchor::Conditional
                    && context.absolute_end() != AbsoluteAnchor::Conditional
                    && (context.absolute_start() == AbsoluteAnchor::Required
                        || context.absolute_end() == AbsoluteAnchor::Required) =>
            {
                let requires_text_start = context.absolute_start() == AbsoluteAnchor::Required;
                let requires_text_end = context.absolute_end() == AbsoluteAnchor::Required;
                Self::Bounded {
                    max_match_len,
                    context: PatternStreamContext {
                        history_bytes: if requires_text_start { 0 } else { 4 },
                        lookahead_bytes: if requires_text_end { 0 } else { 4 },
                        requires_text_start,
                        requires_text_end,
                    },
                }
            }

            // LF or CRLF line anchors, optionally combined
            // with Unicode word boundaries.
            Some(max_match_len)
                if context.is_lf_line_context() || context.is_crlf_line_context() =>
            {
                Self::Bounded {
                    max_match_len,
                    context: PatternStreamContext {
                        history_bytes: 4,
                        lookahead_bytes: 4,
                        requires_text_start: false,
                        requires_text_end: false,
                    },
                }
            }

            _ => Self::Unsupported,
        }
    }
}

#[derive(Debug, Default)]
pub(crate) struct PatternStreamState {
    buffer: Vec<u8>,
    buffer_offset: usize,
    resolved_before: usize,
    next_search_offset: usize,
    #[cfg(test)]
    peak_buffer_capacity: usize,
    #[cfg(test)]
    peak_buffer_len: usize,
}

impl PatternStreamState {
    fn append_bounded(&mut self, bytes: &[u8], capacity_limit: usize) {
        let required = self.buffer.len() + bytes.len();
        debug_assert!(required <= capacity_limit);
        if required > self.buffer.capacity() {
            let capacity = self
                .buffer
                .capacity()
                .saturating_mul(2)
                .max(required)
                .min(capacity_limit);
            self.buffer.reserve_exact(capacity - self.buffer.len());
        }
        self.buffer.extend_from_slice(bytes);
        #[cfg(test)]
        {
            self.peak_buffer_capacity = self.peak_buffer_capacity.max(self.buffer.capacity());
            self.peak_buffer_len = self.peak_buffer_len.max(self.buffer.len());
        }
    }

    #[cfg(test)]
    fn buffered_len(&self) -> usize {
        self.buffer.len()
    }
}

#[derive(Debug)]
struct PatternRule {
    rule_index: RuleIndex,
    pattern: Regex,
    capture: Option<usize>,
    stream_plan: PatternStreamPlan,
    prefilter: Option<AhoCorasick>,
    gate_bit: Option<u8>,
}

impl PatternRule {
    #[cfg(test)]
    const fn stream_plan(&self) -> PatternStreamPlan {
        self.stream_plan
    }

    #[cfg(test)]
    fn scan(&self, source: &str, findings: &mut Vec<InternalFinding>) {
        self.scan_with_emit(source, &mut |finding| {
            findings.push(finding);
            Ok(())
        })
        .expect("Vec emission cannot fail");
    }

    fn scan_with_emit(
        &self,
        source: &str,
        emit: &mut impl FnMut(
            InternalFinding,
        ) -> Result<(), crate::incremental_session::IncrementalError>,
    ) -> Result<(), crate::incremental_session::IncrementalError> {
        match (&self.prefilter, self.capture) {
            (Some(prefilter), Some(capture)) => {
                self.scan_prefiltered_captures(source, emit, prefilter, capture)?
            }
            (Some(prefilter), None) => self.scan_prefiltered_matches(source, emit, prefilter)?,
            (None, None) => {
                for matched in self.pattern.find_iter(source) {
                    emit(InternalFinding::new(
                        self.rule_index,
                        matched.start(),
                        matched.end(),
                    ))?;
                }
            }
            (None, Some(capture)) => {
                for captures in self.pattern.captures_iter(source) {
                    if let Some(matched) = captures.get(capture) {
                        emit(InternalFinding::new(
                            self.rule_index,
                            matched.start(),
                            matched.end(),
                        ))?;
                    }
                }
            }
        }
        Ok(())
    }

    fn scan_prefiltered_captures(
        &self,
        source: &str,
        emit: &mut impl FnMut(
            InternalFinding,
        ) -> Result<(), crate::incremental_session::IncrementalError>,
        prefilter: &AhoCorasick,
        capture: usize,
    ) -> Result<(), crate::incremental_session::IncrementalError> {
        let bytes = source.as_bytes();

        for key_match in prefilter.find_iter(source) {
            let key_start = key_match.start();
            let search_start = optional_quote_start(bytes, key_start);

            let Some(captures) = self.pattern.captures_at(source, search_start) else {
                continue;
            };
            let Some(complete) = captures.get(0) else {
                continue;
            };

            // `captures_at` searches at or after the supplied offset. A
            // prefilter hit is only authoritative as a starting hint, so reject
            // any later regex match and let its own key occurrence trigger it.
            if complete.start() != search_start && complete.start() != key_start {
                continue;
            }

            if let Some(matched) = captures.get(capture) {
                emit(InternalFinding::new(
                    self.rule_index,
                    matched.start(),
                    matched.end(),
                ))?;
            }
        }
        Ok(())
    }

    fn scan_prefiltered_matches(
        &self,
        source: &str,
        emit: &mut impl FnMut(
            InternalFinding,
        ) -> Result<(), crate::incremental_session::IncrementalError>,
        prefilter: &AhoCorasick,
    ) -> Result<(), crate::incremental_session::IncrementalError> {
        let bytes = source.as_bytes();

        for key_match in prefilter.find_iter(source) {
            let key_start = key_match.start();
            let search_start = optional_quote_start(bytes, key_start);

            let Some(matched) = self.pattern.find_at(source, search_start) else {
                continue;
            };

            if matched.start() == search_start || matched.start() == key_start {
                emit(InternalFinding::new(
                    self.rule_index,
                    matched.start(),
                    matched.end(),
                ))?;
            }
        }
        Ok(())
    }

    fn scan_stream_bounded_with_sink(
        &self,
        state: &mut PatternStreamState,
        chunk: &[u8],
        source_offset: usize,
        findings: &mut CandidateSink<'_>,
    ) -> Result<(), ResourceLimitExceeded> {
        let PatternStreamPlan::Bounded {
            max_match_len,
            context,
        } = self.stream_plan
        else {
            return Ok(());
        };
        if (context.requires_text_start && context.requires_text_end) || chunk.is_empty() {
            return self.scan_stream_bounded_window(state, chunk, source_offset, findings);
        }

        // One rule-sized window plus retained context fits in two windows.
        // Four extra bytes allow a complete UTF-8 scalar at an internal edge.
        let window_bytes = max_match_len
            .saturating_add(context.history_bytes)
            .saturating_add(context.lookahead_bytes)
            .saturating_add(4);
        let mut offset = 0;
        while offset < chunk.len() {
            if context.requires_text_start && state.resolved_before > 0 {
                break;
            }
            let mut end = offset.saturating_add(window_bytes).min(chunk.len());
            // Transport input is validated UTF-8. Never split a scalar at an
            // artificial internal edge (transport partitions remain intact).
            while end < chunk.len() && chunk[end] & 0xC0 == 0x80 {
                end -= 1;
            }
            self.scan_stream_bounded_window(
                state,
                &chunk[offset..end],
                source_offset + offset,
                findings,
            )?;
            offset = end;
        }
        Ok(())
    }

    fn scan_stream_bounded_window(
        &self,
        state: &mut PatternStreamState,
        chunk: &[u8],
        source_offset: usize,
        findings: &mut CandidateSink<'_>,
    ) -> Result<(), ResourceLimitExceeded> {
        let PatternStreamPlan::Bounded {
            max_match_len,
            context,
        } = self.stream_plan
        else {
            return Ok(());
        };

        // Absolute start + absolute end:
        // retain the entire candidate only while its maximum length permits it.
        if context.requires_text_start && context.requires_text_end {
            if state.resolved_before > 0 {
                return Ok(());
            }

            if state.buffer.is_empty() {
                state.buffer_offset = source_offset;
            }

            if chunk.len() > max_match_len.saturating_sub(state.buffer.len()) {
                state.buffer.clear();
                state.resolved_before = 1;
                return Ok(());
            }
            state.append_bounded(chunk, max_match_len);

            return Ok(());
        }

        // A match requiring absolute start cannot become valid again
        // after its initial candidate has been resolved.
        if context.requires_text_start && state.resolved_before > 0 {
            return Ok(());
        }

        if state.buffer.is_empty() {
            state.buffer_offset = source_offset;
        }

        let window_bytes = max_match_len
            .saturating_add(context.history_bytes)
            .saturating_add(context.lookahead_bytes)
            .saturating_add(4);
        state.append_bounded(chunk, window_bytes.saturating_mul(2).saturating_add(4));

        let observed_end = state.buffer_offset + state.buffer.len();

        // Absolute end:
        // matching is deferred until the real source EOF.
        if context.requires_text_end {
            let retain_bytes = max_match_len.saturating_add(context.history_bytes);
            let excess = state.buffer.len().saturating_sub(retain_bytes);

            let source = std::str::from_utf8(&state.buffer)
                .expect("pattern stream receives validated UTF-8");

            let mut cut = excess;

            while cut > 0 && !source.is_char_boundary(cut) {
                cut -= 1;
            }

            if cut > 0 {
                state.buffer.drain(..cut);
                state.buffer_offset += cut;
            }

            return Ok(());
        }

        let Ok(source) = std::str::from_utf8(&state.buffer) else {
            return Ok(());
        };

        // A match starting before this frontier has sufficient observed
        // input to be resolved without depending on future chunks.
        let new_resolved_before = observed_end
            .checked_sub(max_match_len)
            .and_then(|offset| offset.checked_sub(context.lookahead_bytes))
            .and_then(|offset| offset.checked_add(1))
            .unwrap_or(0);

        // Keep the resolution frontier separate from the end of the
        // last emitted complete match.
        let search_from = state.resolved_before.max(state.next_search_offset);

        let mut local_search_from = search_from
            .saturating_sub(state.buffer_offset)
            .min(source.len());

        match self.capture {
            Some(capture) => {
                while local_search_from <= source.len() {
                    let Some(captures) = self.pattern.captures_at(source, local_search_from) else {
                        break;
                    };

                    let Some(complete) = captures.get(0) else {
                        break;
                    };

                    let complete_start = state.buffer_offset + complete.start();

                    // This match is not yet stable against future input.
                    if complete_start >= new_resolved_before {
                        break;
                    }

                    let complete_end = state.buffer_offset + complete.end();

                    if let Some(matched) = captures.get(capture) {
                        findings.emit_view(CandidateView {
                            finding: InternalFinding::new(
                                self.rule_index,
                                state.buffer_offset + matched.start(),
                                state.buffer_offset + matched.end(),
                            ),
                            text: Some(matched.as_str()),
                        })?;
                    }

                    // Non-overlapping selection advances using the complete
                    // match, never the projected capture.
                    state.next_search_offset = complete_end;

                    local_search_from = complete.end();

                    // Empty matches are rejected by the rule contract,
                    // but keep an explicit guard against stalled iteration.
                    if complete.start() == complete.end() {
                        break;
                    }
                }
            }

            None => {
                while local_search_from <= source.len() {
                    let Some(matched) = self.pattern.find_at(source, local_search_from) else {
                        break;
                    };

                    let start = state.buffer_offset + matched.start();

                    if start >= new_resolved_before {
                        break;
                    }

                    let end = state.buffer_offset + matched.end();

                    findings.emit_view(CandidateView {
                        finding: InternalFinding::new(self.rule_index, start, end),
                        text: Some(matched.as_str()),
                    })?;

                    state.next_search_offset = end;
                    local_search_from = matched.end();

                    if matched.start() == matched.end() {
                        break;
                    }
                }
            }
        }

        state.resolved_before = state.resolved_before.max(new_resolved_before);

        // Absolute-start candidates need no retained prefix after their
        // only possible starting position has been resolved.
        if context.requires_text_start && state.resolved_before > 0 {
            state.buffer.clear();
            state.buffer_offset = observed_end;
            return Ok(());
        }

        let cut = {
            let source = std::str::from_utf8(&state.buffer)
                .expect("pattern stream receives validated UTF-8");

            let retain_from = state.resolved_before.saturating_sub(context.history_bytes);

            let mut cut = retain_from
                .saturating_sub(state.buffer_offset)
                .min(state.buffer.len());

            while cut > 0 && !source.is_char_boundary(cut) {
                cut -= 1;
            }

            cut
        };

        if cut > 0 {
            state.buffer.drain(..cut);
            state.buffer_offset += cut;
        }
        Ok(())
    }

    fn finish_stream_bounded_with_sink(
        &self,
        state: &mut PatternStreamState,
        findings: &mut CandidateSink<'_>,
    ) -> Result<(), ResourceLimitExceeded> {
        let PatternStreamPlan::Bounded { context, .. } = self.stream_plan else {
            return Ok(());
        };

        // An absolute-start candidate cannot become valid again
        // once its only possible starting position has been resolved.
        if context.requires_text_start && state.resolved_before > 0 {
            return Ok(());
        }

        let source =
            std::str::from_utf8(&state.buffer).expect("pattern stream receives validated UTF-8");

        // The resolution frontier and the non-overlapping match frontier
        // are independent. Both must be respected at EOF.
        let search_from = state.resolved_before.max(state.next_search_offset);

        let mut local_search_from = search_from
            .saturating_sub(state.buffer_offset)
            .min(source.len());

        match self.capture {
            Some(capture) => {
                while local_search_from <= source.len() {
                    let Some(captures) = self.pattern.captures_at(source, local_search_from) else {
                        break;
                    };

                    let Some(complete) = captures.get(0) else {
                        break;
                    };

                    let complete_end = state.buffer_offset + complete.end();

                    if let Some(matched) = captures.get(capture) {
                        findings.emit_view(CandidateView {
                            finding: InternalFinding::new(
                                self.rule_index,
                                state.buffer_offset + matched.start(),
                                state.buffer_offset + matched.end(),
                            ),
                            text: Some(matched.as_str()),
                        })?;
                    }

                    // Advance by the complete match, not by its capture.
                    state.next_search_offset = complete_end;
                    local_search_from = complete.end();

                    // The rule contract excludes empty matches.
                    // Keep a defensive guard against stalled iteration.
                    if complete.start() == complete.end() {
                        break;
                    }
                }
            }

            None => {
                while local_search_from <= source.len() {
                    let Some(matched) = self.pattern.find_at(source, local_search_from) else {
                        break;
                    };

                    let start = state.buffer_offset + matched.start();
                    let end = state.buffer_offset + matched.end();

                    findings.emit_view(CandidateView {
                        finding: InternalFinding::new(self.rule_index, start, end),
                        text: Some(matched.as_str()),
                    })?;

                    state.next_search_offset = end;
                    local_search_from = matched.end();

                    if matched.start() == matched.end() {
                        break;
                    }
                }
            }
        }

        state.resolved_before = state.buffer_offset + state.buffer.len();
        state.buffer.clear();
        state.buffer_offset = state.resolved_before;
        Ok(())
    }

    #[cfg(test)]
    fn scan_stream_bounded(
        &self,
        state: &mut PatternStreamState,
        chunk: &[u8],
        source_offset: usize,
        findings: &mut Vec<InternalFinding>,
    ) {
        with_unlimited_sink(findings, |sink| {
            self.scan_stream_bounded_with_sink(state, chunk, source_offset, sink)
        });
    }

    #[cfg(test)]
    fn finish_stream_bounded(
        &self,
        state: &mut PatternStreamState,
        findings: &mut Vec<InternalFinding>,
    ) {
        with_unlimited_sink(findings, |sink| {
            self.finish_stream_bounded_with_sink(state, sink)
        });
    }
}

/// One shared prefilter pass for all contextual pattern rules.
///
/// Individual rule prefilters remain authoritative execution hints. This gate
/// only determines which of those rule-specific prefilters can possibly match
/// the current source, avoiding one full-source prefilter pass per inactive
/// contextual rule.
#[derive(Debug)]
struct PatternPrefilterGate {
    automaton: AhoCorasick,
    target_masks: Box<[u128]>,
}

impl PatternPrefilterGate {
    fn compile(needles: BTreeMap<&'static str, u128>) -> Result<Option<Self>, ScannerBuildError> {
        if needles.is_empty() {
            return Ok(None);
        }

        let patterns = needles.keys().copied().collect::<Vec<_>>();
        let target_masks = needles
            .values()
            .copied()
            .collect::<Vec<_>>()
            .into_boxed_slice();

        let automaton = AhoCorasickBuilder::new()
            .ascii_case_insensitive(true)
            .match_kind(MatchKind::Standard)
            .build(patterns)
            .map_err(ScannerBuildError::AutomatonBuild)?;

        Ok(Some(Self {
            automaton,
            target_masks,
        }))
    }

    #[inline]
    fn active_mask(&self, source: &str) -> u128 {
        let mut active = 0_u128;

        for matched in self.automaton.find_overlapping_iter(source) {
            active |= self.target_masks[matched.pattern().as_usize()];
        }

        active
    }
}

/// Private execution plan compiled from the scanner's configured rules.
///
/// Literal and prefix rules share one Aho-Corasick automaton. Suffix and
/// regular-expression rules remain in dedicated serial groups until measured
/// workloads justify a more specialized representation.
#[derive(Debug, Default)]
pub(crate) struct CompiledRuleSet {
    metadata: Box<[CompiledRuleMetadata]>,
    multi_pattern: Option<MultiPatternEngine>,
    suffixes: Box<[SuffixRule]>,
    patterns: Box<[PatternRule]>,
    pattern_gate: Option<PatternPrefilterGate>,
}

/// Maximum logical payload lengths of the executor's source buffers.
///
/// These are not allocation limits: Vec capacity can exceed logical length,
/// including after `reserve_exact`, and allocator overhead is not included.
/// Fixed state metadata (including the pattern-state Vec), compiled matchers,
/// and regex-library search/capture workspace are outside this subtotal.
/// Library workspace has no rigorous bound established by this accounting.
/// Caller-owned chunks and findings are also excluded; unlimited findings have
/// no ruleset-derived bound independent of source length.
#[allow(dead_code)] // Component breakdown is retained for internal diagnostics.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct StreamingStorageRequirements {
    pub(crate) overlap_bytes: usize,
    pub(crate) active_prefix_elements: usize,
    pub(crate) active_prefix_bytes: usize,
    pub(crate) boundary_bytes: usize,
    pub(crate) suffix_token_bytes: usize,
    /// One entry per regex, in compiled pattern order (not rule-index order).
    pub(crate) pattern_buffer_bytes: Box<[usize]>,
    pub(crate) persistent_buffer_bytes: usize,
    pub(crate) temporary_buffer_bytes: usize,
    pub(crate) total_buffer_bytes: usize,
}

#[allow(dead_code)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub(crate) enum StreamingStorageError {
    ArithmeticOverflow,
    UnsupportedPattern(RuleIndex),
}

#[allow(dead_code)]
impl StreamingStorageRequirements {
    fn pattern_buffer_bytes(plan: PatternStreamPlan) -> Result<usize, StreamingStorageError> {
        let PatternStreamPlan::Bounded {
            max_match_len,
            context,
        } = plan
        else {
            unreachable!("unsupported plans are rejected before accounting");
        };
        if context.requires_text_start && context.requires_text_end {
            // Overlong candidates are rejected before appending any bytes.
            return Ok(max_match_len);
        }
        // W = M + history + lookahead + 4. Each append is at most W bytes.
        // After trimming, retention is at most M + history + lookahead + 3
        // (the extra bytes preserve a UTF-8 scalar). Even before trimming,
        // 2W + 4 therefore conservatively covers the logical buffer length.
        max_match_len
            .checked_add(context.history_bytes)
            .and_then(|bytes| bytes.checked_add(context.lookahead_bytes))
            .and_then(|bytes| bytes.checked_add(4))
            .and_then(|bytes| bytes.checked_mul(2))
            .and_then(|bytes| bytes.checked_add(4))
            .ok_or(StreamingStorageError::ArithmeticOverflow)
    }

    fn from_components(
        overlap_bytes: usize,
        active_prefix_elements: usize,
        has_suffix_rules: bool,
        pattern_buffer_bytes: Box<[usize]>,
    ) -> Result<Self, StreamingStorageError> {
        let overflow = StreamingStorageError::ArithmeticOverflow;
        let active_prefix_bytes = active_prefix_elements
            .checked_mul(std::mem::size_of::<ActivePrefix>())
            .ok_or(overflow)?;
        // One crossing-boundary temporary holds old overlap and at most an
        // overlap-length prefix of the next chunk; it is dropped before regexes.
        let boundary_bytes = overlap_bytes.checked_mul(2).ok_or(overflow)?;
        let suffix_token_bytes = if has_suffix_rules {
            MAX_SUFFIX_TOKEN_LEN
        } else {
            0
        };
        let persistent_buffer_bytes = pattern_buffer_bytes.iter().try_fold(
            overlap_bytes
                .checked_add(active_prefix_bytes)
                .and_then(|bytes| bytes.checked_add(suffix_token_bytes))
                .ok_or(overflow)?,
            |bytes, pattern| bytes.checked_add(*pattern).ok_or(overflow),
        )?;
        let temporary_buffer_bytes = boundary_bytes;
        let total_buffer_bytes = persistent_buffer_bytes
            .checked_add(temporary_buffer_bytes)
            .ok_or(overflow)?;
        Ok(Self {
            overlap_bytes,
            active_prefix_elements,
            active_prefix_bytes,
            boundary_bytes,
            suffix_token_bytes,
            pattern_buffer_bytes,
            persistent_buffer_bytes,
            temporary_buffer_bytes,
            total_buffer_bytes,
        })
    }
}

/// Planned coordinate storage, independent of matcher and validator budgets.
///
/// This plans one UTF-8 tail with a coordinate anchor, a current cursor, and
/// one discovery-time coordinate pin per prefix rule. It allocates no buffers.
/// Future annotation must occur at matcher emission, before validation can be
/// deferred; accepted-record completion time is not bounded by matcher delay.
/// Logical payload and metadata exclude spare capacity, allocator overhead,
/// compiled matchers, validator workspace, accepted records and final output.
#[allow(dead_code)] // Consumed when candidate-coordinate annotation is enabled.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct CoordinateStoragePlan {
    pub(crate) non_prefix_reach_back_bytes: usize,
    pub(crate) prefix_discovery_reach_back_bytes: usize,
    pub(crate) history_bytes: usize,
    pub(crate) anchor_bytes: usize,
    pub(crate) cursor_bytes: usize,
    pub(crate) max_prefix_pins: usize,
    pub(crate) prefix_pin_bytes: usize,
    pub(crate) total_bytes: usize,
}

#[allow(dead_code)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub(crate) enum CoordinateStoragePlanningError {
    UnsupportedRuleSet,
    ArithmeticOverflow,
}

/// Future pins preserve starts of arbitrarily long prefixes without token text.
#[allow(dead_code)]
#[derive(Debug)]
pub(crate) struct PlannedPrefixCoordinatePin {
    pub(crate) rule_index: RuleIndex,
    pub(crate) coordinates: crate::source_coordinates::SourceCoordinates,
}

#[allow(dead_code)]
impl CoordinateStoragePlan {
    fn from_components(
        non_prefix_reach_back_bytes: usize,
        prefix_discovery_reach_back_bytes: usize,
        max_prefix_pins: usize,
    ) -> Result<Self, CoordinateStoragePlanningError> {
        let overflow = CoordinateStoragePlanningError::ArithmeticOverflow;
        let reach = non_prefix_reach_back_bytes.max(prefix_discovery_reach_back_bytes);
        // At callback base B, older starts/discoveries are within reach of B.
        // Borrow the current callback; retain only this prior tail. An outward
        // UTF-8 cut adds at most three bytes. After discovery, prefix pins must
        // survive independently of this tail until token completion or EOF.
        let history_bytes = if reach == 0 {
            0
        } else {
            reach.checked_add(3).ok_or(overflow)?
        };
        let cursor_bytes = std::mem::size_of::<crate::source_coordinates::SourceCoordinates>();
        let anchor_bytes = if history_bytes == 0 { 0 } else { cursor_bytes };
        let prefix_pin_bytes = max_prefix_pins
            .checked_mul(std::mem::size_of::<PlannedPrefixCoordinatePin>())
            .ok_or(overflow)?;
        let total_bytes = history_bytes
            .checked_add(anchor_bytes)
            .and_then(|value| value.checked_add(cursor_bytes))
            .and_then(|value| value.checked_add(prefix_pin_bytes))
            .ok_or(overflow)?;
        Ok(Self {
            non_prefix_reach_back_bytes,
            prefix_discovery_reach_back_bytes,
            history_bytes,
            anchor_bytes,
            cursor_bytes,
            max_prefix_pins,
            prefix_pin_bytes,
            total_bytes,
        })
    }
}

impl CompiledRuleSet {
    /// Plans coordinates for every semantically admitted matcher, including
    /// rules without validators. This neither allocates retained state nor
    /// changes SourceSession admission. Prefix reach-back is discovery-only.
    #[allow(dead_code)]
    pub(crate) fn coordinate_storage_plan(
        &self,
    ) -> Result<CoordinateStoragePlan, CoordinateStoragePlanningError> {
        if !self.supports_streaming() || !self.supports_streaming_semantics() {
            return Err(CoordinateStoragePlanningError::UnsupportedRuleSet);
        }
        let mut non_prefix = 0;
        let mut discovery = 0;
        let mut pins = 0usize;
        if let Some(engine) = &self.multi_pattern {
            for rule in &engine.rules {
                match rule.kind {
                    MultiPatternKind::Literal => non_prefix = non_prefix.max(rule.needle.len()),
                    MultiPatternKind::Prefix => {
                        discovery = discovery.max(rule.needle.len());
                        pins = pins
                            .checked_add(1)
                            .ok_or(CoordinateStoragePlanningError::ArithmeticOverflow)?;
                    }
                }
            }
        }
        if !self.suffixes.is_empty() {
            non_prefix = non_prefix.max(MAX_SUFFIX_TOKEN_LEN);
        }
        for rule in &self.patterns {
            // Includes complete-match length, assertion context, internal
            // window retention and UTF-8 margins. Captures use this full bound.
            let reach = StreamingStorageRequirements::pattern_buffer_bytes(rule.stream_plan)
                .map_err(|_| CoordinateStoragePlanningError::ArithmeticOverflow)?;
            non_prefix = non_prefix.max(reach);
        }
        CoordinateStoragePlan::from_components(non_prefix, discovery, pins)
    }
}

/// Checked planning errors never imply contextual execution support.
#[allow(dead_code)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub(crate) enum ContextStoragePlanningError {
    UnsupportedRuleSet,
    ArithmeticOverflow,
}

#[allow(dead_code)]
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct ContextRulePlan {
    pub(crate) rule_index: RuleIndex,
    before_bytes: usize,
    after_bytes: usize,
    max_candidate_bytes: usize,
    emission_reach_back: usize,
    pending_lookahead_bytes: usize,
    pub(crate) max_pending_records: usize,
    pub(crate) snapshot_bytes: usize,
}

#[allow(dead_code)]
impl ContextRulePlan {
    fn new(
        rule_index: RuleIndex,
        before_bytes: usize,
        after_bytes: usize,
        max_candidate_bytes: usize,
        emission_reach_back: usize,
    ) -> Result<Self, ContextStoragePlanningError> {
        let overflow = ContextStoragePlanningError::ArithmeticOverflow;
        let snapshot_bytes = before_bytes
            .checked_add(max_candidate_bytes)
            .and_then(|value| value.checked_add(after_bytes))
            .ok_or(overflow)?;
        // Ingestion borrows the FULL validated callback, resolves old
        // ready records before new emissions, and never queues EOF emissions.
        // At callback end E, pending candidates satisfy E - end < A.
        // Their complete-match starts therefore lie in an interval of at most
        // A + C + 1 integer byte positions. Each rule has distinct complete
        // starts: literal occurrences, separated suffix tokens, or regex
        // matches. Both rule constructors reject empty complete regex matches;
        // non-overlap advancement makes admitted complete starts distinct.
        // Projected captures may be empty without changing complete selection.
        // EOF is immediate, never pending. For captures C is the COMPLETE bound;
        // projected starts/ends need not be distinct. No UTF-8 spacing assumed.
        // During ingestion E is already known; ready candidates are immediate.
        let max_pending_records = if after_bytes == 0 {
            0
        } else {
            after_bytes
                .checked_add(max_candidate_bytes)
                .and_then(|value| value.checked_add(1))
                .ok_or(overflow)?
        };
        Ok(Self {
            rule_index,
            before_bytes,
            after_bytes,
            max_candidate_bytes,
            emission_reach_back,
            pending_lookahead_bytes: after_bytes,
            max_pending_records,
            snapshot_bytes,
        })
    }
}

/// Owned bounded validator snapshot. Rule identity comes from the finding.
#[derive(Debug)]
pub(crate) struct PendingValidation<C = ()> {
    pub(crate) finding: InternalFinding,
    pub(crate) candidate_range: std::ops::Range<usize>,
    pub(crate) after_end: usize,
    pub(crate) text: String,
    pub(crate) coordinates: C,
}

/// Validator logical payload, separate from matcher accounting.
/// Excludes accepted output, plan metadata, spare capacity, allocator overhead,
/// fixed session metadata and validator/library temporary workspace. Not a heap cap.
#[allow(dead_code)]
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct ContextualStoragePlan {
    pub(crate) rules: Box<[ContextRulePlan]>,
    pub(crate) history_bytes: usize,
    pending_snapshot_bytes: usize,
    max_pending_records: usize,
    pending_metadata_bytes: usize,
    pub(crate) assembly_scratch_bytes: usize,
    history_replacement_bytes: usize,
    pub(crate) persistent_bytes: usize,
    temporary_bytes: usize,
    total_bytes: usize,
}

#[allow(dead_code)]
impl ContextualStoragePlan {
    fn from_rules(rules: Box<[ContextRulePlan]>) -> Result<Self, ContextStoragePlanningError> {
        let overflow = ContextStoragePlanningError::ArithmeticOverflow;
        let mut history_bytes = 0usize;
        let mut pending_snapshot_bytes = 0usize;
        let mut max_pending_records = 0usize;
        let mut assembly_scratch_bytes = 0usize;
        for rule in &rules {
            // Before a callback begins, earlier candidate starts remain within
            // reach_back of its base. Preserve B additional preceding bytes.
            // Keeping the history cut outward adds at most three UTF-8 bytes.
            history_bytes = history_bytes.max(
                rule.before_bytes
                    .checked_add(rule.emission_reach_back)
                    .and_then(|value| value.checked_add(3))
                    .ok_or(overflow)?,
            );
            pending_snapshot_bytes = pending_snapshot_bytes
                .checked_add(
                    rule.max_pending_records
                        .checked_mul(rule.snapshot_bytes)
                        .ok_or(overflow)?,
                )
                .ok_or(overflow)?;
            max_pending_records = max_pending_records
                .checked_add(rule.max_pending_records)
                .ok_or(overflow)?;
            assembly_scratch_bytes = assembly_scratch_bytes.max(rule.snapshot_bytes);
        }
        // Count the actual record layout, including its String descriptor.
        // Owned text payload is counted separately.
        let record_bytes = std::mem::size_of::<PendingValidation>();
        let pending_metadata_bytes = max_pending_records
            .checked_mul(record_bytes)
            .ok_or(overflow)?;
        let persistent_bytes = history_bytes
            .checked_add(pending_snapshot_bytes)
            .and_then(|value| value.checked_add(pending_metadata_bytes))
            .ok_or(overflow)?;
        // Conservative separate scratch allowances; do not assume reuse.
        let history_replacement_bytes = history_bytes;
        let temporary_bytes = assembly_scratch_bytes
            .checked_add(history_replacement_bytes)
            .ok_or(overflow)?;
        let total_bytes = persistent_bytes
            .checked_add(temporary_bytes)
            .ok_or(overflow)?;
        Ok(Self {
            rules,
            history_bytes,
            pending_snapshot_bytes,
            max_pending_records,
            pending_metadata_bytes,
            assembly_scratch_bytes,
            history_replacement_bytes,
            persistent_bytes,
            temporary_bytes,
            total_bytes,
        })
    }
}

impl CompiledRuleSet {
    /// Plans validator retention without allocating execution buffers.
    #[allow(dead_code)]
    pub(crate) fn contextual_storage_plan(
        &self,
    ) -> Result<ContextualStoragePlan, ContextStoragePlanningError> {
        use crate::validators::dispatch::ValidatorStreamingRequirement;
        if !self.supports_streaming() || !self.supports_streaming_semantics() {
            return Err(ContextStoragePlanningError::UnsupportedRuleSet);
        }
        let mut plans = Vec::new();
        let mut add = |index: RuleIndex, extent: usize, reach_back: usize| {
            if let ValidatorStreamingRequirement::BoundedContext {
                before_bytes,
                after_bytes,
            } = self.metadata[index.get()].validator.streaming_requirement()
            {
                plans.push(ContextRulePlan::new(
                    index,
                    before_bytes,
                    after_bytes,
                    extent,
                    reach_back,
                )?);
            }
            Ok::<_, ContextStoragePlanningError>(())
        };
        if let Some(engine) = &self.multi_pattern {
            for rule in &engine.rules {
                if rule.kind == MultiPatternKind::Literal {
                    add(rule.rule_index, rule.needle.len(), rule.needle.len())?;
                }
                // Semantic admission already excludes byte-dependent prefixes.
            }
        }
        for rule in &self.suffixes {
            add(rule.rule_index, MAX_SUFFIX_TOKEN_LEN, MAX_SUFFIX_TOKEN_LEN)?;
        }
        for rule in &self.patterns {
            let PatternStreamPlan::Bounded { max_match_len, .. } = rule.stream_plan else {
                return Err(ContextStoragePlanningError::UnsupportedRuleSet);
            };
            if matches!(
                self.metadata[rule.rule_index.get()]
                    .validator
                    .streaming_requirement(),
                ValidatorStreamingRequirement::BoundedContext { .. }
            ) {
                let reach = StreamingStorageRequirements::pattern_buffer_bytes(rule.stream_plan)
                    .map_err(|_| ContextStoragePlanningError::ArithmeticOverflow)?;
                add(rule.rule_index, max_match_len, reach)?;
            }
        }
        plans.sort_by_key(|plan| plan.rule_index);
        ContextualStoragePlan::from_rules(plans.into_boxed_slice())
    }

    /// Account for executor-owned logical buffer payloads for valid UTF-8 input.
    /// Unsupported regexes and unrepresentable requirements return an error.
    /// This does not reserve storage or impose a resource budget.
    #[allow(dead_code)]
    pub(crate) fn streaming_storage_requirements(
        &self,
    ) -> Result<StreamingStorageRequirements, StreamingStorageError> {
        let (overlap_bytes, active_prefix_elements) = match &self.multi_pattern {
            Some(engine) => (
                engine
                    .max_needle_len
                    .checked_sub(1)
                    .expect("compiled needles are nonempty"),
                engine
                    .rules
                    .iter()
                    .filter(|rule| rule.kind == MultiPatternKind::Prefix)
                    .count(),
            ),
            None => (0, 0),
        };
        // Each prefix rule can have only one candidate: the left token
        // boundary admits only the current token's start. Token termination
        // removes candidates before discovering those for the next token.
        let pattern_buffer_bytes = self
            .patterns
            .iter()
            .map(|rule| match rule.stream_plan {
                PatternStreamPlan::Unsupported => {
                    Err(StreamingStorageError::UnsupportedPattern(rule.rule_index))
                }
                plan => StreamingStorageRequirements::pattern_buffer_bytes(plan),
            })
            .collect::<Result<Box<[_]>, _>>()?;
        StreamingStorageRequirements::from_components(
            overlap_bytes,
            active_prefix_elements,
            !self.suffixes.is_empty(),
            pattern_buffer_bytes,
        )
    }

    pub(crate) fn compile(rules: Vec<Rule>) -> Result<Self, ScannerBuildError> {
        let mut metadata = Vec::with_capacity(rules.len());
        let mut multi_pattern = Vec::new();
        let mut suffixes = Vec::new();
        let mut patterns = Vec::new();
        let mut gate_needles = BTreeMap::<&'static str, u128>::new();
        let mut next_gate_bit = 0_u8;

        for (index, rule) in rules.into_iter().enumerate() {
            validate_rule(&rule)?;

            let kind = rule.kind();
            let Rule {
                id,
                severity,
                validator,
                matcher,
                remediation,
            } = rule;

            let rule_index = RuleIndex::new(index as u32);
            let pattern_prefilter_needles = pattern_prefilter_needles(id.as_str(), validator);
            let pattern_prefilter = compile_pattern_prefilter(pattern_prefilter_needles)?;
            let gate_bit = pattern_prefilter_needles.and_then(|needles| {
                if next_gate_bit >= 128 {
                    return None;
                }

                let bit = next_gate_bit;
                let mask = 1_u128 << bit;
                next_gate_bit += 1;

                for needle in needles {
                    *gate_needles.entry(needle).or_insert(0) |= mask;
                }

                Some(bit)
            });

            metadata.push(CompiledRuleMetadata {
                id,
                kind,
                severity,
                confidence: Confidence::High,
                validator,
                remediation,
            });

            match matcher {
                Matcher::Literal(needle) => multi_pattern.push(MultiPatternRule {
                    rule_index,
                    kind: MultiPatternKind::Literal,
                    needle,
                }),
                Matcher::Prefix(needle) => multi_pattern.push(MultiPatternRule {
                    rule_index,
                    kind: MultiPatternKind::Prefix,
                    needle,
                }),
                Matcher::Suffix(suffix) => suffixes.push(SuffixRule { rule_index, suffix }),
                Matcher::Pattern {
                    regex,
                    capture,
                    context,
                    maximum_len,
                } => patterns.push(PatternRule {
                    rule_index,
                    pattern: regex,
                    capture,
                    stream_plan: PatternStreamPlan::compile(maximum_len, context),
                    prefilter: pattern_prefilter,
                    gate_bit,
                }),
            }
        }

        Ok(Self {
            metadata: metadata.into_boxed_slice(),
            multi_pattern: MultiPatternEngine::compile(multi_pattern)?,
            suffixes: suffixes.into_boxed_slice(),
            patterns: patterns.into_boxed_slice(),
            pattern_gate: PatternPrefilterGate::compile(gate_needles)?,
        })
    }

    pub(crate) fn scan(&self, source: &str, findings: &mut Vec<InternalFinding>) {
        self.scan_with_emit(source, &mut |finding| {
            findings.push(finding);
            Ok(())
        })
        .expect("unmetered emission cannot fail");
    }

    pub(crate) fn scan_with_emit(
        &self,
        source: &str,
        emit: &mut impl FnMut(
            InternalFinding,
        ) -> Result<(), crate::incremental_session::IncrementalError>,
    ) -> Result<(), crate::incremental_session::IncrementalError> {
        if let Some(engine) = &self.multi_pattern {
            engine.scan_selected(Input::new(source), emit, |_, _| true)?;
        }
        self.scan_non_multi(source, emit, None)
    }

    fn scan_non_multi(
        &self,
        source: &str,
        emit: &mut impl FnMut(
            InternalFinding,
        ) -> Result<(), crate::incremental_session::IncrementalError>,
        mut work: Option<&mut IncrementalWork>,
    ) -> Result<(), crate::incremental_session::IncrementalError> {
        for rule in &self.suffixes {
            if let Some(work) = work.as_deref_mut() {
                work.record(WorkMetric::WholeRuleFallbacks, 1).ok_or(
                    crate::incremental_session::IncrementalError::ExecutionInvariantViolated,
                )?;
            }
            rule.scan_with_emit(source, emit)?;
        }

        if self.pattern_gate.is_some()
            && let Some(work) = work.as_deref_mut()
        {
            work.record(WorkMetric::GatePasses, 1)
                .ok_or(crate::incremental_session::IncrementalError::ExecutionInvariantViolated)?;
            work.record(WorkMetric::GateBytes, source.len())
                .ok_or(crate::incremental_session::IncrementalError::ExecutionInvariantViolated)?;
        }
        let active_patterns = self
            .pattern_gate
            .as_ref()
            .map_or(u128::MAX, |gate| gate.active_mask(source));

        for rule in &self.patterns {
            if let Some(bit) = rule.gate_bit
                && active_patterns & (1_u128 << bit) == 0
            {
                continue;
            }

            if let Some(work) = work.as_deref_mut() {
                work.record(WorkMetric::WholeRuleFallbacks, 1).ok_or(
                    crate::incremental_session::IncrementalError::ExecutionInvariantViolated,
                )?;
            }
            rule.scan_with_emit(source, emit)?;
        }
        Ok(())
    }

    /// Immutable per-rule localization metadata, built only for an incremental session.
    pub(crate) fn incremental_literal_lengths(&self) -> Vec<Option<usize>> {
        let mut lengths = vec![None; self.len()];
        if let Some(engine) = &self.multi_pattern {
            for rule in &engine.rules {
                if rule.kind == MultiPatternKind::Literal {
                    lengths[rule.rule_index.get()] = Some(rule.needle.len());
                }
            }
        }
        lengths
    }

    /// Enumerate owned literal starts in one byte span on the complete source.
    /// Standard overlapping matching retains duplicate pattern identities.
    pub(crate) fn scan_incremental_literals(
        &self,
        source: &str,
        span: std::ops::Range<usize>,
        owners: &[std::ops::Range<usize>],
        emit: &mut impl FnMut(
            InternalFinding,
        ) -> Result<(), crate::incremental_session::IncrementalError>,
        work: &mut IncrementalWork,
    ) -> Result<(), crate::incremental_session::IncrementalError> {
        work.record(
            WorkMetric::LiteralBytes,
            span.end
                .checked_sub(span.start)
                .ok_or(crate::incremental_session::IncrementalError::ExecutionInvariantViolated)?,
        )
        .ok_or(crate::incremental_session::IncrementalError::ExecutionInvariantViolated)?;
        if let Some(engine) = &self.multi_pattern {
            // Overlapping enumeration has no non-overlapping selection history.
            // The span includes complete needles; per-rule start ownership avoids
            // duplicating cached suffix matches while preserving pattern identity.
            engine.scan_selected(Input::new(source).span(span), emit, |rule, start| {
                rule.kind == MultiPatternKind::Literal
                    && owners[rule.rule_index.get()].contains(&start)
            })?;
        }
        Ok(())
    }

    /// Whole-source authoritative fallback; regex gates and per-rule prefilters
    /// retain their existing source-wide behavior and emission multiplicity.
    /// A bounded regex match length does not bound the downstream effects of
    /// non-overlapping selection, so regional replay is not inferred from it.
    pub(crate) fn scan_incremental_fallback(
        &self,
        source: &str,
        emit: &mut impl FnMut(
            InternalFinding,
        ) -> Result<(), crate::incremental_session::IncrementalError>,
        work: &mut IncrementalWork,
    ) -> Result<(), crate::incremental_session::IncrementalError> {
        if let Some(engine) = &self.multi_pattern {
            let prefixes = engine
                .rules
                .iter()
                .filter(|rule| rule.kind == MultiPatternKind::Prefix)
                .count();
            if prefixes > 0 {
                work.record(WorkMetric::WholeRuleFallbacks, prefixes)
                    .ok_or(
                        crate::incremental_session::IncrementalError::ExecutionInvariantViolated,
                    )?;
                work.record(WorkMetric::PrefixPasses, 1).ok_or(
                    crate::incremental_session::IncrementalError::ExecutionInvariantViolated,
                )?;
                work.record(WorkMetric::PrefixBytes, source.len()).ok_or(
                    crate::incremental_session::IncrementalError::ExecutionInvariantViolated,
                )?;
                engine.scan_selected(Input::new(source), emit, |rule, _| {
                    rule.kind == MultiPatternKind::Prefix
                })?;
            }
        }
        self.scan_non_multi(source, emit, Some(work))
    }

    pub(crate) fn metadata(&self, index: RuleIndex) -> &CompiledRuleMetadata {
        &self.metadata[index.get()]
    }

    pub(crate) fn public_metadata(
        &self,
    ) -> impl ExactSizeIterator<Item = crate::RuleMetadata<'_>> + '_ {
        self.metadata.iter().map(|metadata| {
            crate::RuleMetadata::new(
                metadata.id.as_str(),
                metadata.kind,
                metadata.validator.detection_mode(),
                metadata.severity,
                metadata.remediation,
            )
        })
    }

    pub(crate) fn len(&self) -> usize {
        self.metadata.len()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.metadata.is_empty()
    }

    pub(crate) fn scan_multi_pattern_stream_chunk(
        &self,
        multi_pattern_state: &mut MultiPatternStreamState,
        chunk: &[u8],
        source_offset: usize,
        findings: &mut CandidateSink<'_>,
    ) -> Result<(), ResourceLimitExceeded> {
        if let Some(engine) = &self.multi_pattern {
            engine.scan_stream_chunk_with_sink(
                multi_pattern_state,
                chunk,
                source_offset,
                findings,
            )?;
        }
        Ok(())
    }

    pub(crate) fn scan_suffix_stream_chunk(
        &self,
        state: &mut SuffixStreamState,
        chunk: &[u8],
        source_offset: usize,
        findings: &mut CandidateSink<'_>,
    ) -> Result<(), ResourceLimitExceeded> {
        if self.suffixes.is_empty() {
            return Ok(());
        }
        let suffixes = &self.suffixes;

        state.push_chunk_with_sink(chunk, source_offset, |token_start, token| {
            for rule in suffixes {
                rule.scan_stream_token(token_start, token, findings)?;
            }
            Ok(())
        })
    }

    pub(crate) fn finish_multi_pattern_stream(
        &self,
        state: &mut MultiPatternStreamState,
        source_end: usize,
        findings: &mut CandidateSink<'_>,
    ) -> Result<(), ResourceLimitExceeded> {
        if self.multi_pattern.is_some() {
            MultiPatternEngine::finish_stream_with_sink(state, source_end, findings)?;
        }
        Ok(())
    }

    pub(crate) fn finish_suffix_stream(
        &self,
        state: &mut SuffixStreamState,
        findings: &mut CandidateSink<'_>,
    ) -> Result<(), ResourceLimitExceeded> {
        if self.suffixes.is_empty() {
            return Ok(());
        }
        let suffixes = &self.suffixes;

        state.finish_with_sink(|token_start, token| {
            for rule in suffixes {
                rule.scan_stream_token(token_start, token, findings)?;
            }
            Ok(())
        })
    }

    pub(crate) fn supports_streaming(&self) -> bool {
        self.patterns
            .iter()
            .all(|rule| !matches!(rule.stream_plan, PatternStreamPlan::Unsupported))
    }

    /// Semantic admission only; sessions still emit raw, unvalidated candidates.
    /// Validator source requirements are representable by local retained data,
    /// but that data is neither allocated nor included in storage accounting yet.
    pub(crate) fn supports_streaming_semantics(&self) -> bool {
        use crate::validators::dispatch::ValidatorStreamingRequirement;

        let validators_supported =
            self.metadata
                .iter()
                .all(|rule| match rule.validator.streaming_requirement() {
                    ValidatorStreamingRequirement::NoValidation
                    | ValidatorStreamingRequirement::CandidateOnly
                    | ValidatorStreamingRequirement::BoundedContext { .. } => true,
                });
        // Per-rule prefilters enumerate regex matches from keyword positions,
        // which need not agree with ordinary non-overlapping regex iteration.
        // The shared presence gate is a separate whole-source optimization.
        let prefixes_supported = self.multi_pattern.as_ref().is_none_or(|engine| {
            engine.rules.iter().all(|rule| {
                rule.kind != MultiPatternKind::Prefix
                    || matches!(
                        self.metadata[rule.rule_index.get()]
                            .validator
                            .streaming_requirement(),
                        ValidatorStreamingRequirement::NoValidation
                    )
            })
        });
        let suffixes_supported = self.suffixes.iter().all(|rule| {
            let needle = rule.suffix.as_bytes();
            // Streaming compares a complete ASCII token with ends_with; whole
            // scanning enumerates non-overlapping match_indices. A needle with
            // a proper prefix/suffix overlap can have its final occurrence
            // skipped by whole scanning (e.g. "aa" in "aaa"). Reject it until
            // equivalent enumeration is supported, without changing matching.
            needle.len() <= MAX_SUFFIX_TOKEN_LEN
                && needle.iter().copied().all(is_token_byte)
                && !(1..needle.len()).any(|len| needle[..len] == needle[needle.len() - len..])
        });
        validators_supported
            && prefixes_supported
            && suffixes_supported
            && self.patterns.iter().all(|rule| rule.prefilter.is_none())
    }

    /// Persistent per-pattern state metadata, separately from byte buffers.
    pub(crate) fn new_pattern_state_metadata_bytes(&self) -> Option<usize> {
        self.patterns
            .len()
            .checked_mul(std::mem::size_of::<PatternStreamState>())
    }

    pub(crate) fn new_pattern_stream_states(&self) -> Vec<PatternStreamState> {
        self.patterns
            .iter()
            .map(|_| PatternStreamState::default())
            .collect()
    }

    pub(crate) fn scan_pattern_stream_chunk(
        &self,
        states: &mut [PatternStreamState],
        chunk: &[u8],
        source_offset: usize,
        findings: &mut CandidateSink<'_>,
    ) -> Result<(), ResourceLimitExceeded> {
        debug_assert_eq!(states.len(), self.patterns.len());

        for (rule, state) in self.patterns.iter().zip(states) {
            if matches!(rule.stream_plan, PatternStreamPlan::Bounded { .. }) {
                rule.scan_stream_bounded_with_sink(state, chunk, source_offset, findings)?;
            }
        }
        Ok(())
    }

    pub(crate) fn finish_pattern_stream(
        &self,
        states: &mut [PatternStreamState],
        findings: &mut CandidateSink<'_>,
    ) -> Result<(), ResourceLimitExceeded> {
        debug_assert_eq!(states.len(), self.patterns.len());

        for (rule, state) in self.patterns.iter().zip(states) {
            if matches!(rule.stream_plan, PatternStreamPlan::Bounded { .. }) {
                rule.finish_stream_bounded_with_sink(state, findings)?;
            }
        }
        Ok(())
    }
}

fn pattern_prefilter_needles(
    rule_id: &str,
    validator: ValidatorKind,
) -> Option<&'static [&'static str]> {
    match (rule_id, validator) {
        ("aws.secret-access-key", ValidatorKind::Aws) => Some(&[
            "aws_secret_access_key",
            "secret_access_key",
            "aws_secret_key",
        ]),
        ("aws.session-token", ValidatorKind::Aws) => {
            Some(&["aws_session_token", "aws_security_token", "session_token"])
        }
        ("azure.client-secret", ValidatorKind::Azure) => Some(&[
            "microsoft_provider_authentication_secret",
            "azure_client_secret",
            "client_secret_value",
            "clientsecret",
        ]),
        ("azure.storage-account-key", ValidatorKind::Azure) => {
            Some(&["storage_account_key", "azure_storage_key", "account_key"])
        }
        ("azure.shared-access-signature", ValidatorKind::Azure) => {
            Some(&["shared_access_signature", "azure_sas_token", "sas_token"])
        }
        ("gcp.private-key-id", ValidatorKind::Gcp) => Some(&["private_key_id"]),
        ("gcp.private-key", ValidatorKind::Gcp) => Some(&["private_key"]),
        ("gcp.escaped-private-key", ValidatorKind::Gcp) => Some(&["private_key"]),
        ("docker.registry-auth", ValidatorKind::DockerRegistry) => Some(&["auths", "\"auth\""]),
        ("npm.registry-auth-token", ValidatorKind::NpmRegistry) => Some(&["_authtoken", "//"]),
        ("npm.registry-auth", ValidatorKind::NpmRegistry) => Some(&["_auth", "//"]),
        ("npm.registry-password", ValidatorKind::NpmRegistry) => Some(&["_password", "//"]),
        ("composer.http-basic-password", ValidatorKind::ComposerHttpBasicPassword) => {
            Some(&["\"password\""])
        }
        ("composer.bitbucket-consumer-secret", ValidatorKind::ComposerBitbucketConsumerSecret) => {
            Some(&["\"consumer-secret\""])
        }
        ("composer.forgejo-token", ValidatorKind::ComposerForgejoToken) => Some(&["\"token\""]),
        ("cargo.registry-token", ValidatorKind::CargoRegistry) => {
            Some(&["token", "[registry", "[registries."])
        }
        ("cargo.registry-env-token", ValidatorKind::CargoRegistry) => {
            Some(&["cargo_registry_token", "cargo_registries_"])
        }
        ("pypi.repository-token", ValidatorKind::Pypi) => {
            Some(&["password", "username", "__token__"])
        }
        ("nuget.package-source-cleartext-password", ValidatorKind::Nuget) => Some(&["<add"]),
        ("maven.server-password", ValidatorKind::Maven) => Some(&["<password"]),
        ("rubygems.host-api-key", ValidatorKind::RubyGemsHost) => Some(&["gem_host_api_key"]),
        ("swiftpm.registry-token", ValidatorKind::SwiftPm) => Some(&["swiftpm_registry_token"]),
        ("swiftpm.registry-password", ValidatorKind::SwiftPm) => {
            Some(&["swiftpm_registry_password"])
        }
        ("swiftpm.source-control-token", ValidatorKind::SwiftPm) => {
            Some(&["swiftpm_source_control_token"])
        }
        ("swiftpm.netrc-password", ValidatorKind::SwiftPmNetrc) => Some(&["password"]),
        ("gradle.repository-password", ValidatorKind::Gradle) => Some(&["org_gradle_project_"]),
        ("gradle.repository-auth-header-value", ValidatorKind::Gradle) => {
            Some(&["org_gradle_project_"])
        }
        ("netrc.password", ValidatorKind::Netrc) => Some(&["machine", "login", "password"]),
        ("generic.authorization-basic", ValidatorKind::HttpBasic) => {
            Some(&["authorization", "basic"])
        }
        ("generic.password-field", ValidatorKind::Password) => Some(&[
            "admin_password",
            "root_password",
            "password",
            "passwd",
            "pwd",
        ]),
        ("generic.database-password-field", ValidatorKind::Password) => Some(&[
            "database_password",
            "postgres_password",
            "mysql_password",
            "redis_password",
            "db_password",
        ]),
        ("generic.passphrase-field", ValidatorKind::Password) => {
            Some(&["private_key_passphrase", "passphrase"])
        }
        ("generic.sensitive-hash", ValidatorKind::SensitiveHash) => Some(&[
            "credential_hash",
            "password_hash",
            "passwd_hash",
            "api_key_hash",
            "secret_hash",
            "token_hash",
        ]),
        ("generic.api-key", ValidatorKind::GenericCredential) => {
            Some(&["access_key", "api_token", "api_key", "apikey"])
        }
        ("generic.auth-token", ValidatorKind::GenericCredential) => {
            Some(&["access_token", "bearer_token", "auth_token", "token"])
        }
        ("generic.authorization-bearer", ValidatorKind::GenericCredential) => {
            Some(&["authorization"])
        }
        ("generic.database-connection-password", ValidatorKind::DatabaseConnection) => Some(&[
            "postgres://",
            "postgresql://",
            "mysql://",
            "mariadb://",
            "mongodb://",
            "mongodb+srv://",
            "redis://",
            "rediss://",
        ]),
        ("generic.secret", ValidatorKind::GenericCredential) => Some(&[
            "signing_secret",
            "webhook_secret",
            "client_secret",
            "secret_key",
            "secret",
        ]),
        ("wireguard.private-key", ValidatorKind::WireGuard) => Some(&["privatekey", "[interface]"]),
        ("wireguard.preshared-key", ValidatorKind::WireGuard) => Some(&["presharedkey", "[peer]"]),
        _ => None,
    }
}

fn compile_pattern_prefilter(
    needles: Option<&'static [&'static str]>,
) -> Result<Option<AhoCorasick>, ScannerBuildError> {
    let Some(needles) = needles else {
        return Ok(None);
    };

    AhoCorasickBuilder::new()
        .ascii_case_insensitive(true)
        .match_kind(MatchKind::LeftmostFirst)
        .build(needles)
        .map(Some)
        .map_err(ScannerBuildError::AutomatonBuild)
}

#[inline]
fn optional_quote_start(bytes: &[u8], key_start: usize) -> usize {
    if key_start > 0 && matches!(bytes[key_start - 1], b'\'' | b'"') {
        key_start - 1
    } else {
        key_start
    }
}

fn validate_rule(rule: &Rule) -> Result<(), ScannerBuildError> {
    if rule.id.as_str().is_empty() {
        return Err(ScannerBuildError::EmptyRuleId);
    }

    let is_empty = match &rule.matcher {
        Matcher::Literal(value) | Matcher::Prefix(value) | Matcher::Suffix(value) => {
            value.is_empty()
        }
        Matcher::Pattern { .. } => false,
    };

    if is_empty {
        return Err(ScannerBuildError::EmptyMatcher {
            rule_id: rule.id.clone(),
        });
    }

    Ok(())
}

/// Minimal match representation used only during execution.
///
/// Line, column, rule ID, severity, and confidence are deliberately omitted
/// from the hot path. They are resolved once, after deterministic ordering.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub(crate) struct InternalFinding {
    rule_index: RuleIndex,
    start: usize,
    end: usize,
}

impl InternalFinding {
    pub(crate) const fn new(rule_index: RuleIndex, start: usize, end: usize) -> Self {
        Self {
            rule_index,
            start,
            end,
        }
    }

    pub(crate) const fn rule_index(self) -> RuleIndex {
        self.rule_index
    }

    pub(crate) const fn start(self) -> usize {
        self.start
    }

    pub(crate) const fn end(self) -> usize {
        self.end
    }
}

#[inline]
const fn is_token_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-')
}

#[cfg(test)]
mod coordinate_storage_planning_tests {
    use super::*;
    use crate::Severity;

    fn plan(rules: Vec<Rule>) -> CoordinateStoragePlan {
        CompiledRuleSet::compile(rules)
            .unwrap()
            .coordinate_storage_plan()
            .unwrap()
    }

    #[test]
    fn empty_and_literal_plans_separate_payload_and_metadata() {
        let empty = plan(Vec::new());
        assert_eq!(empty.history_bytes, 0);
        assert_eq!(empty.anchor_bytes, 0);
        assert_eq!(empty.max_prefix_pins, 0);
        assert_eq!(
            empty.total_bytes,
            std::mem::size_of::<crate::source_coordinates::SourceCoordinates>()
        );
        let literal = plan(vec![Rule::literal("literal", "éabc", Severity::High)]);
        assert_eq!(literal.non_prefix_reach_back_bytes, 5);
        assert_eq!(literal.history_bytes, 8);
        assert_eq!(literal.anchor_bytes, literal.cursor_bytes);
        assert_eq!(
            literal.total_bytes,
            literal.history_bytes + literal.anchor_bytes + literal.cursor_bytes
        );
        // These rules have no contextual validator plan; coordinates still need history.
        let compiled =
            CompiledRuleSet::compile(vec![Rule::literal("plain", "needle", Severity::High)])
                .unwrap();
        assert_eq!(compiled.contextual_storage_plan().unwrap().history_bytes, 0);
        assert_eq!(compiled.coordinate_storage_plan().unwrap().history_bytes, 9);
    }

    #[test]
    fn prefix_plans_bound_discovery_and_pin_cardinality_not_completion() {
        let prefix = plan(vec![
            Rule::prefix("short", "tok_", Severity::High),
            Rule::prefix("long", "secret_", Severity::High),
        ]);
        assert_eq!(prefix.non_prefix_reach_back_bytes, 0);
        assert_eq!(prefix.prefix_discovery_reach_back_bytes, 7);
        assert_eq!(prefix.history_bytes, 10);
        assert_eq!(prefix.max_prefix_pins, 2);
        assert_eq!(
            prefix.prefix_pin_bytes,
            2 * std::mem::size_of::<PlannedPrefixCoordinatePin>()
        );
        assert_eq!(
            prefix.total_bytes,
            prefix.history_bytes
                + prefix.anchor_bytes
                + prefix.cursor_bytes
                + prefix.prefix_pin_bytes
        );
    }

    #[test]
    fn suffix_and_mixed_plans_cover_full_eligible_tokens() {
        let suffix = plan(vec![Rule::suffix("suffix", "_end", Severity::High)]);
        assert_eq!(suffix.non_prefix_reach_back_bytes, MAX_SUFFIX_TOKEN_LEN);
        assert_eq!(suffix.history_bytes, MAX_SUFFIX_TOKEN_LEN + 3);
        let mixed = plan(vec![
            Rule::literal("literal", "ab", Severity::High),
            Rule::prefix("prefix", "tok_", Severity::High),
            Rule::suffix("suffix", "_end", Severity::High),
            Rule::pattern("regex", "a{1,3}", Severity::High).unwrap(),
        ]);
        assert_eq!(mixed.non_prefix_reach_back_bytes, MAX_SUFFIX_TOKEN_LEN);
        assert_eq!(mixed.history_bytes, MAX_SUFFIX_TOKEN_LEN + 3);
        assert_eq!(mixed.max_prefix_pins, 1);
    }

    #[test]
    fn regex_plans_use_complete_match_and_assertion_retention() {
        for (pattern, expected) in [
            ("a{1,3}", 18),
            (r"\babc\b", 34),
            ("(?m:^abc$)", 34),
            ("(?mR:^abc$)", 34),
            (r"\Aabc\z", 3),
        ] {
            let regex = plan(vec![
                Rule::pattern("regex", pattern, Severity::High).unwrap(),
            ]);
            assert_eq!(regex.non_prefix_reach_back_bytes, expected, "{pattern}");
            assert_eq!(regex.history_bytes, expected + 3);
        }
        let capture = plan(vec![
            Rule::captured_pattern("capture", "x{5}(?P<value>a?)", "value", Severity::High)
                .unwrap(),
        ]);
        assert_eq!(capture.non_prefix_reach_back_bytes, 24); // M=6, not projected length 0..1.
        assert_eq!(capture.history_bytes, 27);
        let multiple = plan(vec![
            Rule::pattern("small", "abc", Severity::High).unwrap(),
            Rule::pattern("large", "a{1000}", Severity::High).unwrap(),
        ]);
        assert_eq!(multiple.non_prefix_reach_back_bytes, 2012);
    }

    #[test]
    fn coordinate_planning_overflow_and_unsupported_rules_are_explicit() {
        for (reach, discovery, pins) in [
            (usize::MAX, 0, 0),
            (0, usize::MAX, 1),
            (0, 1, usize::MAX),
            (usize::MAX - 3, 0, 0),
        ] {
            assert_eq!(
                CoordinateStoragePlan::from_components(reach, discovery, pins).unwrap_err(),
                CoordinateStoragePlanningError::ArithmeticOverflow
            );
        }
        let mut compiled =
            CompiledRuleSet::compile(vec![Rule::pattern("regex", "a", Severity::High).unwrap()])
                .unwrap();
        compiled.patterns[0].stream_plan = PatternStreamPlan::Bounded {
            max_match_len: usize::MAX,
            context: PatternStreamContext::default(),
        };
        assert_eq!(
            compiled.coordinate_storage_plan().unwrap_err(),
            CoordinateStoragePlanningError::ArithmeticOverflow
        );
        for rule in [
            Rule::pattern("unbounded", "a+", Severity::High).unwrap(),
            Rule::suffix("suffix", "!", Severity::High),
            Rule::prefix("prefix", "tok_", Severity::High).with_validator(ValidatorKind::Password),
        ] {
            let compiled = CompiledRuleSet::compile(vec![rule]).unwrap();
            assert_eq!(
                compiled.coordinate_storage_plan().unwrap_err(),
                CoordinateStoragePlanningError::UnsupportedRuleSet
            );
        }
    }

    #[test]
    fn large_rule_limits_are_planned_without_source_or_retention_allocation() {
        let literal = plan(vec![Rule::literal(
            "large",
            "a".repeat(4096),
            Severity::High,
        )]);
        assert_eq!(literal.non_prefix_reach_back_bytes, 4096);
        assert_eq!(literal.history_bytes, 4099);
    }
}

#[cfg(test)]
mod contextual_storage_planning_tests {
    use super::*;
    use crate::{
        Severity,
        source_session::{SourceLimits, SourceSession, ValidatedSourceSession},
    };
    use std::sync::Arc;

    fn compile(rules: Vec<Rule>) -> CompiledRuleSet {
        CompiledRuleSet::compile(rules).unwrap()
    }

    #[test]
    fn all_thirty_contextual_validators_preserve_declared_windows() {
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
        for kind in kinds {
            let compiled = compile(vec![
                Rule::literal("context", "é", Severity::High).with_validator(kind),
            ]);
            let plan = compiled.contextual_storage_plan().unwrap();
            let crate::validators::dispatch::ValidatorStreamingRequirement::BoundedContext {
                before_bytes,
                after_bytes,
            } = kind.streaming_requirement()
            else {
                panic!("fixture must be contextual");
            };
            let rule = &plan.rules[0];
            assert_eq!(
                (rule.before_bytes, rule.after_bytes),
                (before_bytes, after_bytes)
            );
            assert_eq!(rule.max_candidate_bytes, 2);
            assert_eq!(rule.emission_reach_back, 2);
            assert_eq!(rule.pending_lookahead_bytes, after_bytes);
            assert_eq!(plan.history_bytes, before_bytes + 2 + 3);
            assert_eq!(
                rule.max_pending_records,
                if after_bytes == 0 { 0 } else { after_bytes + 3 }
            );
            assert_eq!(
                plan.pending_snapshot_bytes,
                rule.max_pending_records * (before_bytes + 2 + after_bytes)
            );
        }
    }

    #[test]
    fn matcher_bounds_and_multiple_rules_are_accounted_separately() {
        let compiled = compile(vec![
            Rule::literal("literal", "é", Severity::High).with_validator(ValidatorKind::Password),
            Rule::suffix("suffix", "_end", Severity::High).with_validator(ValidatorKind::Maven),
            Rule::pattern("regex", r"\babc\b", Severity::High)
                .unwrap()
                .with_validator(ValidatorKind::Nuget),
            Rule::captured_pattern("capture", "KEY=(?P<value>x)", "value", Severity::High)
                .unwrap()
                .with_validator(ValidatorKind::Maven),
        ]);
        let plan = compiled.contextual_storage_plan().unwrap();
        assert_eq!(plan.rules.len(), 4);
        assert_eq!(
            (
                plan.rules[1].max_candidate_bytes,
                plan.rules[1].emission_reach_back
            ),
            (4096, 4096)
        );
        assert_eq!(plan.rules[2].max_candidate_bytes, 3);
        assert_eq!(
            plan.rules[2].emission_reach_back,
            StreamingStorageRequirements::pattern_buffer_bytes(compiled.patterns[0].stream_plan)
                .unwrap()
        );
        assert_eq!(plan.rules[3].max_candidate_bytes, 5);
        assert_eq!(
            plan.max_pending_records,
            plan.rules
                .iter()
                .map(|rule| rule.max_pending_records)
                .sum::<usize>()
        );
        assert_eq!(
            plan.persistent_bytes,
            plan.history_bytes + plan.pending_snapshot_bytes + plan.pending_metadata_bytes
        );
        assert_eq!(
            plan.total_bytes,
            plan.persistent_bytes + plan.temporary_bytes
        );
        assert_eq!(plan.history_replacement_bytes, plan.history_bytes);
    }

    #[test]
    fn contextual_admission_requires_representable_plans_and_preserves_raw_gates() {
        let contextual = Arc::new(compile(vec![
            Rule::literal("context", "abc", Severity::High).with_validator(ValidatorKind::Maven),
        ]));
        assert!(contextual.contextual_storage_plan().is_ok());
        assert!(SourceSession::new(Arc::clone(&contextual)).is_ok());
        assert!(
            ValidatedSourceSession::new_with_limits(contextual, SourceLimits::default()).is_ok()
        );
        for rules in [
            vec![],
            vec![
                Rule::literal("candidate", "abc", Severity::High)
                    .with_validator(ValidatorKind::Iban),
            ],
        ] {
            let compiled = Arc::new(compile(rules));
            let plan = compiled.contextual_storage_plan().unwrap();
            assert!(plan.rules.is_empty());
            assert_eq!(plan.total_bytes, 0);
            assert!(
                ValidatedSourceSession::new_with_limits(compiled, SourceLimits::default()).is_ok()
            );
        }
        for rule in [
            Rule::prefix("prefix", "tok_", Severity::High).with_validator(ValidatorKind::Password),
            Rule::suffix("suffix", "aa", Severity::High).with_validator(ValidatorKind::Maven),
            Rule::pattern("regex", "a+", Severity::High).unwrap(),
            Rule::pattern("aws.secret-access-key", "abc", Severity::High)
                .unwrap()
                .with_validator(ValidatorKind::Aws),
        ] {
            assert_eq!(
                compile(vec![rule]).contextual_storage_plan().unwrap_err(),
                ContextStoragePlanningError::UnsupportedRuleSet
            );
        }
    }

    #[test]
    fn checked_planning_rejects_overflow_without_large_allocations() {
        let index = RuleIndex::new(0);
        assert_eq!(
            ContextRulePlan::new(index, usize::MAX, 0, 1, 1).unwrap_err(),
            ContextStoragePlanningError::ArithmeticOverflow
        );
        assert_eq!(
            ContextRulePlan::new(index, 0, 1, usize::MAX - 1, 1).unwrap_err(),
            ContextStoragePlanningError::ArithmeticOverflow
        );
        for rule in [
            ContextRulePlan::new(index, 0, 0, 1, usize::MAX).unwrap(),
            ContextRulePlan::new(index, 0, 1, usize::MAX / 2, 1).unwrap(),
            ContextRulePlan::new(index, usize::MAX - 4, 0, 1, 0).unwrap(),
        ] {
            assert_eq!(
                ContextualStoragePlan::from_rules(vec![rule].into_boxed_slice()).unwrap_err(),
                ContextStoragePlanningError::ArithmeticOverflow
            );
        }
        let mut compiled = compile(vec![
            Rule::pattern("regex", "a", Severity::High)
                .unwrap()
                .with_validator(ValidatorKind::Maven),
        ]);
        compiled.patterns[0].stream_plan = PatternStreamPlan::Bounded {
            max_match_len: usize::MAX,
            context: PatternStreamContext::default(),
        };
        assert_eq!(
            compiled.contextual_storage_plan().unwrap_err(),
            ContextStoragePlanningError::ArithmeticOverflow
        );
        assert_eq!(
            ValidatedSourceSession::new_with_limits(
                Arc::new(compiled),
                SourceLimits {
                    max_stream_storage_bytes: Some(usize::MAX),
                    ..SourceLimits::default()
                },
            )
            .unwrap_err(),
            crate::source_session::SourceSessionError::StreamStorageRequirementOverflow,
        );
    }

    #[test]
    fn pending_bound_covers_dense_occurrences_and_empty_projected_captures() {
        for (rule, source, extent) in [
            (
                Rule::literal("literal", "a", Severity::High),
                "a".repeat(64),
                1,
            ),
            (
                Rule::literal("overlap", "aa", Severity::High),
                "a".repeat(64),
                2,
            ),
            (
                Rule::suffix("suffix", "a", Severity::High),
                "a ".repeat(32),
                4096,
            ),
            (
                Rule::captured_pattern("capture", "a(?P<value>b?)", "value", Severity::High)
                    .unwrap(),
                "a".repeat(64),
                2,
            ),
        ] {
            for end in 0..=source.len() {
                let compiled = compile(vec![rule.clone()]);
                let mut findings = Vec::new();
                compiled.scan(&source[..end], &mut findings);
                for after in [1, 2, 7] {
                    let plan =
                        ContextRulePlan::new(RuleIndex::new(0), 0, after, extent, extent).unwrap();
                    let pending = findings
                        .iter()
                        .filter(|finding| end - finding.end() < after)
                        .count();
                    assert!(pending <= plan.max_pending_records);
                }
            }
        }
    }

    #[test]
    fn synthetic_regex_frontier_keeps_empty_complete_starts_distinct_before_eof() {
        for pattern in ["(?P<value>)", "a?(?P<value>)"] {
            let mut compiled = compile(vec![
                Rule::captured_pattern("capture", "x(?P<value>)", "value", Severity::High)
                    .unwrap()
                    .with_validator(ValidatorKind::Maven),
            ]);
            // Exercise the defensive frontier independently of admission.
            // Empty complete matches are not permitted by rule construction.
            compiled.patterns[0].pattern = Regex::new(pattern).unwrap();
            compiled.patterns[0].stream_plan = PatternStreamPlan::Bounded {
                max_match_len: usize::from(pattern.starts_with("a?")),
                context: PatternStreamContext::default(),
            };
            let mut states = compiled.new_pattern_stream_states();
            let mut findings = Vec::new();
            let mut emitted = 0;
            for offset in 0..64 {
                let mut sink = CandidateSink::new(&mut findings, &mut emitted, None);
                compiled
                    .scan_pattern_stream_chunk(&mut states, b"x", offset, &mut sink)
                    .unwrap();
                let distinct = findings
                    .iter()
                    .map(|finding| finding.start)
                    .collect::<std::collections::BTreeSet<_>>();
                assert_eq!(distinct.len(), findings.len());
                let plan = &compiled.contextual_storage_plan().unwrap().rules[0];
                for after in [1, 2, 7] {
                    let bound = ContextRulePlan::new(
                        RuleIndex::new(0),
                        0,
                        after,
                        plan.max_candidate_bytes,
                        plan.emission_reach_back,
                    )
                    .unwrap();
                    assert!(
                        findings
                            .iter()
                            .filter(|finding| offset + 1 - finding.end < after)
                            .count()
                            <= bound.max_pending_records
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod candidate_view_tests {
    use super::*;
    use crate::{Severity, source_session::SourceSession, utf8_transport::Utf8Transport};
    use std::sync::Arc;

    #[test]
    fn borrowed_views_match_source_and_raw_sequence_across_utf8_partitions() {
        for (rule, source, text_available) in [
            (
                Rule::literal("literal", "secret", Severity::High),
                "ésecret secret",
                true,
            ),
            (Rule::literal("unicode", "€", Severity::High), "a€b€", true),
            (
                Rule::prefix("prefix", "tok_", Severity::High),
                "tok_long! tok_eof",
                false,
            ),
            (
                Rule::suffix("suffix", "_end", Severity::High),
                "éabc_end! eof_end",
                true,
            ),
            (
                Rule::pattern("regex", "a.{0,3}b", Severity::High).unwrap(),
                "éa12b axb",
                true,
            ),
            (
                Rule::captured_pattern("capture", "KEY=(?P<value>é{1,2})", "value", Severity::High)
                    .unwrap(),
                "KEY=éé! KEY=é",
                true,
            ),
            (
                Rule::pattern("end", r"abc\z", Severity::High).unwrap(),
                "xxabc",
                true,
            ),
            (
                Rule::captured_pattern("empty-capture", "a(?P<value>b?)", "value", Severity::High)
                    .unwrap(),
                "a! ab",
                true,
            ),
        ] {
            let compiled = Arc::new(CompiledRuleSet::compile(vec![rule]).unwrap());
            let mut whole = Vec::new();
            compiled.scan(source, &mut whole);
            for split in 0..=source.len() {
                let mut observed = Vec::new();
                let mut emitted = 0;
                let mut callback = |view: CandidateView<'_>| {
                    assert_eq!(view.text.is_some(), text_available);
                    if let Some(text) = view.text {
                        assert_eq!(text, &source[view.finding.start..view.finding.end]);
                    }
                    observed.push(view.finding);
                    Ok(())
                };
                {
                    let mut sink = CandidateSink::with_callback(&mut callback, &mut emitted, None);
                    let mut transport = Utf8Transport::default();
                    let mut multi = MultiPatternStreamState::default();
                    let mut suffix = SuffixStreamState::default();
                    let mut patterns = compiled.new_pattern_stream_states();
                    let mut offset = 0;
                    for chunk in [&source.as_bytes()[..split], &source.as_bytes()[split..]] {
                        transport
                            .push(chunk, |text| {
                                compiled
                                    .scan_multi_pattern_stream_chunk(
                                        &mut multi,
                                        text.as_bytes(),
                                        offset,
                                        &mut sink,
                                    )
                                    .unwrap();
                                compiled
                                    .scan_suffix_stream_chunk(
                                        &mut suffix,
                                        text.as_bytes(),
                                        offset,
                                        &mut sink,
                                    )
                                    .unwrap();
                                compiled
                                    .scan_pattern_stream_chunk(
                                        &mut patterns,
                                        text.as_bytes(),
                                        offset,
                                        &mut sink,
                                    )
                                    .unwrap();
                                offset += text.len();
                            })
                            .unwrap();
                    }
                    transport.finish().unwrap();
                    compiled
                        .finish_multi_pattern_stream(&mut multi, offset, &mut sink)
                        .unwrap();
                    compiled
                        .finish_suffix_stream(&mut suffix, &mut sink)
                        .unwrap();
                    compiled
                        .finish_pattern_stream(&mut patterns, &mut sink)
                        .unwrap();
                }
                assert_eq!(emitted, observed.len());
                let mut session = SourceSession::new(Arc::clone(&compiled)).unwrap();
                let mut raw = Vec::new();
                session
                    .scan_chunk(&source.as_bytes()[..split], &mut raw)
                    .unwrap();
                session
                    .scan_chunk(&source.as_bytes()[split..], &mut raw)
                    .unwrap();
                session.finish_scan(&mut raw).unwrap();
                assert_eq!(observed, raw);
                let mut sorted = observed;
                sorted.sort_by_key(|finding| (finding.start, finding.end));
                assert_eq!(sorted, whole);
            }
        }
    }

    #[test]
    fn budget_precedes_callback_and_count_is_cumulative_on_failure() {
        let finding = InternalFinding::new(RuleIndex::new(0), 0, 1);
        let mut calls = 0;
        let mut emitted = 0;
        {
            let mut callback = |view: CandidateView<'_>| {
                calls += 1;
                assert_eq!(view.text, Some("a"));
                Err(ResourceLimitExceeded)
            };
            let mut sink = CandidateSink::with_callback(&mut callback, &mut emitted, Some(0));
            assert_eq!(
                sink.emit_view(CandidateView {
                    finding,
                    text: Some("a")
                }),
                Err(ResourceLimitExceeded)
            );
        }
        assert_eq!(calls, 0);
        assert_eq!(emitted, 0);
        {
            let mut callback = |_: CandidateView<'_>| {
                calls += 1;
                Err(ResourceLimitExceeded)
            };
            let mut sink = CandidateSink::with_callback(&mut callback, &mut emitted, Some(1));
            assert_eq!(sink.emit(finding), Err(ResourceLimitExceeded));
            assert_eq!(sink.emit(finding), Err(ResourceLimitExceeded));
        }
        assert_eq!(calls, 1);
        assert_eq!(emitted, 1);
    }

    #[test]
    fn ingestion_errors_stop_matcher_iteration_and_eof_immediately() {
        for rule in [
            Rule::literal("literal", "a", Severity::High),
            Rule::prefix("prefix", "a", Severity::High),
            Rule::suffix("suffix", "a", Severity::High),
            Rule::pattern("pattern", "a{1,2}", Severity::High).unwrap(),
            Rule::captured_pattern("capture", "(?P<value>a{1,2})", "value", Severity::High)
                .unwrap(),
        ] {
            let compiled = CompiledRuleSet::compile(vec![rule]).unwrap();
            for eof in [false, true] {
                let mut emitted = 0;
                let mut calls = 0;
                let mut callback = |_: CandidateView<'_>| {
                    calls += 1;
                    Err(ResourceLimitExceeded)
                };
                let mut sink = CandidateSink::with_callback(&mut callback, &mut emitted, None);
                let mut multi = MultiPatternStreamState::default();
                let mut suffix = SuffixStreamState::default();
                let mut patterns = compiled.new_pattern_stream_states();
                let result = if let Some(engine) = &compiled.multi_pattern {
                    if eof && engine.rules[0].kind == MultiPatternKind::Prefix {
                        compiled
                            .scan_multi_pattern_stream_chunk(&mut multi, b"a", 0, &mut sink)
                            .unwrap();
                        compiled.finish_multi_pattern_stream(&mut multi, 1, &mut sink)
                    } else {
                        compiled.scan_multi_pattern_stream_chunk(&mut multi, b"a! a!", 0, &mut sink)
                    }
                } else if !compiled.suffixes.is_empty() {
                    if eof {
                        compiled
                            .scan_suffix_stream_chunk(&mut suffix, b"a", 0, &mut sink)
                            .unwrap();
                        compiled.finish_suffix_stream(&mut suffix, &mut sink)
                    } else {
                        compiled.scan_suffix_stream_chunk(&mut suffix, b"a! a!", 0, &mut sink)
                    }
                } else if eof {
                    // A variable-length pattern waits at the source frontier.
                    compiled
                        .scan_pattern_stream_chunk(&mut patterns, b"a", 0, &mut sink)
                        .unwrap();
                    compiled.finish_pattern_stream(&mut patterns, &mut sink)
                } else {
                    compiled.scan_pattern_stream_chunk(&mut patterns, b"a! a!", 0, &mut sink)
                };
                assert_eq!(result, Err(ResourceLimitExceeded));
                assert_eq!(calls, 1);
                assert_eq!(emitted, 1);
            }
        }
    }

    #[test]
    fn delayed_regex_view_is_delivered_before_old_bytes_are_trimmed() {
        let compiled = CompiledRuleSet::compile(vec![
            Rule::pattern("delayed", "a.{0,8}b", Severity::High).unwrap(),
        ])
        .unwrap();
        let mut patterns = compiled.new_pattern_stream_states();
        let mut emitted = 0;
        let mut observed = Vec::new();
        let mut callback = |view: CandidateView<'_>| {
            assert_eq!(view.text, Some("ab"));
            assert_eq!((view.finding.start, view.finding.end), (0, 2));
            observed.push(view.finding);
            Ok(())
        };
        {
            let mut sink = CandidateSink::with_callback(&mut callback, &mut emitted, None);
            compiled
                .scan_pattern_stream_chunk(&mut patterns, b"ab", 0, &mut sink)
                .unwrap();
        }
        assert_eq!(emitted, 0);
        {
            let mut sink = CandidateSink::with_callback(&mut callback, &mut emitted, None);
            compiled
                .scan_pattern_stream_chunk(&mut patterns, &[b'x'; 64], 2, &mut sink)
                .unwrap();
        }
        assert_eq!(emitted, 1);
        assert!(patterns[0].buffer_offset > 0);
    }
}

#[cfg(test)]
mod streaming_semantic_admission_tests {
    use super::*;
    use crate::{
        Severity,
        source_session::{SourceLimits, SourceSession, SourceSessionError},
    };
    use std::sync::Arc;

    fn filtered_rule() -> Rule {
        Rule::pattern(
            "aws.secret-access-key",
            "aws_secret_access_key=abc",
            Severity::High,
        )
        .unwrap()
        .with_validator(ValidatorKind::Aws)
    }

    fn assert_rejected(compiled: CompiledRuleSet) {
        assert!(compiled.supports_streaming());
        assert!(!compiled.supports_streaming_semantics());
        let rules = Arc::new(compiled);
        assert_eq!(
            SourceSession::new(Arc::clone(&rules)).unwrap_err(),
            SourceSessionError::UnsupportedRuleSet
        );
        assert_eq!(
            SourceSession::new_with_max_candidates(Arc::clone(&rules), Some(0)).unwrap_err(),
            SourceSessionError::UnsupportedRuleSet
        );
        assert_eq!(
            SourceSession::new_with_limits(
                rules,
                SourceLimits {
                    max_candidates: Some(0),
                    max_stream_storage_bytes: Some(0),
                }
            )
            .unwrap_err(),
            SourceSessionError::UnsupportedRuleSet
        );
    }

    #[test]
    fn prefix_admission_requires_no_validation() {
        for validator in [
            ValidatorKind::None,
            ValidatorKind::GitHub,
            ValidatorKind::Password,
            ValidatorKind::Maven,
        ] {
            let compiled = CompiledRuleSet::compile(vec![
                Rule::prefix("prefix", "tok_", Severity::High).with_validator(validator),
                Rule::literal("literal", "abc", Severity::High),
                Rule::pattern("regex", "abc", Severity::High).unwrap(),
            ])
            .unwrap();
            if validator == ValidatorKind::None {
                assert!(compiled.supports_streaming_semantics());
                assert!(SourceSession::new(Arc::new(compiled)).is_ok());
            } else {
                assert_rejected(compiled);
            }
        }
    }

    fn suffix_spans(
        compiled: &CompiledRuleSet,
        source: &str,
        split: usize,
    ) -> (Vec<InternalFinding>, Vec<InternalFinding>) {
        let mut whole = Vec::new();
        compiled.scan(source, &mut whole);
        // Exercise the executor directly even for configurations admission rejects.
        let mut streamed = Vec::new();
        let mut emitted = 0;
        let mut sink = CandidateSink::new(&mut streamed, &mut emitted, None);
        let mut state = SuffixStreamState::default();
        compiled
            .scan_suffix_stream_chunk(&mut state, &source.as_bytes()[..split], 0, &mut sink)
            .unwrap();
        compiled
            .scan_suffix_stream_chunk(&mut state, &source.as_bytes()[split..], split, &mut sink)
            .unwrap();
        compiled
            .finish_suffix_stream(&mut state, &mut sink)
            .unwrap();
        (whole, streamed)
    }

    #[test]
    fn suffix_incompatibilities_are_demonstrated_and_rejected() {
        for (needle, source, whole_span, stream_span) in [
            ("!".to_owned(), "a!".to_owned(), Some((0, 2)), None),
            ("a".repeat(4097), "a".repeat(4097), Some((0, 4097)), None),
            ("aa".to_owned(), "aaa".to_owned(), None, Some((0, 3))),
            ("aba".to_owned(), "ababa".to_owned(), None, Some((0, 5))),
            ("é".to_owned(), "aé".to_owned(), Some((0, 3)), None),
        ] {
            let compiled =
                CompiledRuleSet::compile(vec![Rule::suffix("suffix", needle, Severity::High)])
                    .unwrap();
            for split in [0, 1, source.len() / 2, source.len()] {
                // Tokenizer operates on bytes; direct splits also cover UTF-8 seams.
                let (whole, streamed) = suffix_spans(&compiled, &source, split);
                let expected = |span: Option<(usize, usize)>| {
                    span.into_iter()
                        .map(|(start, end)| InternalFinding::new(RuleIndex::new(0), start, end))
                        .collect::<Vec<_>>()
                };
                assert_eq!(whole, expected(whole_span));
                assert_eq!(streamed, expected(stream_span));
            }
            assert_rejected(compiled);
        }
    }

    #[test]
    fn supported_suffixes_preserve_token_boundaries_lengths_and_eof() {
        for len in [4096, 4097, 10000] {
            let token = format!("{}_end", "a".repeat(len - 4));
            for source in [
                token.clone(),
                format!("é{token}!"),
                format!("{token} x_end"),
            ] {
                let compiled =
                    CompiledRuleSet::compile(vec![Rule::suffix("suffix", "_end", Severity::High)])
                        .unwrap();
                assert!(compiled.supports_streaming_semantics());
                let mut whole = Vec::new();
                compiled.scan(&source, &mut whole);
                assert_eq!(
                    whole.len(),
                    usize::from(len == 4096) + usize::from(source.ends_with(" x_end"))
                );
                for split in [0, 2, source.len() / 2, source.len() - 1, source.len()] {
                    let (expected, streamed) = suffix_spans(&compiled, &source, split);
                    assert_eq!(streamed, expected);
                }
                let rules = Arc::new(compiled);
                let mut session = SourceSession::new(Arc::clone(&rules)).unwrap();
                let mut streamed = Vec::new();
                for byte in source.as_bytes() {
                    session
                        .scan_chunk(std::slice::from_ref(byte), &mut streamed)
                        .unwrap();
                }
                session.finish_scan(&mut streamed).unwrap();
                assert_eq!(streamed, whole);
            }
        }
        // The needle itself may reach the bound, provided it has no self-overlap.
        let needle = format!("{}b", "a".repeat(4095));
        let compiled =
            CompiledRuleSet::compile(vec![Rule::suffix("suffix", &*needle, Severity::High)])
                .unwrap();
        assert!(compiled.supports_streaming_semantics());
        let (whole, streamed) = suffix_spans(&compiled, &needle, 4095);
        assert_eq!(whole.len(), 1);
        assert_eq!(streamed, whole);
    }

    #[test]
    fn suffix_admission_covers_small_token_needles_exhaustively() {
        for n in 1..=4 {
            for bits in 0..(1 << n) {
                let needle: String = (0..n)
                    .map(|i| if bits & (1 << i) == 0 { 'a' } else { 'b' })
                    .collect();
                let compiled = CompiledRuleSet::compile(vec![Rule::suffix(
                    "suffix",
                    &*needle,
                    Severity::High,
                )])
                .unwrap();
                if !compiled.supports_streaming_semantics() {
                    continue;
                }
                for len in 0..=7 {
                    for input in 0..(1 << len) {
                        let source: String = (0..len)
                            .map(|i| if input & (1 << i) == 0 { 'a' } else { 'b' })
                            .collect();
                        for split in 0..=len {
                            let (whole, streamed) = suffix_spans(&compiled, &source, split);
                            assert_eq!(streamed, whole, "needle={needle}, source={source}");
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn bounded_per_rule_prefilter_is_rejected_before_storage_admission() {
        for extra in [None, Some(Rule::literal("literal", "abc", Severity::High))] {
            let mut rules = vec![filtered_rule()];
            rules.extend(extra);
            let compiled = Arc::new(CompiledRuleSet::compile(rules).unwrap());
            assert!(compiled.supports_streaming());
            assert!(compiled.patterns[0].prefilter.is_some());
            assert!(compiled.patterns[0].gate_bit.is_some());
            assert!(!compiled.supports_streaming_semantics());
            assert_eq!(
                SourceSession::new(Arc::clone(&compiled)).unwrap_err(),
                SourceSessionError::UnsupportedRuleSet
            );
            assert_eq!(
                SourceSession::new_with_max_candidates(Arc::clone(&compiled), Some(0)).unwrap_err(),
                SourceSessionError::UnsupportedRuleSet
            );
            assert_eq!(
                SourceSession::new_with_limits(
                    compiled,
                    SourceLimits {
                        max_candidates: Some(0),
                        max_stream_storage_bytes: Some(0),
                    },
                )
                .unwrap_err(),
                SourceSessionError::UnsupportedRuleSet
            );
        }
    }

    #[test]
    fn shared_gate_alone_does_not_reject_ordinary_regex_enumeration() {
        let mut compiled = CompiledRuleSet::compile(vec![filtered_rule()]).unwrap();
        // Isolate the shared presence gate from per-rule enumeration. This
        // synthetic configuration proves admission inspects the per-rule field.
        compiled.patterns[0].prefilter = None;
        assert!(compiled.patterns[0].gate_bit.is_some());
        assert!(compiled.supports_streaming_semantics());
        assert!(SourceSession::new(Arc::new(compiled)).is_ok());
    }

    #[test]
    fn unsupported_matcher_plan_remains_rejected_even_with_supported_semantics() {
        let compiled = Arc::new(
            CompiledRuleSet::compile(vec![
                Rule::pattern("unbounded", "a+", Severity::High).unwrap(),
                Rule::literal("literal", "abc", Severity::High),
            ])
            .unwrap(),
        );
        assert!(compiled.supports_streaming_semantics());
        assert!(!compiled.supports_streaming());
        assert_eq!(
            SourceSession::new(compiled).unwrap_err(),
            SourceSessionError::UnsupportedRuleSet
        );
    }
}

#[cfg(test)]
mod streaming_storage_tests {
    use super::*;
    use crate::Severity;

    fn requirements(rules: Vec<Rule>) -> StreamingStorageRequirements {
        CompiledRuleSet::compile(rules)
            .unwrap()
            .streaming_storage_requirements()
            .unwrap()
    }

    #[test]
    fn unused_suffix_processing_and_finalization_leave_state_untouched() {
        for rules in [
            vec![],
            vec![Rule::literal("literal", "secret", Severity::High)],
            vec![Rule::prefix("prefix", "tok_", Severity::High)],
            vec![Rule::pattern("pattern", "abc", Severity::High).unwrap()],
        ] {
            let compiled = CompiledRuleSet::compile(rules).unwrap();
            assert_eq!(
                compiled
                    .streaming_storage_requirements()
                    .unwrap()
                    .suffix_token_bytes,
                0
            );
            let mut state = SuffixStreamState::default();
            let mut findings = Vec::new();
            let mut emitted = 0;
            let mut sink = CandidateSink::new(&mut findings, &mut emitted, Some(0));
            for chunk in [vec![b'a'; 1_000_000], b"_end!".to_vec()] {
                compiled
                    .scan_suffix_stream_chunk(&mut state, &chunk, 0, &mut sink)
                    .unwrap();
                assert!(state.token.is_empty());
                assert_eq!(state.token.capacity(), 0);
                assert!(!state.oversized);
                assert_eq!(state.token_start, 0);
            }
            compiled
                .finish_suffix_stream(&mut state, &mut sink)
                .unwrap();
            assert!(state.token.is_empty());
            assert_eq!(state.token.capacity(), 0);
            assert_eq!(emitted, 0);
            assert!(findings.is_empty());
        }
    }

    #[test]
    fn overflowing_storage_is_rejected_only_when_admission_is_requested() {
        use crate::source_session::{SourceLimits, SourceSession, SourceSessionError};
        use std::sync::Arc;

        let mut compiled = CompiledRuleSet::compile(vec![
            Rule::pattern("pattern", "abc", Severity::High).unwrap(),
        ])
        .unwrap();
        // Inject an unrepresentable rule-derived requirement without trying to
        // compile or allocate a usize-sized regex. Classification is unchanged.
        compiled.patterns[0].stream_plan = plan(usize::MAX);
        let rules = Arc::new(compiled);
        assert!(rules.supports_streaming());
        assert!(SourceSession::new(Arc::clone(&rules)).is_ok());
        assert!(SourceSession::new_with_max_candidates(Arc::clone(&rules), Some(0)).is_ok());
        assert!(
            SourceSession::new_with_limits(Arc::clone(&rules), SourceLimits::default()).is_ok()
        );
        for limit in [0, usize::MAX] {
            assert_eq!(
                SourceSession::new_with_limits(
                    Arc::clone(&rules),
                    SourceLimits {
                        max_candidates: None,
                        max_stream_storage_bytes: Some(limit),
                    }
                )
                .unwrap_err(),
                SourceSessionError::StreamStorageRequirementOverflow
            );
        }
    }

    #[test]
    fn empty_and_suffix_only_account_for_configured_tokenizer() {
        for (rules, expected) in [
            (vec![], 0),
            (
                vec![Rule::suffix("suffix", "_end", Severity::High)],
                MAX_SUFFIX_TOKEN_LEN,
            ),
        ] {
            let storage = requirements(rules);
            assert_eq!(storage.overlap_bytes, 0);
            assert_eq!(storage.active_prefix_elements, 0);
            assert_eq!(storage.active_prefix_bytes, 0);
            assert!(storage.pattern_buffer_bytes.is_empty());
            assert_eq!(storage.suffix_token_bytes, expected);
            assert_eq!(storage.persistent_buffer_bytes, expected);
            assert_eq!(storage.temporary_buffer_bytes, 0);
            assert_eq!(storage.total_buffer_bytes, expected);
        }
    }

    #[test]
    fn literal_overlap_and_boundary_requirements() {
        let storage = requirements(vec![
            Rule::literal("short", "a", Severity::High),
            Rule::literal("long", "secret", Severity::High),
        ]);
        assert_eq!(storage.overlap_bytes, 5);
        assert_eq!(storage.boundary_bytes, 10);
        assert_eq!(storage.active_prefix_bytes, 0);
        assert_eq!(storage.total_buffer_bytes, 15);
    }

    #[test]
    fn multiple_prefix_rules_account_for_element_size() {
        let storage = requirements(vec![
            Rule::prefix("one", "abc_", Severity::High),
            Rule::prefix("two", "abcdef", Severity::High),
            Rule::prefix("three", "ab", Severity::High),
        ]);
        assert_eq!(storage.active_prefix_elements, 3);
        assert_eq!(
            storage.active_prefix_bytes,
            3 * std::mem::size_of::<ActivePrefix>()
        );
        assert_eq!(storage.overlap_bytes, 5);
        assert_eq!(storage.total_buffer_bytes, 15 + storage.active_prefix_bytes);
    }

    #[test]
    fn single_and_multiple_regex_requirements_include_transient_append() {
        let plain = || Rule::pattern("plain", "abc", Severity::High).unwrap();
        assert_eq!(&*requirements(vec![plain()]).pattern_buffer_bytes, &[18]);
        let storage = requirements(vec![
            plain(),
            Rule::pattern("word", r"\babc\b", Severity::High).unwrap(),
            Rule::pattern("both", r"\Aabc\z", Severity::High).unwrap(),
        ]);
        assert_eq!(&*storage.pattern_buffer_bytes, &[18, 34, 3]);
        assert_eq!(storage.total_buffer_bytes, 18 + 34 + 3);
    }

    #[test]
    fn mixed_ruleset_sums_persistent_and_temporary_payloads() {
        let storage = requirements(vec![
            Rule::literal("literal", "secret", Severity::High),
            Rule::prefix("prefix", "abc_", Severity::High),
            Rule::suffix("suffix", "_end", Severity::High),
            Rule::pattern("regex", "abc", Severity::High).unwrap(),
        ]);
        assert_eq!(
            storage.persistent_buffer_bytes,
            5 + std::mem::size_of::<ActivePrefix>() + 4096 + 18
        );
        assert_eq!(storage.temporary_buffer_bytes, 10);
        assert_eq!(
            storage.total_buffer_bytes,
            storage.persistent_buffer_bytes + 10
        );
    }

    fn plan(max_match_len: usize) -> PatternStreamPlan {
        PatternStreamPlan::Bounded {
            max_match_len,
            context: PatternStreamContext {
                history_bytes: 4,
                lookahead_bytes: 4,
                requires_text_start: false,
                requires_text_end: false,
            },
        }
    }

    #[test]
    fn large_requirements_need_no_corresponding_buffer_allocation() {
        let maximum = usize::MAX / 4;
        let bytes = StreamingStorageRequirements::pattern_buffer_bytes(plan(maximum)).unwrap();
        assert_eq!(bytes, 2 * (maximum + 12) + 4);
        let storage =
            StreamingStorageRequirements::from_components(100_000, 10_000, true, Box::new([bytes]))
                .unwrap();
        assert_eq!(
            storage.total_buffer_bytes,
            300_000 + 10_000 * std::mem::size_of::<ActivePrefix>() + 4096 + bytes
        );
        let literal = "x".repeat(8192);
        let compiled = requirements(vec![Rule::literal(
            "large",
            literal.as_str(),
            Severity::High,
        )]);
        assert_eq!(compiled.overlap_bytes, 8191);
        assert_eq!(compiled.boundary_bytes, 16_382);
    }

    #[test]
    fn arithmetic_overflow_is_explicit() {
        let error = Err(StreamingStorageError::ArithmeticOverflow);
        for maximum in [usize::MAX, usize::MAX - 10, usize::MAX / 2] {
            assert_eq!(
                StreamingStorageRequirements::pattern_buffer_bytes(plan(maximum)),
                error
            );
        }
        for (overlap, prefixes, patterns) in [
            (usize::MAX, 0, vec![]),                      // Boundary multiplication.
            (0, usize::MAX, vec![]),                      // ActivePrefix element multiplication.
            (0, 0, vec![usize::MAX]),                     // Persistent sum.
            (0, 0, vec![usize::MAX / 2, usize::MAX / 2]), // Multiple regex sum.
            (usize::MAX / 3, 0, vec![]),                  // Persistent + temporary sum.
        ] {
            assert_eq!(
                StreamingStorageRequirements::from_components(
                    overlap,
                    prefixes,
                    true,
                    patterns.into_boxed_slice()
                ),
                Err(StreamingStorageError::ArithmeticOverflow)
            );
        }
    }

    #[test]
    fn unsupported_regex_has_no_claimed_storage_requirement() {
        for pattern in ["a+", r"(?:\Aabc|def)"] {
            let compiled = CompiledRuleSet::compile(vec![
                Rule::pattern("unsupported", pattern, Severity::High).unwrap(),
            ])
            .unwrap();
            assert!(!compiled.supports_streaming());
            assert_eq!(
                compiled.streaming_storage_requirements(),
                Err(StreamingStorageError::UnsupportedPattern(RuleIndex::new(0)))
            );
        }
    }

    #[test]
    fn absolute_start_end_requirement_covers_complete_and_rejected_candidates() {
        let compiled = CompiledRuleSet::compile(vec![
            Rule::pattern("both", r"\A🦀{4}\z", Severity::High).unwrap(),
        ])
        .unwrap();
        let storage = compiled.streaming_storage_requirements().unwrap();
        assert_eq!(&*storage.pattern_buffer_bytes, &[16]);
        for source in ["🦀".repeat(4), "🦀".repeat(5)] {
            let mut whole = Vec::new();
            compiled.scan(&source, &mut whole);
            for chunk_chars in [1, 4, 5] {
                let mut state = PatternStreamState::default();
                let mut findings = Vec::new();
                for (index, chunk) in source.as_bytes().chunks(4 * chunk_chars).enumerate() {
                    compiled.patterns[0].scan_stream_bounded(
                        &mut state,
                        chunk,
                        index * 4 * chunk_chars,
                        &mut findings,
                    );
                    assert!(state.peak_buffer_len <= 16);
                }
                compiled.patterns[0].finish_stream_bounded(&mut state, &mut findings);
                assert_eq!(findings, whole);
            }
        }
    }

    #[test]
    fn requirements_cover_observed_storage_and_preserve_partition_parity() {
        let compiled = CompiledRuleSet::compile(vec![
            Rule::literal("literal", "secret", Severity::High),
            Rule::prefix("prefix", "ab", Severity::High),
            Rule::prefix("overlap", "abc", Severity::High),
            Rule::suffix("suffix", "_end", Severity::High),
            Rule::pattern("plain", "secret", Severity::High).unwrap(),
            Rule::captured_pattern(
                "capture",
                "KEY=(?P<value>[A-Z]{2})!",
                "value",
                Severity::High,
            )
            .unwrap(),
            Rule::pattern("word", r"\bsecret\b", Severity::High).unwrap(),
            Rule::pattern("lf", r"(?m)^secret$", Severity::High).unwrap(),
            Rule::pattern("crlf", r"(?mR)^secret$", Severity::High).unwrap(),
            Rule::pattern("start", r"\Aabc", Severity::High).unwrap(),
            Rule::pattern("end", r"(?mR)^secret\z", Severity::High).unwrap(),
            Rule::pattern("both", r"\Aabc\z", Severity::High).unwrap(),
        ])
        .unwrap();
        let storage = compiled.streaming_storage_requirements().unwrap();
        let source = format!(
            "abc{}! {}! {}_end secret\r\nKEY=AB! ésecret 🦀\nsecret",
            "abc".repeat(2000),
            "x".repeat(4096),
            "x".repeat(4092)
        );
        let project = |findings: &[InternalFinding]| {
            let mut spans = findings
                .iter()
                .map(|finding| (finding.rule_index(), finding.start(), finding.end()))
                .collect::<Vec<_>>();
            spans.sort_unstable();
            spans
        };
        let mut whole = Vec::new();
        compiled.scan(&source, &mut whole);
        let mut peaks = (0, 0, 0, 0);
        let mut regex_peaks = vec![0; compiled.patterns.len()];
        for partition in [1, 2, 7, 31, 4093, source.len()] {
            let mut multi = MultiPatternStreamState::default();
            let mut suffix = SuffixStreamState::default();
            let mut patterns = compiled.new_pattern_stream_states();
            let mut findings = Vec::new();
            let mut emitted = 0;
            let mut sink = CandidateSink::new(&mut findings, &mut emitted, None);
            let mut transport = crate::utf8_transport::Utf8Transport::default();
            let mut offset = 0;
            for chunk in source.as_bytes().chunks(partition) {
                transport
                    .push(chunk, |text| {
                        // Exact logical length of the boundary temporary constructed
                        // by the executor; no assumption about its Vec capacity.
                        let boundary_len = if multi.overlap.is_empty() || text.is_empty() {
                            0
                        } else {
                            multi.overlap.len() + text.len().min(storage.overlap_bytes)
                        };
                        assert!(boundary_len <= storage.boundary_bytes);
                        peaks.2 = peaks.2.max(boundary_len);
                        compiled
                            .scan_multi_pattern_stream_chunk(
                                &mut multi,
                                text.as_bytes(),
                                offset,
                                &mut sink,
                            )
                            .unwrap();
                        compiled
                            .scan_suffix_stream_chunk(
                                &mut suffix,
                                text.as_bytes(),
                                offset,
                                &mut sink,
                            )
                            .unwrap();
                        compiled
                            .scan_pattern_stream_chunk(
                                &mut patterns,
                                text.as_bytes(),
                                offset,
                                &mut sink,
                            )
                            .unwrap();
                        offset += text.len();
                        assert!(multi.overlap.len() <= storage.overlap_bytes);
                        assert!(multi.active_prefixes.len() <= storage.active_prefix_elements);
                        assert!(suffix.token.len() <= storage.suffix_token_bytes);
                        peaks.0 = peaks.0.max(multi.overlap.len());
                        peaks.1 = peaks.1.max(multi.active_prefixes.len());
                        peaks.3 = peaks.3.max(suffix.token.len());
                        for ((state, bound), peak) in patterns
                            .iter()
                            .zip(&storage.pattern_buffer_bytes)
                            .zip(&mut regex_peaks)
                        {
                            assert!(state.peak_buffer_len <= *bound);
                            *peak = (*peak).max(state.peak_buffer_len);
                        }
                        let persistent = multi.overlap.len()
                            + multi.active_prefixes.len() * std::mem::size_of::<ActivePrefix>()
                            + suffix.token.len()
                            + patterns
                                .iter()
                                .map(|state| state.buffer.len())
                                .sum::<usize>();
                        assert!(persistent <= storage.persistent_buffer_bytes);
                        // Combining these peaks is conservative: the boundary Vec
                        // has already been dropped when the regex buffers grow.
                        assert!(persistent + boundary_len <= storage.total_buffer_bytes);
                    })
                    .unwrap();
            }
            transport.finish().unwrap();
            compiled
                .finish_multi_pattern_stream(&mut multi, offset, &mut sink)
                .unwrap();
            compiled
                .finish_suffix_stream(&mut suffix, &mut sink)
                .unwrap();
            compiled
                .finish_pattern_stream(&mut patterns, &mut sink)
                .unwrap();
            assert_eq!(project(&findings), project(&whole), "partition={partition}");
        }
        assert_eq!(
            peaks,
            (storage.overlap_bytes, 2, storage.boundary_bytes, 4096)
        );
        println!("observed regex logical peaks: {regex_peaks:?}");
        println!(
            "storage audit: overlap={}, active_prefix_elements={}, boundary={}, suffix={}, regex_bounds={:?}, persistent={}, temporary={}, total={}",
            peaks.0,
            peaks.1,
            peaks.2,
            peaks.3,
            storage.pattern_buffer_bytes,
            storage.persistent_buffer_bytes,
            storage.temporary_buffer_bytes,
            storage.total_buffer_bytes
        );
    }
}

#[cfg(test)]
mod builtin_streaming_compatibility_tests {
    use super::{CompiledRuleSet, PatternStreamPlan};
    use crate::{RuleSpec, builtins, rule::Matcher};

    #[derive(Debug, Default, Eq, PartialEq)]
    struct CompatibilityReport {
        total_rules: usize,
        pattern_rules: usize,
        bounded_patterns: usize,
        unsupported_unbounded_ids: Vec<String>,
        unsupported_finite_ids: Vec<String>,
        semantic_rejections: Vec<(String, &'static str)>,
        admitted_ids: Vec<String>,
    }

    fn audit(name: &str, specifications: &[RuleSpec]) -> CompatibilityReport {
        let rules = specifications
            .iter()
            .map(|specification| specification.to_rule().expect("built-in rule must compile"))
            .collect::<Vec<_>>();
        // Compilation consumes the rules. Preserve their existing length
        // metadata by rule index without parsing or analysing patterns again.
        let maximum_lengths = rules
            .iter()
            .map(|rule| match &rule.matcher {
                Matcher::Pattern { maximum_len, .. } => Some(*maximum_len),
                _ => None,
            })
            .collect::<Vec<_>>();
        let compiled =
            CompiledRuleSet::compile(rules.clone()).expect("built-in portfolio must compile");
        let mut report = CompatibilityReport {
            total_rules: compiled.len(),
            pattern_rules: compiled.patterns.len(),
            ..CompatibilityReport::default()
        };

        for rule in rules {
            let id = rule.id.as_str().to_owned();
            let single = CompiledRuleSet::compile(vec![rule]).unwrap();
            if !single.supports_streaming_semantics() {
                let reason = if !single.patterns.is_empty() {
                    "per-rule prefilter"
                } else if !single.suffixes.is_empty() {
                    "suffix needle semantics"
                } else {
                    "byte-dependent prefix"
                };
                report.semantic_rejections.push((id, reason));
            } else if single.supports_streaming()
                && single.contextual_storage_plan().is_ok()
                && single.coordinate_storage_plan().is_ok()
            {
                report.admitted_ids.push(id);
            }
        }
        report.semantic_rejections.sort_unstable();
        report.admitted_ids.sort_unstable();
        assert_eq!(
            compiled.supports_streaming_semantics(),
            report.semantic_rejections.is_empty()
        );
        println!(
            "{name}: semantically_rejected={:?}, individually_admitted={:?}",
            report.semantic_rejections, report.admitted_ids
        );

        for pattern in &compiled.patterns {
            match pattern.stream_plan() {
                PatternStreamPlan::Bounded { .. } => report.bounded_patterns += 1,
                PatternStreamPlan::Unsupported => {
                    let maximum_len = maximum_lengths[pattern.rule_index.get()]
                        .expect("compiled pattern must have matcher length metadata");
                    let id = compiled
                        .metadata(pattern.rule_index)
                        .id()
                        .as_str()
                        .to_owned();
                    match maximum_len {
                        None => report.unsupported_unbounded_ids.push(id),
                        Some(_) => report.unsupported_finite_ids.push(id),
                    }
                }
            }
        }
        report.unsupported_unbounded_ids.sort_unstable();
        report.unsupported_finite_ids.sort_unstable();

        assert_eq!(report.total_rules, specifications.len(), "{name}");
        assert_eq!(
            report.pattern_rules,
            report.bounded_patterns
                + report.unsupported_unbounded_ids.len()
                + report.unsupported_finite_ids.len(),
            "{name}"
        );
        assert_eq!(
            compiled.supports_streaming(),
            report.unsupported_unbounded_ids.is_empty() && report.unsupported_finite_ids.is_empty(),
            "{name}: compatibility report must agree with supports_streaming()"
        );
        println!(
            "{name}: total_rules={}, pattern_rules={}, bounded_patterns={}, unsupported_unbounded_patterns={}, unsupported_finite_patterns={}, unsupported_unbounded_ids={:?}, unsupported_finite_ids={:?}, supports_streaming={}",
            report.total_rules,
            report.pattern_rules,
            report.bounded_patterns,
            report.unsupported_unbounded_ids.len(),
            report.unsupported_finite_ids.len(),
            report.unsupported_unbounded_ids,
            report.unsupported_finite_ids,
            compiled.supports_streaming(),
        );
        report
    }

    #[test]
    fn builtin_portfolio_streaming_compatibility_audit() {
        let portfolios = [
            ("Core", builtins::CURRENT),
            ("Personal", builtins::personal::CURRENT),
            ("Financial", builtins::financial::CURRENT),
        ];
        let mut specifications = Vec::new();
        let mut expected = CompatibilityReport::default();
        for (name, portfolio) in portfolios {
            let report = audit(name, portfolio);
            specifications.extend_from_slice(portfolio);
            expected.total_rules += report.total_rules;
            expected.pattern_rules += report.pattern_rules;
            expected.bounded_patterns += report.bounded_patterns;
            expected
                .semantic_rejections
                .extend(report.semantic_rejections);
            expected.admitted_ids.extend(report.admitted_ids);
            expected
                .unsupported_unbounded_ids
                .extend(report.unsupported_unbounded_ids);
            expected
                .unsupported_finite_ids
                .extend(report.unsupported_finite_ids);
        }
        expected.unsupported_unbounded_ids.sort_unstable();
        expected.unsupported_finite_ids.sort_unstable();
        expected.semantic_rejections.sort_unstable();
        expected.admitted_ids.sort_unstable();
        assert_eq!(audit("Combined", &specifications), expected);
    }
}

#[cfg(test)]
mod layout_tests {
    use super::{InternalFinding, RuleIndex};

    #[test]
    fn print_internal_layout_sizes() {
        println!("RuleIndex: {} bytes", std::mem::size_of::<RuleIndex>());
        println!(
            "InternalFinding: {} bytes",
            std::mem::size_of::<InternalFinding>()
        );
    }
}

#[cfg(test)]
mod validator_metadata_tests {
    use super::*;
    use crate::{Rule, Severity};

    #[test]
    fn compiled_metadata_preserves_validator_kind() {
        let rule = Rule::prefix("github", "ghp_", Severity::Critical)
            .with_validator(ValidatorKind::GitHub);

        let rules = CompiledRuleSet::compile(vec![rule]).expect("rule set should compile");

        assert_eq!(
            rules.metadata(RuleIndex::new(0)).validator(),
            ValidatorKind::GitHub,
        );
    }

    #[test]
    fn public_metadata_preserves_detection_mode() {
        let matcher_only = Rule::literal("literal", "secret", Severity::High);
        let deterministic = Rule::prefix("github", "ghp_", Severity::Critical)
            .with_validator(ValidatorKind::GitHub);
        let contextual = Rule::pattern("password", r#"(?i)password\s*=\s*[^\s]+"#, Severity::High)
            .expect("pattern should compile")
            .with_validator(ValidatorKind::Password);

        let rules = CompiledRuleSet::compile(vec![matcher_only, deterministic, contextual])
            .expect("rule set should compile");
        let metadata = rules.public_metadata().collect::<Vec<_>>();

        assert_eq!(
            metadata[0].detection_mode(),
            crate::DetectionMode::MatcherOnly
        );
        assert_eq!(
            metadata[1].detection_mode(),
            crate::DetectionMode::Deterministic
        );
        assert_eq!(
            metadata[2].detection_mode(),
            crate::DetectionMode::Contextual
        );
    }

    #[test]
    fn provider_specific_metadata_outranks_generic_metadata() {
        let provider = Rule::prefix("github", "ghp_", Severity::Critical)
            .with_validator(ValidatorKind::GitHub);
        let generic = Rule::prefix("generic", "ghp_", Severity::Critical)
            .with_validator(ValidatorKind::GenericCredential);

        let rules =
            CompiledRuleSet::compile(vec![provider, generic]).expect("rule set should compile");

        assert!(
            rules.metadata(RuleIndex::new(0)).priority()
                > rules.metadata(RuleIndex::new(1)).priority()
        );
    }
}

#[cfg(test)]
mod capture_projection_tests {
    use super::*;
    use crate::{Rule, Severity};

    #[test]
    fn captured_pattern_emits_only_named_capture_span() {
        let rule = Rule::captured_pattern(
            "assignment",
            r#"AWS_SECRET_ACCESS_KEY=(?P<value>[A-Za-z0-9/+=]{40})"#,
            "value",
            Severity::Critical,
        )
        .expect("captured pattern should compile");

        let rules = CompiledRuleSet::compile(vec![rule]).expect("rule set should compile");
        let source = "AWS_SECRET_ACCESS_KEY=wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY";
        let expected = source
            .find("wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY")
            .expect("fixture must contain value");

        let mut findings = Vec::new();
        rules.scan(source, &mut findings);

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].start(), expected);
        assert_eq!(
            findings[0].end(),
            expected + "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY".len(),
        );
    }

    #[test]
    fn normal_pattern_still_emits_complete_match_span() {
        let rule = Rule::pattern("assignment", r#"KEY=[A-Za-z0-9_]+"#, Severity::High)
            .expect("pattern should compile");

        let rules = CompiledRuleSet::compile(vec![rule]).expect("rule set should compile");
        let source = "KEY=secret_value";

        let mut findings = Vec::new();
        rules.scan(source, &mut findings);

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].start(), 0);
        assert_eq!(findings[0].end(), source.len());
    }
}

#[cfg(test)]
mod multi_pattern_tests {
    use super::*;
    use crate::{Rule, Severity};

    fn scan(rules: Vec<Rule>, source: &str) -> Vec<(usize, usize, RuleIndex)> {
        let compiled = CompiledRuleSet::compile(rules).expect("rules should compile");
        let mut findings = Vec::new();

        compiled.scan(source, &mut findings);

        findings
            .into_iter()
            .map(|finding| (finding.start(), finding.end(), finding.rule_index()))
            .collect()
    }

    #[test]
    fn literal_matches_without_token_boundaries() {
        let findings = scan(
            vec![Rule::literal("literal", "secret", Severity::High)],
            "xsecrety secret",
        );

        assert_eq!(
            findings,
            vec![(1, 7, RuleIndex::new(0)), (9, 15, RuleIndex::new(0)),]
        );
    }

    #[test]
    fn overlapping_literals_are_preserved() {
        let findings = scan(
            vec![
                Rule::literal("long", "secret", Severity::High),
                Rule::literal("short", "sec", Severity::High),
            ],
            "secret",
        );

        assert_eq!(findings.len(), 2);
        assert!(findings.contains(&(0, 6, RuleIndex::new(0))));
        assert!(findings.contains(&(0, 3, RuleIndex::new(1))));
    }

    #[test]
    fn identical_literals_from_distinct_rules_are_preserved() {
        let findings = scan(
            vec![
                Rule::literal("first", "secret", Severity::High),
                Rule::literal("second", "secret", Severity::High),
            ],
            "secret",
        );

        assert_eq!(findings.len(), 2);
        assert!(findings.contains(&(0, 6, RuleIndex::new(0))));
        assert!(findings.contains(&(0, 6, RuleIndex::new(1))));
    }

    #[test]
    fn prefix_requires_left_token_boundary() {
        let findings = scan(
            vec![Rule::prefix("token", "ghp_", Severity::Critical)],
            "xghp_invalid ghp_valid",
        );

        assert_eq!(findings, vec![(13, 22, RuleIndex::new(0))]);
    }

    #[test]
    fn prefix_extends_through_complete_ascii_token() {
        let findings = scan(
            vec![Rule::prefix("token", "ghp_", Severity::Critical)],
            "ghp_abc-DEF_123!",
        );

        assert_eq!(findings, vec![(0, 15, RuleIndex::new(0))]);
    }

    #[test]
    fn overlapping_prefix_needles_are_preserved() {
        let findings = scan(
            vec![
                Rule::prefix("short", "tok", Severity::High),
                Rule::prefix("long", "token_", Severity::High),
            ],
            "token_value",
        );

        assert_eq!(findings.len(), 2);
        assert!(findings.contains(&(0, 11, RuleIndex::new(0))));
        assert!(findings.contains(&(0, 11, RuleIndex::new(1))));
    }

    #[test]
    fn unicode_adjacent_to_prefix_preserves_utf8_offsets() {
        let findings = scan(
            vec![Rule::prefix("token", "ghp_", Severity::Critical)],
            "😀ghp_value",
        );

        assert_eq!(findings, vec![(4, 13, RuleIndex::new(0))]);
    }

    #[test]
    fn engine_records_longest_multi_pattern_needle() {
        let compiled = CompiledRuleSet::compile(vec![
            Rule::literal("short", "abc", Severity::High),
            Rule::prefix("long", "prefix_", Severity::Critical),
        ])
        .expect("rules should compile");

        let engine = compiled
            .multi_pattern
            .as_ref()
            .expect("multi-pattern engine should exist");

        assert_eq!(engine.max_needle_len, 7);
        assert_eq!(engine.max_overlap_len(), 6);
    }

    #[test]
    fn single_byte_needles_require_no_overlap() {
        let compiled = CompiledRuleSet::compile(vec![Rule::literal("single", "x", Severity::High)])
            .expect("rules should compile");

        let engine = compiled
            .multi_pattern
            .as_ref()
            .expect("multi-pattern engine should exist");

        assert_eq!(engine.max_needle_len, 1);
        assert_eq!(engine.max_overlap_len(), 0);
    }

    #[test]
    fn stream_state_starts_empty() {
        let state = MultiPatternStreamState::default();

        assert!(state.overlap.is_empty());
        assert_eq!(state.byte_before_overlap, None);
        assert!(state.active_prefixes.is_empty());
    }

    #[test]
    fn stream_overlap_is_bounded_by_longest_needle() {
        let compiled =
            CompiledRuleSet::compile(vec![Rule::literal("literal", "secret", Severity::High)])
                .expect("rules should compile");

        let engine = compiled
            .multi_pattern
            .as_ref()
            .expect("multi-pattern engine should exist");

        let mut state = MultiPatternStreamState::default();

        engine.update_stream_overlap(&mut state, b"abcdefgh");

        assert_eq!(state.overlap, b"defgh");
    }

    #[test]
    fn stream_overlap_combines_only_required_tail() {
        let compiled =
            CompiledRuleSet::compile(vec![Rule::literal("literal", "secret", Severity::High)])
                .expect("rules should compile");

        let engine = compiled
            .multi_pattern
            .as_ref()
            .expect("multi-pattern engine should exist");

        let mut state = MultiPatternStreamState::default();

        engine.update_stream_overlap(&mut state, b"abc");
        assert_eq!(state.overlap, b"abc");

        engine.update_stream_overlap(&mut state, b"de");
        assert_eq!(state.overlap, b"abcde");

        engine.update_stream_overlap(&mut state, b"fg");
        assert_eq!(state.overlap, b"cdefg");
    }

    #[test]
    fn stream_overlap_may_retain_partial_utf8_bytes() {
        let compiled =
            CompiledRuleSet::compile(vec![Rule::literal("literal", "abcdef", Severity::High)])
                .expect("rules should compile");

        let engine = compiled
            .multi_pattern
            .as_ref()
            .expect("multi-pattern engine should exist");

        let mut state = MultiPatternStreamState::default();

        engine.update_stream_overlap(&mut state, b"abcdefgh");

        assert_eq!(state.overlap.len(), engine.max_overlap_len());
    }

    fn scan_literal_chunks(rules: Vec<Rule>, chunks: &[&[u8]]) -> Vec<(usize, usize, RuleIndex)> {
        let compiled = CompiledRuleSet::compile(rules).expect("rules should compile");
        let engine = compiled
            .multi_pattern
            .as_ref()
            .expect("multi-pattern engine should exist");

        let mut state = MultiPatternStreamState::default();
        let mut findings = Vec::new();
        let mut offset = 0;

        for chunk in chunks {
            engine.scan_stream_chunk(&mut state, chunk, offset, &mut findings);
            offset += chunk.len();
        }

        findings
            .into_iter()
            .map(|finding| (finding.start(), finding.end(), finding.rule_index()))
            .collect()
    }

    #[test]
    fn streamed_literal_matches_inside_single_chunk() {
        let findings = scan_literal_chunks(
            vec![Rule::literal("literal", "secret", Severity::High)],
            &[b"xxsecretyy"],
        );

        assert_eq!(findings, vec![(2, 8, RuleIndex::new(0))]);
    }

    #[test]
    fn streamed_literal_matches_across_chunk_boundary() {
        let findings = scan_literal_chunks(
            vec![Rule::literal("literal", "secret", Severity::High)],
            &[b"xxsec", b"retyy"],
        );

        assert_eq!(findings, vec![(2, 8, RuleIndex::new(0))]);
    }

    #[test]
    fn streamed_literal_is_emitted_exactly_once() {
        let findings = scan_literal_chunks(
            vec![Rule::literal("literal", "secret", Severity::High)],
            &[b"xxsec", b"ret", b"secret"],
        );

        assert_eq!(
            findings,
            vec![(2, 8, RuleIndex::new(0)), (8, 14, RuleIndex::new(0)),]
        );
    }

    #[test]
    fn streamed_overlapping_literals_are_preserved() {
        let findings = scan_literal_chunks(
            vec![
                Rule::literal("long", "secret", Severity::High),
                Rule::literal("short", "sec", Severity::High),
            ],
            &[b"se", b"cret"],
        );

        assert_eq!(findings.len(), 2);
        assert!(findings.contains(&(0, 6, RuleIndex::new(0))));
        assert!(findings.contains(&(0, 3, RuleIndex::new(1))));
    }

    #[test]
    fn streamed_literal_preserves_absolute_utf8_byte_offsets() {
        let findings = scan_literal_chunks(
            vec![Rule::literal("literal", "secret", Severity::High)],
            &["😀se".as_bytes(), b"cret"],
        );

        assert_eq!(findings, vec![(4, 10, RuleIndex::new(0))]);
    }

    #[test]
    fn active_prefix_continues_across_chunks_until_boundary() {
        let mut state = MultiPatternStreamState {
            active_prefixes: vec![ActivePrefix {
                rule_index: RuleIndex::new(0),
                source_start: 4,
            }],
            ..Default::default()
        };

        let mut findings = Vec::new();

        MultiPatternEngine::continue_active_prefixes(&mut state, b"abc123", 8, &mut findings);

        assert!(findings.is_empty());
        assert_eq!(state.active_prefixes.len(), 1);

        MultiPatternEngine::continue_active_prefixes(&mut state, b"DEF rest", 14, &mut findings);

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].rule_index(), RuleIndex::new(0));
        assert_eq!(findings[0].start(), 4);
        assert_eq!(findings[0].end(), 17);
        assert!(state.active_prefixes.is_empty());
    }

    #[test]
    fn active_prefix_is_emitted_at_end_of_stream() {
        let mut state = MultiPatternStreamState {
            active_prefixes: vec![ActivePrefix {
                rule_index: RuleIndex::new(0),
                source_start: 4,
            }],
            ..Default::default()
        };

        let mut findings = Vec::new();

        MultiPatternEngine::finish_stream(&mut state, 19, &mut findings);

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].start(), 4);
        assert_eq!(findings[0].end(), 19);
        assert!(state.active_prefixes.is_empty());
    }

    #[test]
    fn stream_prefix_accepts_complete_token() {
        let compiled =
            CompiledRuleSet::compile(vec![Rule::prefix("github", "ghp_", Severity::Critical)])
                .expect("rules should compile");

        let engine = compiled
            .multi_pattern
            .as_ref()
            .expect("multi-pattern engine should exist");

        let mut state = MultiPatternStreamState::default();
        let mut findings = Vec::new();

        engine.scan_stream_chunk(&mut state, b"ghp_secret!", 0, &mut findings);

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].start(), 0);
        assert_eq!(findings[0].end(), 10);
        assert!(state.active_prefixes.is_empty());
    }

    #[test]
    fn stream_prefix_remains_active_without_token_boundary() {
        let compiled =
            CompiledRuleSet::compile(vec![Rule::prefix("github", "ghp_", Severity::Critical)])
                .expect("rules should compile");

        let engine = compiled
            .multi_pattern
            .as_ref()
            .expect("multi-pattern engine should exist");

        let mut state = MultiPatternStreamState::default();
        let mut findings = Vec::new();

        engine.scan_stream_chunk(&mut state, b"ghp_secret", 0, &mut findings);

        assert!(findings.is_empty());
        assert_eq!(state.active_prefixes.len(), 1);
        assert_eq!(state.active_prefixes[0].source_start, 0);
    }

    fn assert_active_prefix_cardinality(
        state: &MultiPatternStreamState,
        prefix_rule_count: usize,
    ) -> usize {
        assert!(
            state.active_prefixes.len() <= prefix_rule_count,
            "active candidates exceed configured prefix rules: active={:?}, rules={prefix_rule_count}",
            state.active_prefixes
        );
        let active_rules = state
            .active_prefixes
            .iter()
            .map(|active| active.rule_index)
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(
            active_rules.len(),
            state.active_prefixes.len(),
            "each configured prefix rule may have at most one active candidate"
        );
        state.active_prefixes.len()
    }

    fn assert_prefix_cardinality_and_parity(
        rules: Vec<Rule>,
        source: &str,
        chunk_sizes: &[usize],
    ) -> usize {
        let compiled = CompiledRuleSet::compile(rules).unwrap();
        let engine = compiled.multi_pattern.as_ref().unwrap();
        let prefix_rule_count = engine
            .rules
            .iter()
            .filter(|rule| rule.kind == MultiPatternKind::Prefix)
            .count();
        let mut whole = Vec::new();
        compiled.scan(source, &mut whole);
        let expected = projected_findings(&whole);
        let mut peak = 0;
        for &chunk_size in chunk_sizes {
            let mut state = MultiPatternStreamState::default();
            let mut findings = Vec::new();
            let mut offset = 0;
            for chunk in source.as_bytes().chunks(chunk_size) {
                engine.scan_stream_chunk(&mut state, chunk, offset, &mut findings);
                offset += chunk.len();
                peak = peak.max(assert_active_prefix_cardinality(&state, prefix_rule_count));
                // Empty transport chunks must not duplicate active candidates.
                engine.scan_stream_chunk(&mut state, b"", offset, &mut findings);
                peak = peak.max(assert_active_prefix_cardinality(&state, prefix_rule_count));
            }
            MultiPatternEngine::finish_stream(&mut state, offset, &mut findings);
            assert_eq!(
                assert_active_prefix_cardinality(&state, prefix_rule_count),
                0
            );
            assert_eq!(
                projected_findings(&findings),
                expected,
                "prefix parity: source_bytes={}, chunk_size={chunk_size}",
                source.len()
            );
        }
        println!(
            "active-prefix cardinality: source_bytes={}, prefix_rules={prefix_rule_count}, peak={peak}",
            source.len()
        );
        peak
    }

    #[test]
    fn active_prefix_cardinality_repeated_needles_in_long_token() {
        for repetitions in [16, 1024, 16384] {
            let source = "ghp_valueghp_again-".repeat(repetitions);
            let peak = assert_prefix_cardinality_and_parity(
                vec![Rule::prefix("prefix", "ghp_", Severity::Critical)],
                &source,
                &[source.len(), source.len() / 2 + 1, 7, 1],
            );
            assert_eq!(peak, 1);
        }
    }

    #[test]
    fn active_prefix_cardinality_multiple_configured_rules() {
        let source = "ghp_valueghp_inside! glpat-valueglpat-inside\ntokvaluetokinside\r\n"
            .repeat(1024)
            + "ghp_tail";
        let peak = assert_prefix_cardinality_and_parity(
            vec![
                Rule::prefix("github", "ghp_", Severity::Critical),
                Rule::prefix("gitlab", "glpat-", Severity::Critical),
                Rule::prefix("token", "tok", Severity::High),
                Rule::literal("literal", "inside", Severity::High),
            ],
            &source,
            &[source.len(), 4093, 11, 1],
        );
        assert_eq!(peak, 1, "only the current token contributes active rules");
    }

    #[test]
    fn active_prefix_cardinality_overlapping_and_identical_needles() {
        let source = "token_valuetoktoken_value-".repeat(8192);
        let peak = assert_prefix_cardinality_and_parity(
            vec![
                Rule::prefix("short", "t", Severity::High),
                Rule::prefix("medium", "tok", Severity::High),
                Rule::prefix("long", "token_", Severity::High),
                Rule::prefix("longer", "token_v", Severity::High),
                Rule::prefix("duplicate-needle", "token_", Severity::High),
                Rule::literal("literal", "tok", Severity::High),
            ],
            &source,
            &[source.len(), source.len() / 2 + 1, 4093, 7, 1],
        );
        assert_eq!(
            peak, 5,
            "distinct rules may share the same token-start candidate"
        );
    }

    #[test]
    fn active_prefix_cardinality_boundaries_reset_candidates() {
        let compiled = CompiledRuleSet::compile(vec![
            Rule::prefix("short", "tok", Severity::High),
            Rule::prefix("long", "token_", Severity::High),
        ])
        .unwrap();
        let engine = compiled.multi_pattern.as_ref().unwrap();
        let mut state = MultiPatternStreamState::default();
        let mut findings = Vec::new();
        let mut source = String::new();
        let mut peak = 0;
        for boundary in [" ", "!", "\t", "\n", "\r", "\r\n", "🦀"] {
            let token = "token_valuetoktoken_".repeat(128);
            for chunk in token.as_bytes().chunks(3) {
                engine.scan_stream_chunk(&mut state, chunk, source.len(), &mut findings);
                source.push_str(std::str::from_utf8(chunk).unwrap());
                peak = peak.max(assert_active_prefix_cardinality(&state, 2));
            }
            assert_eq!(state.active_prefixes.len(), 2);
            let previous_findings = findings.len();
            engine.scan_stream_chunk(&mut state, boundary.as_bytes(), source.len(), &mut findings);
            source.push_str(boundary);
            assert_eq!(assert_active_prefix_cardinality(&state, 2), 0);
            assert_eq!(findings.len(), previous_findings + 2);
        }
        // New candidates after every boundary must point to the new token.
        let final_start = source.len();
        engine.scan_stream_chunk(&mut state, b"token_tail", final_start, &mut findings);
        source.push_str("token_tail");
        assert_eq!(assert_active_prefix_cardinality(&state, 2), 2);
        assert!(
            state
                .active_prefixes
                .iter()
                .all(|active| active.source_start == final_start)
        );
        MultiPatternEngine::finish_stream(&mut state, source.len(), &mut findings);
        assert_eq!(assert_active_prefix_cardinality(&state, 2), 0);
        let mut whole = Vec::new();
        compiled.scan(&source, &mut whole);
        assert_eq!(projected_findings(&findings), projected_findings(&whole));
        assert_eq!(peak, 2);
        println!("boundary reset: prefix_rules=2, peak={peak}");
    }

    #[test]
    fn active_prefix_cardinality_needles_containing_boundaries() {
        let source = "a a a!a!a!!a!a!🦀tok🦀tok end ".repeat(1024) + "a a a";
        assert_prefix_cardinality_and_parity(
            vec![
                Rule::prefix("spaces", "a a", Severity::High),
                Rule::prefix("punctuation", "a!a", Severity::High),
                Rule::prefix("boundary-only", "!", Severity::High),
                Rule::prefix("unicode", "🦀tok", Severity::High),
            ],
            &source,
            &[source.len(), 4093, 7, 1],
        );
    }

    #[test]
    fn stream_prefix_rejects_missing_left_boundary() {
        let compiled =
            CompiledRuleSet::compile(vec![Rule::prefix("github", "ghp_", Severity::Critical)])
                .expect("rules should compile");

        let engine = compiled
            .multi_pattern
            .as_ref()
            .expect("multi-pattern engine should exist");

        let mut state = MultiPatternStreamState::default();
        let mut findings = Vec::new();

        engine.scan_stream_chunk(&mut state, b"xghp_secret!", 0, &mut findings);

        assert!(findings.is_empty());
    }

    #[test]
    fn stream_prefix_crosses_chunk_boundary() {
        let compiled =
            CompiledRuleSet::compile(vec![Rule::prefix("github", "ghp_", Severity::Critical)])
                .expect("rules should compile");

        let engine = compiled
            .multi_pattern
            .as_ref()
            .expect("multi-pattern engine should exist");

        let mut state = MultiPatternStreamState::default();
        let mut findings = Vec::new();

        engine.scan_stream_chunk(&mut state, b" gh", 0, &mut findings);

        assert!(findings.is_empty());
        assert_eq!(state.overlap, b" gh");

        engine.scan_stream_chunk(&mut state, b"p_secret!", 3, &mut findings);

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].start(), 1);
        assert_eq!(findings[0].end(), 11);
    }

    #[test]
    fn stream_prefix_crossing_boundary_respects_left_token_boundary() {
        let compiled =
            CompiledRuleSet::compile(vec![Rule::prefix("github", "ghp_", Severity::Critical)])
                .expect("rules should compile");

        let engine = compiled
            .multi_pattern
            .as_ref()
            .expect("multi-pattern engine should exist");

        let mut state = MultiPatternStreamState::default();
        let mut findings = Vec::new();

        engine.scan_stream_chunk(&mut state, b"p_secret!", 4, &mut findings);

        assert!(findings.is_empty());
    }

    #[test]
    fn streamed_multi_pattern_matches_whole_source_semantics() {
        let rules = vec![
            Rule::literal("literal", "secret", Severity::High),
            Rule::prefix("github", "ghp_", Severity::Critical),
        ];

        let compiled = CompiledRuleSet::compile(rules).expect("rules should compile");

        let source = "secret ghp_value! xxsecret ghp_other";
        let mut whole = Vec::new();
        compiled.scan(source, &mut whole);

        let engine = compiled
            .multi_pattern
            .as_ref()
            .expect("multi-pattern engine should exist");

        let chunks: &[&[u8]] = &[
            b"sec",
            b"ret gh",
            b"p_val",
            b"ue! xxse",
            b"cret ghp_",
            b"other",
        ];

        let mut state = MultiPatternStreamState::default();
        let mut streamed = Vec::new();
        let mut offset = 0;

        for chunk in chunks {
            engine.scan_stream_chunk(&mut state, chunk, offset, &mut streamed);
            offset += chunk.len();
        }

        MultiPatternEngine::finish_stream(&mut state, offset, &mut streamed);

        let project =
            |finding: &InternalFinding| (finding.start(), finding.end(), finding.rule_index());

        let mut whole = whole.iter().map(project).collect::<Vec<_>>();
        let mut streamed = streamed.iter().map(project).collect::<Vec<_>>();

        whole.sort_unstable();
        streamed.sort_unstable();

        assert_eq!(streamed, whole);
    }

    #[test]
    fn pattern_stream_plan_distinguishes_lf_and_crlf_line_anchors() {
        let cases = [
            r"(?m)^secret_[A-Za-z0-9]{8}$",
            r"(?m)^secret_[A-Za-z0-9]{8}",
            r"(?m)secret_[A-Za-z0-9]{8}$",
            r"(?mR)^secret_[A-Za-z0-9]{8}$",
            r"(?mR)^secret_[A-Za-z0-9]{8}",
            r"(?mR)secret_[A-Za-z0-9]{8}$",
            r"(?mR)^\bsecret_[A-Za-z0-9]{8}\b$",
        ];

        for pattern in cases {
            let rule = Rule::pattern("line", pattern, Severity::High)
                .expect("line-anchor pattern must compile");

            let rules = CompiledRuleSet::compile(vec![rule]).expect("rule set must compile");

            let plan = rules.patterns[0].stream_plan();

            assert_eq!(
                plan,
                PatternStreamPlan::Bounded {
                    max_match_len: 15,
                    context: PatternStreamContext {
                        history_bytes: 4,
                        lookahead_bytes: 4,
                        requires_text_start: false,
                        requires_text_end: false,
                    },
                },
                "line-anchor pattern must support bounded streaming: {pattern}"
            );
        }

        for pattern in [
            r"(?mR)(?:\Asecret|^token)",
            r"(?mR)(?:secret\z|token$)",
            r"(?m)(?:^secret|(?R:^token))",
            r"(?m)^secret(?R:$)",
            r"(?mR)^secret(?-R:$)",
            r"(?mR)^secret[A-Z]+$",
        ] {
            let rule = Rule::pattern("unsupported-line", pattern, Severity::High).unwrap();
            let rules = CompiledRuleSet::compile(vec![rule]).unwrap();
            assert_eq!(
                rules.patterns[0].stream_plan(),
                PatternStreamPlan::Unsupported,
                "mixed or unbounded line context must remain unsupported: {pattern}"
            );
        }
    }

    #[test]
    fn pattern_stream_plan_accepts_required_absolute_line_contexts() {
        for (pattern, requires_text_start, requires_text_end) in [
            (r"(?m)\Asecret_[A-Za-z0-9]{8}$", true, false),
            (r"(?mR)\Asecret_[A-Za-z0-9]{8}$", true, false),
            (r"(?m)^secret_[A-Za-z0-9]{8}\z", false, true),
            (r"(?mR)^secret_[A-Za-z0-9]{8}\z", false, true),
            (r"(?m)\A^\bsecret_[A-Za-z0-9]{8}\b$\z", true, true),
            (r"(?mR)\A^\bsecret_[A-Za-z0-9]{8}\b$\z", true, true),
        ] {
            let rule = Rule::pattern("mixed", pattern, Severity::High).unwrap();
            let rules = CompiledRuleSet::compile(vec![rule]).unwrap();
            assert_eq!(
                rules.patterns[0].stream_plan(),
                PatternStreamPlan::Bounded {
                    max_match_len: 15,
                    context: PatternStreamContext {
                        history_bytes: if requires_text_start { 0 } else { 4 },
                        lookahead_bytes: if requires_text_end { 0 } else { 4 },
                        requires_text_start,
                        requires_text_end,
                    },
                },
                "required mixed context: {pattern}"
            );
        }
        for pattern in [
            r"(?m)(?:\Asecret|token)$",
            r"(?m)^\Asecret",
            r"(?m)secret\z$",
            r"(?m)\A(?:secret\z|token$)",
            r"(?m)(?:\Asecret|^token)\z",
            r"(?m)\A^secret(?R:$)",
            r"(?mR)\A^secret(?-R:$)\z",
            r"(?mR)(?:^secret|(?-R:^token))\z",
            r"(?m)\Asecret.*$",
        ] {
            let rule = Rule::pattern("unsupported-mixed", pattern, Severity::High).unwrap();
            let rules = CompiledRuleSet::compile(vec![rule]).unwrap();
            assert_eq!(
                rules.patterns[0].stream_plan(),
                PatternStreamPlan::Unsupported,
                "conditional, mixed LF/CRLF or unbounded context: {pattern}"
            );
        }
    }

    #[test]
    fn pattern_stream_plan_accepts_bounded_context_free_pattern() {
        let rule = Rule::pattern("plain", r"secret_[A-Z]{8}", Severity::High).unwrap();
        let rules = CompiledRuleSet::compile(vec![rule]).unwrap();

        assert_eq!(
            rules.patterns[0].stream_plan(),
            PatternStreamPlan::Bounded {
                max_match_len: 15,
                context: PatternStreamContext::default(),
            }
        );
    }

    #[test]
    fn pattern_stream_plan_rejects_unbounded_pattern() {
        let rule = Rule::pattern("unbounded", r"secret_[A-Z]+", Severity::High).unwrap();
        let rules = CompiledRuleSet::compile(vec![rule]).unwrap();

        assert_eq!(
            rules.patterns[0].stream_plan(),
            PatternStreamPlan::Unsupported
        );
    }

    #[test]
    fn pattern_stream_plan_accepts_bounded_text_anchors() {
        let rule = Rule::pattern("anchored", r"\Asecret[A-Z]{4}\z", Severity::High).unwrap();
        let rules = CompiledRuleSet::compile(vec![rule]).unwrap();

        assert_eq!(
            rules.patterns[0].stream_plan(),
            PatternStreamPlan::Bounded {
                max_match_len: 10,
                context: PatternStreamContext {
                    history_bytes: 0,
                    lookahead_bytes: 0,
                    requires_text_start: true,
                    requires_text_end: true,
                },
            }
        );
    }

    #[test]
    fn pattern_stream_plan_rejects_conditional_absolute_anchors() {
        for pattern in [
            r"(?:\Asecret|token)",
            r"(?:secret\z|token)",
            r"(?:\Asecret|token\z)",
        ] {
            let rule = Rule::pattern("conditional", pattern, Severity::High).unwrap();
            let rules = CompiledRuleSet::compile(vec![rule]).unwrap();

            assert_eq!(
                rules.patterns[0].stream_plan(),
                PatternStreamPlan::Unsupported,
                "unexpected streaming support for {pattern}"
            );
        }
    }

    #[test]
    fn pattern_stream_plan_accepts_bounded_word_boundaries() {
        let rule = Rule::pattern("word", r"\bsecret[A-Z]{4}\b", Severity::High).unwrap();
        let rules = CompiledRuleSet::compile(vec![rule]).unwrap();

        assert_eq!(
            rules.patterns[0].stream_plan(),
            PatternStreamPlan::Bounded {
                max_match_len: 10,
                context: PatternStreamContext {
                    history_bytes: 4,
                    lookahead_bytes: 4,
                    requires_text_start: false,
                    requires_text_end: false,
                },
            }
        );
    }

    #[test]
    fn word_boundary_stream_retains_bounded_history() {
        let rule = Rule::pattern("word", r"\bsecret\b", Severity::High).unwrap();
        let rules = CompiledRuleSet::compile(vec![rule]).unwrap();
        let rule = &rules.patterns[0];

        let PatternStreamPlan::Bounded {
            max_match_len,
            context,
        } = rule.stream_plan()
        else {
            panic!("word-boundary pattern must support bounded streaming");
        };

        let mut state = PatternStreamState::default();
        let mut findings = Vec::new();

        let bound = max_match_len + context.history_bytes + context.lookahead_bytes;

        for offset in 0..10_000 {
            rule.scan_stream_bounded(&mut state, b"x", offset, &mut findings);

            assert!(
                state.buffered_len() <= bound,
                "retained {} bytes at offset {offset}; bound={bound}",
                state.buffered_len()
            );
        }

        assert!(findings.is_empty());
    }

    #[test]
    fn word_boundary_stream_emits_only_resolved_matches() {
        let rule = Rule::pattern("word", r"\bsecret\b", Severity::High).unwrap();
        let rules = CompiledRuleSet::compile(vec![rule]).unwrap();
        let rule = &rules.patterns[0];

        let mut state = PatternStreamState::default();
        let mut findings = Vec::new();

        rule.scan_stream_bounded(&mut state, b"secret", 0, &mut findings);

        // A subsequent word character could invalidate the right boundary.
        assert!(findings.is_empty());

        rule.scan_stream_bounded(&mut state, b"X", 6, &mut findings);

        assert!(findings.is_empty());

        rule.scan_stream_bounded(&mut state, b" secret ", 7, &mut findings);

        // The second candidate is still inside the conservative lookahead window.
        assert!(findings.is_empty());

        rule.scan_stream_bounded(&mut state, b"xxxxxxxxxxxxxxxx", 15, &mut findings);

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].start(), 8);
        assert_eq!(findings[0].end(), 14);
    }

    #[test]
    fn text_end_anchor_stream_waits_for_real_eof() {
        let rule = Rule::pattern("end", r"secret\z", Severity::High).unwrap();
        let rules = CompiledRuleSet::compile(vec![rule]).unwrap();
        let rule = &rules.patterns[0];

        let mut state = PatternStreamState::default();
        let mut findings = Vec::new();

        rule.scan_stream_bounded(&mut state, b"secret", 0, &mut findings);

        assert!(
            findings.is_empty(),
            "absolute end anchor must not match temporary buffer EOF"
        );

        rule.scan_stream_bounded(&mut state, b"X", 6, &mut findings);

        assert!(
            findings.is_empty(),
            "candidate invalidated by continuation must not be emitted"
        );

        rule.finish_stream_bounded(&mut state, &mut findings);

        assert!(findings.is_empty());
    }

    #[test]
    fn text_end_anchor_stream_emits_at_real_eof() {
        let rule = Rule::pattern("end", r"secret\z", Severity::High).unwrap();
        let rules = CompiledRuleSet::compile(vec![rule]).unwrap();
        let rule = &rules.patterns[0];

        let mut state = PatternStreamState::default();
        let mut findings = Vec::new();

        rule.scan_stream_bounded(&mut state, b"xx secret", 0, &mut findings);

        assert!(findings.is_empty());

        rule.finish_stream_bounded(&mut state, &mut findings);

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].start(), 3);
        assert_eq!(findings[0].end(), 9);
    }

    #[test]
    fn absolute_end_streaming_remains_bounded() {
        let rule = Rule::pattern("end", r"secret\z", Severity::High).unwrap();
        let rules = CompiledRuleSet::compile(vec![rule]).unwrap();
        let rule = &rules.patterns[0];

        let mut state = PatternStreamState::default();
        let mut findings = Vec::new();

        for offset in 0..10_000 {
            rule.scan_stream_bounded(&mut state, b"x", offset, &mut findings);

            assert!(
                state.buffered_len() <= 14,
                "absolute-end stream retained excessive input"
            );
        }

        assert!(findings.is_empty());

        rule.finish_stream_bounded(&mut state, &mut findings);
        assert!(findings.is_empty());
    }

    #[test]
    fn bounded_pattern_streaming_requires_commit_frontier() {
        let regex = Regex::new(r"a|ab").unwrap();

        let first = regex.find("a").unwrap();
        assert_eq!((first.start(), first.end()), (0, 1));

        let complete = regex.find("ab").unwrap();
        assert_eq!((complete.start(), complete.end()), (0, 1));
    }

    #[test]
    fn bounded_pattern_stream_waits_until_match_is_stable() {
        let rule = Rule::pattern("bounded", r"a[A-Za-z]{0,3}", Severity::High).unwrap();
        let rules = CompiledRuleSet::compile(vec![rule]).unwrap();
        let rule = &rules.patterns[0];

        let mut state = PatternStreamState::default();
        let mut findings = Vec::new();

        rule.scan_stream_bounded(&mut state, b"a", 0, &mut findings);
        assert!(findings.is_empty());

        rule.scan_stream_bounded(&mut state, b"bc", 1, &mut findings);
        assert!(findings.is_empty());

        rule.scan_stream_bounded(&mut state, b"d", 3, &mut findings);

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].start(), 0);
        assert_eq!(findings[0].end(), 4);
    }

    #[test]
    fn bounded_pattern_stream_preserves_absolute_offset() {
        let rule = Rule::pattern("bounded", r"secret_[A-Z]{4}", Severity::High).unwrap();
        let rules = CompiledRuleSet::compile(vec![rule]).unwrap();
        let rule = &rules.patterns[0];

        let mut state = PatternStreamState::default();
        let mut findings = Vec::new();

        rule.scan_stream_bounded(&mut state, b"xx secret_", 0, &mut findings);
        assert!(findings.is_empty());

        rule.scan_stream_bounded(&mut state, b"ABCD yy", 10, &mut findings);

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].start(), 3);
        assert_eq!(findings[0].end(), 14);
    }

    #[test]
    fn bounded_pattern_stream_commits_remaining_match_at_eof() {
        let rule = Rule::pattern("bounded", r"secret_[A-Z]{1,4}", Severity::High).unwrap();
        let rules = CompiledRuleSet::compile(vec![rule]).unwrap();
        let rule = &rules.patterns[0];

        let mut state = PatternStreamState::default();
        let mut findings = Vec::new();

        rule.scan_stream_bounded(&mut state, b"secret_AB", 0, &mut findings);

        assert!(findings.is_empty());

        rule.finish_stream_bounded(&mut state, &mut findings);

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].start(), 0);
        assert_eq!(findings[0].end(), 9);
    }

    #[test]
    fn bounded_pattern_stream_does_not_retain_complete_source() {
        let rule = Rule::pattern("bounded", r"secret_[A-Z]{4}", Severity::High).unwrap();
        let rules = CompiledRuleSet::compile(vec![rule]).unwrap();
        let rule = &rules.patterns[0];

        let mut state = PatternStreamState::default();
        let mut findings = Vec::new();

        for offset in 0..10_000 {
            rule.scan_stream_bounded(&mut state, b"x", offset, &mut findings);

            assert!(
                state.buffered_len() <= 11,
                "retained {} bytes at offset {offset}",
                state.buffered_len()
            );
        }
    }

    fn projected_findings(findings: &[InternalFinding]) -> Vec<(usize, usize, RuleIndex)> {
        let mut projected = findings
            .iter()
            .map(|finding| (finding.start(), finding.end(), finding.rule_index()))
            .collect::<Vec<_>>();

        projected.sort_unstable();
        projected
    }

    fn whole_pattern_findings(rule: &PatternRule, source: &str) -> Vec<(usize, usize, RuleIndex)> {
        let mut findings = Vec::new();
        rule.scan(source, &mut findings);
        projected_findings(&findings)
    }

    #[test]
    fn bounded_pattern_stream_matches_whole_source_for_every_valid_split() {
        let rule = Rule::pattern("bounded", r"secret_[A-Z]{4}", Severity::High).unwrap();
        let rules = CompiledRuleSet::compile(vec![rule]).unwrap();
        let rule = &rules.patterns[0];

        let source = "🦀 xx secret_ABCD yy secret_WXYZ 🦀";
        let expected = whole_pattern_findings(rule, source);

        for split in 0..=source.len() {
            if !source.is_char_boundary(split) {
                continue;
            }

            let mut state = PatternStreamState::default();
            let mut findings = Vec::new();

            rule.scan_stream_bounded(&mut state, &source.as_bytes()[..split], 0, &mut findings);
            rule.scan_stream_bounded(
                &mut state,
                &source.as_bytes()[split..],
                split,
                &mut findings,
            );
            rule.finish_stream_bounded(&mut state, &mut findings);

            assert_eq!(
                projected_findings(&findings),
                expected,
                "stream differs from whole-source scan at split {split}",
            );
        }
    }

    #[test]
    fn bounded_pattern_stream_matches_whole_source_for_single_byte_ascii_chunks() {
        let rule = Rule::pattern("bounded", r"secret_[A-Z]{4}", Severity::High).unwrap();
        let rules = CompiledRuleSet::compile(vec![rule]).unwrap();
        let rule = &rules.patterns[0];

        let source = "xx secret_ABCD yy secret_WXYZ zz";
        let expected = whole_pattern_findings(rule, source);

        let mut state = PatternStreamState::default();
        let mut findings = Vec::new();

        for (offset, byte) in source.as_bytes().iter().enumerate() {
            rule.scan_stream_bounded(
                &mut state,
                std::slice::from_ref(byte),
                offset,
                &mut findings,
            );
        }

        rule.finish_stream_bounded(&mut state, &mut findings);

        assert_eq!(projected_findings(&findings), expected);
    }

    #[test]
    fn bounded_captured_pattern_stream_projects_capture_span() {
        let rule = Rule::captured_pattern(
            "captured",
            r"KEY=(?P<value>[A-Z]{4})!",
            "value",
            Severity::High,
        )
        .unwrap();

        let rules = CompiledRuleSet::compile(vec![rule]).unwrap();
        let rule = &rules.patterns[0];

        assert_eq!(
            rule.stream_plan(),
            PatternStreamPlan::Bounded {
                max_match_len: 9,
                context: PatternStreamContext::default(),
            }
        );

        let source = "xx KEY=ABCD! yy";

        let expected = whole_pattern_findings(rule, source);

        let mut state = PatternStreamState::default();
        let mut findings = Vec::new();

        rule.scan_stream_bounded(&mut state, b"xx KEY=A", 0, &mut findings);
        rule.scan_stream_bounded(&mut state, b"BCD! yy", 8, &mut findings);
        rule.finish_stream_bounded(&mut state, &mut findings);

        assert_eq!(projected_findings(&findings), expected);
        assert_eq!(expected, vec![(7, 11, RuleIndex::new(0))]);
    }

    #[test]
    fn bounded_captured_pattern_stream_matches_whole_source_for_every_valid_split() {
        let rule = Rule::captured_pattern(
            "captured",
            r"KEY=(?P<value>[A-Z]{4})!",
            "value",
            Severity::High,
        )
        .unwrap();

        let rules = CompiledRuleSet::compile(vec![rule]).unwrap();
        let rule = &rules.patterns[0];

        let source = "🦀 xx KEY=ABCD! yy KEY=WXYZ! 🦀";
        let expected = whole_pattern_findings(rule, source);

        for split in 0..=source.len() {
            if !source.is_char_boundary(split) {
                continue;
            }

            let mut state = PatternStreamState::default();
            let mut findings = Vec::new();

            rule.scan_stream_bounded(&mut state, &source.as_bytes()[..split], 0, &mut findings);

            rule.scan_stream_bounded(
                &mut state,
                &source.as_bytes()[split..],
                split,
                &mut findings,
            );

            rule.finish_stream_bounded(&mut state, &mut findings);

            assert_eq!(
                projected_findings(&findings),
                expected,
                "captured stream differs from whole-source scan at split {split}",
            );
        }
    }

    #[test]
    fn bounded_captured_pattern_stream_commits_capture_at_eof() {
        let rule = Rule::captured_pattern(
            "captured",
            r"KEY=(?P<value>[A-Z]{1,4})",
            "value",
            Severity::High,
        )
        .unwrap();

        let rules = CompiledRuleSet::compile(vec![rule]).unwrap();
        let rule = &rules.patterns[0];

        let mut state = PatternStreamState::default();
        let mut findings = Vec::new();

        rule.scan_stream_bounded(&mut state, b"xx KEY=AB", 0, &mut findings);

        // `AB` is a valid match now, but may still grow to ABC/ABCD.
        assert!(findings.is_empty());

        rule.finish_stream_bounded(&mut state, &mut findings);

        assert_eq!(
            projected_findings(&findings),
            vec![(7, 9, RuleIndex::new(0))]
        );
    }

    #[test]
    fn bounded_lf_captured_pattern_preserves_projection_and_non_overlap() {
        let rule = Rule::captured_pattern(
            "lf-captured",
            r"(?m)^KEY=(?P<value>[A-Z]{2,4})!",
            "value",
            Severity::High,
        )
        .unwrap();

        let rules = CompiledRuleSet::compile(vec![rule]).unwrap();
        let rule = &rules.patterns[0];

        assert!(
            matches!(rule.stream_plan(), PatternStreamPlan::Bounded { .. }),
            "bounded LF captured pattern must support streaming"
        );

        let sources = [
            "KEY=AB!",
            "KEY=ABCD!",
            "KEY=ABCD!\nKEY=XY!",
            "xKEY=ABCD!",
            "x\nKEY=ABCD!",
            "KEY=ABCD!\r\nKEY=XY!",
            "🦀\nKEY=ABCD!\nKEY=XY!",
            "KEY=ABCD!KEY=XY!",
            "KEY=ABCD!\nKEY=XYZ!\nKEY=AB!",
        ];

        for source in sources {
            let expected = whole_pattern_findings(rule, source);

            for split in 0..=source.len() {
                if !source.is_char_boundary(split) {
                    continue;
                }

                let mut state = PatternStreamState::default();
                let mut findings = Vec::new();

                rule.scan_stream_bounded(&mut state, &source.as_bytes()[..split], 0, &mut findings);

                rule.scan_stream_bounded(
                    &mut state,
                    &source.as_bytes()[split..],
                    split,
                    &mut findings,
                );

                rule.finish_stream_bounded(&mut state, &mut findings);

                assert_eq!(
                    projected_findings(&findings),
                    expected,
                    "LF capture projection differs: source={source:?}, split={split}",
                );
            }
        }
    }

    #[test]
    fn bounded_lf_stream_retention_does_not_grow_with_source() {
        let rule = Rule::pattern("lf-memory", r"(?m)^secret[A-Z]{2}$", Severity::High).unwrap();

        let rules = CompiledRuleSet::compile(vec![rule]).unwrap();
        let rule = &rules.patterns[0];

        let PatternStreamPlan::Bounded {
            max_match_len,
            context,
        } = rule.stream_plan()
        else {
            panic!("bounded LF pattern must support streaming");
        };

        let retention_limit = max_match_len
            .saturating_add(context.history_bytes)
            .saturating_add(context.lookahead_bytes)
            .saturating_add(4);

        let mut source = String::new();

        for index in 0..4096 {
            if index % 3 == 0 {
                source.push_str("secretAB\n");
            } else if index % 3 == 1 {
                source.push_str("xsecretAB\r\n");
            } else {
                source.push_str("🦀secretCD\n");
            }
        }

        source.push_str("secretXY");

        let expected = whole_pattern_findings(rule, &source);

        let mut state = PatternStreamState::default();
        let mut findings = Vec::new();

        for (offset, byte) in source.as_bytes().iter().enumerate() {
            rule.scan_stream_bounded(
                &mut state,
                std::slice::from_ref(byte),
                offset,
                &mut findings,
            );

            let retained = state.buffer.len();

            assert!(
                retained <= retention_limit,
                "LF streaming retention exceeded bound: \
                 retained={retained}, limit={retention_limit}, offset={offset}",
            );
        }

        rule.finish_stream_bounded(&mut state, &mut findings);

        assert_eq!(
            projected_findings(&findings),
            expected,
            "LF streaming findings differ from whole-source scan"
        );
    }

    #[test]
    fn bounded_crlf_captures_preserve_complete_match_selection() {
        for pattern in [
            r"(?mR)^KEY=(?P<value>[A-Z]{2,4})!$",
            r"(?mR)(?:^KEY=(?P<value>[A-Z]{2})!|[A-Z]{2}!$)",
            r"(?mR)(?:^KEY=[A-Z]{2}!|(?P<value>[A-Z]{2})!$)",
        ] {
            let rule =
                Rule::captured_pattern("crlf-capture", pattern, "value", Severity::High).unwrap();
            let rules = CompiledRuleSet::compile(vec![rule]).unwrap();
            let rule = &rules.patterns[0];
            assert!(matches!(
                rule.stream_plan(),
                PatternStreamPlan::Bounded { .. }
            ));

            for source in [
                "KEY=AB!",
                "KEY=ABCD!\r\nKEY=XY!",
                "KEY=AB!KEY=XY!",
                "KEY=AB!\rKEY=XY!\nKEY=CD!",
                "🦀\r\nKEY=AB!\r\nXY!",
            ] {
                let expected = whole_pattern_findings(rule, source);
                for split in 0..=source.len() {
                    if !source.is_char_boundary(split) {
                        continue;
                    }
                    let mut state = PatternStreamState::default();
                    let mut findings = Vec::new();
                    rule.scan_stream_bounded(
                        &mut state,
                        &source.as_bytes()[..split],
                        0,
                        &mut findings,
                    );
                    rule.scan_stream_bounded(
                        &mut state,
                        &source.as_bytes()[split..],
                        split,
                        &mut findings,
                    );
                    rule.finish_stream_bounded(&mut state, &mut findings);
                    assert_eq!(
                        projected_findings(&findings),
                        expected,
                        "CRLF capture parity: pattern={pattern}, source={source:?}, split={split}"
                    );
                }

                let source = source.repeat(64);
                let expected = whole_pattern_findings(rule, &source);
                let mut state = PatternStreamState::default();
                let mut findings = Vec::new();
                for (offset, byte) in source.as_bytes().iter().enumerate() {
                    rule.scan_stream_bounded(
                        &mut state,
                        std::slice::from_ref(byte),
                        offset,
                        &mut findings,
                    );
                }
                rule.finish_stream_bounded(&mut state, &mut findings);
                assert_eq!(
                    projected_findings(&findings),
                    expected,
                    "CRLF capture parity after repeated trimming: pattern={pattern}"
                );
            }
        }
    }

    #[test]
    fn bounded_crlf_stream_retention_does_not_grow_with_source() {
        let rule =
            Rule::pattern("crlf-memory", r"(?mR)^\bsecret[A-Z]{2}\b$", Severity::High).unwrap();
        let rules = CompiledRuleSet::compile(vec![rule]).unwrap();
        let rule = &rules.patterns[0];
        let PatternStreamPlan::Bounded {
            max_match_len,
            context,
        } = rule.stream_plan()
        else {
            panic!("CRLF pattern must support bounded streaming");
        };
        let retention_limit = max_match_len + context.history_bytes + context.lookahead_bytes + 4;
        let source = "secretAB\r\nxsecretCD\rsecretEF\n🦀\r\nsecretGH\r".repeat(1024) + "secretXY";
        let expected = whole_pattern_findings(rule, &source);
        let mut state = PatternStreamState::default();
        let mut findings = Vec::new();
        for (offset, byte) in source.as_bytes().iter().enumerate() {
            rule.scan_stream_bounded(
                &mut state,
                std::slice::from_ref(byte),
                offset,
                &mut findings,
            );
            assert!(
                state.buffered_len() <= retention_limit,
                "CRLF retention exceeded {retention_limit} at {offset}: {}",
                state.buffered_len()
            );
        }
        assert!(state.buffer_offset > 0, "test must exercise trimming");
        rule.finish_stream_bounded(&mut state, &mut findings);
        assert_eq!(projected_findings(&findings), expected);
        assert!(state.buffer.is_empty(), "EOF must release retained input");
    }

    #[test]
    fn mixed_required_absolute_line_retention_is_bounded() {
        let source = "secretAB\r\nxsecretCD\n🦀\rsecretEF\n".repeat(1024) + "secretXY";
        for pattern in [
            r"(?m)\A\bsecret[A-Z]{2}\b$",
            r"(?mR)\A\bsecret[A-Z]{2}\b$",
            r"(?m)^\bsecret[A-Z]{2}\b\z",
            r"(?mR)^\bsecret[A-Z]{2}\b\z",
            r"(?m)\A^secret[A-Z]{2}$\z",
            r"(?mR)\A^secret[A-Z]{2}$\z",
        ] {
            let rules = CompiledRuleSet::compile(vec![
                Rule::pattern("mixed-memory", pattern, Severity::High).unwrap(),
            ])
            .unwrap();
            let rule = &rules.patterns[0];
            let PatternStreamPlan::Bounded {
                max_match_len,
                context,
            } = rule.stream_plan()
            else {
                panic!("required mixed context must be bounded: {pattern}");
            };
            let limit = max_match_len + context.history_bytes + context.lookahead_bytes + 4;
            let expected = whole_pattern_findings(rule, &source);
            let mut state = PatternStreamState::default();
            let mut findings = Vec::new();
            // The executor receives validated UTF-8; SourceSession separately
            // verifies byte partitions inside code points.
            for (offset, ch) in source.char_indices() {
                rule.scan_stream_bounded(
                    &mut state,
                    &source.as_bytes()[offset..offset + ch.len_utf8()],
                    offset,
                    &mut findings,
                );
                assert!(
                    state.buffered_len() <= limit,
                    "mixed retention: pattern={pattern}, offset={offset}, retained={}, limit={limit}",
                    state.buffered_len()
                );
                if context.requires_text_end {
                    assert!(
                        findings.is_empty(),
                        "absolute-end matching must defer until EOF"
                    );
                }
            }
            if !context.requires_text_start {
                assert!(
                    state.buffer_offset > max_match_len,
                    "test must repeatedly trim"
                );
            } else {
                assert!(
                    state.buffer.is_empty(),
                    "resolved start candidate must release input"
                );
            }
            rule.finish_stream_bounded(&mut state, &mut findings);
            assert_eq!(
                projected_findings(&findings),
                expected,
                "mixed memory parity: {pattern}"
            );
            assert!(state.buffer.is_empty());
        }
    }

    #[test]
    fn bounded_regex_internal_windows_preserve_large_chunk_parity() {
        let source =
            "abcabc KEY=AB! secretAB\r\nsecretCD\nsecretEF\rsecret🦀 𐐀secret ésecret secret𐐀\n"
                .repeat(1024)
                + "secretXY";
        for (pattern, capture) in [
            (r"secret[A-Z]{0,2}", None),
            (r"(?:abc|abcabc)", None),
            (r"(?:abcabc|abc)", None),
            (r"KEY=(?P<value>[A-Z]{2})!", Some("value")),
            (r"(?:KEY=AB!|(?P<value>AB)!)", Some("value")),
            (r"\bsecret[A-Z]{0,2}\b", None),
            (r"(?m)^secret[A-Z]{2}$", None),
            (r"(?mR)^secret[A-Z]{2}$", None),
            (r"(?mR)(?:^secret|secret[A-Z]{2}$)", None),
            (r"(?mR)^secret(?P<value>[A-Z]{2})$", Some("value")),
            (r"\A(?:abcabc|abc)", None),
            (r"\A(?P<value>abc)abc", Some("value")),
            (r"secret[A-Z]{2}\z", None),
            (r"secret(?P<value>[A-Z]{2})\z", Some("value")),
            (r"(?m)\Aabcabc$", None),
            (r"(?mR)\Aabcabc\b", None),
            (r"(?m)^secret[A-Z]{2}\z", None),
            (r"(?mR)^secret[A-Z]{2}\z", None),
            (r"(?mR)\A^abcabc$\z", None),
        ] {
            let rule = match capture {
                Some(capture) => {
                    Rule::captured_pattern("windows", pattern, capture, Severity::High)
                }
                None => Rule::pattern("windows", pattern, Severity::High),
            }
            .unwrap();
            let rules = CompiledRuleSet::compile(vec![rule]).unwrap();
            let rule = &rules.patterns[0];
            let PatternStreamPlan::Bounded {
                max_match_len,
                context,
            } = rule.stream_plan()
            else {
                panic!("test pattern must be bounded: {pattern}");
            };
            let window = max_match_len + context.history_bytes + context.lookahead_bytes + 4;
            let capacity_limit = if context.requires_text_start && context.requires_text_end {
                max_match_len
            } else {
                2 * window + 4
            };
            let project = |findings: &[InternalFinding]| {
                findings
                    .iter()
                    .map(|finding| (finding.start(), finding.end(), finding.rule_index()))
                    .collect::<Vec<_>>()
            };
            let mut whole = Vec::new();
            rule.scan(&source, &mut whole);
            let expected = project(&whole);
            let mut peak = 0;
            for chunk_size in [source.len(), source.len() / 2 + 1, 4093, 17, 1] {
                let mut state = PatternStreamState::default();
                let mut findings = Vec::new();
                let mut transport = crate::utf8_transport::Utf8Transport::default();
                let mut accepted_bytes = 0;
                for chunk in source.as_bytes().chunks(chunk_size) {
                    transport
                        .push(chunk, |text| {
                            rule.scan_stream_bounded(
                                &mut state,
                                text.as_bytes(),
                                accepted_bytes,
                                &mut findings,
                            );
                            accepted_bytes += text.len();
                        })
                        .unwrap();
                    assert!(
                        state.peak_buffer_capacity <= capacity_limit,
                        "peak capacity: pattern={pattern}, chunk_size={chunk_size}, peak={}, limit={capacity_limit}",
                        state.peak_buffer_capacity
                    );
                }
                transport.finish().unwrap();
                rule.finish_stream_bounded(&mut state, &mut findings);
                assert_eq!(
                    project(&findings),
                    expected,
                    "large chunk parity: pattern={pattern}, chunk_size={chunk_size}"
                );
                assert!(state.buffer.capacity() <= capacity_limit);
                peak = peak.max(state.peak_buffer_capacity);
            }
            println!(
                "bounded windows: pattern={pattern:?}, input_bytes={}, window={window}, peak_capacity={peak}, capacity_limit={capacity_limit}",
                source.len()
            );
        }
    }

    #[test]
    fn bounded_regex_peak_capacity_is_independent_of_input_size() {
        let rules = CompiledRuleSet::compile(vec![
            Rule::pattern("memory", r"\bsecret\b", Severity::High).unwrap(),
        ])
        .unwrap();
        let rule = &rules.patterns[0];
        let mut expected_capacity = None;
        for input_bytes in [64 * 1024, 1024 * 1024, 4 * 1024 * 1024] {
            let source = "x".repeat(input_bytes);
            let mut state = PatternStreamState::default();
            let mut findings = Vec::new();
            rule.scan_stream_bounded(&mut state, source.as_bytes(), 0, &mut findings);
            rule.finish_stream_bounded(&mut state, &mut findings);
            assert!(findings.is_empty());
            // M=6, H=4, A=4 => W=18, capacity limit=40.
            assert!(state.peak_buffer_capacity <= 40);
            assert_eq!(state.buffer.capacity(), state.peak_buffer_capacity);
            if let Some(expected) = expected_capacity {
                assert_eq!(state.peak_buffer_capacity, expected);
            }
            expected_capacity = Some(state.peak_buffer_capacity);
            println!(
                "input_bytes={input_bytes}, peak_capacity={}, capacity_after_eof={}",
                state.peak_buffer_capacity,
                state.buffer.capacity()
            );
        }
    }

    #[test]
    fn bounded_regex_rejects_overlong_whole_source_before_copying() {
        for pattern in [r"\Asecret\z", r"(?mR)\A^secret$\z"] {
            let rules = CompiledRuleSet::compile(vec![
                Rule::pattern("whole", pattern, Severity::High).unwrap(),
            ])
            .unwrap();
            let rule = &rules.patterns[0];
            let source = "secret".to_owned() + &"x".repeat(1024 * 1024);
            for split in [0, 3, 6] {
                let mut state = PatternStreamState::default();
                let mut findings = Vec::new();
                rule.scan_stream_bounded(&mut state, &source.as_bytes()[..split], 0, &mut findings);
                let capacity_before_rejection = state.buffer.capacity();
                rule.scan_stream_bounded(
                    &mut state,
                    &source.as_bytes()[split..],
                    split,
                    &mut findings,
                );
                assert_eq!(state.buffer.capacity(), capacity_before_rejection);
                assert_eq!(state.peak_buffer_capacity, capacity_before_rejection);
                assert_eq!(state.resolved_before, 1);
                rule.finish_stream_bounded(&mut state, &mut findings);
                assert_eq!(
                    projected_findings(&findings),
                    whole_pattern_findings(rule, &source)
                );
                assert!(findings.is_empty());
                if split == 0 {
                    assert_eq!(state.peak_buffer_capacity, 0);
                }
            }
        }
    }

    #[test]
    fn bounded_regex_internal_windows_preserve_eof_completion() {
        for pattern in [
            r"secret[A-Z]{1,4}",
            r"\bsecret[A-Z]{1,4}\b",
            r"(?mR)^secret[A-Z]{1,4}$",
            r"secret[A-Z]{1,4}\z",
            r"(?mR)^secret[A-Z]{1,4}\z",
        ] {
            let rules = CompiledRuleSet::compile(vec![
                Rule::pattern("eof-window", pattern, Severity::High).unwrap(),
            ])
            .unwrap();
            let rule = &rules.patterns[0];
            let source = "🦀x\r\n".repeat(32768) + "secretAB";
            let expected = whole_pattern_findings(rule, &source);
            assert_eq!(expected.len(), 1);
            for split in [0, source.len() - 2, source.len() - 8] {
                let mut state = PatternStreamState::default();
                let mut findings = Vec::new();
                rule.scan_stream_bounded(&mut state, &source.as_bytes()[..split], 0, &mut findings);
                rule.scan_stream_bounded(
                    &mut state,
                    &source.as_bytes()[split..],
                    split,
                    &mut findings,
                );
                assert!(
                    findings.is_empty(),
                    "incomplete decision must wait for EOF: {pattern}"
                );
                rule.finish_stream_bounded(&mut state, &mut findings);
                assert_eq!(projected_findings(&findings), expected);
            }
        }
        for pattern in [r"\Asecret\z", r"(?mR)\A^secret$\z"] {
            let rules = CompiledRuleSet::compile(vec![
                Rule::pattern("whole-eof", pattern, Severity::High).unwrap(),
            ])
            .unwrap();
            let rule = &rules.patterns[0];
            for split in 0..=6 {
                let mut state = PatternStreamState::default();
                let mut findings = Vec::new();
                rule.scan_stream_bounded(&mut state, &b"secret"[..split], 0, &mut findings);
                rule.scan_stream_bounded(&mut state, &b"secret"[split..], split, &mut findings);
                assert!(findings.is_empty());
                rule.finish_stream_bounded(&mut state, &mut findings);
                assert_eq!(
                    projected_findings(&findings),
                    whole_pattern_findings(rule, "secret")
                );
                assert!(state.peak_buffer_capacity <= 6);
            }
        }
    }
}
