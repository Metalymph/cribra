//! Owned records and enums shared by future native bindings.

/// Source coordinates copied from a Core finding.
///
/// `start..end` is a zero-based, half-open UTF-8 byte range. `line` and
/// `column` are one-based; columns count Unicode scalar values, not bytes,
/// UTF-16 code units, or grapheme clusters. Converted Core locations refer to
/// UTF-8 character boundaries. These plain data records do not validate values
/// supplied by a consumer. Flow/Share transfer explicit records through Core
/// serde; Core validates transformation ranges and boundaries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct Location {
    /// Inclusive start offset in UTF-8 bytes.
    pub start: u64,
    /// Exclusive end offset in UTF-8 bytes.
    pub end: u64,
    /// One-based line containing the start of the match.
    pub line: u64,
    /// One-based Unicode-scalar column containing the start of the match.
    pub column: u64,
}

/// Impact assigned by the Core, independently of confidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum Severity {
    /// Informational finding.
    Info,
    /// Low impact.
    Low,
    /// Medium impact.
    Medium,
    /// High impact.
    High,
    /// Critical impact.
    Critical,
}

/// Reliability assigned by the Core, independently of severity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum Confidence {
    /// Plausible detection requiring review.
    Low,
    /// Reasonably reliable detection.
    Medium,
    /// Highly reliable detection.
    High,
}

/// Owned stable rule identifier; validation remains the Core's responsibility.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct RuleId {
    /// Exact identifier assigned to the rule, without normalization.
    pub value: String,
}

/// Basic finding metadata, without matched text or source ownership.
///
/// Explanations in scanner reports are resolved by Core against the producing
/// scanner before projection. Plain consumer records carry no scanner authority.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct Finding {
    /// Stable identifier of the matching rule.
    pub rule_id: RuleId,
    /// Exact location of the match in the original source.
    pub location: Location,
    /// Impact assigned to the match.
    pub severity: Severity,
    /// Reliability assigned to the match.
    pub confidence: Confidence,
    /// Recommended Core action, when known.
    #[uniffi(default = None)]
    pub remediation: Option<Remediation>,
    /// Core-validated facts, or None when no compatible scanner authority is available.
    #[uniffi(default = None)]
    pub explanation: Option<Explanation>,
}

/// Caller-supplied source identity, preserved verbatim without normalization.
///
/// Empty IDs and duplicate IDs are allowed, as in Core; reports are associated
/// by input position, not by a map keyed by ID.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct SourceId {
    /// Owned UTF-8 identifier, interpreted only by the caller.
    pub value: String,
}

/// One owned scanner input. Text is input-only and is not returned in reports.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ScanSource {
    /// Identity to preserve in the corresponding report.
    pub id: SourceId,
    /// UTF-8 source text to scan synchronously.
    pub text: String,
}

/// Result for one source, without source text or matched values.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ScanReport {
    /// Caller identity, unchanged from the input at the same position.
    pub source_id: SourceId,
    /// Original source length in UTF-8 bytes.
    pub source_bytes: u64,
    /// Classified findings in deterministic Core order.
    pub findings: Vec<Finding>,
    /// Number of ambiguous candidates, equal to candidates.len().
    pub candidate_count: u64,
    /// Core review status: true for findings or ambiguous candidates.
    pub needs_review: bool,
    /// Review-only candidates in deterministic Core order, without matched text.
    #[uniffi(default = [])]
    pub candidates: Vec<SensitiveCandidate>,
}

/// Aggregate counters copied from Core, including ambiguous candidate totals.
///
/// A batch is clean only when both `total_findings` and `total_candidates` are
/// zero. A report without findings can still contain review-only candidates.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ScanSummary {
    /// Number of input sources, including duplicates and empty sources.
    pub scanned_sources: u64,
    /// Total UTF-8 input byte length.
    pub scanned_bytes: u64,
    /// Reports with at least one classified finding.
    pub reports_with_findings: u64,
    /// Reports with no classified findings, including candidate-only reports.
    pub reports_without_findings: u64,
    /// Total classified findings.
    pub total_findings: u64,
    /// Total ambiguous sensitive candidates.
    pub total_candidates: u64,
    /// Reports containing ambiguous candidates.
    pub reports_with_candidates: u64,
    /// Critical-severity finding count.
    pub critical: u64,
    /// High-severity finding count.
    pub high: u64,
    /// Medium-severity finding count.
    pub medium: u64,
    /// Low-severity finding count.
    pub low: u64,
    /// Informational finding count.
    pub info: u64,
}

/// Ordered batch reports and their Core-derived summary.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ScanResults {
    /// Exactly one report per input, in input order, even for duplicate IDs.
    pub reports: Vec<ScanReport>,
    /// Aggregate counters for this batch only.
    pub summary: ScanSummary,
}

/// Public Core remediation choices available for custom rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum Remediation {
    /// Revoke and rotate a credential.
    RevokeAndRotateCredential,
    /// Rotate a credential.
    RotateCredential,
    /// Rotate a password.
    RotatePassword,
    /// Replace a private key.
    ReplacePrivateKey,
    /// Remove a sensitive value.
    RemoveSensitiveValue,
    /// Review a sensitive hash.
    ReviewSensitiveHash,
    /// Review an exposed password verifier.
    ReviewPasswordVerifier,
}

/// How Core decides whether a rule match becomes a finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum DetectionMode {
    /// The custom matcher is authoritative.
    MatcherOnly,
    /// Built-in validation depends on the matched value's structure.
    Deterministic,
    /// Built-in validation also depends on surrounding source context.
    Contextual,
}

/// Evidence behind a review-only candidate; never finding confidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum CandidateEvidence {
    /// Sensitive-looking structure with insufficient classification context.
    Structural,
}

/// Family of a structurally plausible review-only value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum SensitiveCandidateKind {
    /// Recovery or backup code shape.
    RecoveryLikeCode,
}

/// Typed Core facts without source text or presentation copy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum Explanation {
    /// A rule-backed finding validated against scanner authority.
    Classified {
        /// Core detection authority.
        mode: DetectionMode,
    },
    /// A review-only candidate; no severity, confidence or remediation.
    Ambiguous {
        /// Core candidate evidence.
        evidence: CandidateEvidence,
    },
}

/// Review-only metadata, never a classified finding or a matched value.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct SensitiveCandidate {
    /// Structural family requiring review.
    pub kind: SensitiveCandidateKind,
    /// Exact half-open UTF-8 location in the original source.
    pub location: Location,
    /// Evidence distinct from finding confidence.
    pub evidence: CandidateEvidence,
    /// Facts projected directly from Core's candidate explanation.
    #[uniffi(default = None)]
    pub explanation: Option<Explanation>,
}
