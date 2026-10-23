//! Owned rule facts from Core's immutable scanner metadata iterator.
use crate::{DetectionModeKind, FindingSeverity, RemediationKind, ScanEngine};
use wasm_bindgen::prelude::*;

/// Core public matching families, independent of private matcher representation.
#[wasm_bindgen]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleKind {
    /// Exact literal occurrences.
    Literal = 0,
    /// Complete Core tokens beginning with the prefix.
    Prefix = 1,
    /// Complete Core tokens ending with the suffix.
    Suffix = 2,
    /// Non-empty full regex-match spans.
    Pattern = 3,
}

/// Independently owned metadata. Call generated free() on each returned view.
/// No matcher payload, regex, validator or scanned source is stored or exposed.
#[wasm_bindgen]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleMetadataView {
    rule_id: String,
    kind: RuleKind,
    detection_mode: DetectionModeKind,
    severity: FindingSeverity,
    remediation: RemediationKind,
}

impl From<cribra::RuleMetadata<'_>> for RuleMetadataView {
    fn from(rule: cribra::RuleMetadata<'_>) -> Self {
        Self {
            rule_id: rule.id().to_owned(),
            kind: match rule.kind() {
                cribra::RuleKind::Literal => RuleKind::Literal,
                cribra::RuleKind::Prefix => RuleKind::Prefix,
                cribra::RuleKind::Suffix => RuleKind::Suffix,
                cribra::RuleKind::Pattern => RuleKind::Pattern,
            },
            detection_mode: rule.detection_mode().into(),
            severity: rule.severity().into(),
            remediation: rule.remediation().into(),
        }
    }
}

#[wasm_bindgen]
impl RuleMetadataView {
    /// Stable UTF-8 rule identity, copied from Core.
    #[wasm_bindgen(getter, js_name = ruleId)]
    pub fn rule_id(&self) -> String {
        self.rule_id.clone()
    }
    /// Public matcher family.
    #[wasm_bindgen(getter)]
    pub fn kind(&self) -> RuleKind {
        self.kind
    }
    /// Core's finding validation authority.
    #[wasm_bindgen(getter, js_name = detectionMode)]
    pub fn detection_mode(&self) -> DetectionModeKind {
        self.detection_mode
    }
    /// Default rule severity.
    #[wasm_bindgen(getter)]
    pub fn severity(&self) -> FindingSeverity {
        self.severity
    }
    /// Existing remediation enum, with None representing absence.
    #[wasm_bindgen(getter)]
    pub fn remediation(&self) -> RemediationKind {
        self.remediation
    }
}

#[wasm_bindgen]
impl ScanEngine {
    /// Copies active metadata in Core order. Each call returns fresh owned
    /// views that outlive this engine; free every view separately.
    #[wasm_bindgen(js_name = activeRules)]
    pub fn active_rules(&self) -> Vec<RuleMetadataView> {
        self.scanner.rule_metadata().map(Into::into).collect()
    }
}
