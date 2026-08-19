use std::collections::BTreeMap;

use aho_corasick::{AhoCorasick, AhoCorasickBuilder, MatchKind};
use regex::Regex;

use crate::{
    confidence::Confidence,
    remediation::Remediation,
    rule::{Matcher, Rule, RuleId, RuleKind},
    scanner_builder::ScannerBuildError,
    severity::Severity,
    validators::dispatch::ValidatorKind,
};

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
            ValidatorKind::Password | ValidatorKind::SensitiveHash => 200,
            ValidatorKind::Jwt => 300,
            ValidatorKind::GitHub
            | ValidatorKind::Stripe
            | ValidatorKind::Cloudflare
            | ValidatorKind::Slack
            | ValidatorKind::Telegram
            | ValidatorKind::Aws
            | ValidatorKind::Azure
            | ValidatorKind::Gcp => 500,
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
}

impl MultiPatternEngine {
    fn compile(rules: Vec<MultiPatternRule>) -> Result<Option<Self>, ScannerBuildError> {
        if rules.is_empty() {
            return Ok(None);
        }

        // Overlapping iteration requires standard match semantics. Keeping all
        // overlaps is intentional: rules may share a prefix, use identical
        // needles, or begin at the same source position.
        let automaton = AhoCorasickBuilder::new()
            .match_kind(MatchKind::Standard)
            .build(rules.iter().map(|rule| rule.needle.as_ref()))
            .map_err(ScannerBuildError::AutomatonBuild)?;

        Ok(Some(Self {
            automaton,
            rules: rules.into_boxed_slice(),
        }))
    }

    fn scan(&self, source: &str, findings: &mut Vec<InternalFinding>) {
        let bytes = source.as_bytes();

        for matched in self.automaton.find_overlapping_iter(source) {
            let rule = &self.rules[matched.pattern().as_usize()];
            let start = matched.start();

            match rule.kind {
                MultiPatternKind::Literal => {
                    findings.push(InternalFinding::new(rule.rule_index, start, matched.end()));
                }
                MultiPatternKind::Prefix => {
                    if start > 0 && is_token_byte(bytes[start - 1]) {
                        continue;
                    }

                    let mut end = matched.end();
                    while end < bytes.len() && is_token_byte(bytes[end]) {
                        end += 1;
                    }

                    findings.push(InternalFinding::new(rule.rule_index, start, end));
                }
            }
        }
    }
}

#[derive(Debug)]
struct SuffixRule {
    rule_index: RuleIndex,
    suffix: Box<str>,
}

impl SuffixRule {
    fn scan(&self, source: &str, findings: &mut Vec<InternalFinding>) {
        let bytes = source.as_bytes();

        for (suffix_start, _) in source.match_indices(self.suffix.as_ref()) {
            let end = suffix_start + self.suffix.len();

            if end < bytes.len() && is_token_byte(bytes[end]) {
                continue;
            }

            let mut start = suffix_start;
            while start > 0 && is_token_byte(bytes[start - 1]) {
                start -= 1;
            }

            findings.push(InternalFinding::new(self.rule_index, start, end));
        }
    }
}

#[derive(Debug)]
struct PatternRule {
    rule_index: RuleIndex,
    pattern: Regex,
    capture: Option<usize>,
    prefilter: Option<AhoCorasick>,
    gate_bit: Option<u8>,
}

impl PatternRule {
    fn scan(&self, source: &str, findings: &mut Vec<InternalFinding>) {
        match (&self.prefilter, self.capture) {
            (Some(prefilter), Some(capture)) => {
                self.scan_prefiltered_captures(source, findings, prefilter, capture);
            }
            (Some(prefilter), None) => {
                self.scan_prefiltered_matches(source, findings, prefilter);
            }
            (None, None) => findings.extend(self.pattern.find_iter(source).map(|matched| {
                InternalFinding::new(self.rule_index, matched.start(), matched.end())
            })),
            (None, Some(capture)) => {
                findings.extend(self.pattern.captures_iter(source).filter_map(|captures| {
                    captures.get(capture).map(|matched| {
                        InternalFinding::new(self.rule_index, matched.start(), matched.end())
                    })
                }));
            }
        }
    }

    fn scan_prefiltered_captures(
        &self,
        source: &str,
        findings: &mut Vec<InternalFinding>,
        prefilter: &AhoCorasick,
        capture: usize,
    ) {
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
                findings.push(InternalFinding::new(
                    self.rule_index,
                    matched.start(),
                    matched.end(),
                ));
            }
        }
    }

    fn scan_prefiltered_matches(
        &self,
        source: &str,
        findings: &mut Vec<InternalFinding>,
        prefilter: &AhoCorasick,
    ) {
        let bytes = source.as_bytes();

        for key_match in prefilter.find_iter(source) {
            let key_start = key_match.start();
            let search_start = optional_quote_start(bytes, key_start);

            let Some(matched) = self.pattern.find_at(source, search_start) else {
                continue;
            };

            if matched.start() == search_start || matched.start() == key_start {
                findings.push(InternalFinding::new(
                    self.rule_index,
                    matched.start(),
                    matched.end(),
                ));
            }
        }
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

impl CompiledRuleSet {
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
                Matcher::Pattern { regex, capture } => patterns.push(PatternRule {
                    rule_index,
                    pattern: regex,
                    capture,
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
        if let Some(engine) = &self.multi_pattern {
            engine.scan(source, findings);
        }

        for rule in &self.suffixes {
            rule.scan(source, findings);
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

            rule.scan(source, findings);
        }
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
            "client_secret",
        ]),
        ("azure.storage-account-key", ValidatorKind::Azure) => {
            Some(&["storage_account_key", "azure_storage_key", "account_key"])
        }
        ("azure.shared-access-signature", ValidatorKind::Azure) => {
            Some(&["shared_access_signature", "azure_sas_token", "sas_token"])
        }
        ("gcp.private-key-id", ValidatorKind::Gcp) => Some(&["private_key_id"]),
        ("gcp.client-secret", ValidatorKind::Gcp) => Some(&["client_secret"]),
        ("gcp.private-key", ValidatorKind::Gcp) => Some(&["private_key"]),
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
        ("generic.secret", ValidatorKind::GenericCredential) => {
            Some(&["signing_secret", "webhook_secret", "secret_key", "secret"])
        }
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
    const fn new(rule_index: RuleIndex, start: usize, end: usize) -> Self {
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
