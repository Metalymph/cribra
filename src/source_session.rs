use std::sync::Arc;

use crate::{
    compiled_rule::{CompiledRuleSet, InternalFinding, MultiPatternStreamState, SuffixStreamState},
    utf8_transport::{Utf8Transport, Utf8TransportError},
};

/// Mutable execution state owned by one logical source.
///
/// Streaming execution state belongs here rather than in the shared compiled
/// scanner configuration. Later streaming slices extend this boundary with the
/// concrete matcher, validator, candidate, normalization, and location state
/// they require.
#[derive(Debug, Default)]
struct SourceExecutionState {
    accepted_bytes: usize,
    transport: Utf8Transport,
    multi_pattern: MultiPatternStreamState,
    suffix: SuffixStreamState,
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
}

/// Lifecycle of one logical-source execution.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
enum SourceLifecycle {
    Active,
    Completed,
}

/// Invalid mutation of a source session that is no longer active.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub(crate) enum SourceSessionError {
    NotActive,
    InvalidUtf8(Utf8TransportError),
}

impl From<Utf8TransportError> for SourceSessionError {
    fn from(error: Utf8TransportError) -> Self {
        Self::InvalidUtf8(error)
    }
}

impl SourceSession {
    pub(crate) fn new(rules: Arc<CompiledRuleSet>) -> Self {
        Self {
            rules,
            lifecycle: SourceLifecycle::Active,
            state: SourceExecutionState::default(),
        }
    }

    pub(crate) fn accepted_bytes(&self) -> usize {
        self.state.accepted_bytes
    }

    pub(crate) fn scan_chunk(
        &mut self,
        chunk: &[u8],
        findings: &mut Vec<InternalFinding>,
    ) -> Result<(), SourceSessionError> {
        self.ensure_active()?;

        let rules = &self.rules;
        let state = &mut self.state;

        state.transport.push(chunk, |text| {
            let bytes = text.as_bytes();
            let source_offset = state.accepted_bytes;

            rules.scan_multi_pattern_stream_chunk(
                &mut state.multi_pattern,
                bytes,
                source_offset,
                findings,
            );

            rules.scan_suffix_stream_chunk(&mut state.suffix, bytes, source_offset, findings);

            state.accepted_bytes += bytes.len();
        })?;

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
        self.ensure_active()?;

        self.state.transport.finish()?;

        self.rules.finish_multi_pattern_stream(
            &mut self.state.multi_pattern,
            self.state.accepted_bytes,
            findings,
        );

        self.rules
            .finish_suffix_stream(&mut self.state.suffix, findings);

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Rule, Scanner, Severity};

    #[test]
    fn sessions_share_immutable_compiled_configuration() {
        let scanner = Scanner::default();

        let first = scanner.source_session();
        let second = scanner.source_session();

        assert!(first.shares_rules_with(&second));
    }

    #[test]
    fn session_scans_literal_across_chunk_boundary() {
        let rules = Arc::new(
            CompiledRuleSet::compile(vec![Rule::literal("secret", "secret", Severity::High)])
                .unwrap(),
        );

        let mut session = SourceSession::new(rules);
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

        let mut session = SourceSession::new(rules);
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

        let mut first = SourceSession::new(Arc::clone(&rules));
        let mut second = SourceSession::new(rules);

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
            let mut session = SourceSession::new(Arc::clone(&rules));
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
        let mut session = SourceSession::new(rules);
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
            CompiledRuleSet::compile(vec![Rule::prefix("github", "ghp_", Severity::Critical)])
                .unwrap(),
        );

        let mut session = SourceSession::new(rules);
        let mut findings = Vec::new();

        session.scan_chunk(b"ghp_secret", &mut findings).unwrap();
        session
            .scan_chunk(&[0xF0, 0x9F, 0xA6], &mut findings)
            .unwrap();

        assert_eq!(
            session.finish_scan(&mut findings),
            Err(SourceSessionError::InvalidUtf8(
                Utf8TransportError::Incomplete
            ))
        );

        assert!(findings.is_empty());
        assert_eq!(session.lifecycle(), SourceLifecycle::Active);
    }

    #[test]
    fn session_scans_suffix_across_chunk_boundary() {
        let rules = Arc::new(
            CompiledRuleSet::compile(vec![Rule::suffix("suffix", "_end", Severity::High)]).unwrap(),
        );

        let mut session = SourceSession::new(rules);
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

        let mut session = SourceSession::new(rules);
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

        let mut session = SourceSession::new(rules);
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

        let mut session = SourceSession::new(rules);
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

        let mut session = SourceSession::new(Arc::clone(&rules));
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

            let mut session = SourceSession::new(Arc::clone(&rules));
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
            let mut session = SourceSession::new(Arc::clone(&rules));
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
        let mut session = SourceSession::new(rules);
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
}
