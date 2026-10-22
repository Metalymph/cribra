//! Core-backed in-memory Share Bundle projection.
//!
//! Scanning, transformation selection, validation, source ordering and manifest
//! generation are delegated to [`cribra::transform::ShareBundleBuilder`]. The
//! adapter only copies inputs at the boundary and projects owned outputs.
//! Original source text is not retained after `scanAndBuild()` returns.

#![cfg(feature = "transform")]

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use cribra::ScanSummary;
use cribra::transform::{
    PseudonymizationOptions, ShareBundle as CoreShareBundle,
    ShareBundleBuilder as CoreShareBundleBuilder, ShareManifest, ShareMode,
    ShareModeKind as CoreShareModeKind, SynthesisOptions, TransformError,
};
use wasm_bindgen::prelude::*;

use crate::{ScanEngine, ScanSummaryView, batch::decode_sources, key32_bytes};

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = Date, js_name = now)]
    fn host_now_millis() -> f64;
}

/// Key-free transformation kind stored in a Core share manifest.
#[wasm_bindgen]
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ShareModeKind {
    Redact,
    Template,
    Pseudonymize,
    Synthesize,
    /// A future Core mode not understood by this adapter projection.
    Unknown,
}

impl From<CoreShareModeKind> for ShareModeKind {
    fn from(value: CoreShareModeKind) -> Self {
        match value {
            CoreShareModeKind::Redact => Self::Redact,
            CoreShareModeKind::Template => Self::Template,
            CoreShareModeKind::Pseudonymize => Self::Pseudonymize,
            CoreShareModeKind::Synthesize => Self::Synthesize,
            _ => Self::Unknown,
        }
    }
}

/// Builder for one Core-owned in-memory share bundle.
///
/// No transformation mode is selected by default, preserving Core's
/// `MissingShareMode` failure from `scanAndBuild()`. The most recently selected
/// mode wins. The builder stores no caller-owned key buffer or source text.
#[wasm_bindgen]
pub struct ShareBundleBuilder {
    builder: CoreShareBundleBuilder,
}

impl Default for ShareBundleBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[wasm_bindgen]
impl ShareBundleBuilder {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            builder: CoreShareBundleBuilder::new(),
        }
    }

    /// Selects Core's conservative redaction mode.
    #[wasm_bindgen(js_name = modeRedact)]
    pub fn mode_redact(&mut self) {
        self.replace_mode(ShareMode::Redact);
    }

    /// Selects Core's default semantic-template mode.
    ///
    /// Core's Share Bundle contract does not carry custom template options.
    #[wasm_bindgen(js_name = modeTemplate)]
    pub fn mode_template(&mut self) {
        self.replace_mode(ShareMode::Template);
    }

    /// Selects Core keyed pseudonymization.
    ///
    /// `key` must contain exactly 32 bytes and is copied into Core options.
    /// Omitted `prefix` or `digest_bytes` retain Core defaults; an explicit empty
    /// prefix is preserved. Core clamps digest lengths to its supported range.
    #[wasm_bindgen(js_name = modePseudonymize)]
    pub fn mode_pseudonymize(
        &mut self,
        key: &[u8],
        prefix: Option<String>,
        digest_bytes: Option<usize>,
    ) -> Result<(), JsError> {
        self.mode_pseudonymize_core(key, prefix, digest_bytes)
            .map_err(|error| JsError::new(&error))
    }

    /// Selects Core keyed deterministic synthesis.
    ///
    /// `key` must contain exactly 32 bytes and is copied into Core options.
    /// Omitted `marker` retains Core's default; an explicit empty marker is
    /// preserved.
    #[wasm_bindgen(js_name = modeSynthesize)]
    pub fn mode_synthesize(&mut self, key: &[u8], marker: Option<String>) -> Result<(), JsError> {
        self.mode_synthesize_core(key, marker)
            .map_err(|error| JsError::new(&error))
    }

    /// Scans and transforms the complete ordered source collection atomically.
    ///
    /// The adapter delegates to one Core
    /// `ShareBundleBuilder::scan_and_build_with_generated_at()` call after
    /// reading the host clock. IDs and input order are preserved by Core;
    /// duplicate, empty and
    /// Unicode IDs are valid. Invalid structural inputs fail before Core.
    /// Core failures retain Core's source-free diagnostics.
    #[wasm_bindgen(js_name = scanAndBuild)]
    pub fn scan_and_build(
        &self,
        engine: &ScanEngine,
        #[wasm_bindgen(unchecked_param_type = "ScanBatchSource[]")] sources: Vec<JsValue>,
    ) -> Result<ShareBundle, JsError> {
        let inputs = decode_sources(&sources, "share bundle source")?;
        let generated_at = host_generated_at()?;
        self.scan_and_build_at_core(engine, inputs, generated_at)
            .map_err(|error| match error {
                ShareBundleFailure::Transform(error) => JsError::new(&error.to_string()),
                ShareBundleFailure::Timestamp => {
                    JsError::new("share bundle generation time precedes the Unix epoch")
                }
            })
    }
}

