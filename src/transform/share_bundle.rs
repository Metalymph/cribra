//! Orchestration for producing share-safe transformed source batches.
//!
//! A [`ShareBundle`] is an in-memory domain object. It does not serialize,
//! compress, write files, create archives, access the network, or retain
//! original source text. Those responsibilities belong to callers such as a
//! CLI, WASM adapter, desktop application, or service layer.
//!
//! Sources must be supplied in the same order used to produce the associated
//! [`ScanResults`](crate::ScanResults). The builder validates source count and
//! byte lengths before applying transformations.

use std::{fmt, time::SystemTime};

use crate::{ScanResults, ScanSummary};

use super::{
    PseudonymizationOptions, SynthesisOptions, TransformError, pseudonymize, redact, synthesize,
    template,
};

/// Transformation selected for a share bundle.
#[derive(Debug, Clone, Eq, PartialEq)]
#[non_exhaustive]
pub enum ShareMode {
    /// Replace all detected spans with `[REDACTED]`.
    Redact,
    /// Replace each independent finding with a semantic `<CRIBRA:rule-id>` placeholder.
    Template,
    /// Replace findings with stable keyed pseudonyms.
    Pseudonymize(PseudonymizationOptions),
    /// Replace findings with deterministic synthetic values.
    Synthesize(SynthesisOptions),
}

impl ShareMode {
    /// Returns the non-configurational kind of this mode.
    #[must_use]
    pub const fn kind(&self) -> ShareModeKind {
        match self {
            Self::Redact => ShareModeKind::Redact,
            Self::Template => ShareModeKind::Template,
            Self::Pseudonymize(_) => ShareModeKind::Pseudonymize,
            Self::Synthesize(_) => ShareModeKind::Synthesize,
        }
    }
}

/// Stable transformation identifier stored in share manifests.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[non_exhaustive]
pub enum ShareModeKind {
    /// Conservative redaction.
    Redact,
    /// Semantic template generation.
    Template,
    /// Deterministic keyed pseudonymization.
    Pseudonymize,
    /// Deterministic synthetic-value generation.
    Synthesize,
}

/// One transformed source contained in a [`ShareBundle`].
///
/// The original source is not retained.
#[derive(Debug, Clone, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TransformedSource<K> {
    key: K,
    content: String,
}

impl<K> TransformedSource<K> {
    pub(crate) const fn new(key: K, content: String) -> Self {
        Self { key, content }
    }

    /// Returns the caller-supplied source identifier.
    #[must_use]
    pub const fn key(&self) -> &K {
        &self.key
    }

    /// Returns the transformed UTF-8 content.
    #[must_use]
    pub fn content(&self) -> &str {
        &self.content
    }

    /// Consumes the entry and returns only its transformed content.
    #[must_use]
    pub fn into_content(self) -> String {
        self.content
    }

    /// Consumes the entry and returns `(key, transformed_content)`.
    #[must_use]
    pub fn into_parts(self) -> (K, String) {
        (self.key, self.content)
    }
}

/// Metadata describing how a [`ShareBundle`] was produced.
///
/// The manifest stores aggregate counters only. It never stores source text,
/// matched secret values, findings, or pseudonymization/synthesis keys.
#[derive(Debug, Clone, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ShareManifest {
    mode: ShareModeKind,
    summary: ScanSummary,
    #[cfg_attr(feature = "serde", serde(with = "system_time_wire"))]
    generated_at: SystemTime,
}

impl ShareManifest {
    pub(crate) const fn new(
        mode: ShareModeKind,
        summary: ScanSummary,
        generated_at: SystemTime,
    ) -> Self {
        Self {
            mode,
            summary,
            generated_at,
        }
    }

    /// Returns the transformation kind used for the bundle.
    #[must_use]
    pub const fn mode(&self) -> ShareModeKind {
        self.mode
    }

