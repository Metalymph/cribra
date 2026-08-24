//! WebAssembly interoperability adapter for Cribra.
//!
//! This crate projects the authoritative Rust-native `cribra` core into a
//! JavaScript-friendly WebAssembly API. It contains no DOM, Worker, networking,
//! storage, or application-specific policy logic.
//!
//! Browser consumers own source lifecycle and isolation. Source text crosses
//! the WASM boundary only for explicit operations such as scanning or
//! transformation. Scan results retain metadata only; they never retain the
//! scanned source text or matched sensitive values.

use cribra::{
    CandidateEvidence, Confidence, DetectionMode, Explanation, Finding, Redaction, Remediation,
    Rule, ScanReport, Scanner, SensitiveCandidate, SensitiveCandidateKind, Severity,
    transform::{
        PseudonymizationOptions, SynthesisOptions, TemplateOptions, pseudonymize, redact,
        redact_with, synthesize, template, template_with,
    },
};
use wasm_bindgen::{JsError, prelude::*};

fn key32(key: &[u8]) -> Result<[u8; 32], JsError> {
    key.try_into()
        .map_err(|_| JsError::new("transformation key must contain exactly 32 bytes"))
}

fn transform_error(error: impl std::fmt::Display) -> JsError {
    JsError::new(&error.to_string())
}

#[wasm_bindgen]
pub struct ScanEngine {
    scanner: Scanner,
}

#[wasm_bindgen]
impl ScanEngine {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            scanner: Scanner::default(),
        }
    }

    #[wasm_bindgen(js_name = rulesCount)]
    pub fn rules_count(&self) -> usize {
        self.scanner.rules_count()
    }

    pub fn scan(&self, source: &str) -> ScanResult {
        let mut entries = self.scanner.scan([("source", source)]).into_inner();
        let (_, source_bytes, report) = entries
            .pop()
            .expect("single-source WASM scan must produce exactly one result entry")
            .into_parts();

        ScanResult {
            source_bytes,
            report,
        }
    }

    /// Resolves typed explanation facts for one finding against this engine's
    /// immutable compiled rule metadata.
    ///
    /// Resolution fails closed when the result cannot be mapped back to this
    /// scanner's rule authority.
    #[wasm_bindgen(js_name = explainFinding)]
    pub fn explain_finding(
        &self,
        result: &ScanResult,
        index: usize,
    ) -> Result<ExplanationView, JsError> {
        let finding = result
            .report
            .findings()
            .get(index)
            .ok_or_else(|| JsError::new("finding index out of range"))?;

        finding
            .explanation(&self.scanner)
            .map(ExplanationView::from)
            .ok_or_else(|| JsError::new("finding explanation unavailable for this engine"))
    }
}

impl Default for ScanEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Typed builder for a reusable Cribra WASM scanner.
///
/// The builder owns declarative custom rules only. It exposes the same four
/// public custom-rule families as the Rust core and performs no
/// application-specific policy interpretation.
#[wasm_bindgen]
pub struct ScanEngineBuilder {
    include_builtins: bool,
    rules: Vec<Rule>,
}

#[wasm_bindgen]
impl ScanEngineBuilder {
    /// Creates a builder. When `includeBuiltins` is true, the authoritative
    /// current Cribra catalog is included alongside custom rules.
    #[wasm_bindgen(constructor)]
    pub fn new(include_builtins: bool) -> Self {
        Self {
            include_builtins,
            rules: Vec::new(),
        }
    }

    /// Adds an exact literal rule.
    #[wasm_bindgen(js_name = addLiteral)]
    pub fn add_literal(&mut self, id: &str, value: &str, severity: FindingSeverity) {
        self.rules
            .push(Rule::literal(id, value, Severity::from(severity)));
    }

    /// Adds a token-prefix rule.
    #[wasm_bindgen(js_name = addPrefix)]
    pub fn add_prefix(&mut self, id: &str, value: &str, severity: FindingSeverity) {
        self.rules
            .push(Rule::prefix(id, value, Severity::from(severity)));
    }

    /// Adds a token-suffix rule.
    #[wasm_bindgen(js_name = addSuffix)]
    pub fn add_suffix(&mut self, id: &str, value: &str, severity: FindingSeverity) {
        self.rules
            .push(Rule::suffix(id, value, Severity::from(severity)));
    }

