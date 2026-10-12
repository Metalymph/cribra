//! Synchronous byte-oriented streaming with explicit admission and EOF completion.

use std::{fmt, sync::Arc};

use crate::{
    ScanReport, SensitiveCandidate,
    candidate_detection::StreamingCandidateDetector,
    compiled_rule::CompiledRuleSet,
    scanner::suppress_overlapping_candidates,
    source_coordinates::SourceCoordinates,
    source_session::{
        AcceptedOutput, AnnotatedCandidate, AnnotatedSourceSession, SourceLimits,
        SourceSessionError, ValidatedCandidate,
    },
    utf8_transport::Utf8TransportError,
};

/// Optional limits for one source; the default leaves all three unlimited.
///
/// Storage admission limits logical executor payload, not total heap memory.
/// It excludes Vec spare capacity, allocator overhead, compiled automata,
/// regex/validator workspace and output records. Output records have their own
/// limit and belong to the session until successful report transfer.
#[derive(Debug, Default, Copy, Clone, Eq, PartialEq)]
pub struct StreamLimits {
    max_candidates: Option<usize>,
    max_stream_storage_bytes: Option<usize>,
    max_retained_results: Option<usize>,
}

impl StreamLimits {
    /// Limits cumulative raw rule candidates, including validator rejections.
    /// Structural review candidates are counted by the result limit instead.
    #[must_use]
    pub const fn with_max_candidates(mut self, limit: Option<usize>) -> Self {
        self.max_candidates = limit;
        self
    }

    /// Limits checked static logical executor storage before execution allocation.
    #[must_use]
    pub const fn with_max_stream_storage_bytes(mut self, limit: Option<usize>) -> Self {
        self.max_stream_storage_bytes = limit;
        self
    }

    /// Limits accepted finding records plus structural review records, before
    /// normalization or suppression. Each retained record consumes one slot;
    /// finalization never refunds slots or reinterprets this as final report size.
    #[must_use]
    pub const fn with_max_retained_results(mut self, limit: Option<usize>) -> Self {
        self.max_retained_results = limit;
        self
    }
}

/// Admission or terminal execution failure. Errors contain no source text.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[non_exhaustive]
pub enum StreamError {
    /// Matcher or semantic configuration cannot be streamed exactly.
    UnsupportedRuleSet,
    /// An earlier failure made the session terminal.
    NotActive,
    /// Source bytes contain malformed UTF-8.
    InvalidUtf8,
    /// EOF occurred inside an incomplete UTF-8 scalar.
    IncompleteUtf8,
    /// A raw rule candidate exceeded the cumulative limit.
    CandidateLimitExceeded {
        /// Configured candidate limit.
        limit: usize,
    },
    /// A result record would exceed the retained result limit.
    ResultLimitExceeded {
        /// Configured result limit.
        limit: usize,
    },
    /// The checked logical storage requirement exceeds admission allowance.
    StorageLimitExceeded {
        /// Required logical bytes.
        required: usize,
        /// Configured logical bytes.
        limit: usize,
    },
    /// Static storage arithmetic could not be represented.
    StorageRequirementOverflow,
    /// The absolute source byte offset could not be represented.
    SourceOffsetOverflow,
    /// A source line or Unicode scalar column could not be represented.
    CoordinateOverflow,
    /// An internal candidate/context/coordinate invariant failed.
    ExecutionInvariantViolated,
}

impl fmt::Display for StreamError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "streaming scan failed: {self:?}")
    }
}

impl std::error::Error for StreamError {}