    /// Returns aggregate statistics copied from the original scan results.
    #[must_use]
    pub const fn summary(&self) -> ScanSummary {
        self.summary
    }

    /// Returns when the in-memory bundle was generated.
    #[must_use]
    pub const fn generated_at(&self) -> SystemTime {
        self.generated_at
    }
}

impl fmt::Display for ShareModeKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Redact => "redact",
            Self::Template => "template",
            Self::Pseudonymize => "pseudonymize",
            Self::Synthesize => "synthesize",
        })
    }
}

impl fmt::Display for ShareManifest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(formatter, "transformation: {}", self.mode)?;
        self.summary.fmt(formatter)
    }
}

/// In-memory collection of transformed sources and share-safe metadata.
///
/// The bundle owns transformed content and cloned source keys, but never owns or
/// retains the original source text.
#[derive(Debug, Clone, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ShareBundle<K> {
    sources: Vec<TransformedSource<K>>,
    manifest: ShareManifest,
}

impl ShareBundle<()> {
    /// Creates a share-bundle builder.
    ///
    /// The builder itself is not tied to a source-key type. The final
    /// `ShareBundle<K>` type is inferred from the `ScanResults<K>` passed to
    /// [`ShareBundleBuilder::build`].
    #[must_use]
    pub const fn builder() -> ShareBundleBuilder {
        ShareBundleBuilder::new()
    }
}

impl<K> ShareBundle<K> {
    /// Returns transformed sources in original batch order.
    #[must_use]
    pub fn sources(&self) -> &[TransformedSource<K>] {
        &self.sources
    }

    /// Returns the bundle manifest.
    #[must_use]
    pub const fn manifest(&self) -> &ShareManifest {
        &self.manifest
    }

    /// Returns the original scan summary stored in the manifest.
    #[must_use]
    pub const fn summary(&self) -> ScanSummary {
        self.manifest.summary()
    }

    /// Returns the number of transformed sources.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.sources.len()
    }

    /// Returns `true` when the bundle contains no sources.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.sources.is_empty()
    }

    /// Consumes the bundle and returns only the transformed sources.
    #[must_use]
    pub fn into_sources(self) -> Vec<TransformedSource<K>> {
        self.sources
    }

    /// Consumes the bundle and returns `(transformed_sources, manifest)`.
    #[must_use]
    pub fn into_parts(self) -> (Vec<TransformedSource<K>>, ShareManifest) {
        (self.sources, self.manifest)
    }
}

impl<'a, K> IntoIterator for &'a ShareBundle<K> {
    type Item = &'a TransformedSource<K>;
    type IntoIter = std::slice::Iter<'a, TransformedSource<K>>;

    fn into_iter(self) -> Self::IntoIter {
        self.sources.iter()
    }
}

impl<K> IntoIterator for ShareBundle<K> {
    type Item = TransformedSource<K>;
    type IntoIter = std::vec::IntoIter<TransformedSource<K>>;

    fn into_iter(self) -> Self::IntoIter {
        self.sources.into_iter()
    }
}

/// Builder for an in-memory [`ShareBundle`].
///
/// A transformation mode must be selected explicitly before [`build`](Self::build).
#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub struct ShareBundleBuilder {
    mode: Option<ShareMode>,
}

impl ShareBundleBuilder {
    /// Creates an empty builder with no implicit transformation mode.
    #[must_use]
    pub const fn new() -> Self {
        Self { mode: None }
    }

    /// Selects the transformation used for every source in the bundle.
    #[must_use]
    pub fn mode(mut self, mode: ShareMode) -> Self {
        self.mode = Some(mode);
        self
    }