    /// Adds a regular-expression rule.
    ///
    /// Invalid patterns and patterns capable of zero-length matches are
    /// rejected immediately by the authoritative Rust rule constructor.
    #[wasm_bindgen(js_name = addPattern)]
    pub fn add_pattern(
        &mut self,
        id: &str,
        pattern: &str,
        severity: FindingSeverity,
    ) -> Result<(), JsError> {
        self.add_pattern_core(id, pattern, severity)
            .map_err(|error| JsError::new(&error))
    }

    /// Returns the number of custom rules staged in this builder.
    #[wasm_bindgen(js_name = customRuleCount)]
    pub fn custom_rule_count(&self) -> usize {
        self.rules.len()
    }

    /// Validates and compiles this configuration into an immutable engine.
    pub fn build(self) -> Result<ScanEngine, JsError> {
        self.build_core().map_err(|error| JsError::new(&error))
    }
}

impl ScanEngineBuilder {
    /// Native-safe validation path used by both the WASM export and Rust tests.
    fn add_pattern_core(
        &mut self,
        id: &str,
        pattern: &str,
        severity: FindingSeverity,
    ) -> Result<(), String> {
        let rule = Rule::pattern(id, pattern, Severity::from(severity))
            .map_err(|error| error.to_string())?;
        self.rules.push(rule);
        Ok(())
    }

    /// Native-safe scanner compilation path used by both the WASM export and
    /// Rust tests. JavaScript error construction stays at the exported edge.
    fn build_core(self) -> Result<ScanEngine, String> {
        let mut builder = Scanner::builder();

        if self.include_builtins {
            builder = builder.builtins(cribra::builtins::CURRENT);
        }

        let scanner = builder
            .rules(self.rules)
            .build()
            .map_err(|error| error.to_string())?;

        Ok(ScanEngine { scanner })
    }
}

#[wasm_bindgen]
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum FindingSeverity {
    Info,
    Low,
    Medium,
    High,
    Critical,
}

impl From<Severity> for FindingSeverity {
    fn from(value: Severity) -> Self {
        match value {
            Severity::Info => Self::Info,
            Severity::Low => Self::Low,
            Severity::Medium => Self::Medium,
            Severity::High => Self::High,
            Severity::Critical => Self::Critical,
        }
    }
}

impl From<FindingSeverity> for Severity {
    fn from(value: FindingSeverity) -> Self {
        match value {
            FindingSeverity::Info => Self::Info,
            FindingSeverity::Low => Self::Low,
            FindingSeverity::Medium => Self::Medium,
            FindingSeverity::High => Self::High,
            FindingSeverity::Critical => Self::Critical,
        }
    }
}

#[wasm_bindgen]
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum FindingConfidence {
    Low,
    Medium,
    High,
}

impl From<Confidence> for FindingConfidence {
    fn from(value: Confidence) -> Self {
        match value {
            Confidence::Low => Self::Low,
            Confidence::Medium => Self::Medium,
            Confidence::High => Self::High,
        }
    }
}

#[wasm_bindgen]
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum RemediationKind {
    None,
    RevokeAndRotateCredential,
    RotateCredential,
    RotatePassword,
    ReplacePrivateKey,
    RemoveSensitiveValue,
    ReviewSensitiveHash,
    Unknown,
}

impl From<Option<Remediation>> for RemediationKind {
    fn from(value: Option<Remediation>) -> Self {
        match value {
            None => Self::None,
            Some(Remediation::RevokeAndRotateCredential) => Self::RevokeAndRotateCredential,
            Some(Remediation::RotateCredential) => Self::RotateCredential,
            Some(Remediation::RotatePassword) => Self::RotatePassword,
            Some(Remediation::ReplacePrivateKey) => Self::ReplacePrivateKey,
            Some(Remediation::RemoveSensitiveValue) => Self::RemoveSensitiveValue,
            Some(Remediation::ReviewSensitiveHash) => Self::ReviewSensitiveHash,
            Some(_) => Self::Unknown,
        }
    }
}

#[wasm_bindgen]
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum CandidateKind {
    RecoveryLikeCode,
    Unknown,
}

impl From<SensitiveCandidateKind> for CandidateKind {
    fn from(value: SensitiveCandidateKind) -> Self {
        match value {
            SensitiveCandidateKind::RecoveryLikeCode => Self::RecoveryLikeCode,
            _ => Self::Unknown,
        }
    }
}

#[wasm_bindgen]
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum CandidateEvidenceKind {
    Structural,
    Unknown,
}