impl StreamLimits {
    fn error(self, error: SourceSessionError) -> StreamError {
        match error {
            SourceSessionError::UnsupportedRuleSet => StreamError::UnsupportedRuleSet,
            SourceSessionError::NotActive => StreamError::NotActive,
            SourceSessionError::InvalidUtf8(Utf8TransportError::Malformed) => {
                StreamError::InvalidUtf8
            }
            SourceSessionError::InvalidUtf8(Utf8TransportError::Incomplete) => {
                StreamError::IncompleteUtf8
            }
            SourceSessionError::ResourceLimitExceeded => StreamError::CandidateLimitExceeded {
                limit: self.max_candidates.unwrap_or(usize::MAX),
            },
            SourceSessionError::RetainedResultLimitExceeded => StreamError::ResultLimitExceeded {
                limit: self.max_retained_results.unwrap_or(usize::MAX),
            },
            SourceSessionError::StreamStorageLimitExceeded { required, limit } => {
                StreamError::StorageLimitExceeded { required, limit }
            }
            SourceSessionError::StreamStorageRequirementOverflow => {
                StreamError::StorageRequirementOverflow
            }
            SourceSessionError::SourceOffsetOverflow => StreamError::SourceOffsetOverflow,
            SourceSessionError::CoordinateOverflow(_) => StreamError::CoordinateOverflow,
            _ => StreamError::ExecutionInvariantViolated,
        }
    }
}

struct Output {
    accepted: Vec<AnnotatedCandidate>,
    reviews: Vec<SensitiveCandidate>,
    detector: StreamingCandidateDetector,
    retained: usize,
    limit: Option<usize>,
}

fn retain_result(count: &mut usize, limit: Option<usize>) -> Result<(), SourceSessionError> {
    if limit.is_some_and(|limit| *count >= limit) {
        return Err(SourceSessionError::RetainedResultLimitExceeded);
    }
    *count = count
        .checked_add(1)
        .ok_or(SourceSessionError::RetainedResultLimitExceeded)?;
    Ok(())
}

impl AcceptedOutput<SourceCoordinates> for Output {
    fn accept(
        &mut self,
        candidate: ValidatedCandidate,
        coordinates: SourceCoordinates,
    ) -> Result<(), SourceSessionError> {
        retain_result(&mut self.retained, self.limit)?;
        self.accepted.push(AnnotatedCandidate {
            finding: candidate.finding,
            confidence: candidate.confidence,
            line: coordinates.line(),
            column: coordinates.column(),
        });
        Ok(())
    }

    fn callback_completed(&mut self, text: &str, base: usize) -> Result<(), SourceSessionError> {
        let Self {
            reviews,
            detector,
            retained,
            limit,
            ..
        } = self;
        detector.push(text, base, &mut |candidate| {
            retain_result(retained, *limit)?;
            reviews.push(candidate);
            Ok(())
        })
    }
}

struct Execution {
    session: AnnotatedSourceSession,
    output: Output,
}

/// Opaque execution for one byte-partitioned UTF-8 source.
///
/// Only [`Self::finish`] returns a report, after successful genuine EOF.
/// Failure discards working output and retained source state; subsequent calls
/// return [`StreamError::NotActive`]. Start a new session for a new source.
/// No caller input borrow survives [`Self::push`]. Debug output is redacted.
///
/// ```
/// use cribra::{Rule, Scanner, Severity};
/// let scanner = Scanner::builder()
///     .rule(Rule::literal("credential", "SECRET", Severity::High))
///     .build()?;
/// let mut session = scanner.stream()?;
/// session.push(b"TOKEN=SEC")?;
/// session.push(b"RET")?;
/// let report = session.finish()?;
/// assert_eq!(report.len(), 1);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// Finalization consumes the session:
/// ```compile_fail
/// # let scanner = cribra::Scanner::builder().build().unwrap();
/// let session = scanner.stream().unwrap();
/// let report = session.finish().unwrap();
/// let second = session.finish();
/// ```
pub struct StreamSession {
    execution: Option<Execution>,
    limits: StreamLimits,
}

impl fmt::Debug for StreamSession {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StreamSession")
            .field("active", &self.execution.is_some())
            .finish_non_exhaustive()
    }
}