fn host_generated_at() -> Result<SystemTime, JsError> {
    system_time_from_unix_millis(host_now_millis())
        .ok_or_else(|| JsError::new("host clock did not return a valid Unix millisecond timestamp"))
}

fn system_time_from_unix_millis(millis: f64) -> Option<SystemTime> {
    const MAX_SAFE_MILLIS: f64 = 9_007_199_254_740_991.0;

    if !millis.is_finite() || millis < 0.0 || millis > MAX_SAFE_MILLIS {
        return None;
    }

    let millis = millis.trunc() as u64;
    let seconds = millis / 1_000;
    let nanoseconds = (millis % 1_000) as u32 * 1_000_000;

    UNIX_EPOCH.checked_add(Duration::new(seconds, nanoseconds))
}

#[derive(Debug, Clone, Eq, PartialEq)]
enum ShareBundleFailure {
    Transform(TransformError),
    Timestamp,
}

impl From<TransformError> for ShareBundleFailure {
    fn from(error: TransformError) -> Self {
        Self::Transform(error)
    }
}

impl ShareBundleBuilder {
    fn replace_mode(&mut self, mode: ShareMode) {
        self.builder = std::mem::take(&mut self.builder).mode(mode);
    }

    fn mode_pseudonymize_core(
        &mut self,
        key: &[u8],
        prefix: Option<String>,
        digest_bytes: Option<usize>,
    ) -> Result<(), String> {
        let mut options = PseudonymizationOptions::new(key32_bytes(key)?);
        if let Some(prefix) = prefix {
            options = options.prefix(prefix);
        }
        if let Some(digest_bytes) = digest_bytes {
            options = options.digest_bytes(digest_bytes);
        }
        self.replace_mode(ShareMode::Pseudonymize(options));
        Ok(())
    }

    fn mode_synthesize_core(&mut self, key: &[u8], marker: Option<String>) -> Result<(), String> {
        let mut options = SynthesisOptions::new(key32_bytes(key)?);
        if let Some(marker) = marker {
            options = options.marker(marker);
        }
        self.replace_mode(ShareMode::Synthesize(options));
        Ok(())
    }

    fn scan_and_build_at_core(
        &self,
        engine: &ScanEngine,
        inputs: Vec<(String, String)>,
        generated_at: SystemTime,
    ) -> Result<ShareBundle, ShareBundleFailure> {
        let bundle = self.builder.scan_and_build_with_generated_at(
            &engine.scanner,
            inputs
                .iter()
                .map(|(id, source)| (id.as_str(), source.as_str())),
            generated_at,
        )?;
        ShareBundle::from_core(bundle)
    }

    #[cfg(test)]
    fn scan_and_build_core(
        &self,
        engine: &ScanEngine,
        inputs: Vec<(String, String)>,
    ) -> Result<ShareBundle, ShareBundleFailure> {
        self.scan_and_build_at_core(engine, inputs, SystemTime::now())
    }
}

/// Owned, in-memory Share Bundle returned by `ShareBundleBuilder.scanAndBuild()`.
///
/// The bundle owns transformed content, cloned caller IDs and a Core-derived
/// manifest. It never retains original source text. `entryAt()` and `manifest()`
/// return fresh independently owned projections, so results remain valid after
/// the bundle, builder or engine is freed.
#[wasm_bindgen]
#[derive(Debug)]
pub struct ShareBundle {
    entries: Vec<ShareBundleEntry>,
    manifest: ShareManifestView,
}

#[wasm_bindgen]
impl ShareBundle {
    /// Number of transformed sources, always equal to the input count.
    #[wasm_bindgen(getter)]
    pub fn length(&self) -> usize {
        self.entries.len()
    }