impl From<CandidateEvidence> for CandidateEvidenceKind {
    fn from(value: CandidateEvidence) -> Self {
        match value {
            CandidateEvidence::Structural => Self::Structural,
            _ => Self::Unknown,
        }
    }
}

/// Detection authority projected for classified finding explanations.
#[wasm_bindgen]
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum DetectionModeKind {
    MatcherOnly,
    Deterministic,
    Contextual,
}

impl From<DetectionMode> for DetectionModeKind {
    fn from(value: DetectionMode) -> Self {
        match value {
            DetectionMode::MatcherOnly => Self::MatcherOnly,
            DetectionMode::Deterministic => Self::Deterministic,
            DetectionMode::Contextual => Self::Contextual,
        }
    }
}

/// Stable discriminant for one typed explanation.
#[wasm_bindgen]
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ExplanationKind {
    Classified,
    Ambiguous,
    Unknown,
}

/// Presentation-agnostic explanation facts.
///
/// Classified explanations expose only the scanner-owned detection mode.
/// Ambiguous explanations expose only the candidate evidence. No source text,
/// matched value, rule matcher payload, or presentation copy crosses the
/// boundary.
#[wasm_bindgen]
pub struct ExplanationView {
    kind: ExplanationKind,
    detection_mode: Option<DetectionModeKind>,
    candidate_evidence: Option<CandidateEvidenceKind>,
}

impl From<Explanation> for ExplanationView {
    fn from(explanation: Explanation) -> Self {
        match explanation {
            Explanation::Classified(mode) => Self {
                kind: ExplanationKind::Classified,
                detection_mode: Some(mode.into()),
                candidate_evidence: None,
            },
            Explanation::Ambiguous(evidence) => Self {
                kind: ExplanationKind::Ambiguous,
                detection_mode: None,
                candidate_evidence: Some(evidence.into()),
            },
            _ => Self {
                kind: ExplanationKind::Unknown,
                detection_mode: None,
                candidate_evidence: None,
            },
        }
    }
}

#[wasm_bindgen]
impl ExplanationView {
    #[wasm_bindgen(getter)]
    pub fn kind(&self) -> ExplanationKind {
        self.kind
    }

    /// Returns the classified detection mode, or `undefined` for an ambiguous
    /// candidate explanation.
    #[wasm_bindgen(getter, js_name = detectionMode)]
    pub fn detection_mode(&self) -> Option<DetectionModeKind> {
        self.detection_mode
    }

    /// Returns candidate evidence, or `undefined` for a classified finding
    /// explanation.
    #[wasm_bindgen(getter, js_name = candidateEvidence)]
    pub fn candidate_evidence(&self) -> Option<CandidateEvidenceKind> {
        self.candidate_evidence
    }
}

#[wasm_bindgen]
pub struct FindingView {
    rule_id: String,
    start: usize,
    end: usize,
    line: usize,
    column: usize,
    severity: FindingSeverity,
    confidence: FindingConfidence,
    remediation: RemediationKind,
}

impl From<&Finding> for FindingView {
    fn from(finding: &Finding) -> Self {
        let location = finding.location();

        Self {
            rule_id: finding.rule_id().as_str().to_owned(),
            start: location.start(),
            end: location.end(),
            line: location.line(),
            column: location.column(),
            severity: finding.severity().into(),
            confidence: finding.confidence().into(),
            remediation: finding.remediation().into(),
        }
    }
}

#[wasm_bindgen]
impl FindingView {
    #[wasm_bindgen(getter, js_name = ruleId)]
    pub fn rule_id(&self) -> String {
        self.rule_id.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn start(&self) -> usize {
        self.start
    }

    #[wasm_bindgen(getter)]
    pub fn end(&self) -> usize {
        self.end
    }

    #[wasm_bindgen(getter)]
    pub fn line(&self) -> usize {
        self.line
    }

    #[wasm_bindgen(getter)]
    pub fn column(&self) -> usize {
        self.column
    }

    #[wasm_bindgen(getter)]
    pub fn severity(&self) -> FindingSeverity {
        self.severity
    }

    #[wasm_bindgen(getter)]
    pub fn confidence(&self) -> FindingConfidence {
        self.confidence
    }

    #[wasm_bindgen(getter)]
    pub fn remediation(&self) -> RemediationKind {
        self.remediation
    }
}

#[wasm_bindgen]
pub struct CandidateView {
    kind: CandidateKind,
    evidence: CandidateEvidenceKind,
    start: usize,
    end: usize,
    line: usize,
    column: usize,
}

impl From<&SensitiveCandidate> for CandidateView {
    fn from(candidate: &SensitiveCandidate) -> Self {
        let location = candidate.location();

        Self {
            kind: candidate.kind().into(),
            evidence: candidate.evidence().into(),
            start: location.start(),
            end: location.end(),
            line: location.line(),
            column: location.column(),
        }
    }
}

#[wasm_bindgen]
impl CandidateView {
    #[wasm_bindgen(getter)]
    pub fn kind(&self) -> CandidateKind {
        self.kind
    }

