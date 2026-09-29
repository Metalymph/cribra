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

    fn scan_stream_chunk(
        &self,
        state: &mut MultiPatternStreamState,
        chunk: &[u8],
        source_offset: usize,
        findings: &mut Vec<InternalFinding>,
    ) {
        // 1. Resolve prefixes already active before this chunk.
        Self::continue_active_prefixes(state, chunk, source_offset, findings);

        let overlap_len = state.overlap.len();

        debug_assert!(source_offset >= overlap_len);

        // 2. Discover literal + prefix needles wholly inside this chunk.
        for matched in self.automaton.find_overlapping_iter(chunk) {
            let rule = &self.rules[matched.pattern().as_usize()];
            let absolute_start = source_offset + matched.start();

            match rule.kind {
                MultiPatternKind::Literal => {
                    findings.push(InternalFinding::new(
                        rule.rule_index,
                        absolute_start,
                        source_offset + matched.end(),
                    ));
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
                    );
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
                        findings.push(InternalFinding::new(
                            rule.rule_index,
                            absolute_start,
                            absolute_end,
                        ));
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
                        );
                    }
                }
            }
        }

        // 4. Mutate overlap only after every matcher has observed the old boundary.
        self.update_stream_overlap(state, chunk);
    }

    fn continue_active_prefixes(
        state: &mut MultiPatternStreamState,
        chunk: &[u8],
        source_offset: usize,
        findings: &mut Vec<InternalFinding>,
    ) {
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

            findings.push(InternalFinding::new(
                active.rule_index,
                active.source_start,
                source_offset + token_len,
            ));

            state.active_prefixes.swap_remove(index);
        }
    }

    fn accept_stream_prefix(
        &self,
        state: &mut MultiPatternStreamState,
        rule: &MultiPatternRule,
        source_start: usize,
        needle_end: usize,
        suffix: &[u8],
        findings: &mut Vec<InternalFinding>,
    ) {
        let token_len = suffix
            .iter()
            .position(|&byte| !is_token_byte(byte))
            .unwrap_or(suffix.len());

        if token_len == suffix.len() {
            if !state.active_prefixes.iter().any(|active| {
                active.rule_index == rule.rule_index && active.source_start == source_start
            }) {
                state.active_prefixes.push(ActivePrefix {
                    rule_index: rule.rule_index,
                    source_start,
                });
            }

            return;
        }

        findings.push(InternalFinding::new(
            rule.rule_index,
            source_start,
            needle_end + token_len,
        ));
    }

    fn finish_stream(
        state: &mut MultiPatternStreamState,
        source_end: usize,
        findings: &mut Vec<InternalFinding>,
    ) {
        findings.extend(state.active_prefixes.drain(..).map(|active| {
            InternalFinding::new(active.rule_index, active.source_start, source_end)
        }));
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

    pub(crate) fn scan_stream_chunk(
        &self,
        state: &mut MultiPatternStreamState,
        chunk: &[u8],
        source_offset: usize,
        findings: &mut Vec<InternalFinding>,
    ) {
        if let Some(engine) = &self.multi_pattern {
            engine.scan_stream_chunk(state, chunk, source_offset, findings);
        }
    }

    pub(crate) fn finish_stream(
        &self,
        state: &mut MultiPatternStreamState,
        source_end: usize,
        findings: &mut Vec<InternalFinding>,
    ) {
        if self.multi_pattern.is_some() {
            MultiPatternEngine::finish_stream(state, source_end, findings);
        }
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
}
