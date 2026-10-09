use std::sync::Arc;

use crate::{
    compiled_rule::{
        CompiledRuleSet, InternalFinding, MultiPatternStreamState, PatternStreamState,
        SuffixStreamState,
    },
    utf8_transport::{Utf8Transport, Utf8TransportError},
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
    UnsupportedRuleSet,
    NotActive,
    InvalidUtf8(Utf8TransportError),
}

impl From<Utf8TransportError> for SourceSessionError {
    fn from(error: Utf8TransportError) -> Self {
        Self::InvalidUtf8(error)
    }
}

impl SourceSession {
    pub(crate) fn new(rules: Arc<CompiledRuleSet>) -> Result<Self, SourceSessionError> {
        if !rules.supports_streaming() {
            return Err(SourceSessionError::UnsupportedRuleSet);
        }

        let patterns = rules.new_pattern_stream_states();

        Ok(Self {
            rules,
            lifecycle: SourceLifecycle::Active,
            state: SourceExecutionState {
                accepted_bytes: 0,
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

            rules.scan_pattern_stream_chunk(&mut state.patterns, bytes, source_offset, findings);

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

        self.rules
            .finish_pattern_stream(&mut self.state.patterns, findings);

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
            CompiledRuleSet::compile(vec![Rule::prefix("github", "ghp_", Severity::Critical)])
                .unwrap(),
        );

        let mut session = SourceSession::new(rules).expect("rule set should support streaming");
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
}