    #[wasm_bindgen(getter)]
    pub fn evidence(&self) -> CandidateEvidenceKind {
        self.evidence
    }

    #[wasm_bindgen(getter)]
    pub fn start(&self) -> usize {
        self.start
    }

    #[wasm_bindgen(getter)]
    pub fn end(&self) -> usize {
        self.end
    }

    #[wasm_bindgen(getter)]
    pub fn line(&self) -> usize {
        self.line
    }

    #[wasm_bindgen(getter)]
    pub fn column(&self) -> usize {
        self.column
    }
}

#[wasm_bindgen]
pub struct ScanResult {
    source_bytes: usize,
    report: ScanReport,
}

#[wasm_bindgen]
impl ScanResult {
    #[wasm_bindgen(getter, js_name = sourceBytes)]
    pub fn source_bytes(&self) -> usize {
        self.source_bytes
    }

    #[wasm_bindgen(js_name = findingCount)]
    pub fn finding_count(&self) -> usize {
        self.report.len()
    }

    #[wasm_bindgen(js_name = candidateCount)]
    pub fn candidate_count(&self) -> usize {
        self.report.candidate_len()
    }

    #[wasm_bindgen(js_name = needsReview)]
    pub fn needs_review(&self) -> bool {
        self.report.needs_review()
    }

    #[wasm_bindgen(js_name = hasCritical)]
    pub fn has_critical(&self) -> bool {
        self.report.has_critical()
    }

    #[wasm_bindgen(js_name = findingAt)]
    pub fn finding_at(&self, index: usize) -> Result<FindingView, JsError> {
        self.report
            .findings()
            .get(index)
            .map(FindingView::from)
            .ok_or_else(|| JsError::new("finding index out of range"))
    }

    /// Applies Cribra's conservative default redaction to classified findings.
    ///
    /// Ambiguous candidates are never transformed.
    pub fn redact(&self, source: &str) -> Result<String, JsError> {
        redact(source, &self.report).map_err(transform_error)
    }

    /// Applies caller-selected replacement text to classified findings.
    ///
    /// Ambiguous candidates are never transformed.
    #[wasm_bindgen(js_name = redactWith)]
    pub fn redact_with(&self, source: &str, replacement: &str) -> Result<String, JsError> {
        let redaction = Redaction::new(replacement);
        redact_with(source, &self.report, &redaction).map_err(transform_error)
    }

    /// Produces semantic placeholders using Cribra's default template options.
    pub fn template(&self, source: &str) -> Result<String, JsError> {
        template(source, &self.report).map_err(transform_error)
    }

    /// Produces semantic placeholders with an explicit namespace and optional
    /// per-rule occurrence numbering.
    #[wasm_bindgen(js_name = templateWith)]
    pub fn template_with(
        &self,
        source: &str,
        namespace: &str,
        numbered: bool,
    ) -> Result<String, JsError> {
        let options = TemplateOptions::new()
            .namespace(namespace)
            .numbered(numbered);

        template_with(source, &self.report, &options).map_err(transform_error)
    }

    /// Replaces classified findings with deterministic keyed pseudonyms.
    ///
    /// `key` must contain exactly 32 bytes. The key is borrowed only for this
    /// call and is never retained by the result or adapter.
    pub fn pseudonymize(
        &self,
        source: &str,
        key: &[u8],
        prefix: &str,
        digest_bytes: usize,
    ) -> Result<String, JsError> {
        let options = PseudonymizationOptions::new(key32(key)?)
            .prefix(prefix)
            .digest_bytes(digest_bytes);

        pseudonymize(source, &self.report, &options).map_err(transform_error)
    }