impl StreamSession {
    pub(crate) fn new(
        rules: Arc<CompiledRuleSet>,
        limits: StreamLimits,
    ) -> Result<Self, StreamError> {
        // This calculation and admission happen before execution/output allocation.
        let required = AnnotatedSourceSession::required_storage(&rules)
            .and_then(|value| {
                value
                    .checked_add(std::mem::size_of::<StreamingCandidateDetector>())
                    .and_then(|value| value.checked_add(3)) // UTF-8 transport carry
                    .ok_or(SourceSessionError::StreamStorageRequirementOverflow)
            })
            .map_err(|error| limits.error(error))?;
        if let Some(limit) = limits.max_stream_storage_bytes
            && required > limit
        {
            return Err(StreamError::StorageLimitExceeded { required, limit });
        }
        let session = AnnotatedSourceSession::new_with_limits(
            rules,
            SourceLimits {
                max_candidates: limits.max_candidates,
                max_stream_storage_bytes: None,
            },
        )
        .map_err(|error| limits.error(error))?;
        Ok(Self {
            execution: Some(Execution {
                session,
                output: Output {
                    accepted: Vec::new(),
                    reviews: Vec::new(),
                    detector: StreamingCandidateDetector::default(),
                    retained: 0,
                    limit: limits.max_retained_results,
                },
            }),
            limits,
        })
    }

    /// Feeds bytes in source order. Empty fragments are no-ops. Partial UTF-8
    /// scalars may span calls; definite malformed UTF-8 is terminal.
    /// No partial report is returned after any failure.
    pub fn push(&mut self, bytes: &[u8]) -> Result<(), StreamError> {
        let execution = self.execution.as_mut().ok_or(StreamError::NotActive)?;
        if let Err(error) = execution.session.scan_chunk(bytes, &mut execution.output) {
            self.execution = None;
            return Err(self.limits.error(error));
        }
        Ok(())
    }