    /// Builds transformed sources from scan results and their original UTF-8 text.
    ///
    /// `sources` must contain exactly one source for each scan entry, in the same
    /// order used for scanning. Each source byte length is checked against the
    /// corresponding [`ScanEntry`](crate::ScanEntry) before transformation.
    ///
    /// Source keys are cloned from the scan results so callers may continue
    /// using `results` after bundle generation.
    ///
    /// # Errors
    ///
    /// Returns [`TransformError::MissingShareMode`] when no mode was selected,
    /// [`TransformError::SourceCountMismatch`] when source counts differ,
    /// [`TransformError::SourceLengthMismatch`] when a source no longer matches
    /// the recorded byte length, or any error returned by the selected
    /// transformation.
    pub fn build<'a, K, I>(
        &self,
        results: &ScanResults<K>,
        sources: I,
    ) -> Result<ShareBundle<K>, TransformError>
    where
        K: Clone,
        I: IntoIterator<Item = &'a str>,
    {
        let mode = self.mode.as_ref().ok_or(TransformError::MissingShareMode)?;
        let source_list = sources.into_iter().collect::<Vec<_>>();

        if source_list.len() != results.len() {
            return Err(TransformError::SourceCountMismatch {
                expected: results.len(),
                actual: source_list.len(),
            });
        }

        let mut transformed = Vec::with_capacity(results.len());

        for (index, (entry, source)) in results.iter().zip(source_list).enumerate() {
            if source.len() != entry.source_bytes() {
                return Err(TransformError::SourceLengthMismatch {
                    index,
                    expected_bytes: entry.source_bytes(),
                    actual_bytes: source.len(),
                });
            }

            let content = transform_source(mode, source, entry.report())?;
            transformed.push(TransformedSource::new(entry.key().clone(), content));
        }

        let manifest = ShareManifest::new(mode.kind(), results.summary(), SystemTime::now());

        Ok(ShareBundle {
            sources: transformed,
            manifest,
        })
    }
}

fn transform_source(
    mode: &ShareMode,
    source: &str,
    report: &crate::ScanReport,
) -> Result<String, TransformError> {
    match mode {
        ShareMode::Redact => redact(source, report),
        ShareMode::Template => template(source, report),
        ShareMode::Pseudonymize(options) => pseudonymize(source, report, options),
        ShareMode::Synthesize(options) => synthesize(source, report, options),
    }
}

#[cfg(test)]
mod tests {
    use crate::{Rule, Scanner, Severity};

    use super::*;

    fn scanner() -> Scanner {
        Scanner::builder()
            .rule(Rule::literal("secret", "SECRET", Severity::Critical))
            .build()
            .expect("share bundle test scanner should build")
    }

    #[test]
    fn builder_requires_explicit_mode() {
        let scanner = scanner();
        let source = "TOKEN=SECRET";
        let results = scanner.scan([("memory", source)]);

        assert_eq!(
            ShareBundle::builder().build(&results, [source]),
            Err(TransformError::MissingShareMode),
        );
    }

    #[test]
    fn redaction_bundle_preserves_keys_and_summary() {
        let scanner = scanner();
        let first = "A=SECRET";
        let second = "clean";
        let results = scanner.scan([("a.env", first), ("b.env", second)]);

        let bundle = ShareBundle::builder()
            .mode(ShareMode::Redact)
            .build(&results, [first, second])
            .unwrap();

        assert_eq!(bundle.len(), 2);
        assert_eq!(bundle.sources()[0].key(), &"a.env");
        assert_eq!(bundle.sources()[0].content(), "A=[REDACTED]");
        assert_eq!(bundle.sources()[1].content(), "clean");
        assert_eq!(bundle.summary(), results.summary());
        assert_eq!(bundle.manifest().mode(), ShareModeKind::Redact);
    }

    #[test]
    fn template_mode_routes_to_template_transformation() {
        let scanner = scanner();
        let source = "TOKEN=SECRET";
        let results = scanner.scan([("memory", source)]);

        let bundle = ShareBundle::builder()
            .mode(ShareMode::Template)
            .build(&results, [source])
            .unwrap();

        assert_eq!(bundle.sources()[0].content(), "TOKEN=<CRIBRA:secret>",);
        assert_eq!(bundle.manifest().mode(), ShareModeKind::Template);
    }