    /// Replaces classified findings with deterministic synthetic values.
    ///
    /// `key` must contain exactly 32 bytes. The key is borrowed only for this
    /// call and is never retained by the result or adapter.
    pub fn synthesize(&self, source: &str, key: &[u8], marker: &str) -> Result<String, JsError> {
        let options = SynthesisOptions::new(key32(key)?).marker(marker);

        synthesize(source, &self.report, &options).map_err(transform_error)
    }

    #[wasm_bindgen(js_name = candidateAt)]
    pub fn candidate_at(&self, index: usize) -> Result<CandidateView, JsError> {
        self.report
            .candidates()
            .get(index)
            .map(CandidateView::from)
            .ok_or_else(|| JsError::new("candidate index out of range"))
    }

    /// Returns typed explanation facts for one ambiguous candidate.
    ///
    /// Candidate explanation derives directly from the candidate's existing
    /// evidence and never acquires finding semantics.
    #[wasm_bindgen(js_name = candidateExplanationAt)]
    pub fn candidate_explanation_at(&self, index: usize) -> Result<ExplanationView, JsError> {
        self.report
            .candidates()
            .get(index)
            .map(|candidate| ExplanationView::from(candidate.explanation()))
            .ok_or_else(|| JsError::new("candidate index out of range"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_engine_uses_current_builtins() {
        let engine = ScanEngine::new();
        assert_eq!(engine.rules_count(), cribra::builtins::CURRENT.len());
    }

    #[test]
    fn candidate_only_scan_preserves_review_only_semantics() {
        let engine = ScanEngine::new();
        let result = engine.scan("ABCD-EFGH-IJKL-MNOP");

        assert_eq!(result.finding_count(), 0);
        assert_eq!(result.candidate_count(), 1);
        assert!(result.needs_review());
        assert!(!result.has_critical());

        let candidate = CandidateView::from(&result.report.candidates()[0]);
        assert_eq!(candidate.kind(), CandidateKind::RecoveryLikeCode);
        assert_eq!(candidate.evidence(), CandidateEvidenceKind::Structural);
    }

    #[test]
    fn projected_findings_never_require_source_retention() {
        let engine = ScanEngine::new();
        let source = "GITHUB_TOKEN=ghp_AbCdEf0123456789_AbCdEf0123456789";
        let result = engine.scan(source);

        assert_eq!(result.source_bytes(), source.len());
        assert_eq!(result.finding_count(), 1);
        assert_eq!(result.candidate_count(), 0);

        let finding = FindingView::from(&result.report.findings()[0]);
        assert!(!finding.rule_id().is_empty());
        assert!(finding.end() > finding.start());
        assert_eq!(finding.line(), 1);
        assert!(finding.column() >= 1);
    }

    #[test]
    fn remediation_is_projected_only_for_findings() {
        let engine = ScanEngine::new();
        let result = engine.scan("GITHUB_TOKEN=ghp_AbCdEf0123456789_AbCdEf0123456789");

        let finding = FindingView::from(&result.report.findings()[0]);
        assert_ne!(finding.remediation(), RemediationKind::None);
    }

    #[test]
    fn classified_explanation_reuses_scanner_detection_authority() {
        let engine = ScanEngine::new();
        let result = engine.scan("GITHUB_TOKEN=ghp_AbCdEf0123456789_AbCdEf0123456789");

        let explanation = engine
            .explain_finding(&result, 0)
            .expect("finding explanation should resolve against its engine");

        assert_eq!(explanation.kind(), ExplanationKind::Classified);
        assert!(explanation.detection_mode().is_some());
        assert_eq!(explanation.candidate_evidence(), None);
    }

    #[test]
    fn candidate_explanation_reuses_candidate_evidence_only() {
        let engine = ScanEngine::new();
        let result = engine.scan("ABCD-EFGH-IJKL-MNOP");

        let explanation = result
            .candidate_explanation_at(0)
            .expect("candidate explanation should exist");

        assert_eq!(explanation.kind(), ExplanationKind::Ambiguous);
        assert_eq!(explanation.detection_mode(), None);
        assert_eq!(
            explanation.candidate_evidence(),
            Some(CandidateEvidenceKind::Structural)
        );
    }

    #[test]
    fn redact_transforms_findings_without_touching_candidates() {
        let engine = ScanEngine::new();
        let source = "GITHUB_TOKEN=ghp_AbCdEf0123456789_AbCdEf0123456789";
        let result = engine.scan(source);

        let output = result.redact(source).expect("redaction should succeed");

        assert!(!output.contains("ghp_AbCdEf0123456789_AbCdEf0123456789"));
        assert!(output.contains("[REDACTED]"));
    }

    #[test]
    fn template_uses_typed_report_without_serialization() {
        let engine = ScanEngine::new();
        let source = "GITHUB_TOKEN=ghp_AbCdEf0123456789_AbCdEf0123456789";
        let result = engine.scan(source);

        let output = result
            .template_with(source, "EXAMPLE", true)
            .expect("template should succeed");

        assert!(output.contains("<EXAMPLE:"));
        assert!(!output.contains("ghp_AbCdEf0123456789_AbCdEf0123456789"));
    }

    #[test]
    fn keyed_transforms_are_deterministic_and_do_not_retain_keys() {
        let engine = ScanEngine::new();
        let source = "GITHUB_TOKEN=ghp_AbCdEf0123456789_AbCdEf0123456789";
        let result = engine.scan(source);
        let key = [7_u8; 32];

        let first = result
            .pseudonymize(source, &key, "pseudo_", 16)
            .expect("pseudonymization should succeed");
        let second = result
            .pseudonymize(source, &key, "pseudo_", 16)
            .expect("pseudonymization should be deterministic");
        let synthetic = result
            .synthesize(source, &key, "cribra_synthetic")
            .expect("synthesis should succeed");

        assert_eq!(first, second);
        assert!(first.contains("pseudo_"));
        assert_ne!(synthetic, source);
        assert!(!synthetic.contains("ghp_AbCdEf0123456789_AbCdEf0123456789"));
    }

    #[test]
    fn transforms_leave_candidate_only_source_unchanged() {
        let engine = ScanEngine::new();
        let source = "ABCD-EFGH-IJKL-MNOP";
        let result = engine.scan(source);

        assert_eq!(result.finding_count(), 0);
        assert_eq!(result.candidate_count(), 1);
        assert_eq!(result.redact(source).unwrap(), source);
        assert_eq!(result.template(source).unwrap(), source);
    }

    #[test]
    fn typed_builder_supports_all_public_custom_rule_families() {
        let mut builder = ScanEngineBuilder::new(false);
        builder.add_literal("custom.literal", "EXACT_SECRET", FindingSeverity::High);
        builder.add_prefix("custom.prefix", "live_", FindingSeverity::Critical);
        builder.add_suffix("custom.suffix", "_secret", FindingSeverity::Medium);
        builder
            .add_pattern(
                "custom.pattern",
                r"\bCUSTOM_[A-Z0-9]{8}\b",
                FindingSeverity::Low,
            )
            .expect("valid custom pattern should compile");

        assert_eq!(builder.custom_rule_count(), 4);

        let engine = builder.build().expect("custom scanner should build");
        let result = engine.scan("EXACT_SECRET live_ABC123 token_secret CUSTOM_AB12CD34");

        assert_eq!(engine.rules_count(), 4);
        assert_eq!(result.finding_count(), 4);
    }

    #[test]
    fn typed_builder_can_combine_current_builtins_and_custom_rules() {
        let mut builder = ScanEngineBuilder::new(true);
        builder.add_literal("custom.literal", "EXACT_SECRET", FindingSeverity::High);

        let engine = builder.build().expect("combined scanner should build");

        assert_eq!(engine.rules_count(), cribra::builtins::CURRENT.len() + 1);
        assert_eq!(engine.scan("EXACT_SECRET").finding_count(), 1);
    }

    #[test]
    fn typed_builder_rejects_invalid_and_empty_capable_patterns() {
        let mut invalid = ScanEngineBuilder::new(false);
        assert!(
            invalid
                .add_pattern_core("custom.invalid", "(", FindingSeverity::High)
                .is_err()
        );

        let mut empty = ScanEngineBuilder::new(false);
        assert!(
            empty
                .add_pattern_core("custom.empty", ".*", FindingSeverity::High)
                .is_err()
        );
    }

    #[test]
    fn typed_builder_fails_closed_on_duplicate_rule_ids() {
        let mut builder = ScanEngineBuilder::new(false);
        builder.add_literal("custom.shared", "FIRST", FindingSeverity::High);
        builder.add_literal("custom.shared", "SECOND", FindingSeverity::Critical);

        assert!(builder.build_core().is_err());
    }
}