    /// Whether the bundle contains no transformed sources.
    #[wasm_bindgen(js_name = isEmpty)]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Returns a fresh independently owned entry at the preserved input index.
    #[wasm_bindgen(js_name = entryAt)]
    pub fn entry_at_js(
        &self,
        #[wasm_bindgen(unchecked_param_type = "number")] index: JsValue,
    ) -> Result<ShareBundleEntry, JsError> {
        self.entry_at(crate::numeric::js_index(&index)?)
    }

    /// Returns a fresh projection of Core's manifest metadata.
    pub fn manifest(&self) -> ShareManifestView {
        self.manifest
    }

    /// Returns a fresh projection of Core's aggregate scan summary.
    pub fn summary(&self) -> ScanSummaryView {
        ScanSummaryView::from(self.manifest.summary)
    }
}

impl ShareBundle {
    /// Returns a fresh independently owned entry at a Rust index.
    pub fn entry_at(&self, index: usize) -> Result<ShareBundleEntry, JsError> {
        self.entry_at_core(index)
            .ok_or_else(|| JsError::new("share bundle entry index out of range"))
    }

    fn entry_at_core(&self, index: usize) -> Option<ShareBundleEntry> {
        self.entries.get(index).cloned()
    }

    fn from_core(bundle: CoreShareBundle<&str>) -> Result<Self, ShareBundleFailure> {
        let (sources, manifest) = bundle.into_parts();
        let manifest = ShareManifestView::from_core(manifest)?;
        let entries = sources
            .into_iter()
            .map(|source| {
                let (id, content) = source.into_parts();
                ShareBundleEntry {
                    id: id.to_owned(),
                    content,
                }
            })
            .collect();

        Ok(Self { entries, manifest })
    }
}

/// Owned transformed content for one input source.
///
/// The entry retains its caller-supplied ID and Core-transformed UTF-8 content,
/// never the original source text. Safe to use after the parent bundle, builder
/// or engine is freed.
#[wasm_bindgen]
#[derive(Debug, Clone)]
pub struct ShareBundleEntry {
    id: String,
    content: String,
}

#[wasm_bindgen]
impl ShareBundleEntry {
    /// Returns the caller-supplied ID exactly as received.
    #[wasm_bindgen(getter)]
    pub fn id(&self) -> String {
        self.id.clone()
    }

    /// Returns the Core-transformed UTF-8 content.
    #[wasm_bindgen(getter)]
    pub fn content(&self) -> String {
        self.content.clone()
    }
}

/// Owned projection of Core's share-safe manifest.
///
/// The manifest contains only the transformation kind, aggregate scan counters
/// and Core's actual generation time. It never contains source text, findings,
/// matched values or transformation keys.
#[wasm_bindgen]
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct ShareManifestView {
    mode: ShareModeKind,
    summary: ScanSummary,
    generated_at_seconds: u64,
    generated_at_nanoseconds: u32,
}

impl ShareManifestView {
    fn from_core(manifest: ShareManifest) -> Result<Self, ShareBundleFailure> {
        let generated_at = manifest
            .generated_at()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| ShareBundleFailure::Timestamp)?;

        Ok(Self {
            mode: manifest.mode().into(),
            summary: manifest.summary(),
            generated_at_seconds: generated_at.as_secs(),
            generated_at_nanoseconds: generated_at.subsec_nanos(),
        })
    }
}

#[wasm_bindgen]
impl ShareManifestView {
    /// Transformation kind selected for the bundle.
    #[wasm_bindgen(getter)]
    pub fn mode(&self) -> ShareModeKind {
        self.mode
    }

    /// Core's actual generation time as whole seconds since the Unix epoch.
    #[wasm_bindgen(getter, js_name = generatedAtSeconds)]
    pub fn generated_at_seconds(&self) -> u64 {
        self.generated_at_seconds
    }

    /// Nanoseconds within `generatedAtSeconds`, always below one billion.
    #[wasm_bindgen(getter, js_name = generatedAtNanoseconds)]
    pub fn generated_at_nanoseconds(&self) -> u32 {
        self.generated_at_nanoseconds
    }

    /// Returns a fresh projection of Core's aggregate scan summary.
    pub fn summary(&self) -> ScanSummaryView {
        ScanSummaryView::from(self.summary)
    }
}

#[cfg(test)]
#[path = "share_bundle_tests.rs"]
mod tests;