    #[test]
    fn pseudonymization_mode_routes_options_without_storing_key_in_manifest() {
        let scanner = scanner();
        let source = "TOKEN=SECRET";
        let results = scanner.scan([("memory", source)]);

        let bundle = ShareBundle::builder()
            .mode(ShareMode::Pseudonymize(PseudonymizationOptions::new(
                [7; 32],
            )))
            .build(&results, [source])
            .unwrap();

        assert!(
            bundle.sources()[0]
                .content()
                .starts_with("TOKEN=cribra_pseudo_")
        );
        assert_eq!(bundle.manifest().mode(), ShareModeKind::Pseudonymize,);
    }

    #[test]
    fn synthesis_mode_routes_options() {
        let scanner = scanner();
        let source = "TOKEN=SECRET";
        let results = scanner.scan([("memory", source)]);

        let bundle = ShareBundle::builder()
            .mode(ShareMode::Synthesize(SynthesisOptions::new([9; 32])))
            .build(&results, [source])
            .unwrap();

        assert_ne!(bundle.sources()[0].content(), source);
        assert_eq!(bundle.manifest().mode(), ShareModeKind::Synthesize);
    }

    #[test]
    fn source_count_must_match_results() {
        let scanner = scanner();
        let results = scanner.scan([("a", "SECRET"), ("b", "clean")]);

        assert_eq!(
            ShareBundle::builder()
                .mode(ShareMode::Redact)
                .build(&results, ["SECRET"]),
            Err(TransformError::SourceCountMismatch {
                expected: 2,
                actual: 1,
            }),
        );
    }

    #[test]
    fn source_length_change_is_rejected() {
        let scanner = scanner();
        let results = scanner.scan([("memory", "TOKEN=SECRET")]);

        assert_eq!(
            ShareBundle::builder()
                .mode(ShareMode::Redact)
                .build(&results, ["TOKEN=CHANGED_SECRET"]),
            Err(TransformError::SourceLengthMismatch {
                index: 0,
                expected_bytes: "TOKEN=SECRET".len(),
                actual_bytes: "TOKEN=CHANGED_SECRET".len(),
            }),
        );
    }

    #[test]
    fn consuming_accessors_return_owned_bundle_components() {
        let scanner = scanner();
        let source = "TOKEN=SECRET";
        let results = scanner.scan([(String::from("memory"), source)]);

        let bundle = ShareBundle::builder()
            .mode(ShareMode::Redact)
            .build(&results, [source])
            .unwrap();

        let (sources, manifest) = bundle.into_parts();
        let (key, content) = sources.into_iter().next().unwrap().into_parts();

        assert_eq!(key, "memory");
        assert_eq!(content, "TOKEN=[REDACTED]");
        assert_eq!(manifest.mode(), ShareModeKind::Redact);
    }
}

#[cfg(feature = "serde")]
mod system_time_wire {
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    #[derive(Serialize, Deserialize)]
    struct WireTime {
        secs_since_epoch: u64,
        nanos_since_epoch: u32,
    }

    pub(super) fn serialize<S>(value: &SystemTime, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let duration = value
            .duration_since(UNIX_EPOCH)
            .map_err(serde::ser::Error::custom)?;

        WireTime {
            secs_since_epoch: duration.as_secs(),
            nanos_since_epoch: duration.subsec_nanos(),
        }
        .serialize(serializer)
    }

    pub(super) fn deserialize<'de, D>(deserializer: D) -> Result<SystemTime, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = WireTime::deserialize(deserializer)?;

        if wire.nanos_since_epoch >= 1_000_000_000 {
            return Err(serde::de::Error::custom(
                "nanos_since_epoch must be less than 1,000,000,000",
            ));
        }

        Ok(UNIX_EPOCH + Duration::new(wire.secs_since_epoch, wire.nanos_since_epoch))
    }
}