    /// Consumes this session, validates genuine EOF, and returns the complete
    /// report in whole-source Scanner order. Incomplete UTF-8 and all execution
    /// errors return no report. Normalization/suppression do not refund limits.
    pub fn finish(mut self) -> Result<ScanReport, StreamError> {
        let mut execution = self.execution.take().ok_or(StreamError::NotActive)?;
        execution
            .session
            .finish_scan(&mut execution.output)
            .map_err(|error| self.limits.error(error))?;
        if let Some(candidate) = execution.output.detector.finish() {
            retain_result(&mut execution.output.retained, execution.output.limit)
                .map_err(|error| self.limits.error(error))?;
            execution.output.reviews.push(candidate);
        }
        let mut findings = Vec::new();
        execution
            .session
            .finalize_findings(&mut execution.output.accepted, &mut findings)
            .map_err(|error| self.limits.error(error))?;
        suppress_overlapping_candidates(&findings, &mut execution.output.reviews);
        Ok(ScanReport::new_with_candidates(
            findings,
            execution.output.reviews,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Rule, Scanner, Severity, validators::dispatch::ValidatorKind};

    fn build_scanner(rules: Vec<Rule>) -> Scanner {
        rules
            .into_iter()
            .fold(Scanner::builder(), |builder, rule| builder.rule(rule))
            .build()
            .unwrap()
    }

    fn parity(scanner: &Scanner, source: &str) {
        let expected = scanner
            .scan([((), source)])
            .single_report()
            .unwrap()
            .clone();
        for split in 0..=source.len() {
            let mut session = scanner.stream().unwrap();
            session.push(&source.as_bytes()[..split]).unwrap();
            session.push(b"").unwrap();
            session.push(&source.as_bytes()[split..]).unwrap();
            let report = session.finish().unwrap();
            assert_eq!(report, expected, "split {split}");
            assert_eq!(report.to_string(), expected.to_string());
            #[cfg(feature = "serde")]
            assert_eq!(
                serde_json::to_value(report).unwrap(),
                serde_json::to_value(&expected).unwrap()
            );
        }
        for size in [1, 2, 3, 7, 31, source.len().max(1)] {
            let mut session = scanner.stream().unwrap();
            for chunk in source.as_bytes().chunks(size) {
                session.push(chunk).unwrap();
            }
            assert_eq!(session.finish().unwrap(), expected, "chunk {size}");
        }
        let mut session = scanner.stream().unwrap();
        let mut base = 0;
        let mut seed = 42usize;
        while base < source.len() {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            let end = (base + 1 + seed % 23).min(source.len());
            session.push(&source.as_bytes()[base..end]).unwrap();
            base = end;
        }
        assert_eq!(session.finish().unwrap(), expected);
    }

    #[test]
    fn admitted_unicode_and_delimited_prefix_needles_preserve_report_parity() {
        for needle in ["é", "é!", "!", "!!", "A!", "A A", "🦀_"] {
            let scanner = build_scanner(vec![Rule::prefix("prefix", needle, Severity::High)]);
            parity(
                &scanner,
                &format!("{needle}abc! {needle}{needle}xyz é{needle}end"),
            );
        }
    }

    #[test]
    fn all_44_validator_dispatch_kinds_preserve_full_report_decisions() {
        let kinds = [
            ValidatorKind::None,
            ValidatorKind::DockerRegistry,
            ValidatorKind::NpmRegistry,
            ValidatorKind::CargoRegistry,
            ValidatorKind::Pypi,
            ValidatorKind::Maven,
            ValidatorKind::ComposerHttpBasicPassword,
            ValidatorKind::ComposerBearerToken,
            ValidatorKind::ComposerBitbucketConsumerSecret,
            ValidatorKind::ComposerForgejoToken,
            ValidatorKind::RubyGems,
            ValidatorKind::RubyGemsHost,
            ValidatorKind::SwiftPm,
            ValidatorKind::SwiftPmNetrc,
            ValidatorKind::Gradle,
            ValidatorKind::CodiceFiscale,
            ValidatorKind::NhsNumber,
            ValidatorKind::Pesel,
            ValidatorKind::Ssn,
            ValidatorKind::CardVerificationCode,
            ValidatorKind::OtpProvisioningSecret,
            ValidatorKind::Tailscale,
            ValidatorKind::GitHub,
            ValidatorKind::GitLab,
            ValidatorKind::Stripe,
            ValidatorKind::Iban,
            ValidatorKind::Cloudflare,
            ValidatorKind::Slack,
            ValidatorKind::Telegram,
            ValidatorKind::Jwt,
            ValidatorKind::Nats,
            ValidatorKind::Aws,
            ValidatorKind::Azure,
            ValidatorKind::Gcp,
            ValidatorKind::DatabaseConnection,
            ValidatorKind::HttpBasic,
            ValidatorKind::Pan,
            ValidatorKind::Password,
            ValidatorKind::SensitiveHash,
            ValidatorKind::GenericCredential,
            ValidatorKind::WireGuard,
            ValidatorKind::SystemPasswordVerifier,
            ValidatorKind::Netrc,
            ValidatorKind::Nuget,
        ];
        assert_eq!(kinds.len(), 44);
        for kind in kinds {
            let scanner = build_scanner(vec![
                Rule::literal("candidate", "AlphaBravo123", Severity::High).with_validator(kind),
            ]);
            for source in [
                "AlphaBravo123",
                "password=AlphaBravo123\nABCD-EFGH-IJKL-MNOP",
                "<servers><server><password>AlphaBravo123</password></server></servers>",
                "é\r\n{\"bearer\":{\"repo.example\":\"AlphaBravo123\"}}",
            ] {
                parity(&scanner, source);
            }
        }
    }

    #[test]
    fn individually_admitted_builtins_have_positive_complete_report_parity() {
        use crate::builtins;
        let scanner = Scanner::builder()
            .builtins([
                builtins::NATS_NKEY_SEED,
                builtins::NATS_NKEY_PRIVATE_KEY,
                builtins::TELEGRAM_BOT_TOKEN,
            ])
            .build()
            .unwrap();
        let source = "é\r\ncredential=SUAB6M5NSNGSXV6SQ3TV6LQUH6S2OFHGJDZ2ENNZ4W5VBKBX3Z6TH3RW4E\nprivate=PABLXOMODALVP2WDIKL5VKTUCZID6PURMKV3NJO6VAT5ZI3HHJZDIV3SFWBNBPBXDWW6PWBHH7AZLFHUXJ6LU3EKRKNG5J7NQOKJ6XILM3TA\nbot=123456:ABC-DEF1234ghIkl-zyx57W2v1u123ew11\nABCD-EFGH-IJKL-MNOP";
        assert_eq!(
            scanner.scan([((), source)]).single_report().unwrap().len(),
            3
        );
        parity(&scanner, source);
    }

    #[test]
    fn absolute_anchors_and_suffix_token_bounds_preserve_complete_reports() {
        for pattern in [r"\Aabc", r"abc\z", r"\Aabc\z", "(?m:^abc$)", "(?mR:^abc$)"] {
            let scanner = build_scanner(vec![
                Rule::pattern("anchor", pattern, Severity::High).unwrap(),
            ]);
            for source in ["abc", "abc\r\nabc", "\nabc\n", "abcabc", "", "éabc🦀"] {
                parity(&scanner, source);
            }
        }
        let scanner = build_scanner(vec![Rule::suffix("suffix", "_end", Severity::High)]);
        for size in [4096, 4097, 20_000] {
            let source = format!("{}_end", "a".repeat(size - 4));
            let expected = scanner
                .scan([((), source.as_str())])
                .single_report()
                .unwrap()
                .clone();
            for chunk_size in [1, 7, size] {
                let mut session = scanner.stream().unwrap();
                for chunk in source.as_bytes().chunks(chunk_size) {
                    session.push(chunk).unwrap();
                }
                assert_eq!(session.finish().unwrap(), expected);
            }
        }
    }

    #[test]
    fn terminal_error_mapping_preserves_distinct_checked_failures() {
        use crate::source_coordinates::CoordinateOverflow;
        let limits = StreamLimits::default();
        assert_eq!(
            limits.error(SourceSessionError::SourceOffsetOverflow),
            StreamError::SourceOffsetOverflow
        );
        assert_eq!(
            limits.error(SourceSessionError::StreamStorageRequirementOverflow),
            StreamError::StorageRequirementOverflow
        );
        for overflow in [
            CoordinateOverflow::ByteOffset,
            CoordinateOverflow::Line,
            CoordinateOverflow::Column,
        ] {
            assert_eq!(
                limits.error(SourceSessionError::CoordinateOverflow(overflow)),
                StreamError::CoordinateOverflow
            );
        }
        for error in [
            SourceSessionError::MissingSourceHistory,
            SourceSessionError::MissingCandidateText,
            SourceSessionError::CandidateTextMismatch,
            SourceSessionError::InvalidPendingRange,
            SourceSessionError::PendingCapacityExceeded,
            SourceSessionError::MissingCoordinateHistory,
            SourceSessionError::MissingPrefixCoordinatePin,
        ] {
            assert_eq!(limits.error(error), StreamError::ExecutionInvariantViolated);
        }
        let mut count = usize::MAX;
        assert_eq!(
            retain_result(&mut count, None),
            Err(SourceSessionError::RetainedResultLimitExceeded)
        );
        assert_eq!(count, usize::MAX);
    }

    #[test]
    fn large_full_report_and_long_lived_prefix_parity() {
        let scanner = build_scanner(vec![
            Rule::literal("one", "x", Severity::High),
            Rule::literal("overlap", "xx", Severity::Low),
            Rule::prefix("prefix", "ABCD", Severity::High),
            Rule::prefix("second-prefix", "AB", Severity::Low),
            Rule::pattern("regex", r"y{1,3}", Severity::High).unwrap(),
        ]);
        let source = format!(
            "{} ABCD-EFGH-IJKL-MNOP{} yyy\r\nQRST-UVWX-YZ12-3456",
            "x".repeat(20_000),
            "a".repeat(50_000)
        );
        let expected = scanner
            .scan([((), source.as_str())])
            .single_report()
            .unwrap()
            .clone();
        for size in [1, 7, 4096, source.len()] {
            let mut session = scanner.stream().unwrap();
            for chunk in source.as_bytes().chunks(size) {
                session.push(chunk).unwrap();
            }
            assert_eq!(session.finish().unwrap(), expected);
        }
    }

    #[test]
    fn complete_report_structural_boundaries_coordinates_and_eof() {
        let scanner = build_scanner(vec![]);
        for source in [
            "",
            "ABCD-EFGH-IJKL-MNOP",
            "(ABCD-EFGH-IJKL-MNOP),",
            "é🦀\r\nABCD-EFGH-IJKL-MNOP\rQRST-UVWX-YZ12-3456",
            "XABCD-EFGH-IJKL-MNOP ABCD-EFGH-IJKL-MNOPQ ABCD-EFGH-IJKL-MNOP-QRST",
            "1234-5678-9012-3456 ABCD-EF12-3456-7890 XXXX-XXXX-XXXX-XXXX",
            "_ABCD-EFGH-IJKL-MNOP_",
            "ABCD-EFGH-IJKL-MNOPéQRST-UVWX-YZ12-3456",
        ] {
            parity(&scanner, source);
        }
    }

    #[test]
    fn complete_report_matcher_matrix_empty_captures_and_overlap_suppression() {
        let source = "é🦀\r\nABCD-EFGH-IJKL-MNOP tail_end! tok_value\nline\r\nkey=é12 q end";
        let rules = vec![
            Rule::literal("literal", "EFGH", Severity::Low),
            Rule::literal("equal", "EFGH", Severity::Critical),
            Rule::prefix("prefix", "tok_", Severity::High),
            Rule::suffix("suffix", "_end", Severity::High),
            Rule::pattern("regex", r"\bvalue\b", Severity::High).unwrap(),
            Rule::pattern("lf", "(?m:^line$)", Severity::High).unwrap(),
            Rule::pattern("crlf", "(?mR:^line$)", Severity::High).unwrap(),
            Rule::captured_pattern(
                "capture",
                r"key=(?P<value>é[0-9]{1,2})",
                "value",
                Severity::High,
            )
            .unwrap(),
            Rule::captured_pattern("empty", r"q(?P<value>)", "value", Severity::High).unwrap(),
            Rule::captured_pattern("eof", r"end(?P<value>)\z", "value", Severity::High).unwrap(),
        ];
        parity(&build_scanner(rules), source);
        for pattern in [r"ABCD(?P<value>)", r"(?P<value>)ABCD", r"MNOP(?P<value>)"] {
            parity(
                &build_scanner(vec![
                    Rule::captured_pattern("empty", pattern, "value", Severity::High).unwrap(),
                ]),
                "ABCD-EFGH-IJKL-MNOP",
            );
        }
        // A prefix can suppress a structural candidate discovered much earlier.
        parity(
            &build_scanner(vec![Rule::prefix("prefix", "ABCD", Severity::High)]),
            "ABCD-EFGH-IJKL-MNOP",
        );
    }

    #[test]
    fn complete_report_validator_confidence_priority_and_deferred_validation() {
        let value = "CorrectHorseBatteryStaple";
        let iban = "DE89370400440532013000";
        let rules = vec![
            Rule::literal("iban", iban, Severity::Low).with_validator(ValidatorKind::Iban),
            Rule::suffix("iban-suffix", "3000", Severity::High).with_validator(ValidatorKind::Iban),
            Rule::captured_pattern(
                "iban-capture",
                r"KEY=(?P<value>DE[0-9]{20})",
                "value",
                Severity::High,
            )
            .unwrap()
            .with_validator(ValidatorKind::Iban),
            Rule::literal("rejected", "invalid-iban", Severity::High)
                .with_validator(ValidatorKind::Iban),
            Rule::literal("maven", value, Severity::High).with_validator(ValidatorKind::Maven),
            Rule::suffix("maven-suffix", "Staple", Severity::High)
                .with_validator(ValidatorKind::Maven),
            Rule::captured_pattern(
                "maven-capture",
                "<password>(?P<value>CorrectHorseBatteryStaple)</password>",
                "value",
                Severity::High,
            )
            .unwrap()
            .with_validator(ValidatorKind::Maven),
            Rule::literal("database", "AlphaBravo123", Severity::Low)
                .with_validator(ValidatorKind::DatabaseConnection),
            Rule::literal("password", "AlphaBravo123", Severity::Critical)
                .with_validator(ValidatorKind::Password),
            Rule::literal("lower-priority", "AlphaBravo123", Severity::Critical),
            Rule::literal("partial", "AlphaBravo", Severity::High),
        ];
        parity(
            &build_scanner(rules),
            &format!(
                "é\r\npostgres://password:AlphaBravo123\n<servers><server><password>{value}</password></server></servers> KEY={iban} invalid-iban\nABCD-EFGH-IJKL-MNOP"
            ),
        );
    }

    #[test]
    fn retained_limit_precedes_append_and_cannot_be_refunded_by_finalization() {
        let scanner = build_scanner(vec![Rule::literal(
            "finding",
            "ABCD-EFGH-IJKL-MNOP",
            Severity::High,
        )]);
        let source = b"ABCD-EFGH-IJKL-MNOP ";
        // The eventual report has one finding and zero reviews, but two records
        // were retained. Suppression must not make a limit of one succeed.
        let mut limited = scanner
            .stream_with_limits(StreamLimits::default().with_max_retained_results(Some(1)))
            .unwrap();
        assert_eq!(
            limited.push(source),
            Err(StreamError::ResultLimitExceeded { limit: 1 })
        );
        assert_eq!(limited.push(b""), Err(StreamError::NotActive));
        assert_eq!(limited.finish(), Err(StreamError::NotActive));
        let mut exact = scanner
            .stream_with_limits(StreamLimits::default().with_max_retained_results(Some(2)))
            .unwrap();
        exact.push(source).unwrap();
        assert_eq!(exact.execution.as_ref().unwrap().output.retained, 2);
        assert_eq!(exact.finish().unwrap().len(), 1);
        let scanner = build_scanner(vec![
            Rule::literal("generic", "AlphaBravo123", Severity::High),
            Rule::literal("specialized", "AlphaBravo123", Severity::High)
                .with_validator(ValidatorKind::Password),
        ]);
        let mut session = scanner
            .stream_with_limits(StreamLimits::default().with_max_retained_results(Some(1)))
            .unwrap();
        assert_eq!(
            session.push(b"password=AlphaBravo123"),
            Err(StreamError::ResultLimitExceeded { limit: 1 })
        );
    }

    #[test]
    fn raw_budget_counts_rejections_independently_of_results_across_chunks() {
        let scanner = build_scanner(vec![
            Rule::literal("reject", "bad", Severity::High).with_validator(ValidatorKind::Iban),
            Rule::literal("accept", "good", Severity::High),
        ]);
        let limits = StreamLimits::default()
            .with_max_candidates(Some(2))
            .with_max_retained_results(Some(1));
        let mut session = scanner.stream_with_limits(limits).unwrap();
        session.push(b"bad ").unwrap();
        assert_eq!(session.execution.as_ref().unwrap().output.retained, 0);
        session.push(b"good ").unwrap();
        assert_eq!(session.execution.as_ref().unwrap().output.retained, 1);
        assert_eq!(
            session.push(b"bad"),
            Err(StreamError::CandidateLimitExceeded { limit: 2 })
        );
        assert_eq!(session.finish(), Err(StreamError::NotActive));
        let mut exact = scanner.stream_with_limits(limits).unwrap();
        exact.push(b"bad good").unwrap();
        assert_eq!(exact.finish().unwrap().len(), 1);
    }

    #[test]
    fn zero_limits_and_eof_only_result_exhaustion() {
        let scanner = build_scanner(vec![]);
        let limits = StreamLimits::default()
            .with_max_candidates(Some(0))
            .with_max_retained_results(Some(0));
        let mut empty = scanner.stream_with_limits(limits).unwrap();
        empty.push(b"ordinary text").unwrap();
        assert_eq!(empty.finish().unwrap(), ScanReport::default());
        let mut structural = scanner.stream_with_limits(limits).unwrap();
        structural.push(b"ABCD-EFGH-IJKL-MNOP").unwrap();
        assert_eq!(
            structural.finish(),
            Err(StreamError::ResultLimitExceeded { limit: 0 })
        );
        let scanner = build_scanner(vec![
            Rule::pattern("eof", r"abc\z", Severity::High).unwrap(),
        ]);
        let mut eof = scanner.stream_with_limits(limits).unwrap();
        eof.push(b"abc").unwrap();
        assert_eq!(
            eof.finish(),
            Err(StreamError::CandidateLimitExceeded { limit: 0 })
        );
    }

    #[test]
    fn transport_failures_and_first_execution_error_are_terminal() {
        let scanner = build_scanner(vec![Rule::literal("hit", "é", Severity::High)]);
        let mut session = scanner
            .stream_with_limits(StreamLimits::default().with_max_candidates(Some(0)))
            .unwrap();
        session.push(&[0xc3]).unwrap();
        assert_eq!(
            session.push(&[0xa9, 0xff]),
            Err(StreamError::CandidateLimitExceeded { limit: 0 })
        );
        assert_eq!(session.push(b"anything"), Err(StreamError::NotActive));
        assert_eq!(session.finish(), Err(StreamError::NotActive));
        let mut malformed = scanner.stream().unwrap();
        assert_eq!(malformed.push(&[0xff]), Err(StreamError::InvalidUtf8));
        assert!(malformed.execution.is_none());
        assert_eq!(malformed.finish(), Err(StreamError::NotActive));
        let mut incomplete = scanner.stream().unwrap();
        incomplete.push(&[0xf0, 0x9f]).unwrap();
        assert_eq!(incomplete.finish(), Err(StreamError::IncompleteUtf8));
    }

    #[test]
    fn admission_exact_storage_and_unsupported_portfolios() {
        let scanner = build_scanner(vec![Rule::literal("hit", "needle", Severity::High)]);
        let denied = scanner
            .stream_with_limits(StreamLimits::default().with_max_stream_storage_bytes(Some(0)))
            .unwrap_err();
        let StreamError::StorageLimitExceeded { required, limit: 0 } = denied else {
            panic!("{denied:?}")
        };
        assert!(
            scanner
                .stream_with_limits(
                    StreamLimits::default().with_max_stream_storage_bytes(Some(required - 1))
                )
                .is_err()
        );
        let mut exact = scanner
            .stream_with_limits(
                StreamLimits::default().with_max_stream_storage_bytes(Some(required)),
            )
            .unwrap();
        exact.push(b"needle").unwrap();
        assert_eq!(exact.finish().unwrap().len(), 1);
        assert_eq!(
            Scanner::default().stream().unwrap_err(),
            StreamError::UnsupportedRuleSet
        );
        for rules in [
            vec![Rule::pattern("unbounded", "a+", Severity::High).unwrap()],
            vec![
                Rule::prefix("prefix", "token_", Severity::High)
                    .with_validator(ValidatorKind::Iban),
            ],
            vec![Rule::suffix("suffix", "!", Severity::High)],
            vec![Rule::suffix("overlap", "aa", Severity::High)],
        ] {
            assert_eq!(
                build_scanner(rules).stream().unwrap_err(),
                StreamError::UnsupportedRuleSet
            );
        }
    }

    #[test]
    fn large_callback_limits_stop_before_retaining_many_results() {
        let scanner = build_scanner(vec![Rule::literal("hit", "x", Severity::High)]);
        let mut session = scanner
            .stream_with_limits(StreamLimits::default().with_max_retained_results(Some(3)))
            .unwrap();
        assert_eq!(
            session.push(&vec![b'x'; 1_000_000]),
            Err(StreamError::ResultLimitExceeded { limit: 3 })
        );
        assert!(session.execution.is_none());
        let empty = build_scanner(vec![]);
        let mut session = empty.stream().unwrap();
        session.push(&vec![b'a'; 1_000_000]).unwrap();
        assert!(
            session
                .execution
                .as_ref()
                .unwrap()
                .output
                .reviews
                .is_empty()
        );
        assert_eq!(session.finish().unwrap(), ScanReport::default());
    }

    #[test]
    fn debug_redaction_input_reuse_and_session_independence() {
        let scanner = build_scanner(vec![Rule::literal("hit", "PRIVATE_VALUE", Severity::High)]);
        let mut session = scanner.stream().unwrap();
        let mut input = b"PRIVATE_VALUE".to_vec();
        session.push(&input).unwrap();
        input.fill(b'!');
        assert!(!format!("{session:?}").contains("PRIVATE_VALUE"));
        let other = scanner.stream().unwrap().finish().unwrap();
        assert_eq!(other, ScanReport::default());
        assert_eq!(session.finish().unwrap().len(), 1);
    }
}
