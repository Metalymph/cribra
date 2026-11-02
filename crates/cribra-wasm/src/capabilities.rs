//! Compile-time adapter identity and backend capability metadata.
//!
//! These types are internal adapter details. The future TypeScript package
//! must copy the getters into its own plain-object DTO and call the generated
//! `free()` in a `try`/`finally` block. No adapter class is part of the
//! package's public type surface.

use wasm_bindgen::prelude::*;

/// Backend that produced a capability manifest.
#[wasm_bindgen]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendKind {
    /// The `cribra-wasm` adapter.
    Wasm = 0,
}

/// Cargo feature profile compiled into this artifact.
#[wasm_bindgen]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileKind {
    /// Core profile: scanning and reporting without transformations.
    Core = 0,
    /// Flow profile: the `transform` feature is enabled.
    Flow = 1,
}

#[cfg(feature = "transform")]
const PROFILE: ProfileKind = ProfileKind::Flow;
#[cfg(not(feature = "transform"))]
const PROFILE: ProfileKind = ProfileKind::Core;

/// Backend-level capabilities of this compiled adapter.
///
/// Every field describes an exported operation, never Core parity or a
/// particular scanner's streaming admission. `streaming` is true because
/// `stream()` is exported; individual rulesets may still be rejected by Core
/// with `StreamErrorCode.UnsupportedRuleSet`.
#[wasm_bindgen]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapabilityManifest {
    backend: BackendKind,
    profile: ProfileKind,
    single_source_scan: bool,
    batch_scan: bool,
    streaming: bool,
    incremental: bool,
    redaction: bool,
    templating: bool,
    pseudonymization: bool,
    synthesis: bool,
    share_bundle: bool,
    rule_metadata: bool,
    explanations: bool,
    parallel: bool,
}

impl CapabilityManifest {
    /// Returns the compile-time capability set for this artifact.
    pub(crate) const fn current() -> Self {
        let transform = cfg!(feature = "transform");
        Self {
            backend: BackendKind::Wasm,
            profile: PROFILE,
            single_source_scan: true,
            batch_scan: true,
            streaming: true,
            incremental: true,
            redaction: transform,
            templating: transform,
            pseudonymization: transform,
            synthesis: transform,
            share_bundle: transform,
            rule_metadata: true,
            explanations: true,
            parallel: false,
        }
    }
}

#[wasm_bindgen]
impl CapabilityManifest {
    /// Backend identity.
    #[wasm_bindgen(getter)]
    pub fn backend(&self) -> BackendKind {
        self.backend
    }

    /// Compiled Cargo feature profile.
    #[wasm_bindgen(getter)]
    pub fn profile(&self) -> ProfileKind {
        self.profile
    }

    /// Single-source scanning is exported.
    #[wasm_bindgen(getter, js_name = singleSourceScan)]
    pub fn single_source_scan(&self) -> bool {
        self.single_source_scan
    }

    /// Sequential batch scanning is exported.
    #[wasm_bindgen(getter, js_name = batchScan)]
    pub fn batch_scan(&self) -> bool {
        self.batch_scan
    }

    /// Byte streaming is exported. Admission remains a per-ruleset decision.
    #[wasm_bindgen(getter)]
    pub fn streaming(&self) -> bool {
        self.streaming
    }

    /// Incremental document sessions are exported.
    #[wasm_bindgen(getter)]
    pub fn incremental(&self) -> bool {
        self.incremental
    }

    /// Redaction is exported (`transform` feature).
    #[wasm_bindgen(getter)]
    pub fn redaction(&self) -> bool {
        self.redaction
    }

    /// Templating is exported (`transform` feature).
    #[wasm_bindgen(getter)]
    pub fn templating(&self) -> bool {
        self.templating
    }

    /// Pseudonymization is exported (`transform` feature).
    #[wasm_bindgen(getter)]
    pub fn pseudonymization(&self) -> bool {
        self.pseudonymization
    }

    /// Synthesis is exported (`transform` feature).
    #[wasm_bindgen(getter)]
    pub fn synthesis(&self) -> bool {
        self.synthesis
    }

    /// In-memory Share Bundles are exported (`transform` feature).
    #[wasm_bindgen(getter, js_name = shareBundle)]
    pub fn share_bundle(&self) -> bool {
        self.share_bundle
    }

    /// Active rule metadata is exported.
    #[wasm_bindgen(getter, js_name = ruleMetadata)]
    pub fn rule_metadata(&self) -> bool {
        self.rule_metadata
    }

    /// Finding and candidate explanation resolution is exported.
    #[wasm_bindgen(getter)]
    pub fn explanations(&self) -> bool {
        self.explanations
    }

    /// Parallel execution is not exported by this adapter.
    #[wasm_bindgen(getter)]
    pub fn parallel(&self) -> bool {
        self.parallel
    }
}
